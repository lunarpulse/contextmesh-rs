# OB-12 Option B Semantic Mechanisms Decision Evidence (gate B12)

candidate-commit: c1d485d (OB-11 capability commit; the parent of the OB-12 evidence commit)
procedure-tree: 25e1f2a7cf82ddb92fad884fca826264f72d6f61 (tree of the candidate commit; the OB-12 commit records the semantic-mechanisms decision and this evidence)
gate: scripts/verify-ob12.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
decision: NON-ADOPTION (recorded; not a silent deferral)
option-b-gate: unblocked-by-complete-verdict (OB-01 through OB-11 complete; OB-12 resolves B2's named heavy mechanisms and keeps B2 complete under the frozen spec)

## Scope of this evidence

OB-12 resolves the B2 named heavy mechanisms (embeddings, vector search,
reranking) from the frozen spec `spec-option-b-source-grounded-context-handoff.md`
and package OB-12 from `option-b-delivery-plan.md`. The audit
(`ob-12-semantic-mechanisms-audit.md`) evaluates every surveyed candidate
against the frozen criteria; none passes, so the non-adoption decision is
recorded with the demonstrated baseline:

- additive doc note in `src/lib.rs` only — no code module and no test matrix
  (the delivery plan's `tests/ob12_semantic.rs` exists only "if adopted");
- no change to any other source module and no change to any existing test
  file (verified by the gate's additive-only diff check);
- no new dependency: the locked closure stays at 320 and the feature graph is
  byte-identical to the OA baseline.

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
- dependency-closure: 320 (unchanged; no semantic-mechanism dependency
  entered the closure — the gate additionally asserts the semantic surfaces
  `ort`, `candle-core`, `tch`, `fastembed`, `hnswlib`, `usearch`,
  `tokenizers`, `half`, `onnxruntime` are absent).
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added.

## Decision record

**Decision: NON-ADOPTION.** Every surveyed candidate for the three named
mechanisms fails at least one frozen audit criterion:

- **Embeddings** (`ort`/onnxruntime, `candle`, `tch`, `fastembed`): native
  runtime surfaces, runtime model downloads (violating the offline
  discipline), and new dependencies outside the pinned 320 closure.
- **Vector search** (`hnswlib`, `usearch`, sqlite vector extensions): new
  native dependencies outside the pinned closure; sqlite alternates are
  explicitly on the forbidden-surface list.
- **Reranking** (cross-encoder crates): inherit the embedding failures; no
  offline reranker exists in the frozen closure.

This is a recorded decision with baseline evidence, not a silent deferral. B2
remains complete under the frozen spec: the OB-02 deterministic
lexical/term-frequency selector enforces budget and records provenance, and
its load-bearing quality is demonstrated by the frozen B8 evaluation suite
(withheld-context cases fail, repaired cases pass).

## B12 success evidence

- The audit artifact records the candidate-by-candidate evaluation against the
  frozen criteria and the non-adoption verdict
  (`ob-12-semantic-mechanisms-audit.md`, asserted present by the gate).
- The demonstrated baseline passes: the B8 evaluation suite is green and every
  prior OB matrix (OB-11..OB-01) is green (Step 6 of the gate).
- The dependency closure stays at 320 with no semantic surface present and a
  byte-identical feature graph (Step 3 of the gate).
- No code module or test matrix was added; only the additive lib.rs doc note
  records the decision (Step 2 of the gate).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/lib.rs | doc note for OB-12 (the recorded decision) | record the semantic-mechanisms decision in the crate docs (additive registration only) |
| _bmad-output/verification-artifacts/ob-12-semantic-mechanisms-audit.md (new) | the structured dependency audit (criteria, candidate table, verdict) | the audit artifact that records the non-adoption decision |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs, zero
changes to every other source module, no change to any existing test file,
and that no `tests/ob12_semantic.rs` exists (the decision is non-adoption).

## Acceptance per delivery plan

- Either a compliant mechanism is adopted with pinned artifacts and an audit
  record, or a non-adoption decision is recorded with baseline evidence: yes
  — non-adoption is recorded in the audit artifact and this evidence, with
  the demonstrated lexical baseline and B8 evaluation results.
- B2 remains complete under the frozen spec (selection respects budget,
  provenance recorded, version changes never history): yes — the OB-02
  selector is unchanged and its matrices remain green; the closure, fixtures,
  and feature graph are unchanged.

## Regression

- OB-11 capability matrix green (`cargo test --test ob11_capability`).
- OB-09 summaries matrix green (`cargo test --test ob09_summaries`).
- OB-10 sufficiency matrix green (`cargo test --test ob10_sufficient`).
- OB-08 eval matrix green (`cargo test --test ob08_eval`).
- OB-07 repair matrix green (`cargo test --test ob07_repair`).
- OB-06 omission matrix green (`cargo test --test ob06_omission`).
- OB-05 validity matrix green (`cargo test --test ob05_validity`).
- OB-04 delta matrix green (`cargo test --test ob04_delta`).
- OB-03 closure matrix green (`cargo test --test ob03_closure`).
- OB-02 selection matrix green (`cargo test --test ob02_selection`).
- OB-01 receipt matrix green (`cargo test --test ob01_receipts`).
- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA suites pass).
- OB-08 manifest, OB-02/OB-01/OA-01/OA-03/OA-04 golden fixtures
  byte-identical (sha256 asserted).

## Evidence owners

- Winston (dependency gate) — audit verdict for gate B12.
- Amelia (engineer) — completion verdict for gate B12.
- Lunarpulse (Ask-First approver) — approval of the non-adoption decision.
