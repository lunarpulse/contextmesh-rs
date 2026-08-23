//! Frozen OutcomeLedgerV1 value types, typed text encodings, and limits.
//!
//! Every type here is read-only after construction and validated at the exact
//! frozen v1 boundaries. Constructors check and reject; they never sort,
//! truncate, or silently substitute caller input. The body and envelope
//! composition (and therefore wire parsing of composites) belong to
//! `outcome.rs` in a later stage; this module defines the checked values.

use std::fmt;
use std::str::FromStr;

use base64::Engine as _;
use serde::de::{self, Deserializer, Unexpected};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::error::OutcomeError;

/// Frozen OutcomeLedgerV1 version marker.
pub const OUTCOME_VERSION: u8 = 1;

/// Hard maximum bytes for one canonical artifact (raw input and output).
pub const MAX_OUTCOME_WIRE_BYTES: usize = 2_097_152;

/// Hard maximum total EventId-valued body occurrences.
pub const MAX_OUTCOME_EVENT_REFERENCES: usize = 4_096;

/// Hard maximum entries in the `attempts` array.
pub const MAX_OUTCOME_ATTEMPTS: usize = 1_024;

/// Hard maximum entries in the `dead_ends` array.
pub const MAX_OUTCOME_DEAD_ENDS: usize = 1_024;

/// Hard maximum entries in the `attribution_marks` array.
pub const MAX_OUTCOME_ATTRIBUTION_MARKS: usize = 4_096;

/// Hard maximum warnings.
pub const MAX_OUTCOME_NOTES: usize = 64;

/// Hard maximum UTF-8 bytes for one warning or unavailable reason.
pub const MAX_OUTCOME_NOTE_BYTES: usize = 1_024;

/// Hard cap on strict JSON nesting depth.
pub const MAX_JSON_DEPTH: usize = 64;

/// Hard maximum mechanism identity bytes.
pub const MAX_MECHANISM_IDENTITY_BYTES: usize = 128;

/// Hard maximum mechanism version bytes.
pub const MAX_MECHANISM_VERSION_BYTES: usize = 64;

/// Hard maximum category ASCII bytes.
pub const MAX_CATEGORY_BYTES: usize = 64;

/// Hard maximum external artifact ID bytes.
pub const MAX_EXTERNAL_ARTIFACT_ID_BYTES: usize = 128;

/// Maximum quality parts-per-million value.
pub const MAX_QUALITY_PPM: u64 = 1_000_000;

/// Maximum safe integer (2^53 - 1) per the frozen integer contract.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Typed ID domain separation bytes, final byte `0x00`.
pub const OUTCOME_ID_DOMAIN: &[u8] = b"org.aaif.contextmesh.oc.outcome-ledger-id.v1\0";

/// Signature domain separation bytes, final byte `0x00`.
pub const OUTCOME_SIGNATURE_DOMAIN: &[u8] =
    b"org.aaif.contextmesh.oc.outcome-ledger-signature.v1\0";

/// Input-ref fingerprint domain separation bytes, final byte `0x00`.
pub const INPUT_REF_FINGERPRINT_DOMAIN: &[u8] = b"org.aaif.contextmesh.oc.input-ref-snapshot.v1\0";

const URL_SAFE_NO_PAD: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Caller-configurable limits; every field is nonzero and at or below its
/// frozen hard maximum. `default()` equals every hard maximum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeLimits {
    /// Maximum bytes of one canonical artifact.
    pub max_wire_bytes: usize,
    /// Maximum total EventId-valued body occurrences.
    pub max_event_references: usize,
    /// Maximum `attempts` entries.
    pub max_attempts: usize,
    /// Maximum `dead_ends` entries.
    pub max_dead_ends: usize,
    /// Maximum `attribution_marks` entries.
    pub max_attribution_marks: usize,
    /// Maximum warnings.
    pub max_warnings: usize,
    /// Maximum UTF-8 bytes for one warning or unavailable reason.
    pub max_note_bytes: usize,
    /// Maximum strict JSON nesting depth.
    pub max_json_depth: usize,
    /// Maximum mechanism identity bytes.
    pub max_mechanism_identity_bytes: usize,
    /// Maximum mechanism version bytes.
    pub max_mechanism_version_bytes: usize,
    /// Maximum category ASCII bytes.
    pub max_category_bytes: usize,
    /// Maximum external artifact ID bytes.
    pub max_external_artifact_id_bytes: usize,
}

impl Default for OutcomeLimits {
    fn default() -> Self {
        Self {
            max_wire_bytes: MAX_OUTCOME_WIRE_BYTES,
            max_event_references: MAX_OUTCOME_EVENT_REFERENCES,
            max_attempts: MAX_OUTCOME_ATTEMPTS,
            max_dead_ends: MAX_OUTCOME_DEAD_ENDS,
            max_attribution_marks: MAX_OUTCOME_ATTRIBUTION_MARKS,
            max_warnings: MAX_OUTCOME_NOTES,
            max_note_bytes: MAX_OUTCOME_NOTE_BYTES,
            max_json_depth: MAX_JSON_DEPTH,
            max_mechanism_identity_bytes: MAX_MECHANISM_IDENTITY_BYTES,
            max_mechanism_version_bytes: MAX_MECHANISM_VERSION_BYTES,
            max_category_bytes: MAX_CATEGORY_BYTES,
            max_external_artifact_id_bytes: MAX_EXTERNAL_ARTIFACT_ID_BYTES,
        }
    }
}

impl OutcomeLimits {
    /// Checks every field is nonzero and at or below its hard maximum.
    ///
    /// # Errors
    /// Returns [`OutcomeError::LimitExceeded`] when any field is zero or
    /// above its frozen hard maximum.
    pub fn validate(self) -> Result<(), OutcomeError> {
        let hard = Self::default();
        let fields = [
            (self.max_wire_bytes, hard.max_wire_bytes),
            (self.max_event_references, hard.max_event_references),
            (self.max_attempts, hard.max_attempts),
            (self.max_dead_ends, hard.max_dead_ends),
            (self.max_attribution_marks, hard.max_attribution_marks),
            (self.max_warnings, hard.max_warnings),
            (self.max_note_bytes, hard.max_note_bytes),
            (self.max_json_depth, hard.max_json_depth),
            (
                self.max_mechanism_identity_bytes,
                hard.max_mechanism_identity_bytes,
            ),
            (
                self.max_mechanism_version_bytes,
                hard.max_mechanism_version_bytes,
            ),
            (self.max_category_bytes, hard.max_category_bytes),
            (
                self.max_external_artifact_id_bytes,
                hard.max_external_artifact_id_bytes,
            ),
        ];
        if fields
            .iter()
            .any(|&(value, hard_max)| value == 0 || value > hard_max)
        {
            return Err(OutcomeError::LimitExceeded);
        }
        Ok(())
    }
}

macro_rules! fixed_text_type {
    ($(#[$meta:meta])* $name:ident, $prefix:literal, $size:literal) => {
        $(#[$meta])*
        #[derive(Clone, Eq, Hash, PartialEq)]
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

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.to_string().cmp(&other.to_string())
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl FromStr for $name {
            type Err = OutcomeError;

            fn from_str(text: &str) -> Result<Self, OutcomeError> {
                const ENCODED_LEN: usize = ($size * 8_usize).div_ceil(6);
                if !text.starts_with($prefix) || text.len() != $prefix.len() + ENCODED_LEN {
                    return Err(OutcomeError::Malformed);
                }
                let decoded = URL_SAFE_NO_PAD
                    .decode(&text[$prefix.len()..])
                    .map_err(|_| OutcomeError::Malformed)?;
                let bytes: [u8; $size] =
                    decoded.try_into().map_err(|_| OutcomeError::Malformed)?;
                Ok(Self(bytes))
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(|_| {
                    de::Error::invalid_value(
                        Unexpected::Str(&text),
                        &concat!(stringify!($name), " typed text"),
                    )
                })
            }
        }
    };
}

fixed_text_type!(
    /// Outcome artifact identifier (`ocout1_` plus 32 bytes).
    OutcomeId,
    "ocout1_",
    32
);

fixed_text_type!(
    /// Outcome artifact signature (`ocsig1_` plus 64 bytes).
    OutcomeSignature,
    "ocsig1_",
    64
);

fixed_text_type!(
    /// Input-ref snapshot fingerprint (`ocrefs1_` plus 32 bytes).
    InputRefFingerprint,
    "ocrefs1_",
    32
);

/// Typed BLAKE3 hash text: `blake3_` plus exactly 64 lowercase hex chars.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Blake3HashText(String);

impl Blake3HashText {
    fn validate(text: &str) -> Result<(), OutcomeError> {
        let Some(hex) = text.strip_prefix("blake3_") else {
            return Err(OutcomeError::Malformed);
        };
        let ok_len = hex.len() == 64;
        let ok_alpha = hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if ok_len && ok_alpha {
            Ok(())
        } else {
            Err(OutcomeError::Malformed)
        }
    }

    /// Parses typed hash text, rejecting any non-exact spelling.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for wrong prefix, length,
    /// alphabet, or case.
    pub fn parse(text: &str) -> Result<Self, OutcomeError> {
        Self::validate(text)?;
        Ok(Self(text.to_owned()))
    }

    /// Builds typed hash text from a raw 32-byte BLAKE3 digest.
    #[must_use]
    pub fn from_digest(bytes: [u8; 32]) -> Self {
        Self(format!("blake3_{}", hex_lower(&bytes)))
    }

    /// Renders the exact typed text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

impl fmt::Display for Blake3HashText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for Blake3HashText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl FromStr for Blake3HashText {
    type Err = OutcomeError;

    fn from_str(text: &str) -> Result<Self, OutcomeError> {
        Self::parse(text)
    }
}

impl Serialize for Blake3HashText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Blake3HashText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text)
            .map_err(|_| de::Error::invalid_value(Unexpected::Str(&text), &"blake3_ hash text"))
    }
}

/// UTF-8 date-time text in exact `YYYY-MM-DDTHH:MM:SSZ` UTC form, year >= 1970.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct TimestampText(String);

impl TimestampText {
    fn validate(text: &str) -> Result<(), OutcomeError> {
        let bytes = text.as_bytes();
        if bytes.len() != 20
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes[10] != b'T'
            || bytes[13] != b':'
            || bytes[16] != b':'
            || bytes[19] != b'Z'
        {
            return Err(OutcomeError::Malformed);
        }
        let digit = |i: usize| -> Result<u32, OutcomeError> {
            let b = bytes[i];
            if b.is_ascii_digit() {
                Ok(u32::from(b - b'0'))
            } else {
                Err(OutcomeError::Malformed)
            }
        };
        let year = digit(0)? * 1000 + digit(1)? * 100 + digit(2)? * 10 + digit(3)?;
        let month = digit(5)? * 10 + digit(6)?;
        let day = digit(8)? * 10 + digit(9)?;
        let hour = digit(11)? * 10 + digit(12)?;
        let minute = digit(14)? * 10 + digit(15)?;
        let second = digit(17)? * 10 + digit(18)?;
        if year < 1970 || !(1..=12).contains(&month) {
            return Err(OutcomeError::Malformed);
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err(OutcomeError::Malformed),
        };
        if day == 0 || day > days || hour > 23 || minute > 59 || second > 59 {
            return Err(OutcomeError::Malformed);
        }
        Ok(())
    }

    /// Parses exact UTC timestamp text with full Gregorian validation.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for any non-exact spelling or
    /// invalid date/time component.
    pub fn parse(text: &str) -> Result<Self, OutcomeError> {
        Self::validate(text)?;
        Ok(Self(text.to_owned()))
    }

    /// Renders the exact typed text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TimestampText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for TimestampText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl FromStr for TimestampText {
    type Err = OutcomeError;

    fn from_str(text: &str) -> Result<Self, OutcomeError> {
        Self::parse(text)
    }
}

impl Serialize for TimestampText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for TimestampText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(|_| {
            de::Error::invalid_value(Unexpected::Str(&text), &"YYYY-MM-DDTHH:MM:SSZ text")
        })
    }
}

/// Rejects empty text or text containing any C0/C1 control character.
pub(crate) fn reject_control_chars(text: &str) -> Result<(), OutcomeError> {
    if text.is_empty() || text.chars().any(char::is_control) {
        Err(OutcomeError::Malformed)
    } else {
        Ok(())
    }
}

/// Validates the frozen lowercase ASCII category grammar within a byte bound.
pub fn validate_category(text: &str, max_bytes: usize) -> Result<(), OutcomeError> {
    if text.is_empty() || text.len() > max_bytes {
        return Err(OutcomeError::Malformed);
    }
    let mut previous_separator = true;
    for byte in text.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else if matches!(byte, b'.' | b'_' | b'-') {
            if previous_separator {
                return Err(OutcomeError::Malformed);
            }
            previous_separator = true;
        } else {
            return Err(OutcomeError::Malformed);
        }
    }
    if previous_separator {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

/// Validates an opaque external artifact ID: 1..=max printable ASCII bytes.
pub(crate) fn validate_external_artifact_id(
    text: &str,
    max_bytes: usize,
) -> Result<(), OutcomeError> {
    if text.is_empty() || text.len() > max_bytes || !text.bytes().all(|b| b.is_ascii_graphic()) {
        Err(OutcomeError::Malformed)
    } else {
        Ok(())
    }
}

/// Mechanism provenance: identity, version, and caller-owned config hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MechanismRecordV1 {
    /// Nonempty mechanism identity (at most 128 UTF-8 bytes).
    pub identity: String,
    /// Nonempty mechanism version (at most 64 UTF-8 bytes).
    pub version: String,
    /// Caller-supplied configuration hash binding the exact configuration.
    pub config_hash: Blake3HashText,
}

impl MechanismRecordV1 {
    /// Validates and constructs a mechanism record.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for empty or control-containing
    /// text and [`OutcomeError::LimitExceeded`] for over-length
    /// identity/version.
    pub fn new(
        identity: String,
        version: String,
        config_hash: Blake3HashText,
        limits: &OutcomeLimits,
    ) -> Result<Self, OutcomeError> {
        reject_control_chars(&identity)?;
        reject_control_chars(&version)?;
        if identity.len() > limits.max_mechanism_identity_bytes
            || version.len() > limits.max_mechanism_version_bytes
        {
            return Err(OutcomeError::LimitExceeded);
        }
        Ok(Self {
            identity,
            version,
            config_hash,
        })
    }

    /// Validates an already-owned record against limits.
    ///
    /// # Errors
    /// Same categories as [`MechanismRecordV1::new`].
    pub fn validate(&self, limits: &OutcomeLimits) -> Result<(), OutcomeError> {
        Self::new(
            self.identity.clone(),
            self.version.clone(),
            self.config_hash.clone(),
            limits,
        )
        .map(|_| ())
    }
}

/// Caller-supplied hashes-only task binding; raw task text never enters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskBindingV1 {
    /// Ordinary BLAKE3 of the exact original task bytes.
    pub content_hash: Blake3HashText,
    /// Optional hash of a caller-owned canonical structured representation.
    pub structured_hash: Option<Blake3HashText>,
    /// Optional opaque caller-declared external artifact ID.
    pub external_artifact_id: Option<String>,
}

impl TaskBindingV1 {
    /// Hash-only constructor; never accepts task bytes or notes.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for an invalid external ID.
    pub fn new(
        content_hash: Blake3HashText,
        structured_hash: Option<Blake3HashText>,
        external_artifact_id: Option<String>,
        limits: &OutcomeLimits,
    ) -> Result<Self, OutcomeError> {
        if let Some(id) = &external_artifact_id {
            validate_external_artifact_id(id, limits.max_external_artifact_id_bytes)?;
        }
        Ok(Self {
            content_hash,
            structured_hash,
            external_artifact_id,
        })
    }

    /// Validates an already-owned binding against limits.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for an invalid external ID.
    pub fn validate(&self, limits: &OutcomeLimits) -> Result<(), OutcomeError> {
        if let Some(id) = &self.external_artifact_id {
            validate_external_artifact_id(id, limits.max_external_artifact_id_bytes)?;
        }
        Ok(())
    }
}

/// One local ref name/head pair in an input-ref snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalRefEntry {
    /// Canonical ref name.
    pub name: String,
    /// Snapshot head event (`evt1_` typed text).
    pub head: contextmesh::model::EventId,
}

/// One remote (peer, name, head) triple in an input-ref snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRefEntry {
    /// Canonical peer identity.
    pub peer: String,
    /// Canonical ref name.
    pub name: String,
    /// Snapshot head event (`evt1_` typed text).
    pub head: contextmesh::model::EventId,
}

/// Frozen input-ref snapshot with its domain-separated fingerprint.
///
/// The fingerprint binds the context plus the exact local/remote arrays and
/// is recomputed on construction, so a tampered array cannot carry a stale
/// fingerprint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InputRefSnapshotV1 {
    /// Domain-separated fingerprint over context plus exact arrays.
    pub fingerprint: InputRefFingerprint,
    /// Local refs, ascending unique by name.
    pub local: Vec<LocalRefEntry>,
    /// Remote refs, ascending unique by `(peer, name)`.
    pub remote: Vec<RemoteRefEntry>,
}

impl InputRefSnapshotV1 {
    /// Validates ordering/uniqueness and derives the bound fingerprint.
    ///
    /// The fingerprint is computed, never caller-supplied here; a caller
    /// claiming a fingerprint uses [`InputRefSnapshotV1::from_parts`].
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on disorder or duplicates.
    pub fn new(
        context: contextmesh::model::ContextId,
        local: Vec<LocalRefEntry>,
        remote: Vec<RemoteRefEntry>,
    ) -> Result<Self, OutcomeError> {
        Self::validate_order(&local, &remote)?;
        let fingerprint = Self::compute_fingerprint(&context, &local, &remote)?;
        Ok(Self {
            fingerprint,
            local,
            remote,
        })
    }

    /// Accepts a caller-supplied fingerprint and verifies it binds exactly.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on disorder/duplicates and
    /// [`OutcomeError::IdMismatch`] when the fingerprint does not bind the
    /// exact canonical arrays for `context`.
    pub fn from_parts(
        context: contextmesh::model::ContextId,
        fingerprint: InputRefFingerprint,
        local: Vec<LocalRefEntry>,
        remote: Vec<RemoteRefEntry>,
    ) -> Result<Self, OutcomeError> {
        Self::validate_order(&local, &remote)?;
        let computed = Self::compute_fingerprint(&context, &local, &remote)?;
        if computed != fingerprint {
            return Err(OutcomeError::IdMismatch);
        }
        Ok(Self {
            fingerprint,
            local,
            remote,
        })
    }

    /// Recomputes the canonical fingerprint for exact arrays and a context.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Noncanonical`] if the canonical input cannot
    /// be rendered.
    pub fn compute_fingerprint(
        context: &contextmesh::model::ContextId,
        local: &[LocalRefEntry],
        remote: &[RemoteRefEntry],
    ) -> Result<InputRefFingerprint, OutcomeError> {
        #[derive(Serialize)]
        struct FingerprintInput<'a> {
            context: &'a contextmesh::model::ContextId,
            local: &'a [LocalRefEntry],
            remote: &'a [RemoteRefEntry],
        }
        let canonical = crate::json::jcs(&FingerprintInput {
            context,
            local,
            remote,
        })?;
        let mut input = Vec::with_capacity(INPUT_REF_FINGERPRINT_DOMAIN.len() + canonical.len());
        input.extend_from_slice(INPUT_REF_FINGERPRINT_DOMAIN);
        input.extend_from_slice(&canonical);
        Ok(InputRefFingerprint::from_bytes(blake3::hash(&input).into()))
    }

    /// Validates local ascending-unique-by-name and remote
    /// ascending-unique-by-`(peer, name)` ordering.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on any duplicate or disorder.
    pub fn validate_order(
        local: &[LocalRefEntry],
        remote: &[RemoteRefEntry],
    ) -> Result<(), OutcomeError> {
        for pair in local.windows(2) {
            if pair[0].name >= pair[1].name {
                return Err(OutcomeError::Malformed);
            }
        }
        for pair in remote.windows(2) {
            if (pair[0].peer.as_str(), pair[0].name.as_str())
                >= (pair[1].peer.as_str(), pair[1].name.as_str())
            {
                return Err(OutcomeError::Malformed);
            }
        }
        Ok(())
    }

    /// Total snapshot-head count toward the event-reference occurrence cap.
    #[must_use]
    pub fn head_count(&self) -> usize {
        self.local.len() + self.remote.len()
    }
}

/// Terminal marker: caller names the terminal event or an exact unterminated
/// reason; no discovery, `null`, or fallback exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TerminalV1 {
    /// The caller-named terminal event.
    Event {
        /// The terminal event (`evt1_` typed text).
        event: contextmesh::model::EventId,
    },
    /// An explicit unterminated state with one of four exact reasons.
    Unterminated {
        /// Exactly one of `no-terminal-event`, `cancelled-before-terminal`,
        /// `collector-ended`, or `unknown`.
        reason: UnterminatedReason,
    },
}

/// Exact frozen unterminated reasons.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum UnterminatedReason {
    /// No terminal event exists.
    #[serde(rename = "no-terminal-event")]
    NoTerminalEvent,
    /// The run was cancelled before a terminal event.
    #[serde(rename = "cancelled-before-terminal")]
    CancelledBeforeTerminal,
    /// The collector ended without a terminal event.
    #[serde(rename = "collector-ended")]
    CollectorEnded,
    /// The reason is not classifiable.
    #[serde(rename = "unknown")]
    Unknown,
}

/// Caller-declared outcome value; terminal status never infers it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum OutcomeValue {
    /// The run succeeded.
    #[serde(rename = "succeeded")]
    Succeeded,
    /// The run failed.
    #[serde(rename = "failed")]
    Failed,
    /// The run partially completed.
    #[serde(rename = "partial")]
    Partial,
    /// The run was cancelled.
    #[serde(rename = "cancelled")]
    Cancelled,
    /// The outcome is unknown.
    #[serde(rename = "unknown")]
    Unknown,
}

/// Caller-declared outcome with evidence and provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeRecordV1 {
    /// One of the five exact outcome values.
    pub value: OutcomeValue,
    /// Evidence events; may be empty, always ascending unique.
    pub evidence: Vec<contextmesh::model::EventId>,
    /// The mechanism that declared this outcome.
    pub provenance: MechanismRecordV1,
}

impl OutcomeRecordV1 {
    /// Validates evidence ordering/uniqueness and mechanism bounds.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on disorder/duplicates or bad
    /// mechanism text, and [`OutcomeError::LimitExceeded`] on mechanism
    /// over-length.
    pub fn new(
        value: OutcomeValue,
        evidence: Vec<contextmesh::model::EventId>,
        provenance: MechanismRecordV1,
        limits: &OutcomeLimits,
    ) -> Result<Self, OutcomeError> {
        validate_event_id_list(&evidence)?;
        provenance.validate(limits)?;
        Ok(Self {
            value,
            evidence,
            provenance,
        })
    }
}

/// Quality: tagged available ppm or an exact bounded unavailable reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QualityV1 {
    /// A mechanism-local assessment under the signed configuration.
    Available {
        /// Parts-per-million in `0..=1_000_000`.
        value_ppm: u64,
        /// Evidence events; always ascending unique.
        evidence: Vec<contextmesh::model::EventId>,
        /// The mechanism that produced the value.
        provenance: MechanismRecordV1,
    },
    /// Quality is not available, with a bounded reason.
    Unavailable {
        /// Why quality is unavailable (at most 1,024 UTF-8 bytes).
        reason: String,
        /// The mechanism that would have produced the value.
        provenance: MechanismRecordV1,
    },
}

impl QualityV1 {
    /// Validates a quality value against frozen bounds.
    ///
    /// # Errors
    /// Returns [`OutcomeError::LimitExceeded`] for ppm above 1,000,000 or an
    /// overlong reason and [`OutcomeError::Malformed`] for empty/control
    /// reason text or disordered evidence.
    pub fn new(value: QualityV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        match value {
            Self::Available {
                value_ppm,
                evidence,
                provenance,
            } => {
                if value_ppm > MAX_QUALITY_PPM {
                    return Err(OutcomeError::LimitExceeded);
                }
                validate_event_id_list(&evidence)?;
                provenance.validate(limits)?;
                Ok(Self::Available {
                    value_ppm,
                    evidence,
                    provenance,
                })
            }
            Self::Unavailable { reason, provenance } => {
                validate_note(&reason, limits)?;
                provenance.validate(limits)?;
                Ok(Self::Unavailable { reason, provenance })
            }
        }
    }
}

/// Cost: tagged available safe-integer value or bounded unavailable reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CostValueV1 {
    /// A measured/recorded nonnegative safe integer (0 is a recorded zero).
    Available {
        /// The recorded value within `0..=2^53-1`.
        value: u64,
        /// The source/collector mechanism.
        provenance: MechanismRecordV1,
    },
    /// The cost was not recorded, with a bounded reason.
    Unavailable {
        /// Why the cost is unavailable (at most 1,024 UTF-8 bytes).
        reason: String,
        /// The would-be source/collector mechanism.
        provenance: MechanismRecordV1,
    },
}

impl CostValueV1 {
    /// Validates a cost value against frozen bounds.
    ///
    /// # Errors
    /// Returns [`OutcomeError::LimitExceeded`] for values above the safe
    /// integer maximum or overlong reasons and [`OutcomeError::Malformed`]
    /// for empty/control reason text.
    pub fn new(value: CostValueV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        match value {
            Self::Available { value, provenance } => {
                if value > MAX_SAFE_INTEGER {
                    return Err(OutcomeError::LimitExceeded);
                }
                provenance.validate(limits)?;
                Ok(Self::Available { value, provenance })
            }
            Self::Unavailable { reason, provenance } => {
                validate_note(&reason, limits)?;
                provenance.validate(limits)?;
                Ok(Self::Unavailable { reason, provenance })
            }
        }
    }
}

/// The five independently tagged cost fields; none is ever inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostLedgerV1 {
    /// Wall-clock milliseconds.
    pub wall_clock_ms: CostValueV1,
    /// Tool-call count.
    pub tool_calls: CostValueV1,
    /// Retry count.
    pub retries: CostValueV1,
    /// Input token count.
    pub input_tokens: CostValueV1,
    /// Output token count.
    pub output_tokens: CostValueV1,
}

impl CostLedgerV1 {
    /// Validates every field independently.
    ///
    /// # Errors
    /// Propagates [`CostValueV1::new`] categories.
    pub fn new(ledger: CostLedgerV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        Ok(Self {
            wall_clock_ms: CostValueV1::new(ledger.wall_clock_ms, limits)?,
            tool_calls: CostValueV1::new(ledger.tool_calls, limits)?,
            retries: CostValueV1::new(ledger.retries, limits)?,
            input_tokens: CostValueV1::new(ledger.input_tokens, limits)?,
            output_tokens: CostValueV1::new(ledger.output_tokens, limits)?,
        })
    }

    /// Validates an already-owned ledger.
    ///
    /// # Errors
    /// Propagates [`CostValueV1::new`] categories.
    pub fn validate(&self, limits: &OutcomeLimits) -> Result<(), OutcomeError> {
        Self::new(
            CostLedgerV1 {
                wall_clock_ms: self.wall_clock_ms.clone(),
                tool_calls: self.tool_calls.clone(),
                retries: self.retries.clone(),
                input_tokens: self.input_tokens.clone(),
                output_tokens: self.output_tokens.clone(),
            },
            limits,
        )
        .map(|_| ())
    }
}

/// Attempt status; the same five exact values as outcome.
pub use OutcomeValue as AttemptStatus;

/// Attempt error: available category+fingerprint or bounded unavailable reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AttemptErrorV1 {
    /// A recorded diagnostic error category and fingerprint.
    Available {
        /// Frozen lowercase ASCII category (1..=64 bytes).
        category: String,
        /// Opaque fingerprint of the error (never raw error text).
        fingerprint: Blake3HashText,
    },
    /// No error detail is available, with a bounded reason.
    Unavailable {
        /// Why the error detail is unavailable (at most 1,024 UTF-8 bytes).
        reason: String,
    },
}

/// One attempt node in the attempt tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptV1 {
    /// Contiguous zero-padded ID `attempt1_000000`..=`attempt1_NNNNNN`.
    pub attempt_id: String,
    /// `None` only for the single root.
    pub parent_attempt_id: Option<String>,
    /// One of the five exact statuses.
    pub status: AttemptStatus,
    /// Opaque operation fingerprint.
    pub operation_fingerprint: Blake3HashText,
    /// Referenced events; always ascending unique.
    pub event_refs: Vec<contextmesh::model::EventId>,
    /// Available diagnostic or unavailable reason.
    pub error: AttemptErrorV1,
    /// Per-attempt costs using the exact five-field schema.
    pub costs: CostLedgerV1,
    /// The attempt's provenance.
    pub provenance: MechanismRecordV1,
}

impl AttemptV1 {
    /// Validates one attempt (tree contiguity is array-level).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for bad ID/category/notes or
    /// disordered refs and [`OutcomeError::LimitExceeded`] for over-length
    /// text or unsafe integers.
    pub fn new(attempt: AttemptV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        validate_attempt_id(&attempt.attempt_id)?;
        if let Some(parent) = &attempt.parent_attempt_id {
            validate_attempt_id(parent)?;
            if parent == &attempt.attempt_id {
                return Err(OutcomeError::Malformed);
            }
        }
        validate_event_id_list(&attempt.event_refs)?;
        attempt.costs.validate(limits)?;
        attempt.provenance.validate(limits)?;
        match &attempt.error {
            AttemptErrorV1::Available { category, .. } => {
                validate_category(category, limits.max_category_bytes)?;
            }
            AttemptErrorV1::Unavailable { reason } => validate_note(reason, limits)?,
        }
        Ok(attempt)
    }
}

/// One recorded dead end.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeadEndV1 {
    /// Contiguous zero-padded ID `dead1_000000`..=`dead1_NNNNNN`.
    pub dead_end_id: String,
    /// The referenced attempt (must exist in the attempt tree).
    pub attempt_id: String,
    /// Frozen lowercase ASCII failure category.
    pub failure_category: String,
    /// Opaque error fingerprint.
    pub error_fingerprint: Blake3HashText,
    /// Referenced events; always ascending unique.
    pub event_refs: Vec<contextmesh::model::EventId>,
    /// One of four exact dispositions.
    pub disposition: Disposition,
    /// The dead end's provenance.
    pub provenance: MechanismRecordV1,
}

/// Exact frozen dead-end dispositions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Disposition {
    /// Still unresolved.
    #[serde(rename = "unresolved")]
    Unresolved,
    /// Abandoned.
    #[serde(rename = "abandoned")]
    Abandoned,
    /// Superseded.
    #[serde(rename = "superseded")]
    Superseded,
    /// Recovered.
    #[serde(rename = "recovered")]
    Recovered,
}

impl DeadEndV1 {
    /// Validates one dead end (target-attempt existence is array-level).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for bad IDs/category or disordered
    /// refs and [`OutcomeError::LimitExceeded`] for over-length category.
    pub fn new(dead_end: DeadEndV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        validate_dead_end_id(&dead_end.dead_end_id)?;
        validate_attempt_id(&dead_end.attempt_id)?;
        validate_category(&dead_end.failure_category, limits.max_category_bytes)?;
        validate_event_id_list(&dead_end.event_refs)?;
        dead_end.provenance.validate(limits)?;
        Ok(dead_end)
    }
}

/// Exact frozen attribution candidate labels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AttributionLabel {
    /// A load-bearing candidate.
    #[serde(rename = "load-bearing-candidate")]
    LoadBearingCandidate,
    /// A supporting candidate.
    #[serde(rename = "supporting-candidate")]
    SupportingCandidate,
    /// A dead-end candidate.
    #[serde(rename = "dead-end-candidate")]
    DeadEndCandidate,
    /// An unknown candidate.
    #[serde(rename = "unknown")]
    Unknown,
}

/// One caller-supplied attribution mark; a signed claim, never a score.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionMarkV1 {
    /// The marked event.
    pub event: contextmesh::model::EventId,
    /// One of the four exact candidate labels.
    pub label: AttributionLabel,
    /// Evidence events; always ascending unique.
    pub evidence: Vec<contextmesh::model::EventId>,
    /// The marking mechanism.
    pub mechanism: MechanismRecordV1,
}

impl AttributionMarkV1 {
    /// Validates one mark (composite ordering is array-level).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for disordered evidence and
    /// propagates mechanism validation categories.
    pub fn new(mark: AttributionMarkV1, limits: &OutcomeLimits) -> Result<Self, OutcomeError> {
        validate_event_id_list(&mark.evidence)?;
        mark.mechanism.validate(limits)?;
        Ok(mark)
    }

    fn composite_key(&self) -> (String, &'static str, String, String, String) {
        (
            self.event.to_string(),
            self.label.text(),
            self.mechanism.identity.clone(),
            self.mechanism.version.clone(),
            self.mechanism.config_hash.as_str().to_owned(),
        )
    }
}

impl AttributionLabel {
    /// Exact frozen wire text for this label.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::LoadBearingCandidate => "load-bearing-candidate",
            Self::SupportingCandidate => "supporting-candidate",
            Self::DeadEndCandidate => "dead-end-candidate",
            Self::Unknown => "unknown",
        }
    }
}

/// Validates strictly ascending unique canonical EventId text order.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on any duplicate or disorder.
pub fn validate_event_id_list(events: &[contextmesh::model::EventId]) -> Result<(), OutcomeError> {
    for pair in events.windows(2) {
        if pair[0].to_string() >= pair[1].to_string() {
            return Err(OutcomeError::Malformed);
        }
    }
    Ok(())
}

/// Validates note text: nonempty, control-free, within the byte bound.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] for empty/control text and
/// [`OutcomeError::LimitExceeded`] beyond the byte bound.
pub fn validate_note(text: &str, limits: &OutcomeLimits) -> Result<(), OutcomeError> {
    reject_control_chars(text)?;
    if text.len() > limits.max_note_bytes {
        return Err(OutcomeError::LimitExceeded);
    }
    Ok(())
}

fn validate_attempt_id(text: &str) -> Result<(), OutcomeError> {
    let Some(number) = text.strip_prefix("attempt1_") else {
        return Err(OutcomeError::Malformed);
    };
    if number.len() != 6 || !number.bytes().all(|b| b.is_ascii_digit()) {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

fn validate_dead_end_id(text: &str) -> Result<(), OutcomeError> {
    let Some(number) = text.strip_prefix("dead1_") else {
        return Err(OutcomeError::Malformed);
    };
    if number.len() != 6 || !number.bytes().all(|b| b.is_ascii_digit()) {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

/// Validates a whole attempts array: contiguity, one root, parent-before-child,
/// connectivity, acyclicity, and the attempt-count bound.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] for any structural violation and
/// [`OutcomeError::LimitExceeded`] beyond the attempt bound.
pub fn validate_attempt_tree(
    attempts: &[AttemptV1],
    limits: &OutcomeLimits,
) -> Result<(), OutcomeError> {
    if attempts.len() > limits.max_attempts {
        return Err(OutcomeError::LimitExceeded);
    }
    if attempts.is_empty() {
        return Ok(());
    }
    for (index, attempt) in attempts.iter().enumerate() {
        let expected = format!("attempt1_{index:06}");
        if attempt.attempt_id != expected {
            return Err(OutcomeError::Malformed);
        }
        match &attempt.parent_attempt_id {
            None => {
                if index != 0 {
                    return Err(OutcomeError::Malformed);
                }
            }
            Some(parent) => {
                if index == 0 {
                    return Err(OutcomeError::Malformed);
                }
                let parent_index = parent
                    .strip_prefix("attempt1_")
                    .and_then(|n| n.parse::<usize>().ok())
                    .ok_or(OutcomeError::Malformed)?;
                if parent_index >= index {
                    return Err(OutcomeError::Malformed);
                }
            }
        }
    }
    // Exactly one root is guaranteed: index 0 must be the root and every later
    // node has an earlier parent, so the parent chain strictly decreases and
    // terminates at index 0 without cycles or disconnected nodes.
    if attempts[0].parent_attempt_id.is_some() {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

/// Validates a whole dead-ends array: contiguity, existing targets, bounds.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] for gaps or absent targets and
/// [`OutcomeError::LimitExceeded`] beyond the dead-end bound.
pub fn validate_dead_ends(
    dead_ends: &[DeadEndV1],
    attempts: &[AttemptV1],
    limits: &OutcomeLimits,
) -> Result<(), OutcomeError> {
    if dead_ends.len() > limits.max_dead_ends {
        return Err(OutcomeError::LimitExceeded);
    }
    let attempt_ids: Vec<&str> = attempts.iter().map(|a| a.attempt_id.as_str()).collect();
    for (index, dead_end) in dead_ends.iter().enumerate() {
        let expected = format!("dead1_{index:06}");
        if dead_end.dead_end_id != expected {
            return Err(OutcomeError::Malformed);
        }
        if !attempt_ids.contains(&dead_end.attempt_id.as_str()) {
            return Err(OutcomeError::Malformed);
        }
    }
    Ok(())
}

/// Validates a whole attribution-marks array: composite ascending-unique
/// order and the mark-count bound.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on duplicates or disorder and
/// [`OutcomeError::LimitExceeded`] beyond the mark bound.
pub fn validate_attribution_marks(
    marks: &[AttributionMarkV1],
    limits: &OutcomeLimits,
) -> Result<(), OutcomeError> {
    if marks.len() > limits.max_attribution_marks {
        return Err(OutcomeError::LimitExceeded);
    }
    for pair in marks.windows(2) {
        if pair[0].composite_key() >= pair[1].composite_key() {
            return Err(OutcomeError::Malformed);
        }
    }
    Ok(())
}

/// Validates a warnings array: caller order preserved, no duplicates, each
/// note within bounds, count within bound.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on duplicates or bad text and
/// [`OutcomeError::LimitExceeded`] beyond note/count bounds.
pub fn validate_warnings(warnings: &[String], limits: &OutcomeLimits) -> Result<(), OutcomeError> {
    if warnings.len() > limits.max_warnings {
        return Err(OutcomeError::LimitExceeded);
    }
    let mut seen = std::collections::HashSet::new();
    for warning in warnings {
        validate_note(warning, limits)?;
        if !seen.insert(warning.as_str()) {
            return Err(OutcomeError::Malformed);
        }
    }
    Ok(())
}

/// Deserializes a `status`-tagged enum with exact key sets per variant.
///
/// Unlike derived internally-tagged deserialization, this rejects any mixed
/// or extra member (the frozen "exactly one variant is present" rule).
pub(crate) fn deserialize_tagged<'de, D, T, F>(deserializer: D, parse: F) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: Fn(&Value) -> Result<T, OutcomeError>,
{
    let value = Value::deserialize(deserializer)?;
    parse(&value).map_err(de::Error::custom)
}

impl<'de> Deserialize<'de> for TerminalV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?;
            match status {
                "event" => {
                    crate::json::require_exact_keys(value, &["status", "event"])?;
                    let event = serde_json::from_value::<contextmesh::model::EventId>(
                        value.get("event").cloned().ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    Ok(Self::Event { event })
                }
                "unterminated" => {
                    crate::json::require_exact_keys(value, &["status", "reason"])?;
                    let reason = serde_json::from_value::<UnterminatedReason>(
                        value
                            .get("reason")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    Ok(Self::Unterminated { reason })
                }
                _ => Err(OutcomeError::Malformed),
            }
        })
    }
}

impl<'de> Deserialize<'de> for QualityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?;
            match status {
                "available" => {
                    crate::json::require_exact_keys(
                        value,
                        &["status", "value_ppm", "evidence", "provenance"],
                    )?;
                    let value_ppm = value
                        .get("value_ppm")
                        .and_then(Value::as_u64)
                        .ok_or(OutcomeError::Malformed)?;
                    if value_ppm > MAX_QUALITY_PPM {
                        return Err(OutcomeError::LimitExceeded);
                    }
                    let evidence = serde_json::from_value::<Vec<contextmesh::model::EventId>>(
                        value
                            .get("evidence")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    let provenance = serde_json::from_value::<MechanismRecordV1>(
                        value
                            .get("provenance")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    validate_event_id_list(&evidence)?;
                    Ok(Self::Available {
                        value_ppm,
                        evidence,
                        provenance,
                    })
                }
                "unavailable" => {
                    crate::json::require_exact_keys(value, &["status", "reason", "provenance"])?;
                    let reason = value
                        .get("reason")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned();
                    let provenance = serde_json::from_value::<MechanismRecordV1>(
                        value
                            .get("provenance")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    validate_note(&reason, &OutcomeLimits::default())?;
                    Ok(Self::Unavailable { reason, provenance })
                }
                _ => Err(OutcomeError::Malformed),
            }
        })
    }
}

impl<'de> Deserialize<'de> for CostValueV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?;
            match status {
                "available" => {
                    crate::json::require_exact_keys(value, &["status", "value", "provenance"])?;
                    let amount = value
                        .get("value")
                        .and_then(Value::as_u64)
                        .ok_or(OutcomeError::Malformed)?;
                    if amount > MAX_SAFE_INTEGER {
                        return Err(OutcomeError::LimitExceeded);
                    }
                    let provenance = serde_json::from_value::<MechanismRecordV1>(
                        value
                            .get("provenance")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    Ok(Self::Available {
                        value: amount,
                        provenance,
                    })
                }
                "unavailable" => {
                    crate::json::require_exact_keys(value, &["status", "reason", "provenance"])?;
                    let reason = value
                        .get("reason")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned();
                    let provenance = serde_json::from_value::<MechanismRecordV1>(
                        value
                            .get("provenance")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?;
                    validate_note(&reason, &OutcomeLimits::default())?;
                    Ok(Self::Unavailable { reason, provenance })
                }
                _ => Err(OutcomeError::Malformed),
            }
        })
    }
}

impl<'de> Deserialize<'de> for AttemptErrorV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?;
            match status {
                "available" => {
                    crate::json::require_exact_keys(value, &["status", "category", "fingerprint"])?;
                    let category = value
                        .get("category")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned();
                    let fingerprint = Blake3HashText::parse(
                        value
                            .get("fingerprint")
                            .and_then(Value::as_str)
                            .ok_or(OutcomeError::Malformed)?,
                    )?;
                    validate_category(&category, MAX_CATEGORY_BYTES)?;
                    Ok(Self::Available {
                        category,
                        fingerprint,
                    })
                }
                "unavailable" => {
                    crate::json::require_exact_keys(value, &["status", "reason"])?;
                    let reason = value
                        .get("reason")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned();
                    validate_note(&reason, &OutcomeLimits::default())?;
                    Ok(Self::Unavailable { reason })
                }
                _ => Err(OutcomeError::Malformed),
            }
        })
    }
}

impl<'de> Deserialize<'de> for MechanismRecordV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(value, &["identity", "version", "config_hash"])?;
            let identity = value
                .get("identity")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?
                .to_owned();
            let version = value
                .get("version")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?
                .to_owned();
            let config_hash = Blake3HashText::parse(
                value
                    .get("config_hash")
                    .and_then(Value::as_str)
                    .ok_or(OutcomeError::Malformed)?,
            )?;
            Self::new(identity, version, config_hash, &OutcomeLimits::default())
        })
    }
}

impl<'de> Deserialize<'de> for TaskBindingV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(
                value,
                &["content_hash", "structured_hash", "external_artifact_id"],
            )?;
            let content_hash = Blake3HashText::parse(
                value
                    .get("content_hash")
                    .and_then(Value::as_str)
                    .ok_or(OutcomeError::Malformed)?,
            )?;
            let structured_hash = match value.get("structured_hash") {
                Some(Value::String(text)) => Some(Blake3HashText::parse(text)?),
                Some(Value::Null) | None => None,
                Some(_) => return Err(OutcomeError::Malformed),
            };
            let external_artifact_id = match value.get("external_artifact_id") {
                Some(Value::String(text)) => Some(text.clone()),
                Some(Value::Null) | None => None,
                Some(_) => return Err(OutcomeError::Malformed),
            };
            Self::new(
                content_hash,
                structured_hash,
                external_artifact_id,
                &OutcomeLimits::default(),
            )
        })
    }
}

impl<'de> Deserialize<'de> for OutcomeRecordV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(value, &["value", "evidence", "provenance"])?;
            let record_value = serde_json::from_value::<OutcomeValue>(
                value.get("value").cloned().ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let evidence = serde_json::from_value::<Vec<contextmesh::model::EventId>>(
                value
                    .get("evidence")
                    .cloned()
                    .ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let provenance = serde_json::from_value::<MechanismRecordV1>(
                value
                    .get("provenance")
                    .cloned()
                    .ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            Self::new(
                record_value,
                evidence,
                provenance,
                &OutcomeLimits::default(),
            )
        })
    }
}

impl<'de> Deserialize<'de> for AttemptV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(
                value,
                &[
                    "attempt_id",
                    "parent_attempt_id",
                    "status",
                    "operation_fingerprint",
                    "event_refs",
                    "error",
                    "costs",
                    "provenance",
                ],
            )?;
            let attempt_id = value
                .get("attempt_id")
                .and_then(Value::as_str)
                .ok_or(OutcomeError::Malformed)?
                .to_owned();
            let parent_attempt_id = match value.get("parent_attempt_id") {
                Some(Value::String(text)) => Some(text.clone()),
                Some(Value::Null) | None => None,
                Some(_) => return Err(OutcomeError::Malformed),
            };
            let status = serde_json::from_value::<AttemptStatus>(
                value
                    .get("status")
                    .cloned()
                    .ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let operation_fingerprint = Blake3HashText::parse(
                value
                    .get("operation_fingerprint")
                    .and_then(Value::as_str)
                    .ok_or(OutcomeError::Malformed)?,
            )?;
            let event_refs = serde_json::from_value::<Vec<contextmesh::model::EventId>>(
                value
                    .get("event_refs")
                    .cloned()
                    .ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let error = serde_json::from_value::<AttemptErrorV1>(
                value.get("error").cloned().ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let costs = serde_json::from_value::<CostLedgerV1>(
                value.get("costs").cloned().ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            let provenance = serde_json::from_value::<MechanismRecordV1>(
                value
                    .get("provenance")
                    .cloned()
                    .ok_or(OutcomeError::Malformed)?,
            )
            .map_err(|_| OutcomeError::Malformed)?;
            Self::new(
                Self {
                    attempt_id,
                    parent_attempt_id,
                    status,
                    operation_fingerprint,
                    event_refs,
                    error,
                    costs,
                    provenance,
                },
                &OutcomeLimits::default(),
            )
        })
    }
}

impl<'de> Deserialize<'de> for DeadEndV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(
                value,
                &[
                    "dead_end_id",
                    "attempt_id",
                    "failure_category",
                    "error_fingerprint",
                    "event_refs",
                    "disposition",
                    "provenance",
                ],
            )?;
            Self::new(
                Self {
                    dead_end_id: value
                        .get("dead_end_id")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned(),
                    attempt_id: value
                        .get("attempt_id")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned(),
                    failure_category: value
                        .get("failure_category")
                        .and_then(Value::as_str)
                        .ok_or(OutcomeError::Malformed)?
                        .to_owned(),
                    error_fingerprint: Blake3HashText::parse(
                        value
                            .get("error_fingerprint")
                            .and_then(Value::as_str)
                            .ok_or(OutcomeError::Malformed)?,
                    )?,
                    event_refs: serde_json::from_value(
                        value
                            .get("event_refs")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                    disposition: serde_json::from_value(
                        value
                            .get("disposition")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                    provenance: serde_json::from_value(
                        value
                            .get("provenance")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                },
                &OutcomeLimits::default(),
            )
        })
    }
}

impl<'de> Deserialize<'de> for AttributionMarkV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tagged(deserializer, |value| {
            crate::json::require_exact_keys(value, &["event", "label", "evidence", "mechanism"])?;
            Self::new(
                Self {
                    event: serde_json::from_value(
                        value.get("event").cloned().ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                    label: serde_json::from_value(
                        value.get("label").cloned().ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                    evidence: serde_json::from_value(
                        value
                            .get("evidence")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                    mechanism: serde_json::from_value(
                        value
                            .get("mechanism")
                            .cloned()
                            .ok_or(OutcomeError::Malformed)?,
                    )
                    .map_err(|_| OutcomeError::Malformed)?,
                },
                &OutcomeLimits::default(),
            )
        })
    }
}
