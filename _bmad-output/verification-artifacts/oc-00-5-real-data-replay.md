# OC-0.5 Hermes Real-Data Replay Evidence

date: 2026-08-21
branch: `OC-AttentionLedger`
repo-head: `7394c9b57d56ce0d557abf5924240cc09a08fcc2`
source-snapshot-sha256: `74cf2df7ff5752e3171ce8232396fbde2c27e3a914eed0da38f40cd3c3eba910`
aggregate-fingerprint: `c3b98e747131ece0dc5445e10bf96c354fe6ea64929f0b92ab9f13d20e2b312f`
verdict: DIRECTIONALLY POSITIVE — prior fusion improves silver-proxy retrieval; not causal gold and not a C1–C5 completion claim
privacy: aggregate-only; raw messages, IDs, paths, URLs, reasoning, and credentials are not emitted

## 1. Purpose

This replay tests whether the Option C mechanisms show useful direction on actual Hermes agent transcripts after Option B merged into `main`. It is the synthetic-to-real bridge recommended after OC-00.

It answers narrower questions than the draft spec:

1. How often do the validated M0/M1n value nominators find candidates in actual terminal episodes?
2. Does a PageRank prior seeded from prior terminal episodes improve retrieval of historical tool events under a fixed budget?
3. Does the Thorn proxy suppress repeated failures without harming ranking?
4. Are the aggregate results deterministic on a fixed SQLite snapshot?

It does **not** answer whether an event was causally load-bearing, whether a recipient comprehended the result, or whether the agent would succeed after receiving selected history.

## 2. Sources and reliability

| Source | Accessed | Reliability | Role |
|---|---|---:|---|
| consistent SQLite backup of `~/.hermes/state.db` | 2026-08-21 | Primary operational transcript; not OA signed DAG | real episode/event corpus |
| Option C prototype parsers and E2 PPR | 2026-08-21 | Primary mechanism reference | M0/M1/PPR compatibility |
| Option B lexical selector | 2026-08-21 | Primary implementation | baseline semantics |
| Option C draft | 2026-08-21 | Primary draft | alpha/beta/formula/budget contract |
| `results-aggregate.json` | 2026-08-21 | Primary execution output | reported metrics |

The live DB was not read as a moving evaluation target. Python `sqlite3.Connection.backup()` produced an integrity-checked private snapshot. The snapshot is ignored by the repository’s `*.db` rule.

## 3. Commands actually executed

```bash
python3 _bmad-output/implementation-artifacts/oc-real-data-replay/replay.py \
  snapshot \
  --source /home/cosmo/.hermes/state.db \
  --dest _bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db

python3 _bmad-output/implementation-artifacts/oc-real-data-replay/replay.py \
  run \
  --db _bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db \
  --out _bmad-output/implementation-artifacts/oc-real-data-replay/results-aggregate.json \
  --repo .

python3 _bmad-output/implementation-artifacts/oc-real-data-replay/replay.py \
  verify \
  --db _bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db \
  --results _bmad-output/implementation-artifacts/oc-real-data-replay/results-aggregate.json
```

The run command was executed twice on the same snapshot. Both produced:

```text
c3b98e747131ece0dc5445e10bf96c354fe6ea64929f0b92ab9f13d20e2b312f
```

Verification output:

```text
REPLAY VERIFIED: snapshot hash exact; aggregate-only output; no raw content keys; network=false
DETERMINISTIC REPLAY: PASS
```

## 4. Privacy and leakage boundary

The harness processes raw text only in memory and never writes it to outputs. Aggregate JSON contains no:

- message content, task, or answer
- session, user, chat, or thread identifier
- file path, URL, tool argument, or credential
- reasoning or system prompt
- per-event pseudonym or small-group example

Network calls are absent. The harness is Python stdlib-only.

Split leakage controls:

- parent/child session families remain in one split
- splits are temporal by each family’s first terminal episode
- query candidates are strictly earlier than the query
- current family is excluded from its historical candidate pool
- current episode tool results and answer define only the silver proxy label, never the ranking prior
- message token fields are not invented when absent

## 5. Corpus and split

Observed snapshot:

| Item | Count |
|---|---:|
| sessions | 148 |
| active messages | 11,704 |
| tool-role messages | 5,877 |
| eligible terminal episodes | 470 |
| eligible tool events inside those episodes | 4,770 |
| parent-session families | 80 |
| train / validation / test families | 48 / 16 / 16 |
| train / validation / test episodes | 327 / 75 / 68 |
| message rows with token_count | 0 |

A terminal episode is a user-task segment ending at a non-empty assistant `finish_reason='stop'`. This is an episode boundary, **not** a task-success label.

The selection evaluation used 59 of 68 test episodes. Nine had no eligible silver proxy gold. The historical candidate pool averaged 4,087.983 events and silver proxy gold averaged 117.831 events per evaluated query.

## 6. Methods

### 6.1 M0/M1n nomination

The replay keeps the prototype-compatible distinction:

- M0: long lowercase hex, integer, and date raw-value overlap from terminal answer back to tool results
- M1n: M0 plus humanized-number normalization such as `9.5M ↔ 9500000`

A high-precision lineage proxy is counted when a normalized answer value:

1. is absent from the user task,
2. appears in exactly one candidate tool result,
3. appears in the terminal answer.

This proxy is circular with respect to value overlap. It measures coverage and pipeline behavior; it cannot establish M0/M1 precision.

### 6.2 Prior and Thorn proxies

Nodes are historical tool-result events. Edges are:

- consecutive tool results in the same session
- bounded shared-entity edges, fanout 7

Entity extraction includes hashed local fingerprints for tool identity, URLs, paths, Rust-like symbols, and normalized values. Raw entities are not serialized.

Positive seeds are M1n-nominated prior tool results. Thorn seeds are error-like results followed by a same-tool retry and not positively nominated. Both are silver proxies.

PPR parameters match the draft/prototype direction:

- damping: 0.85
- iterations: 20
- alpha: 2.0
- beta: 0.6
- fixed budgets: 6 and 12

### 6.3 Selection proxy label

A historical event is proxy-relevant when it shares a bounded-frequency hashed entity with the current episode’s tool results or terminal answer. This tests cross-session entity continuity. It does not prove the current agent read or needed the historical event.

Compared arms:

1. Option B lexical TF
2. deterministic random
3. lexical × prior
4. lexical × thorn suppression
5. full draft multiplication
6. exploratory additive fusion

The additive arm is explicitly outside the current C4 contract.

## 7. Actual results

### 7.1 M0/M1n nomination coverage

| Metric | Observed |
|---|---:|
| terminal episodes | 470 |
| candidate tool events | 4,770 |
| M0 nominations | 1,141 |
| M1n nominations | 1,163 |
| M1n additions beyond M0 | 22 |
| episodes with no M0 nomination | 251 |
| episodes with no M1n nomination | 248 |
| unique-value silver lineages | 240 |
| unique lineages captured by M0 | 234 |
| unique lineages captured by M1n | 240 |

Interpretation:

- Exact/raw value backtracking works on real transcripts but leaves more than half of terminal episodes without any nomination.
- M1n closes all six unique-value lineage misses left by M0 in this snapshot, but its total incremental reach is modest: 22 additional candidate nominations.
- This supports keeping M0/M1 as cheap nominators, not treating them as the complete attribution layer.
- Precision is not reported because the silver label is constructed from the same value-flow relation.

### 7.2 Historical selection, budget 6

| Arm | Precision@6 | Recall@6 | nDCG@6 | Any-hit@6 |
|---|---:|---:|---:|---:|
| random | 0.022599 | 0.000751 | 0.016431 | 0.135593 |
| lexical TF | 0.166667 | 0.007051 | 0.165527 | 0.406780 |
| lexical × prior | **0.225989** | 0.008906 | **0.218528** | **0.508475** |
| lexical × thorn | 0.166667 | 0.007106 | 0.162275 | 0.406780 |
| full multiplicative | 0.223164 | **0.009353** | 0.212001 | 0.491525 |
| exploratory additive | 0.177966 | 0.007410 | 0.173149 | 0.423729 |

### 7.3 Historical selection, budget 12

| Arm | Precision@12 | Recall@12 | nDCG@12 | Any-hit@12 |
|---|---:|---:|---:|---:|
| random | 0.021186 | 0.001511 | 0.017678 | 0.203390 |
| lexical TF | 0.152542 | 0.012720 | 0.156669 | 0.457627 |
| lexical × prior | **0.217514** | **0.018743** | **0.215564** | **0.576271** |
| lexical × thorn | 0.151130 | 0.012639 | 0.153434 | 0.440678 |
| full multiplicative | 0.210452 | 0.018283 | 0.207506 | 0.542373 |
| exploratory additive | 0.163842 | 0.013453 | 0.165141 | 0.457627 |

## 8. What the results mean

### Finding A — real-data prior direction survives

At both budgets, lexical × prior exceeds lexical TF on precision, recall, nDCG, and any-hit. The full prior+thorn formula also exceeds lexical TF on the primary ranking metrics. Therefore the synthetic E2 direction is not isolated to the synthetic corpus.

This is evidence that past outcome-linked event structure can help rank historical records under the chosen entity-continuity proxy.

It is not evidence of causal load-bearingness or downstream task success.

### Finding B — the current Thorn proxy is not yet useful

Thorn-only is nearly equal to or slightly below lexical TF, and full multiplication is generally below prior-only despite remaining above lexical. The error+same-tool-retry proxy is therefore too weak or too broadly propagated for a positive deployment claim.

C3 needs:

- explicit failure classification by tool type
- world-state binding and expiry
- recipient/task conditioning
- false-suppression measurement

before thorn suppression is approved.

### Finding C — additive fusion did not solve this evaluation

The exploratory additive arm only slightly exceeds lexical at some metrics and remains below multiplicative prior fusion. This does not rehabilitate the draft’s zero-TF behavior. The replay recorded zero evaluated episodes where **all** proxy-gold candidates had TF=0, so this dataset slice does not constitute a strict vocabulary-mismatch test.

A dedicated zero-overlap stratum is still required before selecting the production fusion formula.

### Finding D — recall remains low

The proxy gold set is broad and the historical pool is large, so recall at fixed budgets remains low even when precision and any-hit improve. This reinforces the previous diagnosis of whole-history dilution and coarse entity edges.

Before C4 approval, test:

- task-similar top-k session seeding
- entity-type weighting
- time decay
- summary-level candidate narrowing
- recipient-conditioned priors

one at a time with ablation.

## 9. Reasoning process and rejected interpretations

### “Prior fusion is now proven useful”

Rejected. The label is entity continuity, not human or causal gold. The allowed conclusion is directional proxy improvement only.

### “M1 has perfect recall”

Rejected. It captured all 240 unique-value silver lineages, but this silver set is defined from normalized value flow and excludes paraphrase, reasoning, silent reuse, and redundant support.

### “Thorns work because full fusion beats lexical”

Rejected. Prior-only beats full fusion on most budget-12 metrics; the improvement is attributable mainly to positive prior, not demonstrated negative knowledge.

### “Additive is worse, so keep multiplicative forever”

Rejected. The test lacked an all-gold-zero-TF stratum. Additive remains a contract question requiring targeted evaluation.

### “finish_reason=stop means success”

Rejected. It only means generation ended normally.

## 10. Conclusion derivation matrix

| Question | Method | Observed | Conclusion |
|---|---|---|---|
| Is snapshot stable? | SQLite backup + integrity + SHA | PASS | yes |
| Is replay deterministic? | two runs, aggregate fingerprint | exact match | yes |
| Does M1n add real coverage? | terminal value lineage | 22 extra nominations; six extra unique lineage captures | modestly yes |
| Does positive prior improve proxy ranking? | same pool/budget temporal test | all primary metrics above lexical | directionally yes |
| Does current thorn proxy help? | thorn-only/full ablation | thorn-only flat/down; full below prior-only on most metrics | not demonstrated |
| Is strict vocabulary mismatch solved? | all-gold-zero-TF count | zero qualifying evaluated episodes | unanswered |
| Is C1–C5 complete? | compare evidence to contracts | no signed OA artifacts, causal judge, human gold, or B8 task outcome | no |

## 11. Validity threats

1. Hermes `state.db` is not the signed Option A DAG.
2. Session order and parent links are DAG surrogates.
3. Positive and thorn labels are silver proxies.
4. Entity continuity is not actual information use.
5. `finish_reason='stop'` is not success.
6. `token_count` is null for all message rows, so no token economics are claimed.
7. Tool names are missing on many rows.
8. Reasoning fields are excluded for privacy, hiding silent information use.
9. One user environment and roughly three months of data limit external validity.
10. Alpha/beta are inherited prototype values, not validated production optima.
11. The broad proxy gold set depresses recall and may reward generic recurring entities.
12. Duplicate content components were not used to reshape the current split; a stricter publication-grade study must group normalized duplicate components.
13. No confidence interval or human inter-annotator agreement is available yet.

## 12. Required next experiment

Before approving the C4 formula, run a preregistered human-gold replay:

1. sample terminal episodes by temporal family and vocabulary-overlap stratum
2. label candidates `required`, `supporting`, `irrelevant`, `dead_end`, `uncertain`
3. keep label text local and uncommitted
4. freeze alpha/beta/extractor/PPR before opening test labels
5. report family-cluster bootstrap intervals
6. include a strict TF=0 subset
7. run actual B8-style withheld/repaired task evaluation on a smaller, approved set
8. measure thorn false-suppression and expiry

## 13. Decision recommendation

- **Approve continuation to OC-0.5 design freeze:** yes.
- **Approve C4 deployment claim:** no.
- **Approve positive-prior mechanism for further implementation:** yes, behind recorded provenance and evaluation gates.
- **Approve current Thorn proxy:** no.
- **Freeze current multiplicative formula:** no; strict TF=0 evaluation is missing.
- **Treat this as C5 completion:** no; it is research evidence only.

The real-data replay strengthens the central Option C thesis: outcomes can create reusable selection priors. It also narrows the weakest parts: negative knowledge and strict vocabulary mismatch remain unsolved, and human/B8 evaluation is still necessary.

## 14. Final continuation verification after disk expansion

The fixed snapshot and recorded aggregate were rechecked after the VM disk was expanded. The environment had 22 GiB free on a 48 GiB root volume (53% used). Rust had not been removed by cleanup; Cargo 1.97.0 remained at `~/.cargo/bin/cargo`, but that directory was absent from the resumed shell's `PATH`. Rust verification therefore used the explicit binary path rather than reinstalling the toolchain.

Commands actually executed:

```bash
python3 _bmad-output/implementation-artifacts/oc-real-data-replay/replay.py \
  run \
  --db _bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db \
  --out _bmad-output/implementation-artifacts/oc-real-data-replay/private/results-final-verify.json \
  --repo .

python3 _bmad-output/implementation-artifacts/oc-real-data-replay/replay.py \
  verify \
  --db _bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db \
  --results _bmad-output/implementation-artifacts/oc-real-data-replay/private/results-final-verify.json

~/.cargo/bin/cargo test --all-targets --locked

cd _bmad-output/implementation-artifacts/oc-prototype-validation
~/.cargo/bin/cargo run --locked --quiet
python3 compare.py
```

Observed:

- recorded aggregate fingerprint: `c3b98e747131ece0dc5445e10bf96c354fe6ea64929f0b92ab9f13d20e2b312f`
- continuation rerun fingerprint: `c3b98e747131ece0dc5445e10bf96c354fe6ea64929f0b92ab9f13d20e2b312f`
- replay privacy/integrity verification: **PASS**
- deterministic aggregate comparison: **PASS**
- root Option A + Option B `cargo test --all-targets --locked`: **PASS** (zero failures; intentionally ignored fixture/cold-build tests remain ignored)
- OC Python-to-Rust gate: **PASS** (`E1` exact, `E3` exact, `E2` exact within the declared 0.002 tolerance)

For effect-size context, prior-only fusion versus lexical TF improved the silver-proxy ranking as follows:

| Budget | Precision | Recall | nDCG | Any-hit |
|---|---:|---:|---:|---:|
| 6 | +35.6% | +26.3% | +32.0% | +25.0% |
| 12 | +42.6% | +47.4% | +37.6% | +25.9% |

These are relative improvements over the lexical arm, not confidence intervals and not task-success effects. They do not change the report's directional-only verdict.
