# Option C — Salience Provenance Layer ("Attention Ledger")

Founder charter (2026-08-20, lunarpulse_): the frozen dependency discipline of
Options A/B is OPENED for this design session. No dependency constraint applies
to the design: embeddings, vector search, rerankers, LLM judges, wall-clock
time, native runtimes are all permitted mechanisms.

What survives unconstrained (structural lessons, not constraints):
- Annotations are **signed artifacts referencing OA EventIds**, verified
  fail-closed against the DAG (B1 discipline).
- Selector/attribution provenance (identity, version, config hash) is recorded
  in every artifact.
- OA history is never mutated; Option C only adds a derived layer.

## 1. Concept

Option A answers *what happened* (proven, signed). Option B answers *what to
hand over* (bounded, verified, but lexical only — OB-12 non-adoption left the
semantic seat empty). **Option C answers *why it mattered***: which events were
load-bearing, which were dead ends, what they cost in time and tokens.

The creative inversion: salience is not predicted — it is **measured by
intervention**. Attribution by counterfactual replay ("proof-of-attention"),
with cheap deterministic mechanisms nominating candidates for expensive causal
ones. This is the layer MAP-Graph/Zep/GraphRAG do not have: they rank by
correlation; Option C can *prove* an event changed the outcome.

## 2. Mechanism ladder (cheap → expensive, each recorded with provenance)

| Tier | Mechanism | Cost | Catches | Blind spot |
|---|---|---|---|---|
| M0 | string-overlap backtracking (raw value tokens flowing into the answer) | ~0 | exact re-use of numbers/ids/paths | reformatted values, redundancy |
| M1 | normalized/semantic nomination (numeric parsing, embeddings) | low | reformatted ("9.5M" ↔ 9500000), paraphrase | still correlational |
| M2 | citation analysis (what the recipient explicitly references) | low | deliberate credit | silent reuse |
| M3 | single-event counterfactual ablation (re-judge without event X) | N judge calls | lone causal carriers | redundant pairs (removing one changes nothing) |
| M4 | Shapley-sampling coalition attribution | m·k judge calls | splits credit across redundant carriers; near-exact causal shares | cost; needs nomination from M0–M3 |

Ladder composition rule: **cheap mechanisms nominate, expensive mechanisms
verify.** M3/M4 never run on the whole DAG — only on the shortlist M0–M2
produce. Final attribution = fused score, every component tagged with its
mechanism, version, and config hash.

## 3. Artifacts (all signed, content-addressed, OB-receipt discipline)

- `OutcomeLedgerV1` — per task termination: task hash, outcome+quality,
  **cost ledger (wall-clock ms, tokens, tool calls, retries)**, attempt tree
  with error fingerprints, load-bearing set with per-event attribution +
  mechanism tags, dead-end list.
- `ThornIndexV1` — dead-end fingerprints (failure mode × entity × cost).
  Negative knowledge: a new task embedding near a thorn gets an explicit
  warning header, not silence. (Founder's "failed records are clues" —
  formalized.)
- `SaliencePriorV1` — propagated priors over the cross-session entity/provenance
  graph (personalized PageRank with annotation seeds; thorns as negative
  seeds). Versioned; the training lineage of any learned selector.

## 4. Flywheel

1. Sessions terminate → OutcomeLedger written (M0/M1 automatic; M3/M4 sampled).
2. Priors propagate over entity edges (same paths, symbols, fingerprints).
3. Next selection = lexical-TF baseline × (1 + α·prior) × (1 − β·thorn proximity).
4. B8-style eval gates each selector/prior version (load-bearingness must be
   demonstrated, never asserted).
5. Accumulated ledgers = labeled data → learning-to-rank selector upgrades.

## 5. Reconciliation with OB-12 (amnesty clause)

OB-12's NON-ADOPTION stays true **for the core closure** (320 crates,
offline). Option C lives in a separate crate/feature (`contextmesh-salience`)
with its own dependency budget; heavy mechanisms (embeddings, judge models)
sit behind the selector/attribution **trait** as sidecar adapters, exactly the
"thin mapping onto stable library types" the OA spec reserves for adapters.
A founder-signed decision record supersedes OB-12's scope note, not its
method.

## 6. Validation plan (this prototype)

- E1: ladder vs ground truth on synthetic agent sessions — P/R/F1 + logical
  judge-call cost per mechanism. Expected: M0 precise but misses
  reformatted/redundant; M1n fixes reformatting; M3 finds lone carriers but not
  redundant pairs; M4 completes. (Validates "nominate cheap, verify causal".)
- E2: cross-session propagation — recall@budget of genuinely useful history,
  `ob-baseline-lexical-tf` reimplementation vs prior-boosted selection, with
  deliberate task/event vocabulary mismatch.
- E3: cost ledger — useful vs wasted effort; judge-call economics of the ladder.

## 7. Port plan (after Python validation)

Rust crate `contextmesh-salience`: `ledger.rs` (OutcomeLedgerV1 + signing,
reusing ed25519-dalek/blake3), `attribution.rs` (M0/M1 deterministic cores +
trait hooks for M3/M4 judges), `propagate.rs` (PPR over entity graph),
`select.rs` (fusion selector), `thorn.rs`. Gates C1–C5 mirror the OB
intent/success style; B8-style eval suite gates every selector version.
