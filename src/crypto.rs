//! Domain-separated BLAKE3 identity and strict Ed25519 signatures.

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::error::{ContractError, Result};
use crate::model::{AuthorId, ContextId, EventBodyV1, EventId, EventSignature, SignedEventV1};

/// BLAKE3 derive-key context frozen for version-1 event IDs.
pub const EVENT_ID_DOMAIN: &str = "org.aaif.contextmesh.event-id.v1";
/// ASCII prefix, including NUL separator, frozen for signature messages.
pub const SIGNATURE_DOMAIN: &[u8] = b"org.aaif.contextmesh.signature.v1\0";

/// An in-memory Ed25519 signing identity.
///
/// The private signing key is never serializable or exposed. Dalek zeroizes it
/// on drop; temporary seed buffers used by constructors are also zeroized.
pub struct SigningIdentity {
    signing_key: SigningKey,
}

impl SigningIdentity {
    /// Generates a new identity from fallible operating-system entropy.
    pub fn generate() -> Result<Self> {
        let mut seed = Zeroizing::new([0_u8; 32]);
        getrandom::fill(seed.as_mut()).map_err(|_| ContractError::Entropy)?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    /// Constructs a deterministic identity solely for checked-in test fixtures.
    ///
    /// This is not a key-storage API. Callers must never use a published fixture
    /// seed as a production identity.
    #[doc(hidden)]
    pub fn from_fixture_seed(seed: [u8; 32]) -> Self {
        let seed = Zeroizing::new(seed);
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Returns the public author identity.
    #[must_use]
    pub fn author(&self) -> AuthorId {
        AuthorId::from_bytes(self.signing_key.verifying_key().to_bytes())
    }

    /// Creates, validates, identifies, and signs a body owned by this identity.
    pub fn create_event(
        &self,
        context: ContextId,
        parents: Vec<EventId>,
        kind: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<SignedEventV1> {
        let body = EventBodyV1::new(context, parents, kind, self.author(), payload)?;
        self.sign_body(body)
    }

    /// Signs an already-constructed body after checking its author identity.
    pub fn sign_body(&self, body: EventBodyV1) -> Result<SignedEventV1> {
        body.validate()?;
        if body.author() != self.author() {
            return Err(ContractError::AuthorMismatch);
        }
        let event_id = derive_event_id(&body)?;
        let signature = self.sign_event_id(event_id);
        Ok(SignedEventV1::from_verified_parts(
            event_id, body, signature,
        ))
    }

    fn sign_event_id(&self, event_id: EventId) -> EventSignature {
        let message = signing_message(event_id);
        EventSignature::from_bytes(self.signing_key.sign(&message).to_bytes())
    }
}

/// Derives the version-1 BLAKE3 event ID from canonical body bytes.
pub fn derive_event_id(body: &EventBodyV1) -> Result<EventId> {
    let canonical = body.canonical_bytes()?;
    let mut hasher = blake3::Hasher::new_derive_key(EVENT_ID_DOMAIN);
    hasher.update(&canonical);
    Ok(EventId::from_bytes(*hasher.finalize().as_bytes()))
}

/// Builds the exact domain-separated signature message.
#[must_use]
pub fn signing_message(event_id: EventId) -> Vec<u8> {
    let mut message = Vec::with_capacity(SIGNATURE_DOMAIN.len() + 32);
    message.extend_from_slice(SIGNATURE_DOMAIN);
    message.extend_from_slice(&event_id.to_bytes());
    message
}

pub(crate) fn verify_parts(
    body: &EventBodyV1,
    supplied_id: EventId,
    signature: EventSignature,
) -> Result<()> {
    body.validate()?;
    let expected_id = derive_event_id(body)?;
    if supplied_id != expected_id {
        return Err(ContractError::IdMismatch);
    }
    let author_bytes = body.author().to_bytes();
    let verifying_key =
        VerifyingKey::from_bytes(&author_bytes).map_err(|_| ContractError::SignatureInvalid)?;
    let signature_bytes = signature.to_bytes();
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key
        .verify_strict(&signing_message(supplied_id), &signature)
        .map_err(|_| ContractError::SignatureInvalid)
}

impl SigningIdentity {
    /// Signs a caller-supplied domain-separated message (Option B reuse point).
    ///
    /// This is the Option B accessor over the same private key used for
    /// events; it introduces no new signature primitive. The caller must pass
    /// a frozen ASCII domain constant so cross-purpose message reuse is
    /// impossible (receipts use `receipt::RECEIPT_SIGNATURE_DOMAIN`).
    pub fn sign_domain_message(&self, domain: &[u8], message: &[u8]) -> Vec<u8> {
        let mut prefixed = Vec::with_capacity(domain.len() + message.len());
        prefixed.extend_from_slice(domain);
        prefixed.extend_from_slice(message);
        self.signing_key.sign(&prefixed).to_bytes().to_vec()
    }
}

/// Strictly verifies a domain-separated signed message (Option B reuse point).
///
/// Mirrors `verify_parts`' strict Ed25519 verification over an explicit domain
/// prefix, so Option B artifacts share Option A's signature discipline without
/// reusing event-specific messages.
pub fn verify_domain_message(
    author: AuthorId,
    domain: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<()> {
    let signature_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| ContractError::SignatureInvalid)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let mut prefixed = Vec::with_capacity(domain.len() + message.len());
    prefixed.extend_from_slice(domain);
    prefixed.extend_from_slice(message);
    let verifying_key = VerifyingKey::from_bytes(&author.to_bytes())
        .map_err(|_| ContractError::SignatureInvalid)?;
    verifying_key
        .verify_strict(&prefixed, &signature)
        .map_err(|_| ContractError::SignatureInvalid)
}

impl SigningIdentity {
    /// Constructs a production identity from an explicit 32-byte seed.
    ///
    /// The supplied copy is zeroized after key construction. This is the loader
    /// for D-05-01 key-file custody, not a storage API: the seed still never
    /// leaves this process through public values.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let seed = Zeroizing::new(seed);
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Generates an atomic D-05-01 seed file and returns only the public author.
    pub fn generate_key_file(path: &std::path::Path) -> crate::error::KeyFileResult<AuthorId> {
        let mut seed = Zeroizing::new([0_u8; 32]);
        getrandom::fill(seed.as_mut()).map_err(|_| crate::error::KeyFileError::Unavailable)?;
        let author = Self::from_seed(*seed).author();
        write_secret_file(path, seed.as_slice())?;
        Ok(author)
    }

    /// Atomically creates an OA-04-format token1_ bearer-token file.
    pub fn generate_token_file(path: &std::path::Path) -> crate::error::KeyFileResult<()> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| crate::error::KeyFileError::Unavailable)?;
        use base64::Engine as _;
        let text = format!(
            "token1_{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        );
        bytes.fill(0);
        write_secret_file(path, text.as_bytes())
    }

    /// Loads and verifies a D-05-01 seed file into a signing identity.
    pub fn load_key_file(path: &std::path::Path) -> crate::error::KeyFileResult<Self> {
        let seed = read_secret_file(path, 32)?;
        Ok(Self::from_seed(seed))
    }

    /// Explicitly repairs insecure seed/token file permissions to 0600.
    pub fn repair_key_file_permissions(path: &std::path::Path) -> crate::error::KeyFileResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
            let before = std::fs::symlink_metadata(path)
                .map_err(|_| crate::error::KeyFileError::Unavailable)?;
            if !before.file_type().is_file() || before.file_type().is_symlink() {
                return Err(crate::error::KeyFileError::Malformed);
            }
            let mut permissions = before.permissions();
            permissions.set_mode(0o600);
            std::fs::set_permissions(path, permissions)
                .map_err(|_| crate::error::KeyFileError::Unavailable)?;
            let after = std::fs::symlink_metadata(path)
                .map_err(|_| crate::error::KeyFileError::Unavailable)?;
            if after.mode() & 0o077 != 0
                || after.dev() != before.dev()
                || after.ino() != before.ino()
            {
                return Err(crate::error::KeyFileError::Unavailable);
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(crate::error::KeyFileError::Unsupported)
        }
    }
}

/// Atomically creates a new secret file with 0600 permissions, never
/// overwriting and never following symlinks (D-05-01).
fn write_secret_file(path: &std::path::Path, bytes: &[u8]) -> crate::error::KeyFileResult<()> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        if !std::fs::symlink_metadata(parent)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
        {
            return Err(crate::error::KeyFileError::Unavailable);
        }
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(crate::error::KeyFileError::AlreadyExists);
        }
        let mut suffix = [0_u8; 6];
        getrandom::fill(&mut suffix).map_err(|_| crate::error::KeyFileError::Unavailable)?;
        let temp = parent.join(format!(
            ".contextmesh-secret-{}-{}.tmp",
            std::process::id(),
            suffix
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ));
        let write = || -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::hard_link(&temp, path)?;
            std::fs::remove_file(&temp)?;
            std::fs::File::open(parent)?.sync_all()?;
            Ok(())
        };
        match write() {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&temp);
                Err(crate::error::KeyFileError::AlreadyExists)
            }
            Err(_) => {
                let _ = std::fs::remove_file(&temp);
                Err(crate::error::KeyFileError::Unavailable)
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, bytes);
        Err(crate::error::KeyFileError::Unsupported)
    }
}

/// Loads a secret file under the OA-04 token-file discipline and zeroizes it.
fn read_secret_file(
    path: &std::path::Path,
    expected_len: u64,
) -> crate::error::KeyFileResult<[u8; 32]> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let before =
            std::fs::symlink_metadata(path).map_err(|_| crate::error::KeyFileError::Unavailable)?;
        if !before.file_type().is_file() || before.file_type().is_symlink() {
            return Err(crate::error::KeyFileError::Malformed);
        }
        if before.len() != expected_len {
            return Err(crate::error::KeyFileError::Malformed);
        }
        if before.permissions().mode() & 0o077 != 0 {
            return Err(crate::error::KeyFileError::InsecurePermissions);
        }
        let mut file =
            std::fs::File::open(path).map_err(|_| crate::error::KeyFileError::Unavailable)?;
        let opened = file
            .metadata()
            .map_err(|_| crate::error::KeyFileError::Unavailable)?;
        if opened.file_type().is_file()
            && opened.dev() == before.dev()
            && opened.ino() == before.ino()
            && opened.len() == expected_len
            && opened.permissions().mode() & 0o077 == 0
        {
            let mut bytes = Zeroizing::new([0_u8; 32]);
            use std::io::Read as _;
            file.read_exact(bytes.as_mut())
                .map_err(|_| crate::error::KeyFileError::Malformed)?;
            let after = std::fs::symlink_metadata(path)
                .map_err(|_| crate::error::KeyFileError::Unavailable)?;
            if after.file_type().is_file()
                && !after.file_type().is_symlink()
                && after.dev() == opened.dev()
                && after.ino() == opened.ino()
                && after.permissions().mode() & 0o077 == 0
                && after.len() == expected_len
            {
                return Ok(*bytes);
            }
            return Err(crate::error::KeyFileError::Unavailable);
        }
        if opened.permissions().mode() & 0o077 != 0 || before.permissions().mode() & 0o077 != 0 {
            return Err(crate::error::KeyFileError::InsecurePermissions);
        }
        Err(crate::error::KeyFileError::Malformed)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, expected_len);
        Err(crate::error::KeyFileError::Unsupported)
    }
}
