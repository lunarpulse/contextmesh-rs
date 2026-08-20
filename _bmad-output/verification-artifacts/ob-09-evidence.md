# OB-09 Option B Hierarchical and Project Summaries Evidence (gate B9)

candidate-commit: 321ecbc (OB-10 sufficiency commit; the parent of the OB-09 evidence commit)
procedure-tree: acd4cbd4abaa77a02153e72416f028b01f46f4e1 (tree of the candidate commit; the OB-09 commit adds the summary module, the summary matrix, the verifier, and this evidence)
gate: scripts/verify-ob09.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 through OB-08 and OB-10 complete; OB-09 is the B9 hierarchical and project summaries package of the Option B delivery plan)

## Scope of this evidence

OB-09 implements gate B9 (hierarchical and project summaries) from the frozen
spec `spec-option-b-source-grounded-context-handoff.md` and package OB-09 from
`option-b-delivery-plan.md`. It is purely additive over Option A and
OB-01..OB-08, OB-10:

- new `src/summary.rs` (the OB-09 work module): the content-addressed
  `SummaryId`, the `SummaryPayload` at the three hierarchy levels
  (event → ref → project), the `Summary` record with the deterministic
  builders (`Summary::event`, `Summary::ref_summary`, `Summary::project`),
  DAG verification (`verify_against_dag`), and the `SummaryVerification`
  report;
- additive doc note and `pub mod summary;` registration in `src/lib.rs` only;
- no change to `src/selection.rs` (OB-10), `src/eval.rs` (OB-08),
  `src/repair.rs` (OB-07), `src/handoff.rs` (OB-06), `src/delta.rs` (OB-04),
  `src/closure.rs` (OB-03), `src/compiler.rs` / `src/receipt.rs` (OB-02 /
  OB-01 companions), or any Option A module (verified by the gate's
  additive-only diff check), no new dependency, no CLI change. The summary
  builders reuse the B4 verified ancestry walk read-only.

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
- dependency-closure: 320 (unchanged; OB-09 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. Summary derivation is
  self-contained in summary.rs and reads the store read-only.

## Design notes

**Content-addressed summaries.** A `Summary` commits its `SummaryId` (a
domain-separated BLAKE3 digest under `org.aaif.contextmesh.summary-id.v1`) to
the canonical RFC 8785/JCS wire of its payload — the level tag, the context,
exactly the covered events, and the derived note. Tampering with any field
breaks the content address, so verification recomputes the id and rejects a
mismatch as `Tampered`.

**References exactly the covered events.** Each level's covered set is the
payload's event list: an event summary covers its single event, a ref summary
covers exactly the ref's verified ancestry (the same strict walk the B4
recipient state uses), and a project summary covers the canonical union of
every local ref's verified ancestry in the context. Verification checks every
referenced event is present in the DAG under the summary's context; a missing
event is a `Drifted` rejection and a cross-context event is `WrongContext`.

**Hierarchy and determinism.** The three levels nest by coverage
(event ⊆ ref ⊆ project), giving a recipient the right altitude to enter a
large history. The builders are deterministic: identical DAGs produce
byte-identical summaries, and the summary wires round-trip.

## B9 success evidence

- An event summary verifies against the DAG, references exactly its event,
  and carries the derived note (`event_summary_verifies_and_references_exactly_its_event`).
- A ref summary verifies and references exactly the main ref's ancestry
  (`ref_summary_verifies_and_references_exactly_its_ancestry`).
- A project summary verifies and references exactly the context's events
  (`project_summary_verifies_and_references_exactly_the_context`).
- Tampering with the content address or with the payload (the derived note) is
  rejected as `Tampered` (`tampered_summary_is_rejected`).
- A summary verified against a store that no longer holds its referenced
  events is rejected as `Drifted` (`drifted_summary_is_rejected`).
- Identical DAGs produce identical content addresses and byte-identical wires,
  and the `sum1_` text identity round-trips
  (`summaries_are_content_addressed_and_deterministic`).
- A project summary covers the canonical union of multiple local refs'
  ancestries (`project_summary_covers_the_union_of_local_refs`).
- The hierarchy nests by coverage: event ⊆ ref ⊆ project
  (`hierarchy_event_ref_project_nest_by_coverage`).
- The canonical summary wire round-trips and still verifies
  (`summary_wire_round_trips`).
- An unknown ref and an empty context fail closed (`UnknownRef` / `Empty`)
  (`unknown_ref_and_empty_context_fail_closed`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/summary.rs (new) | `SummaryId`, `SummaryLevel`, `SummaryPayload` (event/ref/project), `Summary` (builders + `verify_against_dag` + `to_wire`), `SummaryVerification`, `SummaryError` | the OB-09 work module: derived, verifiable hierarchical summaries over Option A history |
| src/lib.rs | doc note for OB-09 and `pub mod summary;` | record the gate in the crate docs (additive registration only) |

The gate asserts zero deleted lines in lib.rs, tests/common/mod.rs, and
src/summary.rs, zero changes to delta.rs, receipt.rs, compiler.rs,
closure.rs, handoff.rs, repair.rs, eval.rs, selection.rs, crypto.rs, cli.rs,
model/store/error/sync/provider/http modules, and that src/summary.rs +
src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- A summary verifies against the DAG and references exactly its covered
  events: yes (verification checks every referenced event is present under
  the summary's context; the payload's event list is exactly the covered
  set).
- Tampering with a summary or its referenced events is rejected: yes
  (content-address recomputation rejects tampered records; missing or
  cross-context referenced events reject drifted records).

## Regression

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

- Mary (analyst) — summary design verdict for gate B9.
- Amelia (engineer) — completion verdict for gate B9.
- Lunarpulse — final approval.
