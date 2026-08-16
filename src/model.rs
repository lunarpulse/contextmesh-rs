//! Frozen version-1 signed-event wire model and bounded canonical JSON.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Number, Value};

use crate::crypto;
use crate::error::{ContractError, Result};

/// Frozen event body version.
pub const EVENT_VERSION: u8 = 1;
/// Maximum number of parents in a version-1 body.
pub const MAX_PARENTS: usize = 64;
/// Maximum event-kind length in ASCII bytes.
pub const MAX_KIND_BYTES: usize = 64;
/// Maximum JSON payload depth, counting the payload root as depth one.
pub const MAX_PAYLOAD_DEPTH: usize = 64;
/// Maximum RFC 8785 canonical payload size.
pub const MAX_CANONICAL_PAYLOAD_BYTES: usize = 1_048_576;
/// Maximum RFC 8785 canonical body size.
pub const MAX_CANONICAL_BODY_BYTES: usize = 1_114_112;
/// Maximum raw envelope size accepted from the wire.
pub const MAX_RAW_WIRE_BYTES: usize = 2_097_152;
/// Largest exactly interoperable integer magnitude accepted in JSON numbers.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

const DUPLICATE_MARKER: &str = "__contextmesh_duplicate_object_member__";

macro_rules! fixed_text_type {
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

fixed_text_type!(
    /// A BLAKE3-derived immutable event identifier (evt1_ plus 32 bytes).
    EventId,
    "evt1_",
    32
);
fixed_text_type!(
    /// An opaque version-1 context identifier (ctx1_ plus 32 bytes).
    ContextId,
    "ctx1_",
    32
);
fixed_text_type!(
    /// An Ed25519 verifying-key identity (ed25519_ plus 32 bytes).
    AuthorId,
    "ed25519_",
    32
);
fixed_text_type!(
    /// An Ed25519 event signature (sig1_ plus 64 bytes).
    EventSignature,
    "sig1_",
    64
);

impl Ord for EventId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl PartialOrd for EventId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The complete immutable, signed version-1 event body.
#[derive(Clone, Debug, Serialize)]
pub struct EventBodyV1 {
    version: u8,
    context: ContextId,
    parents: Vec<EventId>,
    kind: String,
    author: AuthorId,
    payload: Value,
}

impl EventBodyV1 {
    /// Constructs and validates a version-1 body.
    pub fn new(
        context: ContextId,
        parents: Vec<EventId>,
        kind: impl Into<String>,
        author: AuthorId,
        payload: Value,
    ) -> Result<Self> {
        let body = Self {
            version: EVENT_VERSION,
            context,
            parents,
            kind: kind.into(),
            author,
            payload,
        };
        body.validate()?;
        Ok(body)
    }

    /// Strictly parses a body JSON value, rejecting duplicate and unknown fields.
    pub fn from_json(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_RAW_WIRE_BYTES {
            return Err(ContractError::WireTooLarge);
        }
        let value = strict_json(input)?;
        parse_body(value)
    }

    /// Returns the frozen body version (always 1).
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Returns the opaque context identifier.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns parents in their required canonical order.
    #[must_use]
    pub fn parents(&self) -> &[EventId] {
        &self.parents
    }

    /// Returns the validated event kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the Ed25519 author identity included in the signed body.
    #[must_use]
    pub const fn author(&self) -> AuthorId {
        self.author
    }

    /// Returns the opaque JSON payload.
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    /// Re-runs every semantic and canonical-size validation rule.
    pub fn validate(&self) -> Result<()> {
        if self.version != EVENT_VERSION {
            return Err(ContractError::UnsupportedVersion);
        }
        if self.parents.len() > MAX_PARENTS {
            return Err(ContractError::LimitExceeded);
        }
        validate_kind(&self.kind)?;
        if self
            .parents
            .windows(2)
            .any(|pair| pair[0].to_string() >= pair[1].to_string())
        {
            return Err(ContractError::ParentOrder);
        }
        validate_json_value(&self.payload, 1)?;
        let payload = canonicalize(&self.payload)?;
        if payload.len() > MAX_CANONICAL_PAYLOAD_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        let body = canonicalize(self)?;
        if body.len() > MAX_CANONICAL_BODY_BYTES {
            return Err(ContractError::LimitExceeded);
        }
        Ok(())
    }

    /// Returns the RFC 8785/JCS canonical body bytes after full validation.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonicalize(self)
    }
}

/// The complete immutable version-1 signed-event envelope.
#[derive(Clone, Debug, Serialize)]
pub struct SignedEventV1 {
    event_id: EventId,
    body: EventBodyV1,
    signature: EventSignature,
}

impl SignedEventV1 {
    pub(crate) const fn from_verified_parts(
        event_id: EventId,
        body: EventBodyV1,
        signature: EventSignature,
    ) -> Self {
        Self {
            event_id,
            body,
            signature,
        }
    }

    /// Parses, bounds, validates, recomputes, and strictly verifies a wire event.
    ///
    /// No event is returned unless every validation and cryptographic check has
    /// completed successfully.
    pub fn from_wire(input: &[u8]) -> Result<Self> {
        if input.len() > MAX_RAW_WIRE_BYTES {
            return Err(ContractError::WireTooLarge);
        }
        let value = strict_json(input)?;
        let mut object = into_object(value)?;
        reject_unknown(&object, &["event_id", "body", "signature"])?;
        let event_id_value = take_required(&mut object, "event_id")?;
        let body_value = take_required(&mut object, "body")?;
        let signature_value = take_required(&mut object, "signature")?;

        let body = parse_body(body_value)?;
        let event_id = parse_text(event_id_value)?;
        let signature = parse_text(signature_value)?;
        crypto::verify_parts(&body, event_id, signature)?;
        Ok(Self::from_verified_parts(event_id, body, signature))
    }

    /// Returns the recomputed and verified event identifier.
    #[must_use]
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    /// Returns the complete signed body.
    #[must_use]
    pub const fn body(&self) -> &EventBodyV1 {
        &self.body
    }

    /// Returns the strict Ed25519 signature.
    #[must_use]
    pub const fn signature(&self) -> EventSignature {
        self.signature
    }

    /// Independently revalidates the body, ID, author key, and signature.
    pub fn verify(&self) -> Result<()> {
        crypto::verify_parts(&self.body, self.event_id, self.signature)
    }

    /// Renders the entire envelope as RFC 8785/JCS canonical wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>> {
        self.verify()?;
        canonicalize(self)
    }
}

/// Validates and canonicalizes an arbitrary payload using the v1 rules.
pub fn canonical_payload_bytes(payload: &Value) -> Result<Vec<u8>> {
    validate_json_value(payload, 1)?;
    let bytes = canonicalize(payload)?;
    if bytes.len() > MAX_CANONICAL_PAYLOAD_BYTES {
        return Err(ContractError::LimitExceeded);
    }
    Ok(bytes)
}

fn parse_body(value: Value) -> Result<EventBodyV1> {
    let mut object = into_object(value)?;
    reject_unknown(
        &object,
        &["version", "context", "parents", "kind", "author", "payload"],
    )?;
    let version_value = take_required(&mut object, "version")?;
    let context_value = take_required(&mut object, "context")?;
    let parents_value = take_required(&mut object, "parents")?;
    let kind_value = take_required(&mut object, "kind")?;
    let author_value = take_required(&mut object, "author")?;
    let payload = take_required(&mut object, "payload")?;

    let version = match version_value {
        Value::Number(number) if number.as_u64() == Some(u64::from(EVENT_VERSION)) => EVENT_VERSION,
        Value::Number(_) => return Err(ContractError::UnsupportedVersion),
        _ => return Err(ContractError::JsonSyntax),
    };
    let context = parse_text(context_value)?;
    let parents = match parents_value {
        Value::Array(values) => values
            .into_iter()
            .map(parse_text)
            .collect::<Result<Vec<EventId>>>()?,
        _ => return Err(ContractError::JsonSyntax),
    };
    let kind = into_string(kind_value)?;
    let author = parse_text(author_value)?;

    let body = EventBodyV1 {
        version,
        context,
        parents,
        kind,
        author,
        payload,
    };
    body.validate()?;
    Ok(body)
}

pub(crate) fn strict_json(input: &[u8]) -> Result<Value> {
    if input.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ContractError::JsonSyntax);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = StrictValue::deserialize(&mut deserializer).map_err(map_json_error)?;
    deserializer.end().map_err(map_json_error)?;
    Ok(value.0)
}

fn map_json_error(error: serde_json::Error) -> ContractError {
    let message = error.to_string();
    if message.contains(DUPLICATE_MARKER) {
        ContractError::DuplicateKey
    } else if message.contains("number out of range") {
        ContractError::UnsafeNumber
    } else {
        ContractError::JsonSyntax
    }
}

pub(crate) fn into_object(value: Value) -> Result<Map<String, Value>> {
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(ContractError::JsonSyntax),
    }
}

pub(crate) fn into_string(value: Value) -> Result<String> {
    match value {
        Value::String(text) => Ok(text),
        _ => Err(ContractError::JsonSyntax),
    }
}

pub(crate) fn parse_text<T: FromStr<Err = ContractError>>(value: Value) -> Result<T> {
    into_string(value)?.parse()
}

pub(crate) fn reject_unknown(object: &Map<String, Value>, allowed: &[&str]) -> Result<()> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        Err(ContractError::UnknownField)
    } else {
        Ok(())
    }
}

pub(crate) fn take_required(object: &mut Map<String, Value>, key: &str) -> Result<Value> {
    object.remove(key).ok_or(ContractError::MissingField)
}

fn validate_kind(kind: &str) -> Result<()> {
    let bytes = kind.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_KIND_BYTES || !bytes[0].is_ascii_lowercase() {
        return Err(ContractError::InvalidKind);
    }
    let mut previous_separator = false;
    for byte in &bytes[1..] {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else if matches!(byte, b'.' | b'_' | b'-') && !previous_separator {
            previous_separator = true;
        } else {
            return Err(ContractError::InvalidKind);
        }
    }
    if previous_separator {
        return Err(ContractError::InvalidKind);
    }
    Ok(())
}

pub(crate) fn validate_json_value(value: &Value, depth: usize) -> Result<()> {
    if depth > MAX_PAYLOAD_DEPTH {
        return Err(ContractError::LimitExceeded);
    }
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => validate_number(number),
        Value::Array(values) => {
            for value in values {
                validate_json_value(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(object) => {
            for value in object.values() {
                validate_json_value(value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn validate_number(number: &Number) -> Result<()> {
    if let Some(value) = number.as_i64() {
        if value.unsigned_abs() > MAX_SAFE_INTEGER {
            return Err(ContractError::UnsafeNumber);
        }
        return Ok(());
    }
    if let Some(value) = number.as_u64() {
        if value > MAX_SAFE_INTEGER {
            return Err(ContractError::UnsafeNumber);
        }
        return Ok(());
    }
    let value = number.as_f64().ok_or(ContractError::UnsafeNumber)?;
    if !value.is_finite() || (value.fract() == 0.0 && value.abs() > MAX_SAFE_INTEGER as f64) {
        return Err(ContractError::UnsafeNumber);
    }
    Ok(())
}

pub(crate) fn canonicalize<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    serde_jcs::to_vec(value).map_err(|_| ContractError::Canonicalization)
}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor)
    }
}

struct StrictValueVisitor;

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object members")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(StrictValue)
            .ok_or_else(|| E::custom("number out of range"))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictValue>()? {
            values.push(value.0);
        }
        Ok(StrictValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(DUPLICATE_MARKER));
            }
            let value = map.next_value::<StrictValue>()?;
            values.insert(key, value.0);
        }
        Ok(StrictValue(Value::Object(values)))
    }
}
