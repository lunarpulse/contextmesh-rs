//! Option B progressive context repair (gate B7).
//!
//! On comprehension or task failure, a repair sequence iteratively
//! re-includes omitted context and re-handoffs within a bounded repair loop.
//! The loop is driven by a task-outcome callback: OB-08's eval suite supplies
//! eval-driven convergence signals, and a scripted challenge supplies them in
//! direct use and in tests — the loop itself never fabricates an outcome.
//! Every attempt is recorded to a distinct JSON-lines history file that never
//! touches Option A's store and is not a second embedded database in the
//! store sense, and the sequence always reports convergence or
//! non-convergence. On non-convergence the original handoff is left intact;
//! a stale handoff is never re-negotiated (B5 composes into B7 through the
//! handoff's own validity check inside the B6 follow-up).

use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::closure::ClosedSelection;
use crate::delta::RecipientState;
use crate::handoff::{Handoff, HandoffError};
use crate::model::EventId;
use crate::store::Store;

/// Bounds that make a repair sequence finite (gate B7).
///
/// A sequence is always bounded: at most `max_iterations` task-outcome
/// evaluations, at most `max_re_included_events` distinct sources re-included,
/// and no handoff whose delta exceeds `max_delta_bytes` bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairBounds {
    /// Maximum number of task-outcome evaluations in one repair sequence.
    pub max_iterations: usize,
    /// Maximum number of distinct sources one sequence may re-include.
    pub max_re_included_events: usize,
    /// Maximum total bytes the delta of any handoff in the sequence may carry.
    pub max_delta_bytes: usize,
}

impl RepairBounds {
    /// Constructs bounds, failing closed when any bound is zero.
    pub fn new(
        max_iterations: usize,
        max_re_included_events: usize,
        max_delta_bytes: usize,
    ) -> Result<Self, RepairError> {
        if max_iterations == 0 || max_re_included_events == 0 || max_delta_bytes == 0 {
            return Err(RepairError::InvalidState);
        }
        Ok(Self {
            max_iterations,
            max_re_included_events,
            max_delta_bytes,
        })
    }
}

/// The task-outcome signal that drives one repair step (gate B7).
///
/// The driver of the repair loop evaluates the task against the current
/// handoff and returns exactly one of these outcomes. Outcome signals come
/// from OB-08's eval suite (eval-driven convergence) or from a scripted
/// challenge.
#[allow(clippy::large_enum_variant)] // NeedsSource carries the recomputed closed selection
pub enum TaskOutcome {
    /// The task succeeded with the current handoff: the sequence converges.
    Success,
    /// The task failed: the named omitted source must be re-included, and the
    /// driver supplies the closed selection that carries it. The source must
    /// be a listed omission of the current handoff, or the sequence fails
    /// closed — a repair cannot invent a source that was never omitted.
    NeedsSource {
        /// The omitted source the task needs.
        event: EventId,
        /// The recipient's stated reason, recorded on the challenge.
        note: String,
        /// The recomputed closed selection that carries the source.
        closed: ClosedSelection,
    },
    /// The task failed and no re-inclusion would help.
    Failure {
        /// The recorded reason the sequence cannot progress.
        note: String,
    },
}

/// Why a bounded repair sequence did not converge (gate B7).
///
/// The reason is typed, deterministic, and recorded on the terminal history
/// record, so non-convergence is auditable from the evidence alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NonConvergence {
    /// The sequence exhausted `RepairBounds::max_iterations`.
    IterationBudgetExceeded {
        /// The iteration bound that was reached.
        iterations: usize,
    },
    /// The sequence exhausted `RepairBounds::max_re_included_events`.
    ReInclusionBudgetExceeded {
        /// The number of sources already re-included.
        re_included: usize,
    },
    /// A follow-up handoff's delta exceeded `RepairBounds::max_delta_bytes`.
    ByteBudgetExceeded {
        /// The delta byte size that exceeded the bound.
        bytes: usize,
    },
    /// The task driver reported a failure with no helpful re-inclusion.
    OutcomeFailure {
        /// The recorded failure note.
        note: String,
    },
}

impl std::fmt::Display for NonConvergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IterationBudgetExceeded { iterations } => {
                write!(f, "iteration budget exceeded ({iterations})")
            }
            Self::ReInclusionBudgetExceeded { re_included } => {
                write!(f, "re-inclusion budget exceeded ({re_included})")
            }
            Self::ByteBudgetExceeded { bytes } => {
                write!(f, "delta byte budget exceeded ({bytes})")
            }
            Self::OutcomeFailure { note } => write!(f, "task failure: {note}"),
        }
    }
}

/// The terminal marker of a repair history record (gate B7).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TerminalRecord {
    /// The sequence converged on the handoff described by this record.
    Converged,
    /// The sequence stopped without converging, with the recorded reason.
    NonConverged(NonConvergence),
}

/// One recorded repair attempt — one line of the history file (gate B7).
///
/// The record describes one task-outcome evaluation: the handoff that was
/// offered (its delta events, remaining listed omissions, and delta bytes),
/// what the driver asked for, and whether this record is the terminal
/// convergent or non-convergent record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepairAttempt {
    /// Position of this record in the history file (assigned on append).
    pub sequence: u64,
    /// The repair iteration this record belongs to (0-based).
    pub iteration: usize,
    /// The source re-included for the next attempt, when the driver asked.
    pub re_included: Option<EventId>,
    /// The recorded note of the re-inclusion challenge or of the failure.
    pub note: Option<String>,
    /// The delta events of the offered handoff, in canonical order.
    pub events: Vec<EventId>,
    /// The remaining listed omissions of the offered handoff, canonical order.
    pub omissions: Vec<EventId>,
    /// The delta total bytes of the offered handoff.
    pub bytes: usize,
    /// Terminal marker: `Some` exactly on the final record of a sequence.
    pub terminal: Option<TerminalRecord>,
}

/// The outcome of one bounded repair sequence (gate B7).
#[derive(Debug)]
pub struct RepairReport {
    converged: bool,
    iterations: usize,
    re_included: Vec<EventId>,
    non_convergence: Option<NonConvergence>,
    handoff: Handoff,
}

impl RepairReport {
    /// Returns true when the sequence converged within its bounds.
    #[must_use]
    pub const fn converged(&self) -> bool {
        self.converged
    }

    /// Returns the number of task-outcome evaluations performed.
    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    /// Returns the sources re-included by the sequence, in the order they
    /// were re-included.
    #[must_use]
    pub fn re_included(&self) -> &[EventId] {
        &self.re_included
    }

    /// Returns the non-convergence reason, or `None` when the sequence
    /// converged.
    #[must_use]
    pub const fn non_convergence(&self) -> Option<&NonConvergence> {
        self.non_convergence.as_ref()
    }

    /// Returns the handoff the sequence offers. On convergence this is the
    /// handoff the task succeeded against; on non-convergence it is the
    /// original handoff, left intact.
    #[must_use]
    pub const fn handoff(&self) -> &Handoff {
        &self.handoff
    }
}

/// Stable typed repair failures (gate B7).
#[derive(Debug, Error)]
pub enum RepairError {
    /// The repair history file could not be read or written.
    #[error("repair history I/O failed")]
    Io(#[from] std::io::Error),
    /// A repair history record could not be serialized or parsed.
    #[error("repair history serialization failed")]
    Serialize(#[from] serde_json::Error),
    /// The underlying handoff negotiation failed (gate B6 fails closed).
    #[error("repair handoff negotiation failed")]
    Handoff(#[from] HandoffError),
    /// The repair configuration or state is invalid.
    #[error("repair configuration or state is invalid")]
    InvalidState,
}

/// The repair-history store: an append-only JSON-lines file (gate B7).
///
/// The history is a distinct file that records the attempt history of repair
/// sequences. It never touches Option A's store and is not a second embedded
/// database in the store sense — it is plain JSON lines, one record per
/// attempt, written with an append-only file handle. Nothing is ever
/// rewritten or truncated.
#[derive(Debug)]
pub struct RepairHistory {
    path: PathBuf,
    file: std::io::BufWriter<File>,
    sequence: u64,
}

impl RepairHistory {
    /// Opens (creating if needed) the history file for appending.
    ///
    /// Existing records are counted so the sequence numbering continues
    /// across runs.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepairError> {
        let path = path.as_ref().to_path_buf();
        let existed = path.exists();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let sequence = if existed {
            Self::read_attempts(&path)?.len() as u64
        } else {
            0
        };
        Ok(Self {
            path,
            file: std::io::BufWriter::new(file),
            sequence,
        })
    }

    /// Returns the history file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the number of attempts written so far (the next sequence
    /// number).
    #[must_use]
    pub const fn attempts(&self) -> u64 {
        self.sequence
    }

    /// Appends one attempt record as a JSON line and flushes the file.
    pub fn append(&mut self, attempt: &mut RepairAttempt) -> Result<(), RepairError> {
        attempt.sequence = self.sequence;
        self.sequence += 1;
        let mut line = serde_json::to_vec(attempt)?;
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.file.flush()?;
        Ok(())
    }

    /// Reads every attempt record from the given history file, in order.
    ///
    /// Blank lines are skipped; a malformed line fails closed.
    pub fn read_attempts(path: impl AsRef<Path>) -> Result<Vec<RepairAttempt>, RepairError> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        let mut attempts = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            attempts.push(serde_json::from_str(line)?);
        }
        Ok(attempts)
    }
}

/// Runs one bounded repair sequence (gate B7).
///
/// The loop evaluates the task driver against the current handoff at most
/// `bounds.max_iterations` times. A `Success` outcome converges the sequence
/// on the current handoff; a `Failure` outcome reports non-convergence
/// immediately; a `NeedsSource` outcome re-includes the named source through
/// the handoff's negotiation (B6) — the source must be a listed omission of
/// the current handoff and the supplied closed selection must really land it
/// in the follow-up delta, or the sequence fails closed. Every attempt is
/// recorded to the history file, and on non-convergence the original handoff
/// is left intact.
pub async fn run_repair<D, Fut>(
    store: &Store,
    handoff: &Handoff,
    recipient: &RecipientState,
    bounds: &RepairBounds,
    mut driver: D,
    history: &mut RepairHistory,
) -> Result<RepairReport, RepairError>
where
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    if bounds.max_iterations == 0
        || bounds.max_re_included_events == 0
        || bounds.max_delta_bytes == 0
    {
        return Err(RepairError::InvalidState);
    }
    let original = handoff.clone();
    let mut current = handoff.clone();
    let mut re_included: Vec<EventId> = Vec::new();

    for iteration in 0..bounds.max_iterations {
        let outcome = driver(&current).await;
        let mut attempt = RepairAttempt {
            sequence: 0,
            iteration,
            re_included: None,
            note: None,
            events: current.events(),
            omissions: current.omissions().iter().map(|o| o.event()).collect(),
            bytes: current.delta().total_bytes(),
            terminal: None,
        };
        match outcome {
            TaskOutcome::Success => {
                attempt.terminal = Some(TerminalRecord::Converged);
                history.append(&mut attempt)?;
                return Ok(RepairReport {
                    converged: true,
                    iterations: iteration + 1,
                    re_included,
                    non_convergence: None,
                    handoff: current,
                });
            }
            TaskOutcome::Failure { note } => {
                let non_convergence = NonConvergence::OutcomeFailure { note: note.clone() };
                attempt.note = Some(note);
                attempt.terminal = Some(TerminalRecord::NonConverged(non_convergence.clone()));
                history.append(&mut attempt)?;
                return Ok(RepairReport {
                    converged: false,
                    iterations: iteration + 1,
                    re_included,
                    non_convergence: Some(non_convergence),
                    handoff: original,
                });
            }
            TaskOutcome::NeedsSource {
                event,
                note,
                closed,
            } => {
                if re_included.len() >= bounds.max_re_included_events {
                    let non_convergence = NonConvergence::ReInclusionBudgetExceeded {
                        re_included: re_included.len(),
                    };
                    attempt.note = Some(note);
                    attempt.terminal = Some(TerminalRecord::NonConverged(non_convergence.clone()));
                    history.append(&mut attempt)?;
                    return Ok(RepairReport {
                        converged: false,
                        iterations: iteration + 1,
                        re_included,
                        non_convergence: Some(non_convergence),
                        handoff: original,
                    });
                }
                let challenge = current.challenge(event, &note)?;
                let follow_up = current
                    .follow_up(store, &closed, recipient, &challenge)
                    .await?;
                if follow_up.delta().total_bytes() > bounds.max_delta_bytes {
                    let non_convergence = NonConvergence::ByteBudgetExceeded {
                        bytes: follow_up.delta().total_bytes(),
                    };
                    attempt.note = Some(note);
                    attempt.terminal = Some(TerminalRecord::NonConverged(non_convergence.clone()));
                    history.append(&mut attempt)?;
                    return Ok(RepairReport {
                        converged: false,
                        iterations: iteration + 1,
                        re_included,
                        non_convergence: Some(non_convergence),
                        handoff: original,
                    });
                }
                attempt.re_included = Some(event);
                attempt.note = Some(note);
                history.append(&mut attempt)?;
                re_included.push(event);
                current = follow_up;
            }
        }
    }

    // The iteration budget was exhausted without a convergent outcome.
    let mut attempt = RepairAttempt {
        sequence: 0,
        iteration: bounds.max_iterations,
        re_included: None,
        note: None,
        events: current.events(),
        omissions: current.omissions().iter().map(|o| o.event()).collect(),
        bytes: current.delta().total_bytes(),
        terminal: Some(TerminalRecord::NonConverged(
            NonConvergence::IterationBudgetExceeded {
                iterations: bounds.max_iterations,
            },
        )),
    };
    history.append(&mut attempt)?;
    Ok(RepairReport {
        converged: false,
        iterations: bounds.max_iterations,
        re_included,
        non_convergence: Some(NonConvergence::IterationBudgetExceeded {
            iterations: bounds.max_iterations,
        }),
        handoff: original,
    })
}
