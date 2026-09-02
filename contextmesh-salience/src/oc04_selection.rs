//! OC-04 — Prior-assisted selection integration (spec v12, frozen).
//!
//! 4B scope: `Oc04ConfigV1` (u64-only, prereg-verbatim constants, domain-
//! separated config hash), `SelectionInfluenceV1` (6-member JCS body +
//! placeholder-discipline ID), `SelectionExecutionV1` (19-member JCS body +
//! Ed25519 signed envelope, issuance + full-recompute verification), and
//! [`VerifiedPrior`] — the ONLY token constructor over OC-03's rebuild-based
//! `verify_prior`, with the §7.1 canonical-payload gate. Execution binding
//! (B3–B8 chain) arrives at 4E; union/rerank at 4C/4D. Thorn is
//! structurally absent (P4 scope).

use std::path::{Path, PathBuf};

use contextmesh::crypto::{SigningIdentity, verify_domain_message};
use contextmesh::model::canonical_payload_bytes;

use crate::error::OutcomeError;
use crate::prior::{
    PriorConfigV1, PriorSeedV1, ReportContribution, SaliencePriorV1, SessionPayloads, verify_prior,
};

/// Config-hash ID prefix (spec §5, OC prefix discipline).
pub const OC04_CONFIG_ID_PREFIX: &str = "oc04cfg1_";

/// Influence-record ID prefix (spec §5).
pub const OC04_INFLUENCE_ID_PREFIX: &str = "oc04inf1_";

/// Execution-envelope ID prefix (spec §5).
pub const OC04_EXECUTION_ID_PREFIX: &str = "oc04exec1_";

/// Influence ID derivation domain (spec §9).
pub const OC04_INFLUENCE_ID_DOMAIN: &[u8] = b"oc-04-inf-v1-id\0";

/// Execution ID derivation domain (spec §9).
pub const OC04_EXECUTION_ID_DOMAIN: &[u8] = b"oc-04-exec-v1-id\0";

/// Config canonicalization domain (spec §6 derivation table; frozen at 4B).
pub const OC04_CONFIG_HASH_DOMAIN: &[u8] = b"oc-04-config-v1\0";

/// Ed25519 signature domain for the execution envelope (spec §5/§6).
pub const OC04_EXEC_SIGNATURE_DOMAIN: &[u8] = b"oc-04-exec-v1\0";

/// Frozen P1 prereg constant: lexical arm cap (verbatim).
pub const LEXICAL_ARM_CAP: u64 = 64;

/// Frozen P1 prereg constant: prior arm cap (verbatim).
pub const PRIOR_ARM_CAP: u64 = 30;

/// Frozen P1 prereg constant: per-arm min-max clip above, ppm (verbatim).
pub const CLIP_ABOVE_PPM: u64 = 1_000_000;

/// Frozen P1 prereg constant: per-arm min-max clip below, ppm (verbatim).
pub const CLIP_BELOW_PPM: u64 = 0;

/// New OC-04 freeze: orphan prior-entity counter bound, fail-closed.
pub const ORPHAN_PRIOR_ENTITIES_MAX: u32 = 1024;

/// JSON wire value for the lexical-only entry reason (spec §6).
pub const ENTRY_REASON_LEXICAL: &str = "lexical";
/// JSON wire value for the prior-only entry reason (spec §6).
pub const ENTRY_REASON_PRIOR: &str = "prior";
/// JSON wire value for the both-arms entry reason (spec §6).
pub const ENTRY_REASON_BOTH: &str = "both";

/// Canonical OC-04 selection configuration. Every field is u64 (no floats,
/// spec §6 note 4). The frozen values are the P1 prereg's
/// `selection_pipeline` + `evaluation.score_normalization` consumed
/// VERBATIM plus the two stage-plan caps. Not tunable: any deviation is
/// rejected (S04) — the config hash binds the frozen policy into every
/// downstream artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Oc04ConfigV1 {
    /// Schema version (frozen: 1).
    pub version: u64,
    /// Prereg `per_arm_caps.lexical_arm_cap` (frozen: 64).
    pub lexical_arm_cap: u64,
    /// Prereg `per_arm_caps.prior_arm_cap` (frozen: 30).
    pub prior_arm_cap: u64,
    /// Prereg `evaluation.score_normalization.clip_above_ppm` (frozen:
    /// 1,000,000).
    pub clip_above_ppm: u64,
    /// Prereg `evaluation.score_normalization.clip_below_ppm` (frozen: 0).
    pub clip_below_ppm: u64,
    /// Frozen P1 preregistration SHA-256 seal (OC-02/OC-03 precedent).
    pub prereg_reference: &'static str,
}

impl Default for Oc04ConfigV1 {
    fn default() -> Self {
        Self {
            version: 1,
            lexical_arm_cap: LEXICAL_ARM_CAP,
            prior_arm_cap: PRIOR_ARM_CAP,
            clip_above_ppm: CLIP_ABOVE_PPM,
            clip_below_ppm: CLIP_BELOW_PPM,
            prereg_reference: crate::prior::PREREG_SHA256,
        }
    }
}

impl Oc04ConfigV1 {
    /// Fail if any member deviates from the frozen values (matrix S04 —
    /// the configuration is not tunable).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on any deviation.
    pub fn validate_frozen(&self) -> Result<(), OutcomeError> {
        if *self != Self::default() {
            return Err(OutcomeError::Malformed);
        }
        Ok(())
    }

    /// Deterministic canonical bytes (JCS single-line JSON, lexicographic
    /// member order: clip_above_ppm, clip_below_ppm, lexical_arm_cap,
    /// prereg_reference, prior_arm_cap, version).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when the config is not frozen.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        self.validate_frozen()?;
        let mut json = String::from("{");
        push_json_u64(&mut json, "clip_above_ppm", self.clip_above_ppm);
        json.push(',');
        push_json_u64(&mut json, "clip_below_ppm", self.clip_below_ppm);
        json.push(',');
        push_json_u64(&mut json, "lexical_arm_cap", self.lexical_arm_cap);
        json.push(',');
        push_json_string(&mut json, "prereg_reference", self.prereg_reference);
        json.push(',');
        push_json_u64(&mut json, "prior_arm_cap", self.prior_arm_cap);
        json.push(',');
        push_json_u64(&mut json, "version", self.version);
        json.push('}');
        Ok(json.into_bytes())
    }

    /// Lowercase-hex BLAKE3 over `oc-04-config-v1\0` + canonical config
    /// bytes (spec §6 derivation table, `config_hash` row).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when the config is not frozen.
    pub fn config_hash(&self) -> Result<String, OutcomeError> {
        self.canonical_bytes().map(|bytes| {
            let mut hasher = blake3::Hasher::new();
            hasher.update(OC04_CONFIG_HASH_DOMAIN);
            hasher.update(&bytes);
            hasher.finalize().to_hex().to_string()
        })
    }
}

/// One ordered influence entry (spec §6): an EventId canonical text plus
/// its normalized per-arm ppm values, the combined score, and the recorded
/// entry reason. Ordered by the rerank key (score desc, then canonical
/// EventId text ascending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInfluenceEntryV1 {
    event_id_text: String,
    entry_reason: &'static str,
    lexical_ppm: u64,
    prior_ppm: u64,
    score_ppm: u128,
}

impl SelectionInfluenceEntryV1 {
    /// Constructs one entry, validating the exact string enum, the
    /// NON-MEMBER-ZERO rule, and the arithmetic identity
    /// `score_ppm = lexical_ppm + prior_ppm` (checked; fail-closed on
    /// overflow).
    ///
    /// Change control (membership-truth separation, founder-approved at the
    /// 4D gate): per-arm min-max normalization (§7.2) legitimately collapses
    /// an arm's minimum member to `0` ppm, and a zero ppm does NOT imply
    /// absence from that arm — `entry_reason` records UNION MEMBERSHIP
    /// (§7.1), while the ppm values record NORMALIZED RELATIVE MAGNITUDE.
    /// The one-way implication REMAINS normative (§6): a NON-MEMBER arm
    /// renders `ppm = 0` — reason `lexical` requires `prior_ppm = 0`,
    /// reason `prior` requires `lexical_ppm = 0` — while `both` admits any
    /// magnitude combination including the double-collapse `(0, 0)`.
    /// Membership-vs-reason consistency is asserted by `rerank` against the
    /// union outcome and re-verified in the §7.5 chain — not here.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on a reason outside
    /// `lexical|prior|both`, on a non-member arm with a nonzero magnitude,
    /// or on score overflow.
    pub fn new(
        event_id_text: impl Into<String>,
        entry_reason: &'static str,
        lexical_ppm: u64,
        prior_ppm: u64,
    ) -> Result<Self, OutcomeError> {
        let nonmember_zero = match entry_reason {
            // §6: a non-member arm renders ppm = 0 (one-way rule — the
            // converse does NOT hold: a member may legitimately collapse
            // to 0 by min-max normalization).
            ENTRY_REASON_LEXICAL => prior_ppm == 0,
            ENTRY_REASON_PRIOR => lexical_ppm == 0,
            ENTRY_REASON_BOTH => true,
            _ => false,
        };
        if !nonmember_zero {
            return Err(OutcomeError::Malformed);
        }
        // Prereg overflow policy: checked arithmetic, fail closed.
        let score_ppm = u128::from(lexical_ppm)
            .checked_add(u128::from(prior_ppm))
            .ok_or(OutcomeError::Malformed)?;
        Ok(Self {
            event_id_text: event_id_text.into(),
            entry_reason,
            lexical_ppm,
            prior_ppm,
            score_ppm,
        })
    }

    /// Read-only canonical EventId text.
    #[must_use]
    pub fn event_id_text(&self) -> &str {
        &self.event_id_text
    }

    /// Read-only entry reason (`lexical|prior|both`).
    #[must_use]
    pub fn entry_reason(&self) -> &'static str {
        self.entry_reason
    }

    /// Read-only lexical-arm normalized ppm.
    #[must_use]
    pub fn lexical_ppm(&self) -> u64 {
        self.lexical_ppm
    }

    /// Read-only prior-arm normalized ppm.
    #[must_use]
    pub fn prior_ppm(&self) -> u64 {
        self.prior_ppm
    }

    /// Read-only combined score ppm (`lexical_ppm + prior_ppm`).
    #[must_use]
    pub fn score_ppm(&self) -> u128 {
        self.score_ppm
    }
}

/// The signed 6-member influence record (spec §6). The ID is computed LAST
/// over placeholder-substituted canonical bytes (OC-02/OC-03 discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInfluenceV1 {
    version: u64,
    influence_id: String,
    config_hash: String,
    prior_id: String,
    task_fingerprint: String,
    entries: Vec<SelectionInfluenceEntryV1>,
}

impl SelectionInfluenceV1 {
    /// Assembles and seals one influence record: validates the config, the
    /// entry-reason/arithmetic identities, the rerank ordering of
    /// `entries`, and derives `influence_id` per §9.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on any validation failure.
    pub fn assemble(
        config: &Oc04ConfigV1,
        prior_id: impl Into<String>,
        task_fingerprint: impl Into<String>,
        entries: Vec<SelectionInfluenceEntryV1>,
    ) -> Result<Self, OutcomeError> {
        config.validate_frozen()?;
        let mut seen: Vec<&str> = Vec::new();
        for entry in &entries {
            let expected = u128::from(entry.lexical_ppm) + u128::from(entry.prior_ppm);
            if entry.score_ppm() != expected {
                return Err(OutcomeError::Malformed);
            }
            // Union dedup is normative at schema level too: the same
            // canonical EventId text may appear at most once (4C/4D union
            // dedup upstream can never produce duplicates).
            if seen.contains(&entry.event_id_text()) {
                return Err(OutcomeError::Malformed);
            }
            seen.push(entry.event_id_text());
        }
        // Normative rerank order: score desc, then canonical EventId text
        // ascending (spec §6 entries row).
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| {
            b.score_ppm()
                .cmp(&a.score_ppm())
                .then_with(|| a.event_id_text().cmp(b.event_id_text()))
        });
        if sorted != entries {
            return Err(OutcomeError::Malformed);
        }
        let config_hash = config.config_hash()?;
        let placeholder = Self {
            version: 1,
            influence_id: "influence_id".to_owned(),
            config_hash,
            prior_id: prior_id.into(),
            task_fingerprint: task_fingerprint.into(),
            entries,
        };
        let canonical = placeholder.canonical_bytes()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(OC04_INFLUENCE_ID_DOMAIN);
        hasher.update(&canonical);
        let influence_id = format!(
            "{OC04_INFLUENCE_ID_PREFIX}{}",
            base64_url_no_pad(hasher.finalize().as_bytes())
        );
        Ok(Self {
            influence_id,
            ..placeholder
        })
    }

    /// Read-only derived ID (`oc04inf1_` + base64url no-pad).
    #[must_use]
    pub fn influence_id(&self) -> &str {
        &self.influence_id
    }

    /// Test-visible placeholder re-render: returns canonical bytes with
    /// the id member = literal `"influence_id"` (S05's independent
    /// derivation check recomputes the §9 hash over these). Not a
    /// constructor — the record is unchanged.
    #[doc(hidden)]
    #[must_use]
    pub fn placeholder_bytes_for_test(&self) -> Vec<u8> {
        let mut ph = self.clone();
        ph.influence_id = "influence_id".to_owned();
        ph.canonical_bytes()
            .expect("frozen record re-renders cleanly")
    }

    /// Read-only config hash.
    #[must_use]
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// Read-only prior artifact ID (copied verbatim from the verified
    /// token).
    #[must_use]
    pub fn prior_id(&self) -> &str {
        &self.prior_id
    }

    /// Read-only task fingerprint (copied verbatim from the OC-02 report).
    #[must_use]
    pub fn task_fingerprint(&self) -> &str {
        &self.task_fingerprint
    }

    /// Read-only ordered entries (rerank order).
    #[must_use]
    pub fn entries(&self) -> &[SelectionInfluenceEntryV1] {
        &self.entries
    }

    /// Deterministic canonical bytes (JCS single-line JSON, lexicographic
    /// member order: config_hash, entries, influence_id, prior_id,
    /// task_fingerprint, version; spec §6). Entries render event_id (wire
    /// member name `event_id`; the Rust field is event_id_text),
    /// entry_reason, lexical_ppm, prior_ppm, score_ppm in rerank order.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when members are not frozen.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        let mut json = String::from("{");
        push_json_string(&mut json, "config_hash", &self.config_hash);
        json.push_str(",\"entries\":[");
        for (i, e) in self.entries.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            let mut entry = String::from("{");
            push_json_string(&mut entry, "entry_reason", e.entry_reason);
            json.push_str(&entry);
            json.push_str(",\"event_id\":");
            json.push_str(&json_quote(&e.event_id_text));
            json.push_str(",\"lexical_ppm\":");
            json.push_str(&e.lexical_ppm.to_string());
            json.push_str(",\"prior_ppm\":");
            json.push_str(&e.prior_ppm.to_string());
            json.push_str(",\"score_ppm\":");
            json.push_str(&e.score_ppm().to_string());
            json.push('}');
        }
        json.push_str("],");
        push_json_string(&mut json, "influence_id", &self.influence_id);
        json.push(',');
        push_json_string(&mut json, "prior_id", &self.prior_id);
        json.push(',');
        push_json_string(&mut json, "task_fingerprint", &self.task_fingerprint);
        json.push_str(",\"version\":");
        json.push_str(&self.version.to_string());
        json.push('}');
        Ok(json.into_bytes())
    }
}

/// The 19-member execution body (spec §6, JCS lexicographic at render).
/// Every member is derived per the §6 derivation table; `recipient_head`
/// is JSON `null` when absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionExecutionBodyV1 {
    /// §6: hex(BLAKE3(`oc-04-b3cand-v1\0` + sorted-dedup candidate IDs)).
    pub b3_candidate_fingerprint: String,
    /// §6: hex(BLAKE3(`oc-04-b3policy-v1\0` + NUL-joined `kinds()` order)).
    pub b3_policy_fingerprint: String,
    /// §6: hex(BLAKE3(`oc-04-b6warn-v1\0` + uncertainty() exposure)).
    pub b6_warnings_hash: String,
    /// §6: caller `SelectionBudget.max_exported_bytes` copy (u64).
    pub budget_max_bytes: u64,
    /// §6: caller `SelectionBudget.max_selected_events` copy (u64).
    pub budget_max_events: u64,
    /// §6: B3 closed count (u64 decimal).
    pub closed_count: u64,
    /// §6: hex(BLAKE3(`oc-04-closed-v1\0` + closed IDs)).
    pub closed_hash: String,
    /// §6: `Oc04ConfigV1::config_hash`.
    pub config_hash: String,
    /// §6: versioned critical projection string.
    pub critical_projection: String,
    /// §6: B4 delta count (u64 decimal).
    pub delta_count: u64,
    /// §6: hex(BLAKE3(`oc-04-delta-v1\0` + B4 wire bytes)).
    pub delta_hash: String,
    /// §9: derived execution ID (`oc04exec1_` + base64url no-pad).
    pub execution_id: String,
    /// §6: hex(BLAKE3(`oc-04-handoff-v1\0` + final post-B7 handoff wire)).
    pub handoff_hash: String,
    /// §6: influence record ID (copied).
    pub influence_id: String,
    /// §6: len(reranked pre-closure set) (u64).
    pub pre_closure_count: u64,
    /// §6: hex(BLAKE3(`oc-04-preclosure-v1\0` + pre-closure IDs)).
    pub pre_closure_ids_hash: String,
    /// §6: verified prior artifact ID (copied).
    pub prior_id: String,
    /// §6: B5-verified recipient head canonical text, `None` → JSON null.
    pub recipient_head: Option<String>,
    /// §6: decimal JSON integer `1` (OC-02/OC-03 precedent).
    pub version: u64,
}

/// The signed execution envelope: `{ body, signer, signature }` (spec §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedExecutionV1 {
    body: SelectionExecutionBodyV1,
    signer: Vec<u8>,
    signature: Vec<u8>,
}

impl SignedExecutionV1 {
    /// Issues the signed envelope over domain `oc-04-exec-v1\0` +
    /// canonical(body) (spec §6). The body's `execution_id` MUST be the
    /// §9-derived ID (prefix + placeholder discipline enforced here —
    /// a body carrying the raw placeholder or a non-derived id is
    /// rejected at issuance).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] if the body's `version` is not
    /// the frozen decimal integer `1` (spec §6), if its `execution_id`
    /// does not equal `derive_execution_id(&body)`, or if any derived
    /// member fails the §6 invariant checks (config hash shape, hex
    /// fingerprints, projection prefix, id prefixes).
    pub fn issue(
        body: SelectionExecutionBodyV1,
        signer: &SigningIdentity,
    ) -> Result<Self, OutcomeError> {
        validate_execution_body(&body)?;
        if body.execution_id != derive_execution_id(&body) {
            return Err(OutcomeError::Malformed);
        }
        let canonical = render_execution_body(&body);
        let signature = signer.sign_domain_message(OC04_EXEC_SIGNATURE_DOMAIN, &canonical);
        Ok(Self {
            body,
            signer: signer.author().to_bytes().to_vec(),
            signature,
        })
    }

    /// Full-recompute verification: the signature is verified over the
    /// RE-RENDERED canonical body bytes — the recorded signature and body
    /// are never trusted (spec §6; OC-03 rebuild-based lesson).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on any structural or
    /// signature failure.
    pub fn verify(&self) -> Result<(), OutcomeError> {
        if self.signature.len() != 64 || self.signer.len() != 32 {
            return Err(OutcomeError::Malformed);
        }
        let canonical = render_execution_body(self.body());
        let mut author = [0_u8; 32];
        author.copy_from_slice(&self.signer);
        let author = contextmesh::model::AuthorId::from_bytes(author);
        verify_domain_message(
            author,
            OC04_EXEC_SIGNATURE_DOMAIN,
            &canonical,
            &self.signature,
        )
        .map_err(|_| OutcomeError::Malformed)
    }

    /// Read-only body access.
    #[must_use]
    pub fn body(&self) -> &SelectionExecutionBodyV1 {
        &self.body
    }

    /// Read-only signer (32-byte AuthorId).
    #[must_use]
    pub fn signer(&self) -> &[u8] {
        &self.signer
    }

    /// Read-only signature (64-byte Ed25519).
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }
}

/// The ONLY token over a verified OC-03 prior (spec §1/§8): constructed
/// exclusively by [`VerifiedPrior::verify`], which runs the full
/// rebuild-based `verify_prior` plus the §7.1 canonical-payload gate.
/// Field privacy (E07 compile gate, 4F) rests on this type owning the
/// artifact without exposing construction.
#[derive(Debug, Clone)]
pub struct VerifiedPrior {
    prior: SaliencePriorV1,
}

impl VerifiedPrior {
    /// Verifies prior artifact bytes against the caller's chain inputs by
    /// REBUILDING every intermediate (OC-03 §8), after first rejecting any
    /// sessions/events payload string that is not itself canonical payload
    /// text (spec §7.1 canonicalization gate — X09).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on non-canonical payload text,
    /// any structural failure, or rebuild divergence.
    pub fn verify(
        bytes: &[u8],
        sessions: &[SessionPayloads<'_>],
        reports: &[ReportContribution],
        events: &[(&str, &str)],
        config: &PriorConfigV1,
    ) -> Result<Self, OutcomeError> {
        // Canonicalization gate: every payload string must round-trip
        // through canonical_payload_bytes byte-identically (§7.1).
        for session in sessions {
            for payload in session.payloads() {
                let value: serde_json::Value =
                    serde_json::from_str(payload).map_err(|_| OutcomeError::Malformed)?;
                let canonical =
                    canonical_payload_bytes(&value).map_err(|_| OutcomeError::Malformed)?;
                if canonical != payload.as_bytes() {
                    return Err(OutcomeError::Malformed);
                }
            }
        }
        for (_event_text, payload) in events {
            // The payload string must parse as JSON and round-trip through
            // canonical_payload_bytes byte-identically (§7.1 gate — the
            // events list carries (event text, payload string) pairs).
            let value: serde_json::Value =
                serde_json::from_str(payload).map_err(|_| OutcomeError::Malformed)?;
            let canonical = canonical_payload_bytes(&value).map_err(|_| OutcomeError::Malformed)?;
            if canonical != payload.as_bytes() {
                return Err(OutcomeError::Malformed);
            }
        }
        // Full OC-03 rebuild-based verification (never trusts recorded
        // intermediates).
        verify_prior(bytes, sessions, reports, events, config)?;
        let prior = crate::prior::parse_prior_bytes(bytes)?;
        Ok(Self { prior })
    }

    /// Read-only derived prior ID (OC-03).
    #[must_use]
    pub fn prior_id(&self) -> &str {
        self.prior.prior_id()
    }

    /// Positive vector entries, read-only, entity-name-ascending — the
    /// OC-03 `PriorSeedV1` view without conversion (spec §8).
    #[must_use]
    pub fn positive_seeds(&self) -> &[PriorSeedV1] {
        self.prior.vector()
    }
}

/// RAII guard over `verify_execution`'s scratch `RepairHistory` (spec §8):
/// fail-closed reservation — rejects same-path as the production history
/// and any pre-existing file, reserves via `File::create_new`, and DELETES
/// the file on drop (`RepairHistory` has no drop cleanup; OC-04 owns the
/// guard). Full B7 replay wiring arrives with `verify_execution` at 4E;
/// the guard ships here because its reservation contract is schema-level
/// (X12/X12b adversarial rows).
#[derive(Debug)]
pub struct ScratchHistoryGuard {
    path: PathBuf,
}

impl ScratchHistoryGuard {
    /// Fail-closed, atomically reserves `path` for a fresh scratch
    /// history. `production_path` is the caller's live `RepairHistory`
    /// path; `path == production_path` → Err, existing file → Err,
    /// then `File::create_new` reserves the path before any history is
    /// opened at it.
    ///
    /// # Errors
    /// Returns [`std::io::Error`] on same-path rejection, pre-existing
    /// file, or reservation failure.
    pub fn reserve(path: &Path, production_path: &Path) -> std::io::Result<Self> {
        if path == production_path {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "scratch history path must differ from the production history path",
            ));
        }
        let file = std::fs::File::create_new(path)?;
        drop(file);
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// The reserved scratch path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchHistoryGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// §6 invariant checks over the derived/structural members of an
/// execution body, enforced at issuance so a caller holding the public
/// struct cannot seal a semantically invalid envelope (Quality-QB3):
/// version frozen at 1, all hash/fingerprint members lowercase hex,
/// ID prefixes, and the versioned critical-projection prefix.
fn validate_execution_body(body: &SelectionExecutionBodyV1) -> Result<(), OutcomeError> {
    if body.version != 1 {
        return Err(OutcomeError::Malformed);
    }
    for (value, min_len) in [
        (&body.b3_candidate_fingerprint, 0),
        (&body.b3_policy_fingerprint, 0),
        (&body.b6_warnings_hash, 0),
        (&body.closed_hash, 0),
        (&body.delta_hash, 0),
        (&body.handoff_hash, 0),
        (&body.pre_closure_ids_hash, 0),
    ] {
        if !is_lowercase_hex(value, min_len) {
            return Err(OutcomeError::Malformed);
        }
    }
    if !is_lowercase_hex(&body.config_hash, 64) {
        return Err(OutcomeError::Malformed);
    }
    if !body.prior_id.starts_with(crate::prior::PRIOR_ID_PREFIX) {
        return Err(OutcomeError::Malformed);
    }
    if !body.influence_id.starts_with(OC04_INFLUENCE_ID_PREFIX) {
        return Err(OutcomeError::Malformed);
    }
    if !body.critical_projection.is_empty() && !body.critical_projection.starts_with("critproj1:") {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

/// Lowercase-hex ASCII check with a minimum length floor.
fn is_lowercase_hex(value: &str, min_len: usize) -> bool {
    value.len() >= min_len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Renders the 19-member execution body in JCS lexicographic member order
/// (spec §6): b3_candidate_fingerprint, b3_policy_fingerprint,
/// b6_warnings_hash, budget_max_bytes, budget_max_events, closed_count,
/// closed_hash, config_hash, critical_projection, delta_count, delta_hash,
/// execution_id, handoff_hash, influence_id, pre_closure_count,
/// pre_closure_ids_hash, prior_id, recipient_head, version.
#[must_use]
pub fn render_execution_body(body: &SelectionExecutionBodyV1) -> Vec<u8> {
    let mut json = String::from("{");
    push_json_string(
        &mut json,
        "b3_candidate_fingerprint",
        &body.b3_candidate_fingerprint,
    );
    json.push(',');
    push_json_string(
        &mut json,
        "b3_policy_fingerprint",
        &body.b3_policy_fingerprint,
    );
    json.push(',');
    push_json_string(&mut json, "b6_warnings_hash", &body.b6_warnings_hash);
    json.push_str(",\"budget_max_bytes\":");
    json.push_str(&body.budget_max_bytes.to_string());
    json.push_str(",\"budget_max_events\":");
    json.push_str(&body.budget_max_events.to_string());
    json.push_str(",\"closed_count\":");
    json.push_str(&body.closed_count.to_string());
    json.push(',');
    push_json_string(&mut json, "closed_hash", &body.closed_hash);
    json.push(',');
    push_json_string(&mut json, "config_hash", &body.config_hash);
    json.push(',');
    push_json_string(&mut json, "critical_projection", &body.critical_projection);
    json.push_str(",\"delta_count\":");
    json.push_str(&body.delta_count.to_string());
    json.push(',');
    push_json_string(&mut json, "delta_hash", &body.delta_hash);
    json.push(',');
    push_json_string(&mut json, "execution_id", &body.execution_id);
    json.push(',');
    push_json_string(&mut json, "handoff_hash", &body.handoff_hash);
    json.push(',');
    push_json_string(&mut json, "influence_id", &body.influence_id);
    json.push_str(",\"pre_closure_count\":");
    json.push_str(&body.pre_closure_count.to_string());
    json.push(',');
    push_json_string(
        &mut json,
        "pre_closure_ids_hash",
        &body.pre_closure_ids_hash,
    );
    json.push(',');
    push_json_string(&mut json, "prior_id", &body.prior_id);
    json.push_str(",\"recipient_head\":");
    match &body.recipient_head {
        Some(head) => json.push_str(&json_quote(head)),
        None => json.push_str("null"),
    }
    json.push_str(",\"version\":");
    json.push_str(&body.version.to_string());
    json.push('}');
    json.into_bytes()
}

/// Derives the execution ID per §9: BLAKE3 over `oc-04-exec-v1-id\0` +
/// canonical body bytes with the id member = literal `"execution_id"`,
/// prefix `oc04exec1_` + base64url no-pad (placeholder discipline —
/// compute LAST, before signing).
#[must_use]
pub fn derive_execution_id(body: &SelectionExecutionBodyV1) -> String {
    let mut placeholder = body.clone();
    placeholder.execution_id = "execution_id".to_owned();
    let canonical = render_execution_body(&placeholder);
    let mut hasher = blake3::Hasher::new();
    hasher.update(OC04_EXECUTION_ID_DOMAIN);
    hasher.update(&canonical);
    format!(
        "{OC04_EXECUTION_ID_PREFIX}{}",
        base64_url_no_pad(hasher.finalize().as_bytes())
    )
}

/// Appends a JCS-escaped JSON string member. Internal to this module; the
/// escaping must stay byte-identical across all OC-04 renderers.
fn push_json_string(output: &mut String, key: &str, value: &str) {
    output.push('"');
    output.push_str(key);
    output.push_str("\":");
    output.push_str(&json_quote(value));
}

fn push_json_u64(output: &mut String, key: &str, value: u64) {
    output.push('"');
    output.push_str(key);
    output.push_str("\":");
    output.push_str(&value.to_string());
}

/// Minimal JSON string quoting, matching the crate's existing renderers
/// byte-for-byte (prior.rs/attribution.rs precedent): short escapes for
/// `\b`/`\f`/`\n`/`\r`/`\t`, lowercase-hex `\u00xx` for other C0
/// controls, non-ASCII passed through.
fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// base64url, no padding (ocprior1_ precedent, spec §9 note 5).
fn base64_url_no_pad(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_execution_body() -> SelectionExecutionBodyV1 {
        let mut body = SelectionExecutionBodyV1 {
            b3_candidate_fingerprint: String::new(),
            b3_policy_fingerprint: String::new(),
            b6_warnings_hash: String::new(),
            budget_max_bytes: 1,
            budget_max_events: 1,
            closed_count: 0,
            closed_hash: String::new(),
            config_hash: "0".repeat(64),
            critical_projection: String::new(),
            delta_count: 0,
            delta_hash: String::new(),
            execution_id: "execution_id".to_owned(),
            handoff_hash: String::new(),
            influence_id: format!("{OC04_INFLUENCE_ID_PREFIX}fixture"),
            pre_closure_count: 0,
            pre_closure_ids_hash: String::new(),
            prior_id: format!("{}fixture", crate::prior::PRIOR_ID_PREFIX),
            recipient_head: None,
            version: 1,
        };
        body.execution_id = derive_execution_id(&body);
        body
    }

    #[test]
    fn tampered_execution_signature_rejected_by_envelope_verifier() {
        let signer = SigningIdentity::from_fixture_seed([91; 32]);
        let mut envelope = SignedExecutionV1::issue(valid_execution_body(), &signer)
            .expect("valid fixture envelope");
        envelope.verify().expect("precondition: original verifies");
        envelope.signature[0] ^= 0x01;
        assert!(
            envelope.verify().is_err(),
            "the envelope verifier itself must reject a one-bit signature mutation"
        );
    }
}
