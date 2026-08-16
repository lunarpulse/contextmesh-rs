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
