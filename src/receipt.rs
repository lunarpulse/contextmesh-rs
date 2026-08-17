//! Option B agent-experience receipts (gate B1).
//!
//! A receipt is a self-contained, content-addressed, signed Option B artifact
//! that references Option A event IDs plus a task/recipient-state binding and
//! selector provenance. Receipts are Option B artifacts: they are exported as
//! canonical JSON files, they never enter Option A's store, and they never
//! mutate Option A history.
//!
//! The signature and identity scheme is Option A's own (Ed25519
//! `verify_strict` plus BLAKE3 domain separation); no new signature primitive
//! is introduced. Determinism is guaranteed on this structural/verification
//! path only — the receipt ID, canonical bytes, and signature checks are
//! deterministic; the semantic selection that produced the receipt is
//! recorded, not re-derived.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::{Value, json};

use crate::crypto::{SigningIdentity, verify_domain_message};
use crate::error::{ContractError, Result};
use crate::model::{
    AuthorId, ContextId, EventId, canonical_payload_bytes, into_object, into_string, parse_text,
    reject_unknown, strict_json, take_required, validate_json_value,
};
use crate::store::Store;

/// BLAKE3 derive-key context for version-1 receipt IDs.
pub const RECEIPT_ID_DOMAIN: &str = "org.aaif.contextmesh.receipt-id.v1";
/// ASCII prefix, including NUL separator, for receipt signature messages.
pub const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"org.aaif.contextmesh.receipt-signature.v1\0";
/// Receipt schema version.
pub const RECEIPT_VERSION: u8 = 1;
/// Maximum number of referenced Option A events in one receipt.
pub const MAX_RECEIPT_EVENTS: usize = 4096;
/// Maximum task verbatim bytes.
pub const MAX_TASK_BYTES: usize = 65_536;
/// Maximum selector identity bytes.
pub const MAX_SELECTOR_IDENTITY_BYTES: usize = 128;
/// Maximum selector version bytes.
pub const MAX_SELECTOR_VERSION_BYTES: usize = 64;
/// Maximum selector configuration-hash bytes.
pub const MAX_SELECTOR_CONFIG_HASH_BYTES: usize = 128;
/// Maximum bytes in one omission/uncertainty note.
pub const MAX_RECEIPT_NOTE_BYTES: usize = 512;
/// Maximum number of omission/uncertainty notes in one receipt.
pub const MAX_RECEIPT_NOTES: usize = 4096;
/// Maximum canonical receipt wire bytes.
pub const MAX_RECEIPT_WIRE_BYTES: usize = 2_097_152;

macro_rules! receipt_fixed_type {
    ($(#[$meta:meta])* $name:ident, $prefix:literal, $size:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        pub struct $name([u8; $size]);

        impl $name {
            /// Constructs the typed value from its exact raw bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $size]) -> Self {
                Self(bytes)
            }

            /// Returns a copy of the exact raw bytes.
            #[must_use]
            pub const fn to_bytes(self) -> [u8; $size] {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str($prefix)?;
                formatter.write_str(&URL_SAFE_NO_PAD.encode(self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, formatter)
            }
        }

        impl FromStr for $name {
            type Err = ContractError;

            fn from_str(text: &str) -> Result<Self> {
                const ENCODED_LEN: usize = ($size * 8_usize).div_ceil(6);
                if !text.starts_with($prefix) || text.len() != $prefix.len() + ENCODED_LEN {
                    return Err(ContractError::InvalidEncoding);
                }
                let encoded = &text[$prefix.len()..];
                let decoded = URL_SAFE_NO_PAD
                    .decode(encoded)
                    .map_err(|_| ContractError::InvalidEncoding)?;
                let bytes: [u8; $size] = decoded
                    .try_into()
                    .map_err(|_| ContractError::InvalidEncoding)?;
                let value = Self(bytes);
                if value.to_string() != text {
                    return Err(ContractError::InvalidEncoding);
                }
                Ok(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(de::Error::custom)
            }
        }
    };
}

receipt_fixed_type!(
    /// A BLAKE3-derived immutable receipt identifier (rcpt1_ plus 32 bytes).
    ReceiptId,
    "rcpt1_",
    32
);
receipt_fixed_type!(
    /// An Ed25519 receipt signature (rsig1_ plus 64 bytes).
    ReceiptSignature,
    "rsig1_",
    64
);

/// Task description exactly as accepted, recorded verbatim plus a content hash
/// (resolved decision 1).
///
/// A structured canonical form is recorded only when the caller supplies one;
/// the system never claims to derive a deterministic canonical form from free
/// text.
#[derive(Clone, Debug, Serialize)]
pub struct TaskRecordV1 {
    verbatim: String,
    content_hash: String,
    structured: Option<Value>,
}

impl TaskRecordV1 {
    /// Records a free-text task verbatim, hashing its bytes with BLAKE3.
    pub fn from_verbatim(verbatim: String, structured: Option<Value>) -> Result<Self> {
        let content_hash = task_content_hash(verbatim.as_bytes());
        Self::new(verbatim, content_hash, structured)
    }

    /// Constructs a task record with an explicit content hash.
    pub fn new(verbatim: String, content_hash: String, structured: Option<Value>) -> Result<Self> {
        let task = Self {
            verbatim,
            content_hash,
            structured,
        };
        task.validate()?;
        Ok(task)
    }

    /// Re-runs every semantic and size validation rule.
    pub fn validate(&self) -> Result<()> {
        let bytes = self.verbatim.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_TASK_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        if self.content_hash.is_empty() || !self.content_hash.starts_with("blake3_") {
            return Err(ContractError::InvalidEncoding);
        }
        if let Some(structured) = &self.structured {
            validate_json_value(structured, 1)?;
            canonical_payload_bytes(structured)?;
        }
        Ok(())
    }

    /// Returns the task verbatim exactly as accepted.
    #[must_use]
    pub fn verbatim(&self) -> &str {
        &self.verbatim
    }

    /// Returns the recorded BLAKE3 content hash of the verbatim text.
    #[must_use]
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    /// Returns the caller-supplied structured canonical form, if any.
    #[must_use]
    pub fn structured(&self) -> Option<&Value> {
        self.structured.as_ref()
    }
}

/// BLAKE3 content hash of task text in `blake3_` + lowercase-hex form.
pub fn task_content_hash(text: &[u8]) -> String {
    let digest = blake3::hash(text);
    let mut encoded = String::with_capacity(7 + 64);
    encoded.push_str("blake3_");
    for byte in digest.as_bytes() {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

/// Selector provenance recorded in every receipt (spec Always rule): identity,
/// version, and configuration hash.
#[derive(Clone, Debug, Serialize)]
pub struct SelectorRecordV1 {
    identity: String,
    version: String,
    config_hash: String,
}

impl SelectorRecordV1 {
    /// Constructs a validated selector record.
    pub fn new(identity: String, version: String, config_hash: String) -> Result<Self> {
        let selector = Self {
            identity,
            version,
            config_hash,
        };
        selector.validate()?;
        Ok(selector)
    }

    /// Re-runs every semantic and size validation rule.
    pub fn validate(&self) -> Result<()> {
        let identity = self.identity.as_bytes();
        let version = self.version.as_bytes();
        let config_hash = self.config_hash.as_bytes();
        if identity.is_empty() || identity.len() > MAX_SELECTOR_IDENTITY_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        if version.is_empty() || version.len() > MAX_SELECTOR_VERSION_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        if config_hash.is_empty() || config_hash.len() > MAX_SELECTOR_CONFIG_HASH_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        Ok(())
    }

    /// Returns the selector identity.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Returns the selector version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the selector configuration hash.
    #[must_use]
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }
}

/// Recipient known-history binding: the checkpoint B4/B5 build on.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RecipientStateV1 {
    head: EventId,
}

impl RecipientStateV1 {
    /// Constructs a recipient-state binding against a known-history head.
    #[must_use]
    pub const fn new(head: EventId) -> Self {
        Self { head }
    }

    /// Returns the recipient known-history head event ID.
    #[must_use]
    pub const fn head(&self) -> EventId {
        self.head
    }
}

/// The complete immutable version-1 receipt body.
#[derive(Clone, Debug, Serialize)]
pub struct ReceiptBodyV1 {
    version: u8,
    context: ContextId,
    events: Vec<EventId>,
    task: TaskRecordV1,
    recipient: RecipientStateV1,
    selector: SelectorRecordV1,
    omissions: Vec<String>,
    uncertainty: Vec<String>,
    created_at: String,
    author: AuthorId,
}

impl ReceiptBodyV1 {
    /// Constructs and validates a version-1 receipt body.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: ContextId,
        events: Vec<EventId>,
        task: TaskRecordV1,
        recipient: RecipientStateV1,
        selector: SelectorRecordV1,
        omissions: Vec<String>,
        uncertainty: Vec<String>,
        created_at: String,
        author: AuthorId,
    ) -> Result<Self> {
        let body = Self {
            version: RECEIPT_VERSION,
            context,
            events,
            task,
            recipient,
            selector,
            omissions,
            uncertainty,
            created_at,
            author,
        };
        body.validate()?;
        Ok(body)
    }

    /// Re-runs every semantic and size validation rule.
    pub fn validate(&self) -> Result<()> {
        if self.version != RECEIPT_VERSION {
            return Err(ContractError::UnsupportedVersion);
        }
        if self.events.len() > MAX_RECEIPT_EVENTS {
            return Err(ContractError::LimitExceeded);
        }
        if self
            .events
            .windows(2)
            .any(|pair| pair[0].to_string() >= pair[1].to_string())
        {
            return Err(ContractError::ParentOrder);
        }
        self.task.validate()?;
        self.selector.validate()?;
        if self.omissions.len() + self.uncertainty.len() > MAX_RECEIPT_NOTES {
            return Err(ContractError::LimitExceeded);
        }
        for note in self.omissions.iter().chain(self.uncertainty.iter()) {
            let bytes = note.as_bytes();
            if bytes.is_empty() || bytes.len() > MAX_RECEIPT_NOTE_BYTES {
                return Err(ContractError::LimitExceeded);
            }
        }
        if !valid_utc_timestamp(&self.created_at) {
            return Err(ContractError::InvalidEncoding);
        }
        let canonical = crate::model::canonicalize(self)?;
        if canonical.len() > MAX_RECEIPT_WIRE_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        Ok(())
    }

    /// Returns the RFC 8785/JCS canonical body bytes after full validation.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::model::canonicalize(self)
    }

    /// Returns the frozen receipt schema version (always 1).
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Returns the Option A context all referenced events must belong to.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns referenced Option A event IDs in canonical order.
    #[must_use]
    pub fn events(&self) -> &[EventId] {
        &self.events
    }

    /// Returns the recorded task description.
    #[must_use]
    pub const fn task(&self) -> &TaskRecordV1 {
        &self.task
    }

    /// Returns the recipient known-history binding.
    #[must_use]
    pub const fn recipient(&self) -> &RecipientStateV1 {
        &self.recipient
    }

    /// Returns the recorded selector provenance.
    #[must_use]
    pub const fn selector(&self) -> &SelectorRecordV1 {
        &self.selector
    }

    /// Returns the explicit omission notes (populated from B6 onward).
    #[must_use]
    pub fn omissions(&self) -> &[String] {
        &self.omissions
    }

    /// Returns the explicit uncertainty markers.
    #[must_use]
    pub fn uncertainty(&self) -> &[String] {
        &self.uncertainty
    }

    /// Returns the RFC 3339 UTC creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    /// Returns the signing author identity.
    #[must_use]
    pub const fn author(&self) -> AuthorId {
        self.author
    }
}

/// The complete immutable version-1 signed receipt envelope.
#[derive(Clone, Debug, Serialize)]
pub struct SignedReceiptV1 {
    receipt_id: ReceiptId,
    body: ReceiptBodyV1,
    signature: ReceiptSignature,
}

impl SignedReceiptV1 {
    /// Creates, validates, identifies, and signs a receipt body.
    pub fn issue(identity: &SigningIdentity, body: ReceiptBodyV1) -> Result<Self> {
        body.validate()?;
        if body.author() != identity.author() {
            return Err(ContractError::AuthorMismatch);
        }
        let receipt_id = derive_receipt_id(&body)?;
        let signature_bytes =
            identity.sign_domain_message(RECEIPT_SIGNATURE_DOMAIN, &receipt_id.to_bytes());
        let signature_bytes: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| ContractError::SignatureInvalid)?;
        let receipt = Self::from_verified_parts(
            receipt_id,
            body,
            ReceiptSignature::from_bytes(signature_bytes),
        );
        receipt.verify()?;
        Ok(receipt)
    }

    pub(crate) const fn from_verified_parts(
        receipt_id: ReceiptId,
        body: ReceiptBodyV1,
        signature: ReceiptSignature,
    ) -> Self {
        Self {
            receipt_id,
            body,
            signature,
        }
    }

    /// Parses, bounds, validates, recomputes, and strictly verifies wire bytes.
    ///
    /// No receipt is returned unless every validation and cryptographic check
    /// has completed successfully.
    pub fn from_wire(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_RECEIPT_WIRE_BYTES {
            return Err(ContractError::WireTooLarge);
        }
        let value = strict_json(input)?;
        let mut object = into_object(value)?;
        reject_unknown(&object, &["receipt_id", "body", "signature"])?;
        let receipt_id_value = take_required(&mut object, "receipt_id")?;
        let body_value = take_required(&mut object, "body")?;
        let signature_value = take_required(&mut object, "signature")?;

        let body = parse_body(body_value)?;
        let receipt_id = parse_text(receipt_id_value)?;
        let signature = parse_text(signature_value)?;
        verify_parts(&body, receipt_id, signature)?;
        Ok(Self::from_verified_parts(receipt_id, body, signature))
    }

    /// Independently revalidates the body, ID, author key, and signature.
    pub fn verify(&self) -> Result<()> {
        verify_parts(&self.body, self.receipt_id, self.signature)
    }

    /// Renders the entire envelope as RFC 8785/JCS canonical wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>> {
        self.verify()?;
        crate::model::canonicalize(self)
    }

    /// Returns the recomputed and verified receipt identifier.
    #[must_use]
    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }

    /// Returns the complete validated receipt body.
    #[must_use]
    pub const fn body(&self) -> &ReceiptBodyV1 {
        &self.body
    }

    /// Returns the strict Ed25519 receipt signature.
    #[must_use]
    pub const fn signature(&self) -> ReceiptSignature {
        self.signature
    }

    /// Verifies every referenced event against the Option A store.
    ///
    /// A reference is satisfied when the event exists in the store (only
    /// authorized, admitted events are stored), belongs to the receipt's stated
    /// context, and reparses with strict cryptographic verification. The
    /// recipient known-history head is checked the same way, so a receipt
    /// bound to an unknown or foreign recipient state fails closed.
    pub async fn verify_against_dag(
        &self,
        store: &Store,
    ) -> crate::error::StoreResult<ReceiptDagReport> {
        let context = self.body.context();
        let mut findings = Vec::new();
        let mut checked = 0usize;
        for event_id in self.body.events() {
            match check_event(store, context, *event_id).await? {
                EventCheck::Present => checked += 1,
                EventCheck::WrongContext => findings.push(ReceiptDagFinding {
                    reason: "wrong-context",
                    event: *event_id,
                }),
                EventCheck::Missing => findings.push(ReceiptDagFinding {
                    reason: "missing",
                    event: *event_id,
                }),
            }
        }
        let head = self.body.recipient().head();
        match check_event(store, context, head).await? {
            EventCheck::Present => checked += 1,
            EventCheck::WrongContext => findings.push(ReceiptDagFinding {
                reason: "recipient-wrong-context",
                event: head,
            }),
            EventCheck::Missing => findings.push(ReceiptDagFinding {
                reason: "recipient-missing",
                event: head,
            }),
        }
        Ok(ReceiptDagReport {
            valid: findings.is_empty(),
            checked_events: checked,
            findings,
        })
    }
}

enum EventCheck {
    Present,
    WrongContext,
    Missing,
}

async fn check_event(
    store: &Store,
    context: ContextId,
    id: EventId,
) -> crate::error::StoreResult<EventCheck> {
    match store.event(id).await? {
        Some(event) => Ok(if event.body().context() == context {
            EventCheck::Present
        } else {
            EventCheck::WrongContext
        }),
        None => Ok(EventCheck::Missing),
    }
}

/// One bounded non-secret finding about a receipt's DAG references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptDagFinding {
    /// Stable reason class: `missing`, `wrong-context`, `recipient-missing`,
    /// or `recipient-wrong-context`.
    pub reason: &'static str,
    /// The affected event.
    pub event: EventId,
}

/// Complete bounded outcome of receipt-to-DAG verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptDagReport {
    /// True only when every reference exists, is authorized, and is in context.
    pub valid: bool,
    /// Number of events verified present and in context (including the head).
    pub checked_events: usize,
    /// Bounded non-secret findings.
    pub findings: Vec<ReceiptDagFinding>,
}

/// Derives the version-1 BLAKE3 receipt ID from canonical body bytes.
pub fn derive_receipt_id(body: &ReceiptBodyV1) -> Result<ReceiptId> {
    let canonical = body.canonical_bytes()?;
    let mut hasher = blake3::Hasher::new_derive_key(RECEIPT_ID_DOMAIN);
    hasher.update(&canonical);
    Ok(ReceiptId::from_bytes(*hasher.finalize().as_bytes()))
}

fn parse_body(value: Value) -> Result<ReceiptBodyV1> {
    let mut object = into_object(value)?;
    reject_unknown(
        &object,
        &[
            "version",
            "context",
            "events",
            "task",
            "recipient",
            "selector",
            "omissions",
            "uncertainty",
            "created_at",
            "author",
        ],
    )?;
    let version_value = take_required(&mut object, "version")?;
    let context_value = take_required(&mut object, "context")?;
    let events_value = take_required(&mut object, "events")?;
    let task_value = take_required(&mut object, "task")?;
    let recipient_value = take_required(&mut object, "recipient")?;
    let selector_value = take_required(&mut object, "selector")?;
    let omissions_value = take_required(&mut object, "omissions")?;
    let uncertainty_value = take_required(&mut object, "uncertainty")?;
    let created_at_value = take_required(&mut object, "created_at")?;
    let author_value = take_required(&mut object, "author")?;

    let version = match version_value {
        Value::Number(number) if number.as_u64() == Some(u64::from(RECEIPT_VERSION)) => {
            RECEIPT_VERSION
        }
        Value::Number(_) => return Err(ContractError::UnsupportedVersion),
        _ => return Err(ContractError::JsonSyntax),
    };
    let context = parse_text(context_value)?;
    let events = match events_value {
        Value::Array(values) => values
            .into_iter()
            .map(parse_text)
            .collect::<Result<Vec<EventId>>>()?,
        _ => return Err(ContractError::JsonSyntax),
    };
    let task = parse_task(task_value)?;
    let recipient = parse_recipient(recipient_value)?;
    let selector = parse_selector(selector_value)?;
    let omissions = parse_notes(omissions_value)?;
    let uncertainty = parse_notes(uncertainty_value)?;
    let created_at = into_string(created_at_value)?;
    let author = parse_text(author_value)?;

    let body = ReceiptBodyV1 {
        version,
        context,
        events,
        task,
        recipient,
        selector,
        omissions,
        uncertainty,
        created_at,
        author,
    };
    body.validate()?;
    Ok(body)
}

fn parse_task(value: Value) -> Result<TaskRecordV1> {
    let mut object = into_object(value)?;
    reject_unknown(&object, &["verbatim", "content_hash", "structured"])?;
    let verbatim = into_string(take_required(&mut object, "verbatim")?)?;
    let content_hash = into_string(take_required(&mut object, "content_hash")?)?;
    let structured = match object.remove("structured") {
        Some(Value::Null) | None => None,
        Some(value) => Some(value),
    };
    TaskRecordV1::new(verbatim, content_hash, structured)
}

fn parse_recipient(value: Value) -> Result<RecipientStateV1> {
    let mut object = into_object(value)?;
    reject_unknown(&object, &["head"])?;
    let head = parse_text(take_required(&mut object, "head")?)?;
    Ok(RecipientStateV1::new(head))
}

fn parse_selector(value: Value) -> Result<SelectorRecordV1> {
    let mut object = into_object(value)?;
    reject_unknown(&object, &["identity", "version", "config_hash"])?;
    let identity = into_string(take_required(&mut object, "identity")?)?;
    let version = into_string(take_required(&mut object, "version")?)?;
    let config_hash = into_string(take_required(&mut object, "config_hash")?)?;
    SelectorRecordV1::new(identity, version, config_hash)
}

fn parse_notes(value: Value) -> Result<Vec<String>> {
    match value {
        Value::Array(values) => values.into_iter().map(into_string).collect(),
        _ => Err(ContractError::JsonSyntax),
    }
}

fn verify_parts(
    body: &ReceiptBodyV1,
    supplied_id: ReceiptId,
    signature: ReceiptSignature,
) -> Result<()> {
    body.validate()?;
    let expected_id = derive_receipt_id(body)?;
    if supplied_id != expected_id {
        return Err(ContractError::IdMismatch);
    }
    verify_domain_message(
        body.author(),
        RECEIPT_SIGNATURE_DOMAIN,
        &supplied_id.to_bytes(),
        &signature.to_bytes(),
    )
}

/// Renders the current UTC time as an RFC 3339 second-precision string.
///
/// Deterministic structural formatting without a wall-clock dependency; the
/// second precision is sufficient for receipt provenance.
#[must_use]
pub fn utc_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs();
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(i64::try_from(days).unwrap_or(0));
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant's civil-from-days algorithm for proleptic Gregorian dates.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (
        year,
        u32::try_from(month).unwrap_or(0),
        u32::try_from(day).unwrap_or(0),
    )
}

/// Strictly validates the `YYYY-MM-DDTHH:MM:SSZ` UTC form produced here.
fn valid_utc_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let mut value = 0u32;
        for byte in &bytes[range] {
            if !byte.is_ascii_digit() {
                return None;
            }
            value = value * 10 + u32::from(*byte - b'0');
        }
        Some(value)
    };
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        digits(0..4),
        digits(5..7),
        digits(8..10),
        digits(11..13),
        digits(14..16),
        digits(17..19),
    ) else {
        return false;
    };
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }
    hour <= 23 && minute <= 59 && second <= 59
}

/// Writes a receipt as canonical wire bytes to a new file.
pub fn export_receipt(receipt: &SignedReceiptV1, path: &Path) -> std::io::Result<()> {
    let wire = receipt.to_wire().map_err(std::io::Error::other)?;
    std::fs::write(path, wire)
}

/// Reads and strictly verifies a receipt artifact from a file.
pub fn import_receipt(path: &Path) -> std::io::Result<SignedReceiptV1> {
    let wire = std::fs::read(path)?;
    SignedReceiptV1::from_wire(&wire).map_err(std::io::Error::other)
}

/// Convenience accessor for CLI reports: renders the receipt as a JSON value.
#[must_use]
pub fn receipt_json(receipt: &SignedReceiptV1) -> Value {
    json!({
        "receipt_id": receipt.receipt_id().to_string(),
        "context": receipt.body().context().to_string(),
        "author": receipt.body().author().to_string(),
        "events": receipt.body().events().len(),
        "task_hash": receipt.body().task().content_hash(),
        "recipient_head": receipt.body().recipient().head().to_string(),
        "selector": receipt.body().selector().identity(),
        "selector_version": receipt.body().selector().version(),
        "created_at": receipt.body().created_at()
    })
}
