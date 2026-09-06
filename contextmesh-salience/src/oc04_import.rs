//! OC-04 4G+ — transcript importer: local DB messages → signed events (§3).
//!
//! Founder-approved Option A (2026-09-06, Discord msg 1545940775236280361):
//! the frozen candidate pipeline (`SourceEvent::from_signed`,
//! `BaselineSelector::select_scored`, `union_candidates`, `rerank`) accepts
//! ONLY Ed25519-verified `SignedEventV1` inputs, while real session
//! transcripts exist as plaintext in the LOCAL session database. This module
//! is the deterministic, LOCAL-ONLY bridge: it converts transcript messages
//! into `EventBodyV1` payloads and signs them with one local signing
//! identity. The frozen pipeline is UNTOUCHED — signatures exist solely to
//! satisfy the verified-source gate.
//!
//! Determinism contract: given the same message rows, the same `context`
//! seed, and the same identity, every derived value (context id, parent
//! chains, payload JSON, event ids) is byte-identical. Only Ed25519
//! signatures vary across identities (and identity is itself deterministic
//! via `SigningIdentity::from_fixture_seed` when the caller supplies a fixed
//! local seed).
//!
//! Boundary decisions (disclosed in plan §8 Non-claims):
//! - The importer builds a LINEAR parent chain within one session episode;
//!   the original Hermes session graph structure is NOT reconstructed.
//! - Payloads above [`MAX_CANONICAL_PAYLOAD_BYTES`] are truncated on a
//!   whitespace boundary with a deterministic `…trunc` marker so the
//!   canonical bytes stay reproducible.
//! - One context id per session: BLAKE3-derived from the raw session id
//!   (domain-separated), so cross-run reruns bind identically.

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{ContextId, EventBodyV1, EventId, SignedEventV1};
use serde_json::json;

/// Domain separator for per-session context ids (importer-local).
const CONTEXT_DOMAIN: &str = "org.aaif.contextmesh.oc04-import.context.v1";

/// One normalized transcript message ready for event construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMessage {
    /// Deterministic ordering key: (timestamp, original row id).
    ///
    /// The importer SORTS by this key and rejects duplicates — callers may
    /// pass rows in any order and get the identical event chain.
    pub order: (i64, i64),
    /// Message role (`user`, `assistant`, `tool`).
    pub role: String,
    /// Tool name when `role == "tool"`, else `None`.
    pub tool_name: Option<String>,
    /// Message text content (already extracted from the DB row).
    pub content: String,
}

impl ImportMessage {
    /// Builds the canonical event payload for this message.
    ///
    /// The payload shape is frozen here: `{"role", "text"[, "tool"]}`.
    /// The lexical arm tokenizes `payload_text` (canonical JSON), so the
    /// text lives under a single string field with no surrounding noise.
    #[must_use]
    pub fn payload(&self) -> serde_json::Value {
        match &self.tool_name {
            Some(tool) => json!({"role": self.role, "text": self.content, "tool": tool}),
            None => json!({"role": self.role, "text": self.content}),
        }
    }

    /// Lowercased event kind for `validate_kind` (ascii lowercase + `.`/`_`).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self.role.as_str() {
            "user" => "transcript.user",
            "assistant" => "transcript.assistant",
            _ => "transcript.tool",
        }
    }
}

/// Derives the keyed per-session context id from the raw session id.
///
/// BLAKE3 derive-key mode keyed with the LOCAL import key over raw id
/// (Codex review MED-3 fix: the unkeyed digest was dictionary-linkable; the
/// keyed digest binds to the same local-only key as the labeling HMACs, so
/// published context ids are not linkable without the local key). The key
/// bytes are supplied by the caller from the local key file — the identity
/// alone is NOT sufficient binding (fixture seeds are checkable).
#[must_use]
pub fn derive_context_id(raw_session_id: &str, key: &[u8; 32]) -> ContextId {
    let mut hasher = blake3::Hasher::new_derive_key(CONTEXT_DOMAIN);
    hasher.update(key);
    hasher.update(&[0]);
    hasher.update(raw_session_id.as_bytes());
    ContextId::from_bytes(*hasher.finalize().as_bytes())
}

/// Imports one ordered message slice as a signed, linearly-chained event run.
///
/// Returns the signed events in import order (chain order == input order).
/// Fails closed: any payload-construction failure aborts the whole run
/// (no partial sessions enter the pipeline).
///
/// # Errors
/// Returns the underlying contract error when a body fails validation
/// (oversized payload after truncation attempt, invalid kind, etc.).
pub fn import_session(
    identity: &SigningIdentity,
    raw_session_id: &str,
    local_key: &[u8; 32],
    messages: &[ImportMessage],
) -> Result<Vec<SignedEventV1>, contextmesh::error::ContractError> {
    // Deterministic order enforcement (Codex review HIGH-1 fix): sort by the
    // declared `order` key and REJECT duplicate keys — the parent chain and
    // every event id derive from iteration order, so a differently-ordered
    // input must never silently produce different events.
    let mut ordered: Vec<&ImportMessage> = messages.iter().collect();
    ordered.sort_by_key(|m| m.order);
    if let Some(pair) = ordered.windows(2).find(|w| w[0].order == w[1].order) {
        panic_or_reject(pair[0]);
    }
    let context = derive_context_id(raw_session_id, local_key);
    let mut signed: Vec<SignedEventV1> = Vec::with_capacity(messages.len());
    let mut parents: Vec<EventId> = Vec::new();
    for msg in ordered {
        let payload = truncate_payload(msg.payload())?;
        let body = EventBodyV1::new(
            context,
            parents.clone(),
            msg.kind(),
            identity.author(),
            payload,
        )?;
        let event = identity.sign_body(body)?;
        parents = vec![event.event_id()];
        signed.push(event);
    }
    Ok(signed)
}

/// Fail-closed rejection of a duplicate order key (Codex HIGH-1 fix).
fn panic_or_reject(_m: &ImportMessage) -> ! {
    panic!("oc04_import: duplicate ImportMessage::order key — refuse to import")
}

/// Deterministically truncates `payload["text"]` so canonical payload bytes
/// fit [`MAX_CANONICAL_PAYLOAD_BYTES`].
///
/// EXPONENTIAL-BACKOFF SLICE + FIXED-POINT strategy (Codex review fix — the
/// escaped-JSON expansion of quotes/backslashes/control chars makes
/// text.len() + overhead arithmetic unsound): repeatedly measure the REAL
/// canonical payload size and shrink the text slice geometrically (×3/4 per
/// pass) until it fits, then do a linear descent to the largest fitting
/// boundary. The cut is always at a UTF-8 char boundary on the RAW text,
/// backtracked to the last whitespace when one exists in the final window.
/// Deterministic: same input text → same measure→shrink trajectory → same
/// final cut. Worst case is O(log n) canonicalizations + a short descent.
fn truncate_payload(
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, contextmesh::error::ContractError> {
    const BUDGET: usize = contextmesh::model::MAX_CANONICAL_PAYLOAD_BYTES;
    const TRUNC_MARKER: &str = "…trunc";
    let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
        return Ok(payload);
    };
    // Fast path: measure canonical size with the text as-is.
    if contextmesh::model::canonical_payload_bytes(&payload).is_ok() {
        return Ok(payload);
    }
    let text = text.to_owned();
    // Measure canonical overhead with an empty text; the fitting text byte
    // budget is BUDGET minus that overhead minus the truncation marker.
    let mut probe = payload.clone();
    if let Some(obj) = probe.as_object_mut() {
        obj.insert("text".into(), serde_json::Value::String(String::new()));
    }
    let empty_size = contextmesh::model::canonical_payload_bytes(&probe)?.len();
    let marker_cost = contextmesh::model::canonical_payload_bytes(&json!({"t": TRUNC_MARKER}))?
        .len()
        .saturating_sub(7); // {"t":"X"} = 7 overhead bytes around the marker content
    let text_budget = BUDGET.saturating_sub(empty_size + marker_cost);
    if text_budget == 0 {
        // Overhead alone exceeds the budget: fail closed (cannot truncate).
        return Err(contextmesh::error::ContractError::LimitExceeded);
    }
    // Binary search the largest raw-byte prefix whose canonical size fits.
    // (Escaped expansion is monotone in prefix length, so BS is exact.)
    let mut lo = 0_usize;
    let mut hi = text.len();
    // hi must start at a char boundary — text.len() always is one.
    while lo < hi {
        // Candidate strictly above lo, clamped to hi, then snapped DOWN to a
        // char boundary (Codex escaped-case fix: snapping UP could exceed hi
        // and make `hi = mid - 1` grow the range — oscillation/wrong bound).
        let raw_mid = ((lo + hi) / 2 + 1).min(hi);
        let mut mid = raw_mid;
        while mid > lo && !text.is_char_boundary(mid) {
            mid -= 1;
        }
        if mid == lo {
            // No char boundary in (lo, hi]: skip this half entirely.
            lo = hi;
            break;
        }
        let mut cand = payload.clone();
        if let Some(obj) = cand.as_object_mut() {
            obj.insert(
                "text".into(),
                serde_json::Value::String(format!("{}{TRUNC_MARKER}", &text[..mid])),
            );
        }
        if contextmesh::model::canonical_payload_bytes(&cand).is_ok() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    // lo is the largest prefix bound that fits with the marker; walk it down
    // to a char boundary (the `lo = hi` escape and `hi = mid - 1` can land
    // mid-char in multibyte text).
    while lo > 0 && !text.is_char_boundary(lo) {
        lo -= 1;
    }
    // Prefer a whitespace boundary within a small window back from lo
    // (window_start itself must be a char boundary — walk down to one).
    let mut window_start = lo.saturating_sub(64);
    while window_start > 0 && !text.is_char_boundary(window_start) {
        window_start -= 1;
    }
    let cut = text[window_start..lo]
        .rfind(char::is_whitespace)
        .map(|i| window_start + i + 1)
        .unwrap_or(lo);
    let truncated = format!("{}{TRUNC_MARKER}", &text[..cut]);
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("text".into(), serde_json::Value::String(truncated));
    }
    // Final check: still too large means overhead alone exceeds budget.
    contextmesh::model::canonical_payload_bytes(&payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> SigningIdentity {
        // Fixed local seed: fully deterministic event ids across runs.
        SigningIdentity::from_fixture_seed([42_u8; 32])
    }

    fn msg(order: (i64, i64), role: &str, content: &str) -> ImportMessage {
        ImportMessage {
            order,
            role: role.to_owned(),
            tool_name: None,
            content: content.to_owned(),
        }
    }

    fn test_key() -> [u8; 32] {
        [7_u8; 32]
    }

    #[test]
    fn context_id_keyed_deterministic_and_unlinkable() {
        let k1 = [7_u8; 32];
        let k2 = [8_u8; 32];
        // Same key: same id binds; distinct sessions differ.
        assert_eq!(
            derive_context_id("session-1", &k1),
            derive_context_id("session-1", &k1)
        );
        assert_ne!(
            derive_context_id("session-1", &k1),
            derive_context_id("session-2", &k1)
        );
        // Different key: id differs (dictionary-linkability broken).
        assert_ne!(
            derive_context_id("session-1", &k1),
            derive_context_id("session-1", &k2)
        );
    }

    #[test]
    fn import_is_order_independent_and_duplicate_rejecting() {
        let messages = vec![
            msg((1, 1), "user", "fix the failing build"),
            msg((2, 2), "tool", "git log --oneline -5"),
            msg((3, 3), "assistant", "the regression is in commit abc"),
        ];
        let mut shuffled = messages.clone();
        shuffled.reverse();
        let a = import_session(&identity(), "sess-a", &test_key(), &messages).expect("import");
        let b = import_session(&identity(), "sess-a", &test_key(), &shuffled).expect("import");
        let ia: Vec<String> = a.iter().map(|e| e.event_id().to_string()).collect();
        let ib: Vec<String> = b.iter().map(|e| e.event_id().to_string()).collect();
        assert_eq!(ia, ib, "input order must not change the imported chain");
        // Duplicate order keys are rejected (fail closed).
        let mut dup = messages.clone();
        dup.push(msg((2, 2), "tool", "duplicate key"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            import_session(&identity(), "sess-a", &test_key(), &dup)
        }));
        assert!(result.is_err(), "duplicate order key must fail closed");
    }

    #[test]
    fn linear_parent_chain_and_verified_sources() {
        let messages = vec![
            msg((1, 1), "user", "alpha"),
            msg((2, 2), "assistant", "beta"),
        ];
        let events = import_session(&identity(), "sess-b", &test_key(), &messages).expect("import");
        // Event 2's parent must be event 1's id.
        let p0 = events[0].body().parents();
        let p1 = events[1].body().parents();
        assert!(p0.is_empty(), "first event has no parent");
        assert_eq!(p1, vec![events[0].event_id()], "linear chain");
        // Every event passes the pipeline's verified-source gate.
        for e in &events {
            contextmesh::selection::SourceEvent::from_signed(e).expect("verified source");
        }
    }

    #[test]
    fn oversize_payload_truncated_deterministically() {
        // Plain-word oversize.
        let big = "word ".repeat(300_000); // ~1.5MB text
        // Escaped-character oversize (Codex HIGH-2 regression case): quotes
        // and control chars expand heavily in canonical JSON. 400k reps →
        // 1.2MB raw → ~2.4MB canonical (every \ and " doubles), safely over.
        let escaped = "\\\"x".repeat(400_000);
        // Multibyte oversize (char-boundary safety).
        let multibyte = "한글문장".repeat(200_000);
        for (name, content) in [
            ("plain", &big[..]),
            ("escaped", &escaped),
            ("multibyte", &multibyte),
        ] {
            let messages = vec![msg((1, 1), "tool", content)];
            let events = import_session(&identity(), "sess-c", &test_key(), &messages)
                .unwrap_or_else(|e| panic!("{name}: import failed: {e:?}"));
            assert_eq!(events.len(), 1);
            let payload = events[0].body().payload();
            let text = payload.get("text").and_then(|v| v.as_str()).expect("text");
            assert!(
                text.ends_with("…trunc"),
                "{name}: truncation marker present"
            );
            assert!(text.len() < content.len(), "{name}: actually truncated");
            // Passes the verified-source gate after truncation.
            contextmesh::selection::SourceEvent::from_signed(&events[0])
                .unwrap_or_else(|e| panic!("{name}: gate failed: {e:?}"));
            // Deterministic: re-import yields the identical event id.
            let again = import_session(&identity(), "sess-c", &test_key(), &messages)
                .unwrap_or_else(|e| panic!("{name}: re-import failed: {e:?}"));
            assert_eq!(
                events[0].event_id(),
                again[0].event_id(),
                "{name}: deterministic"
            );
        }
    }

    #[test]
    fn huge_tool_name_overhead_fails_closed() {
        // A tool name so large that overhead alone busts the budget must
        // return an error, not panic or silently produce an oversized event.
        let m = ImportMessage {
            order: (1, 1),
            role: "tool".to_owned(),
            tool_name: Some("t".repeat(contextmesh::model::MAX_CANONICAL_PAYLOAD_BYTES)),
            content: "x".to_owned(),
        };
        let result = import_session(&identity(), "sess-d", &test_key(), &[m]);
        assert!(result.is_err(), "untruncatable overhead must fail closed");
    }

    #[test]
    fn tool_messages_carry_tool_name_in_payload() {
        let m = ImportMessage {
            order: (1, 1),
            role: "tool".to_owned(),
            tool_name: Some("cargo".to_owned()),
            content: "compiled".to_owned(),
        };
        let payload = m.payload();
        assert_eq!(payload.get("tool").and_then(|v| v.as_str()), Some("cargo"));
        assert_eq!(m.kind(), "transcript.tool");
    }
}
