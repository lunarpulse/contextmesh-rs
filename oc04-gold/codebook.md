# OC-04 4G+ Gold Label Codebook (v1.0 — FROZEN, plan v5.2 §4)

Operational definitions of the five frozen labels via a decision tree,
with worked examples and counterexamples. Judgment target: **the user
task's correct outcome (B5)** — a label answers "was this event part of
what the session needed to reach its stated goal?", not "is this text
topically similar?".

- `dead_end` = explicit observable pursuit-and-rejection only (B5).
- `uncertain` = audit-only; adjudicated before scoring (B6). Rate > 15%
  per annotator triggers codebook revision (§5.2b pilot discipline).
- Rule R-9: no free-text reasons containing task content.
- Revision discipline: post-freeze revisions require FULL re-label under
  the revised codebook; pre-revision labels archived, never merged (§5.3,
  BLOCKER NEW-2).

---

## 1. Decision tree (apply top-down; first match wins)

```
Q0. Does the event contain content from this session (not another
    session's transcript, not boilerplate)?  NO  → irrelevant
Q1. Did the session's stated task REQUIRE this content to reach its
    correct outcome (without it, the recorded outcome is wrong or the
    task stalls)?                            YES → required
Q2. Did the session USE the content and it improved/constrained the
    outcome, though other paths existed?      YES → supporting
Q3. Is there explicit evidence the session PURSUED and REJECTED this
    content (opened, quoted, then contradicted/discarded)?  YES → dead_end
Q4. None of the above with confidence (missing context, conflicting
    evidence, redaction blocks judgment, codebook gap)?     → uncertain
```

Precedence: `irrelevant` > `required` > `supporting` > `dead_end` >
`uncertain`. An event that is both used and later rejected is `dead_end`
(Q3 dominates Q2 — the observable is the rejection).

## 2. `required`

**Operational definition**: the recorded correct outcome depends on this
event; removing it makes the outcome wrong, incomplete, or impossible to
verify.

### Worked examples

**R-1 (required, positive)**: Task: "renew the TLS certificate for
api.example.com". The event contains the ACME account contact email used
for the renewal; the resulting report records a renewed cert tied to that
contact. Remove the event → the renewal could not have completed as
recorded. → `required`.

**R-2 (required, positive)**: Task: "fix the failing nightly build". The
event is the exact failing test log naming the regression introduced by
commit `a1b2c3`; the outcome cites that commit as fixed. → `required`.

**R-3 (required, positive)**: Task: "produce the Q3 invoice for customer
X". The event holds customer X's billing address; the invoice embeds it.
→ `required`.

### Counterexamples (NOT `required`)

**R-X1**: The event mentions TLS certificates generally (news article).
The renewal did not need it — any generic background. → `irrelevant`
(Q1 fails: outcome independent).

**R-X2**: The event contains the *old* certificate's expiry date, which
the annotator finds "helpful context". The recorded outcome never uses
it. → `irrelevant`, not `supporting` — no observable use.

**R-X3**: A second event also contains a required fact (duplicate).
Label `required` (dependence is on content, not uniqueness). Do not
downgrade to `supporting` because a duplicate exists.

## 3. `supporting`

**Operational definition**: the session used the content and it measurably
shaped or constrained the outcome, but the outcome could have been reached
without it.

### Worked examples

**S-1 (supporting, positive)**: Task: "choose a CDN vendor". The event is
a prior internal benchmark of one candidate; the outcome selects that
vendor citing the benchmark, though public benchmarks sufficed. →
`supporting`.

**S-2 (supporting, positive)**: Task: "write the incident postmortem".
The event is an unrelated earlier postmortem whose *format* the session
reuses. Outcome shaped, not dependent. → `supporting`.

**S-3 (supporting, positive)**: Task: "estimate migration duration". The
event is a past migration estimate the session adjusts downward; final
estimate also uses new measurement. → `supporting`.

### Counterexamples (NOT `supporting`)

**S-X1**: The event is topically related (another team's migration) but
no quote, token, or structure from it appears in the outcome trail. →
`irrelevant`.

**S-X2**: The event was opened in a browser tab but never referenced. →
`irrelevant` (exposure ≠ use).

**S-X3**: The event contained a decisive credential. That is dependence
(Q1: without it the task stalls) → `required`, not `supporting`.

## 4. `dead_end`

**Operational definition**: explicit observable pursuit-and-rejection —
the session engaged the content and then discarded/contradicted it. Pure
non-use is NOT a dead end.

### Worked examples

**D-1 (dead_end, positive)**: Session drafts an answer from the event's
spec paragraph, then finds the spec was superseded and rewrites. →
`dead_end`.

**D-2 (dead_end, positive)**: Session runs the event's suggested config
flag, gets an error, and removes it citing the event as source. →
`dead_end`.

**D-3 (dead_end, positive)**: Session quotes the event's vendor claim,
checks it against logs, marks it "incorrect — ignore". → `dead_end`.

### Counterexamples (NOT `dead_end`)

**D-X1**: The event was simply never opened. → `irrelevant` (absence of
use is absence of pursuit).

**D-X2**: The session found the event unhelpful in an annotation-free
sense but kept and used a sub-part. → `supporting` (use dominates).

**D-X3**: The session considered the content and still neither used nor
rejected it explicitly. → `uncertain` if intent is unclear.

## 5. `irrelevant`

**Operational definition**: fails Q0/Q1 — foreign-session content,
boilerplate, or content whose absence changes nothing observable.

### Worked examples

**I-1**: Calendar spam forwarded into the corpus. → `irrelevant`.

**I-2**: A duplicate of the session's own prompt echoed back by tooling. →
`irrelevant` (no new content).

**I-3**: An adjacent session's transcript fragment (cross-session bleed).
→ `irrelevant` (Q0 fails), even though topically on-task.

### Counterexamples (NOT `irrelevant`)

**I-X1**: Content the annotator personally finds low-value but the
outcome used → score by use: `required`/`supporting`.

**I-X2**: Redacted content whose redaction summary suggests task
relevance → `uncertain` (`redaction_blocks_judgment`), NOT `irrelevant`.

**I-X3**: Rejected content → `dead_end` (pursuit happened).

## 6. `uncertain` (audit-only; adjudicated before scoring)

Structured reason codes (§5.4, frozen; exactly one, optional note ≤80
chars, no task content):

| code | use when |
|---|---|
| `ambiguous_task_goal` | the session's goal itself is not determinable |
| `insufficient_context` | linked events needed for Q1 are missing |
| `conflicting_evidence` | events support both `required` and `dead_end` |
| `redaction_blocks_judgment` | privacy redaction removed the deciding evidence |
| `codebook_gap` | the case is real but no rule above covers it |

**Worked examples**

**U-1**: Prompt says "continue" with no antecedent; relevance of every
candidate is undecidable → `uncertain` / `ambiguous_task_goal`.

**U-2**: The deciding page was redacted; summary suggests use →
`uncertain` / `redaction_blocks_judgment`.

**U-3**: Two events contradict on whether the config was applied; the
judgment flips depending on which is true → `uncertain` /
`conflicting_evidence`.

### Counterexamples (NOT `uncertain`)

**U-X1**: Annotator is merely slow/tired → do not park work in
`uncertain`; apply the tree or flag to the other annotator's attention
through the protocol, not the label.

**U-X2**: The tree clearly yields `dead_end` but the annotator dislikes
the label → the tree decides; `uncertain` is not a softening device.

**U-X3**: Content is borderline between `required` and `supporting` →
Q1 decides (dependence = required). Only genuine undecidability between
*different branches* is `uncertain` — and borderline-severity within one
branch is not undecidability.

---

## 7. Adjudication (§5.4)

All non-`uncertain` disagreements are resolved by a written rule applied
by a third party (or the founder under blindness — adjudicator sees NO
arm, rank, score, system identity, or outcome summary). Each adjudication
records the clause applied. Post-consensus labels are used only for
scoring, NEVER for κ (κ is computed on raw independent labels, §5.3).

Every `uncertain` is adjudicated to one of the four decisive labels
before scoring; the audit trail (original `uncertain` + reason code +
resolution) ships with the corpus. The gold set MUST NOT contain
unresolved `uncertain` — the harness (`oc04_gold_realdata`) fails closed
on any such row.

## 8. Change log

| version | change |
|---|---|
| v1.0 | Initial freeze at plan v5.2 §4: decision tree, precedence, ≥3 examples + counterexamples per label, structured reason codes, R-9, adjudication blindness. |
