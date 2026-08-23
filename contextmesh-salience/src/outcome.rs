//! OutcomeLedgerV1 body/envelope composition and structural verification.
//!
//! This module is deliberately Store-free: it validates the signed artifact's
//! shape, bounds, snapshot binding, ID, and signature, but makes no claim that
//! referenced events exist or that refs are current.

use contextmesh::crypto::{SigningIdentity, verify_domain_message};
use contextmesh::model::{AuthorId, ContextId};
use serde::Serialize;
use serde_json::Value;

use crate::error::OutcomeError;
use crate::json;
use crate::types::{
    AttemptV1, AttributionMarkV1, CostLedgerV1, DeadEndV1, InputRefSnapshotV1, OUTCOME_ID_DOMAIN,
    OUTCOME_SIGNATURE_DOMAIN, OUTCOME_VERSION, OutcomeId, OutcomeLimits, OutcomeRecordV1,
    OutcomeSignature, QualityV1, TaskBindingV1, TerminalV1, TimestampText, validate_attempt_tree,
    validate_attribution_marks, validate_dead_ends, validate_warnings,
};

/// The exact, immutable version-1 outcome-ledger body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutcomeLedgerBodyV1 {
    version: u8,
    context: ContextId,
    input_refs: InputRefSnapshotV1,
    task: TaskBindingV1,
    terminal: TerminalV1,
    outcome: OutcomeRecordV1,
    quality: QualityV1,
    costs: CostLedgerV1,
    attempts: Vec<AttemptV1>,
    dead_ends: Vec<DeadEndV1>,
    attribution_marks: Vec<AttributionMarkV1>,
    warnings: Vec<String>,
    created_at: TimestampText,
    author: AuthorId,
}

impl OutcomeLedgerBodyV1 {
    /// Constructs a fully checked version-1 body without sorting or repair.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: ContextId,
        input_refs: InputRefSnapshotV1,
        task: TaskBindingV1,
        terminal: TerminalV1,
        outcome: OutcomeRecordV1,
        quality: QualityV1,
        costs: CostLedgerV1,
        attempts: Vec<AttemptV1>,
        dead_ends: Vec<DeadEndV1>,
        attribution_marks: Vec<AttributionMarkV1>,
        warnings: Vec<String>,
        created_at: TimestampText,
        author: AuthorId,
        limits: OutcomeLimits,
    ) -> Result<Self, OutcomeError> {
        let body = Self {
            version: OUTCOME_VERSION,
            context,
            input_refs,
            task,
            terminal,
            outcome,
            quality,
            costs,
            attempts,
            dead_ends,
            attribution_marks,
            warnings,
            created_at,
            author,
        };
        body.validate(limits)?;
        Ok(body)
    }

    /// Revalidates all body schema, ordering, aggregate, and byte bounds.
    pub fn validate(&self, limits: OutcomeLimits) -> Result<(), OutcomeError> {
        limits.validate()?;
        if self.version != OUTCOME_VERSION {
            return Err(OutcomeError::UnsupportedVersion);
        }
        InputRefSnapshotV1::from_parts(
            self.context,
            self.input_refs.fingerprint.clone(),
            self.input_refs.local.clone(),
            self.input_refs.remote.clone(),
        )?;
        self.task.validate(&limits)?;
        validate_terminal(&self.terminal)?;
        OutcomeRecordV1::new(
            self.outcome.value,
            self.outcome.evidence.clone(),
            self.outcome.provenance.clone(),
            &limits,
        )?;
        QualityV1::new(self.quality.clone(), &limits)?;
        self.costs.validate(&limits)?;
        for attempt in &self.attempts {
            AttemptV1::new(attempt.clone(), &limits)?;
        }
        validate_attempt_tree(&self.attempts, &limits)?;
        for dead_end in &self.dead_ends {
            DeadEndV1::new(dead_end.clone(), &limits)?;
        }
        validate_dead_ends(&self.dead_ends, &self.attempts, &limits)?;
        for mark in &self.attribution_marks {
            AttributionMarkV1::new(mark.clone(), &limits)?;
        }
        validate_attribution_marks(&self.attribution_marks, &limits)?;
        validate_warnings(&self.warnings, &limits)?;
        self.count_event_references(limits)?;
        if self.canonical_bytes_unchecked()?.len() > limits.max_wire_bytes {
            return Err(OutcomeError::LimitExceeded);
        }
        Ok(())
    }

    /// Returns exact JCS body bytes after revalidation.
    pub fn canonical_bytes(&self, limits: OutcomeLimits) -> Result<Vec<u8>, OutcomeError> {
        self.validate(limits)?;
        self.canonical_bytes_unchecked()
    }

    fn canonical_bytes_unchecked(&self) -> Result<Vec<u8>, OutcomeError> {
        json::jcs(self)
    }

    fn count_event_references(&self, limits: OutcomeLimits) -> Result<(), OutcomeError> {
        let mut total = 0_usize;
        let mut add = |count: usize| -> Result<(), OutcomeError> {
            total = total
                .checked_add(count)
                .ok_or(OutcomeError::LimitExceeded)?;
            if total > limits.max_event_references {
                return Err(OutcomeError::LimitExceeded);
            }
            Ok(())
        };
        add(self.input_refs.local.len())?;
        add(self.input_refs.remote.len())?;
        if matches!(self.terminal, TerminalV1::Event { .. }) {
            add(1)?;
        }
        add(self.outcome.evidence.len())?;
        if let QualityV1::Available { evidence, .. } = &self.quality {
            add(evidence.len())?;
        }
        for attempt in &self.attempts {
            add(attempt.event_refs.len())?;
        }
        for dead_end in &self.dead_ends {
            add(dead_end.event_refs.len())?;
        }
        for mark in &self.attribution_marks {
            add(1)?;
            add(mark.evidence.len())?;
        }
        Ok(())
    }

    /// Returns the frozen version marker.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }
    /// Returns the referenced ContextMesh context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }
    /// Returns the input-ref snapshot.
    #[must_use]
    pub const fn input_refs(&self) -> &InputRefSnapshotV1 {
        &self.input_refs
    }
    /// Returns the hash-only task binding.
    #[must_use]
    pub const fn task(&self) -> &TaskBindingV1 {
        &self.task
    }
    /// Returns the caller-declared terminal marker.
    #[must_use]
    pub const fn terminal(&self) -> &TerminalV1 {
        &self.terminal
    }
    /// Returns the caller-declared outcome record.
    #[must_use]
    pub const fn outcome(&self) -> &OutcomeRecordV1 {
        &self.outcome
    }
    /// Returns the quality record.
    #[must_use]
    pub const fn quality(&self) -> &QualityV1 {
        &self.quality
    }
    /// Returns the overall cost ledger.
    #[must_use]
    pub const fn costs(&self) -> &CostLedgerV1 {
        &self.costs
    }
    /// Returns the attempt tree in caller-provided canonical order.
    #[must_use]
    pub fn attempts(&self) -> &[AttemptV1] {
        &self.attempts
    }
    /// Returns dead ends in caller-provided canonical order.
    #[must_use]
    pub fn dead_ends(&self) -> &[DeadEndV1] {
        &self.dead_ends
    }
    /// Returns attribution marks in caller-provided canonical order.
    #[must_use]
    pub fn attribution_marks(&self) -> &[AttributionMarkV1] {
        &self.attribution_marks
    }
    /// Returns meaningful caller warning order.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    /// Returns the validated creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> &TimestampText {
        &self.created_at
    }
    /// Returns the signature-selecting author key.
    #[must_use]
    pub const fn author(&self) -> AuthorId {
        self.author
    }
}

/// The immutable signed OutcomeLedgerV1 envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SignedOutcomeLedgerV1 {
    outcome_id: OutcomeId,
    body: OutcomeLedgerBodyV1,
    signature: OutcomeSignature,
}

impl SignedOutcomeLedgerV1 {
    /// Validates, derives, signs, and self-verifies a body owned by `identity`.
    pub fn issue(
        identity: &SigningIdentity,
        body: OutcomeLedgerBodyV1,
        limits: OutcomeLimits,
    ) -> Result<Self, OutcomeError> {
        body.validate(limits)?;
        if body.author() != identity.author() {
            return Err(OutcomeError::IdMismatch);
        }
        let outcome_id = derive_outcome_id(&body, limits)?;
        let raw_signature =
            identity.sign_domain_message(OUTCOME_SIGNATURE_DOMAIN, &outcome_id.clone().to_bytes());
        let raw_signature: [u8; 64] = raw_signature
            .try_into()
            .map_err(|_| OutcomeError::SignatureInvalid)?;
        let ledger = Self {
            outcome_id,
            body,
            signature: OutcomeSignature::from_bytes(raw_signature),
        };
        ledger.verify(limits)?;
        Ok(ledger)
    }

    /// Strictly parses, validates, canonicality-checks, and verifies wire bytes.
    pub fn from_wire(input: &[u8], limits: OutcomeLimits) -> Result<Self, OutcomeError> {
        limits.validate()?;
        if input.len() > limits.max_wire_bytes {
            return Err(OutcomeError::LimitExceeded);
        }
        let value = json::parse_strict(input)?;
        if value_depth(&value) > limits.max_json_depth {
            return Err(OutcomeError::LimitExceeded);
        }
        let (outcome_id, body, signature) = parse_envelope(&value, limits)?;
        let canonical = json::jcs(&value)?;
        if canonical != input {
            return Err(OutcomeError::Noncanonical);
        }
        let expected = derive_outcome_id(&body, limits)?;
        if outcome_id != expected {
            return Err(OutcomeError::IdMismatch);
        }
        verify_domain_message(
            body.author(),
            OUTCOME_SIGNATURE_DOMAIN,
            &outcome_id.clone().to_bytes(),
            &signature.clone().to_bytes(),
        )
        .map_err(|_| OutcomeError::SignatureInvalid)?;
        Ok(Self {
            outcome_id,
            body,
            signature,
        })
    }

    /// Revalidates the body, derived ID, and strict domain signature.
    pub fn verify(&self, limits: OutcomeLimits) -> Result<(), OutcomeError> {
        self.body.validate(limits)?;
        let expected = derive_outcome_id(&self.body, limits)?;
        if self.outcome_id != expected {
            return Err(OutcomeError::IdMismatch);
        }
        verify_domain_message(
            self.body.author(),
            OUTCOME_SIGNATURE_DOMAIN,
            &self.outcome_id.clone().to_bytes(),
            &self.signature.clone().to_bytes(),
        )
        .map_err(|_| OutcomeError::SignatureInvalid)
    }

    /// Renders exact JCS envelope bytes after full revalidation.
    pub fn to_wire(&self, limits: OutcomeLimits) -> Result<Vec<u8>, OutcomeError> {
        self.verify(limits)?;
        let wire = json::jcs(self)?;
        if wire.len() > limits.max_wire_bytes {
            return Err(OutcomeError::LimitExceeded);
        }
        Ok(wire)
    }

    /// Returns the derived outcome identifier.
    #[must_use]
    pub fn outcome_id(&self) -> OutcomeId {
        self.outcome_id.clone()
    }
    /// Returns the validated immutable body.
    #[must_use]
    pub const fn body(&self) -> &OutcomeLedgerBodyV1 {
        &self.body
    }
    /// Returns the strict Ed25519 signature.
    #[must_use]
    pub fn signature(&self) -> OutcomeSignature {
        self.signature.clone()
    }
}

/// Derives the typed outcome ID from literal domain bytes plus exact body JCS.
pub fn derive_outcome_id(
    body: &OutcomeLedgerBodyV1,
    limits: OutcomeLimits,
) -> Result<OutcomeId, OutcomeError> {
    let canonical = body.canonical_bytes(limits)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(OUTCOME_ID_DOMAIN);
    hasher.update(&canonical);
    Ok(OutcomeId::from_bytes(*hasher.finalize().as_bytes()))
}

fn validate_terminal(terminal: &TerminalV1) -> Result<(), OutcomeError> {
    match terminal {
        TerminalV1::Event { .. } | TerminalV1::Unterminated { .. } => Ok(()),
    }
}

fn parse_envelope(
    value: &Value,
    limits: OutcomeLimits,
) -> Result<(OutcomeId, OutcomeLedgerBodyV1, OutcomeSignature), OutcomeError> {
    json::require_exact_keys(value, &["outcome_id", "body", "signature"])?;
    let outcome_id = parse_text(required(value, "outcome_id")?)?.parse::<OutcomeId>()?;
    let body = parse_body(required(value, "body")?, limits)?;
    let signature = parse_text(required(value, "signature")?)?.parse::<OutcomeSignature>()?;
    Ok((outcome_id, body, signature))
}

fn parse_body(value: &Value, limits: OutcomeLimits) -> Result<OutcomeLedgerBodyV1, OutcomeError> {
    json::require_exact_keys(
        value,
        &[
            "version",
            "context",
            "input_refs",
            "task",
            "terminal",
            "outcome",
            "quality",
            "costs",
            "attempts",
            "dead_ends",
            "attribution_marks",
            "warnings",
            "created_at",
            "author",
        ],
    )?;
    let version = required(value, "version")?
        .as_u64()
        .ok_or(OutcomeError::Malformed)?;
    if version != u64::from(OUTCOME_VERSION) {
        return Err(OutcomeError::UnsupportedVersion);
    }
    let context = parse_text(required(value, "context")?)?
        .parse::<ContextId>()
        .map_err(|_| OutcomeError::Malformed)?;
    let input_refs = parse_input_refs(required(value, "input_refs")?, context)?;
    let task = parse_nested(required(value, "task")?)?;
    let terminal = parse_nested(required(value, "terminal")?)?;
    let outcome = parse_nested(required(value, "outcome")?)?;
    let quality = parse_nested(required(value, "quality")?)?;
    let costs = parse_nested(required(value, "costs")?)?;
    let attempts = parse_vec(required(value, "attempts")?)?;
    let dead_ends = parse_vec(required(value, "dead_ends")?)?;
    let attribution_marks = parse_vec(required(value, "attribution_marks")?)?;
    let warnings = parse_strings(required(value, "warnings")?)?;
    let created_at = TimestampText::parse(parse_text(required(value, "created_at")?)?)?;
    let author = parse_text(required(value, "author")?)?
        .parse::<AuthorId>()
        .map_err(|_| OutcomeError::Malformed)?;
    let body = OutcomeLedgerBodyV1 {
        version: OUTCOME_VERSION,
        context,
        input_refs,
        task,
        terminal,
        outcome,
        quality,
        costs,
        attempts,
        dead_ends,
        attribution_marks,
        warnings,
        created_at,
        author,
    };
    body.validate(limits)?;
    Ok(body)
}

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value, OutcomeError> {
    value.get(name).ok_or(OutcomeError::Malformed)
}

fn value_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(value_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(value_depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn parse_text(value: &Value) -> Result<&str, OutcomeError> {
    value.as_str().ok_or(OutcomeError::Malformed)
}

fn parse_nested<T>(value: &Value) -> Result<T, OutcomeError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value.clone()).map_err(|_| OutcomeError::Malformed)
}

fn parse_vec<T>(value: &Value) -> Result<Vec<T>, OutcomeError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value.clone()).map_err(|_| OutcomeError::Malformed)
}

fn parse_strings(value: &Value) -> Result<Vec<String>, OutcomeError> {
    let values = value.as_array().ok_or(OutcomeError::Malformed)?;
    values
        .iter()
        .map(|value| parse_text(value).map(str::to_owned))
        .collect()
}

fn parse_input_refs(value: &Value, context: ContextId) -> Result<InputRefSnapshotV1, OutcomeError> {
    json::require_exact_keys(value, &["fingerprint", "local", "remote"])?;
    let fingerprint = parse_text(required(value, "fingerprint")?)?
        .parse()
        .map_err(|_| OutcomeError::Malformed)?;
    let local = parse_vec(required(value, "local")?)?;
    let remote = parse_vec(required(value, "remote")?)?;
    InputRefSnapshotV1::from_parts(context, fingerprint, local, remote)
}
