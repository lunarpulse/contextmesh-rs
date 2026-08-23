//! Local strict JSON parsing and the canonical JCS helper for OC-01.
//!
//! The parser rejects a leading BOM, trailing data after the top-level value,
//! duplicate object members at every depth, unsafe or non-finite numbers, and
//! nesting deeper than the frozen limit (64). It is deliberately local to the
//! salience crate: no core-internal JSON surface is exposed or reused.
//! Canonical serialization is RFC 8785 JCS via `serde_jcs`.

use serde::Serialize;
use serde::de::{self, DeserializeSeed, Visitor};
use serde_json::{Deserializer, Value};
use std::collections::HashSet;
use std::fmt;

use crate::error::OutcomeError;
use crate::types::MAX_JSON_DEPTH;

/// Parses `input` under the strict OC-01 JSON contract.
///
/// Rejects a BOM, trailing data, duplicate members at any depth, non-finite
/// numbers, and depth over [`MAX_JSON_DEPTH`]. Every syntactic failure maps to
/// [`OutcomeError::Malformed`]; canonicality is decided later by byte
/// comparison, not here.
pub fn parse_strict(input: &[u8]) -> Result<Value, OutcomeError> {
    if input.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(OutcomeError::Malformed);
    }
    let mut deserializer = Deserializer::from_slice(input);
    let value = StrictNodeVisitor { depth: 1 }
        .deserialize(&mut deserializer)
        .map_err(to_malformed)?;
    deserializer.end().map_err(to_malformed)?;
    Ok(value)
}

/// Serializes `value` to exact RFC 8785 JCS bytes.
///
/// Rendering in-memory data can only fail through serializer misuse or a
/// non-canonicalizable number; both map to [`OutcomeError::Noncanonical`].
pub fn jcs<T>(value: &T) -> Result<Vec<u8>, OutcomeError>
where
    T: Serialize + ?Sized,
{
    serde_jcs::to_vec(value).map_err(|_| OutcomeError::Noncanonical)
}

/// Asserts `value` is an object whose member-name set is exactly `keys`.
///
/// Any missing or unknown member fails with [`OutcomeError::Malformed`]. This
/// is the shared exact-shape check used by every tagged variant parser.
pub fn require_exact_keys(value: &Value, keys: &[&str]) -> Result<(), OutcomeError> {
    let object = value.as_object().ok_or(OutcomeError::Malformed)?;
    if object.len() != keys.len() {
        return Err(OutcomeError::Malformed);
    }
    for key in keys {
        if !object.contains_key(*key) {
            return Err(OutcomeError::Malformed);
        }
    }
    Ok(())
}

fn to_malformed(error: impl fmt::Display) -> OutcomeError {
    // The strict parser is a syntax gate; message text is deliberately unused.
    let _ = error;
    OutcomeError::Malformed
}

struct StrictNodeVisitor {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictNodeVisitor {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictNodeVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any strict JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E>
    where
        E: de::Error,
    {
        if value.is_finite() {
            Ok(Value::from(value))
        } else {
            Err(de::Error::custom("non-finite number"))
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        if self.depth > MAX_JSON_DEPTH {
            return Err(de::Error::custom("maximum depth exceeded"));
        }
        let mut items = Vec::new();
        let nested = StrictNodeVisitor {
            depth: self.depth + 1,
        };
        while let Some(item) = seq.next_element_seed(nested.clone())? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        if self.depth > MAX_JSON_DEPTH {
            return Err(de::Error::custom("maximum depth exceeded"));
        }
        let mut members = serde_json::Map::new();
        let mut seen = HashSet::new();
        let nested = StrictNodeVisitor {
            depth: self.depth + 1,
        };
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom("duplicate member"));
            }
            let value = map.next_value_seed(nested.clone())?;
            members.insert(key, value);
        }
        Ok(Value::Object(members))
    }
}

impl Clone for StrictNodeVisitor {
    fn clone(&self) -> Self {
        Self { depth: self.depth }
    }
}
