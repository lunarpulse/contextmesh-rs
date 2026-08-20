# OB-02 Option B Selection Evidence (gate B2)

candidate-commit: 1dd397a (OB-01 receipt commit; the parent of the OB-02 evidence commit)
procedure-tree: 5a908f832db29a398a9640c3fd5c982b6077307f (tree of the candidate commit; the OB-02 commit adds the selection core, the context compiler, the test matrix, the golden fixture, the verifier, and this evidence)
gate: scripts/verify-ob02.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 complete; OB-02 is the B2 core of the Option B delivery plan)

## Scope of this evidence

OB-02 implements gate B2 (task-conditioned source selection) from the frozen
spec `spec-option-b-source-grounded-context-handoff.md` and package OB-02 from
`option-b-delivery-plan.md`. It is purely additive over Option A and over
OB-01:

- new module `src/selection.rs` — the `Selector` trait, the `SelectionBudget`
  type (maximum selected event count plus maximum exported byte size), the
  deterministic baseline lexical/term-frequency selector (no new
  dependencies), the store-backed `select_sources` entry point, selector
  provenance, and the I/O edge-case matrix;
- new module `src/compiler.rs` — the context compiler that assembles the
  bounded set of source references and enforces the budget at handoff time
  (over-budget refused with a typed error, never truncated);
- additive module registration in `src/lib.rs` and one additive attribute in
  the shared test helper `tests/common/mod.rs`;
- no change to `src/receipt.rs` (OB-01), no change to any Option A module
  (verified by the gate's additive-only diff check), no new dependency, no CLI
  change.

## Environment and toolchain

| Item | Value |
|---|---|
| rustc | 1.97.0 (2d8144b78 2026-07-07) |
| cargo | 1.97.0 (c980f4866 2026-06-30) |
| toolchain source | rust-toolchain.toml override (1.97.0-x86_64-unknown-linux-gnu) |
| native prerequisite | cc present and usable (Turso bundled sqlite3.c build) |
| env overrides | none (gate runs with CARGO_NET_OFFLINE=true; no RUSTC/RUSTFLAGS/CARGO_BUILD_*/CARGO_TARGET_DIR) |
| worktree | clean at gate start and rerun |

## Supply-chain audit

- Direct dependencies (normal): unchanged from the OA baseline — turso =0.7.2
  (no features), tokio =1.53.1 (io-util, net, process, rt, signal, sync,
  time), clap =4.6.6 (derive, error-context, help, std, usage), axum =0.8.9
  (http1, json, tokio), reqwest =0.13.4 (json), blake3 =1.8.6 (std), serde,
  serde_json, serde_jcs =0.2.0, ed25519-dalek =3.0.0, base64, getrandom,
  zeroize. Dev: tokio =1.53.1 (macros, net, rt, sync, time).
- dependency-closure: 320 (unchanged; OB-02 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The baseline selector's tokenizer
  and term-frequency scorer are self-contained in selection.rs.

## Golden fixture

- `tests/fixtures/ob02-selection-golden.json`
- sha256: f2b52d826699c7116cba9cf182dd99dbb01b46ed736b43e5db997caa9d1787cb
- Provenance: `regenerate_golden_fixture` (ignored test) reconstructs it from
  the deterministic inputs (author A = fixture seed 7, author B = seed 9,
  context byte 8, B appends two text-bearing `agent.request` children to A's
  genesis, task "summarize the request chain" with a caller-supplied
  structured query, baseline selector `ob-baseline-lexical-tf` 0.1.0, budget
  `{max_selected_events: 3, max_exported_bytes: 4096}`).
- The non-ignored `golden_fixture_matches_reconstruction` test asserts the
  committed bytes still equal this exact reconstruction.

## B2 success evidence

- Budget respected for count and bytes: a selection whose ranked set exceeds
  `max_selected_events` is refused with `BudgetExceeded`, and one whose total
  exported byte size exceeds `max_exported_bytes` is refused — never truncated
  (`selection_respects_event_count_budget`, `selection_respects_byte_budget`).
- Provenance recorded: every result carries a validated `SelectorRecordV1`
  (identity, version, config hash). Two selector versions over the same
  history produce identical references but distinct provenance, and the new
  version's record never rewrites the old one (`two_selector_versions_produce_distinct_provenance`).
- Both task forms produce selections: free text and structured queries select
  identical references with identical canonical wire bytes; the task record
  captures verbatim + BLAKE3 content hash and the caller-supplied structured
  form (`structured_and_free_text_tasks_both_produce_selections`).
- Edge cases per the I/O matrix: empty history → `NoSources` marker with empty
  selection; empty/absent task → `EmptyTask` fail-closed; no matching source →
  `NoMatch` marker with empty selection plus an uncertainty note; selector
  failure → `SelectorError` fail-closed with the local-ref head unchanged
  (`empty_history_produces_no_sources_marker`, `empty_task_fails_closed`,
  `no_match_produces_empty_selection_with_uncertainty`,
  `selector_error_fails_closed_prior_state_intact`).
- Determinism: the same task/history/budget/selector produce byte-identical
  canonical results across runs; term-frequency ties break by canonical
  EventId text order (`selection_is_deterministic_across_runs`,
  `tie_break_is_canonical_event_order`).
- Unverifiable candidate fails closed: a candidate ID absent from the store is
  refused (`unverifiable_candidate_fails_closed`).
- Ranking is load-bearing and deterministic: the golden ranking is child2 (6)
  > child1 (5) > genesis (2) by term frequency
  (`golden_fixture_ranks_sources_deterministically`).
- Composes with OB-01: a receipt built from the selection's references,
  uncertainty notes, task record, and selector provenance verifies against the
  DAG with `checked_events == 4` (`selection_composes_with_receipt`).

## Additive changes (all additions, no deletions)

| File | Addition | Why |
|---|---|---|
| src/selection.rs | new module | selector trait, budget, baseline selector, provenance, edge cases |
| src/compiler.rs | new module | bounded source-reference assembly and budget enforcement |
| src/lib.rs | `pub mod selection;` `pub mod compiler;` + doc note | register the new Option B modules |
| tests/common/mod.rs | `#[allow(dead_code)]` on the shared `child` helper | each integration binary uses a subset of the shared helpers |

The gate asserts zero deleted lines in these files and zero changes to
receipt.rs, crypto.rs, cli.rs, model/store/error/sync/provider/http modules.

## Acceptance per delivery plan

- Selection respects the budget for count and bytes; over-budget selections are
  refused, not truncated: yes (BudgetExceeded matrix).
- The receipt records selector provenance; two selector versions produce
  distinct provenance records over the same history: yes.
- Free text and structured tasks both produce selections; receipts record the
  task verbatim and its content hash: yes (selection + OB-01 composition).
- Edge cases behave per the I/O matrix: yes (markers, fail-closed paths).

## Regression

- OB-01 receipt matrix green (`cargo test --test ob01_receipts`).
- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA suites pass).
- OB-01/OA-01/OA-03/OA-04 golden fixtures byte-identical (sha256 asserted).

## Evidence owners

- Amelia (engineer) — completion verdict for gate B2.
- Sally (UX) — review of the task-intake shape (free text + structured query).
- Lunarpulse — final approval.
