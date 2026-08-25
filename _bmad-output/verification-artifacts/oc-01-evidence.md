# OC-01 Implementation Evidence

**Status:** Final four-layer record. All eight acceptance gates (OC01-SETUP,
OC01-SCHEMA, OC01-CRYPTO, OC01-DAG, OC01-IO, OC01-ADVERSARIAL,
OC01-REGRESSION, OC01-EVIDENCE) have passing machine evidence at the commits
and commands recorded below. The verdict is limited to artifact
integrity/provenance recording (C1's OC-01 portion) and asserts nothing about
causal attribution, priors, selection utility, comprehension, or Option C
completion. OC-02 remains blocked until the separate P1 preregistration gate
passes (spec §15).

## 1. Sources

| Item | Value | Authority |
|---|---|---|
| Frozen spec/matrix baseline | `0cf192b625384283d10c008d4a0e984ae9d0be08` | git object |
| Stage 1A workspace boundary | `2edbe8d86dea645496dde44db7e3a2e4f6cc404e` | git object |
| Heavy-chain delegation kit | `5ef6be91246e83cad754a2639cd61380accf7dce` | git object |
| Gate allowlist fix (kit paths) | `59fd165bc850187d3983b4ccda7ddb90f180e81e` | git object |
| Evidence bundle commit | `9d363bd6a10ef29aa1a8f7af0d9e532d7ba625ab` | git object |
| Stage 1B acceptance evidence | `67a27be` (interim record preserved in §5) | git object |
| Stage 2A frozen primitives | `bb7f982` | git object |
| Stage 2B body/envelope | `11f4e85` | git object |
| Stage 2C crypto/golden/tamper | `16969a3` | git object |
| Stage 2D DAG verification + spec/matrix amendment | `0da83fe` | git object |
| Stage 2E bounded import/export | `af34449` | git object |
| Stage 2F adversarial/boundary vectors | `36fb0bc` | git object |
| Evidence stage (this document + gate machine audits + README) | this commit | git object |
| Toolchain | rustc/cargo 1.97.0 pinned by `rust-toolchain.toml`; all runs offline (`CARGO_NET_OFFLINE=true`, `--locked`) | repo pin + run output |
| Package surface | `contextmesh-salience` (workspace member, one-way path dependency on `contextmesh`); source under `contextmesh-salience/src/{error,json,types,outcome,verify,io}.rs`; tests under `contextmesh-salience/tests/{oc01_workspace,oc01_schema,oc01_crypto,oc01_dag,oc01_io,oc01_adversarial}.rs` + `tests/support/oc01_fixed_dag.rs` + `tests/fixtures/` | spec §11 file map |
| Golden fixture SHA-256 | terminal `4355c9e821d59ede0be3bf57ff04902d33b9011591b3b3bd667a756ca3f03978`; unterminated `0528eaac6307c606d45cb3661529d7d3bbcd7c0246fe7a2a79f9b22359505ae0` | reviewer-recomputed, equals committed bytes |
| Dependency closure | core reachable packages 320; registry closure SHA-256 `ae86da65ff5138bb51836d303ec9370ad9da8c8f112ad84ad59b5e362136113d`; feature tree SHA-256 `658b4fe016b1bc8ba748d31d88f61216df06afbd2c931059579bdff8375f461c`; zero new registry identities; zero forbidden capabilities | `python3 -I scripts/check-core-dependencies.py` output |
| Historical chain evidence | OA-07 `9c275f0f83b320d697dc9ccccc2b51ee60a05114`, OB-13 `1df53344afc29ac7730e373de1fb4a46def3a9f5`; bundle `oc-01-heavy-chain-bundle/bundle.txt` (13,043 lines, final SHA-256 `08edeb93271d01cab3dee9d45d6f7d9295ee10c2fc196b3858d636353bcf6836`) | preserved §5 + git objects |
| Operator approvals | Stage 2F commit approval Discord `1541795912106639380`; evidence-stage start Discord `1541800102979436596`; gate-amendment change-control approval Discord `1541805893144813651`; earlier stage approvals recorded in the preserved §5 record and stage transcripts | Discord records |

Reviewer commands (agent host, offline, `TMPDIR` redirected to a private
cache because the system temporary directory is capacity-constrained):

```bash
python3 -I scripts/check-core-dependencies.py
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
OC01_INNER_CURRENT_GATE=1 cargo test -p contextmesh-salience --locked
cargo test -p contextmesh --locked
OC01_INNER_CURRENT_GATE=1 cargo test --workspace --locked
bash scripts/verify-oc01.sh --self-test
bash scripts/verify-oc01.sh --planned-surface-only
bash scripts/verify-oc01.sh
sha256sum contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-*.json
git grep -E 'token1_[A-Za-z0-9_-]{43}'
```

`OC01_INNER_CURRENT_GATE=1` skips the cold-build historical chain tests inside
ordinary test invocations; the chains themselves are covered by the preserved
hash-bound bundle (§5) and by `verify-oc01.sh --historical-release-chains`
when a capable host is available. Source authority: git objects are
cryptographic; test/gate output lines are agent-host run output bound to the
recorded commands; Discord records are operator approvals.

## 2. Reasoning

- **Why gate-machine audits (R06–R10) are now executable checks:** the matrix
  names five exact checks (`evidence_sources_layer_is_complete`,
  `evidence_four_layers_and_gate_ids_are_complete`,
  `evidence_privacy_and_claim_language`, `claim_audit_is_limited_to_oc01`,
  `downstream_gate_requires_oc01_and_p1_preregistration`) that did not exist
  in `verify-oc01.sh`. Evidence accepted only as prose would make the
  OC01-EVIDENCE gate unexecutable, so the functions were added to the gate
  script under founder-approved change control (Discord `1541805893144813651`)
  and are invoked by `evidence_stage` before the workspace regression.
- **Why the immutable-surface check was narrowed:** the baseline check froze
  the entire `_bmad-output/verification-artifacts` directory against
  `0cf192b`, but the same matrix requires OC-01's own evidence
  (`oc-01-evidence.md`, `oc-01-heavy-chain-bundle/bundle.txt`) to be committed
  after that baseline — a structural self-contradiction noted in the preserved
  §5 record. The amendment excludes exactly those two OC-01-owned paths; every
  pre-baseline artifact (OA/OB/OC-00 evidence, all `verify-oa*.sh`,
  `verify-ob*.sh`) remains frozen. This restores executability without
  weakening any historical freeze.
- **Why the allowlist was extended:** the spec and matrix were amended at
  `0da83fe` (founder-approved D09 scope narrowing) and the bundle was committed
  at `9d363bd`, but those three paths were not on `allowed_path()`, so
  `--planned-surface-only` failed at every later HEAD. The extension lists
  them explicitly; future edits to spec/matrix remain change-controlled by §16
  (founder approval + coordinated spec/matrix/gate updates), which the evidence
  records here make auditable.
- **Why historical chains are covered by the hash-bound bundle instead of a
  fresh local run:** each chain needs a detached-worktree full build
  (60–120+ min, multi-GB rustc peak); the agent host previously hit
  memory-cgroup OOM. Script identity is proven cryptographically (bundle
  manifest hashes equal git blobs at the historical commits), and both chains
  show PASS inside the SHA-256-bound bundle (§5). Re-running would add no
  information and risks another OOM; a capable host may still re-run
  `--historical-release-chains` as an invalidator probe.
- **Why boundary arithmetic in I19/I22 is trusted:** an independent quality
  review verified `count_event_references` (local+remote heads, terminal,
  outcome evidence, attempt/dead-end refs, 1+evidence per mark, `checked_add`)
  against the I19 base-4 and I22 cap−2 vector shapes, and the passing tests
  themselves execute those boundaries.
- **Alternatives rejected:** accepting prose-only evidence (matrix requires
  machine-auditable named checks); weakening the baseline freeze wholesale
  (forbidden by W13/§16); re-running historical chains on the OOM-prone host
  (no information gain); adding a production instrumentation seam to prove
  issuance order (rejected at 2D in favor of static audit + boundary tests);
  promoting attribution marks to causal evidence (prohibited by §16).
- **Assumptions:** the P1 preregistration record is external to OC-01 and is
  neither defined nor approved here; Discord approval records are authoritative
  for operator intent; the delegation host bundle interior lines are
  operator-supplied but hash-bound.

## 3. Conclusion derivation

| Gate (matrix rows) | Observation (exact) | Conclusion (bounded) |
|---|---|---|
| OC01-SETUP (W01–W13) | `2edbe8d` workspace boundary; closure JSON: 320 packages, 0 new registry IDs, `salience_direct_dependencies_exact:true`, forbidden capabilities `[]`; `--self-test` → `ok: stages_execute_in_dependency_order`; bundle §5 W09–W13 rows | Workspace shape, one-way dependency, zero external additions, and sanctioned legacy-verifier migration hold |
| OC01-SCHEMA (I01–I24, P09–P11) | `cargo test --test oc01_schema` 20/20; golden/unterminated fixture bytes equal committed files (SHA-256 above); generator `#[ignore]`d | Every exact v1 shape/tag/order/bound rule and both canonical vectors pass |
| OC01-CRYPTO (I25–I26, P01–P06, P09–P10) | `cargo test --test oc01_crypto` 8 passed + 1 ignored; 2C compliance GO + quality APPROVE; tamper matrix rejects all 13 components; P06 precedence vectors return earliest category | Literal domains, typed encodings, author match, tamper rejection, and frozen precedence hold |
| OC01-DAG (P07–P08, D01–D10) | `cargo test --test oc01_dag` 12/12; 2D re-reviews GO after D03/D04 all-9-roles and D09 public-API narrowing; issue-failure matrix bounded by before/after snapshots | Referenced events verify same-context, admission supplies authorization, snapshots bind exactly, moved inputs return stale-input with no partial artifact |
| OC01-IO (X01–X05, X20) | `cargo test --test oc01_io` 5/5; 2E compliance GO + quality APPROVE (3 claim-honesty fixes applied: best-effort unlink wording, writer drop order, unverified parse-order comment removed) | Bounded regular-file import/export returns only complete verified artifacts and never writes Option A storage |
| OC01-ADVERSARIAL (I18–I26, P03–P04, X05–X20) | `cargo test --test oc01_adversarial` 27/27; 2F compliance GO (`deleg_5bab364f`) + quality APPROVE (`deleg_efaf6a2e`); I19/I22 arithmetic independently verified against `count_event_references`; X03/X04 alignment vectors added | Every maximum/+1, hostile JSON, ordering, privacy, stable-error, and no-partial-output vector passes |
| OC01-REGRESSION (W04–W13, R01–R05) | Full workspace regression exit 0 (20 suites, 0 failures, historical chains skipped only via the inner gate env var, chains covered by bundle §5); core tests `cargo test -p contextmesh --locked` pass; demos `demo.sh`/`demo-ob.sh` pass inside `workspace_stage`; closure JSON unchanged (320/0/0) | Option A/B wires, fixtures, store schema, forbidden surfaces, and demos are unchanged by all OC-01 work |
| OC01-EVIDENCE (R06–R10) | This document + `verify-oc01.sh` machine audits: `ok: evidence_sources_layer_is_complete`, `ok: evidence_four_layers_and_gate_ids_are_complete`, `ok: evidence_privacy_and_claim_language`, `ok: claim_audit_is_limited_to_oc01`, `ok: downstream_gate_requires_oc01_and_p1_preregistration` (AND truth table 0/0/0, 1/0/0, 0/1/0, 1/1/1); token scan clean | Four-layer evidence and claim audit are executable and verdict-limited to artifact integrity/provenance; OC-02 stays blocked without P1 |

Row-level name mapping note (honest deviation): R01–R05 name descriptive
roll-ups (`workspace_build_lint_and_test_matrix`,
`historical_release_verifier_chains_pass_unchanged`,
`current_workspace_full_regression`, `final_dependency_closure_recheck`,
`gate_is_offline_nonrecording_and_clean`) that are not literal function/test
identifiers; their substance is executed by `workspace_stage`,
`--historical-release-chains`/the two `oc01_workspace.rs` chain tests, the
closure preflight, and the gate's offline/non-recording design respectively.
R02's substance is additionally bound by the preserved bundle. A future
amendment may add literal wrappers; no check result depends on that renaming.

Per-stage independent review verdicts (double-blind, review-only):

| Stage | Compliance | Quality |
|---|---|---|
| 1A | GO | APPROVE (blocker: predictable nonce → fixed, re-verified APPROVE) |
| 1B | GO (bundle acceptance) | — (acceptance record §5) |
| 2A | GO | APPROVE |
| 2B | GO | APPROVE (0 blockers, 0 warnings) |
| 2C | GO | APPROVE (1 fmt warning, fixed) |
| 2D | NO-GO → migration approved → GO | REJECT → fixes → GO/APPROVE |
| 2E | GO (2 non-blocking flags → resolved by 2F vectors) | APPROVE (5 warnings: 3 fixed, 2 accepted: std-only TOCTOU, self-referential category array) |
| 2F | GO (`deleg_5bab364f`) | APPROVE (`deleg_efaf6a2e`) |

## 4. Invalidators

Any of the following would reverse, pause, or narrow these conclusions:

- Any `scripts/verify-oa*.sh` / `verify-ob*.sh` byte differs from its bundle
  manifest hash or historical blob (script drift).
- Golden/unterminated fixture bytes differ from the SHA-256 values above
  without a founder-approved change-control record (vector drift).
- `Cargo.lock`/workspace identity drift changing the closure (320), adding a
  registry identity, or altering the feature-tree hash (dependency drift).
- A failing re-run of any named command, including
  `verify-oc01.sh --historical-release-chains` on a capable host (stale
  evidence).
- Tampering with the preserved bundle (its interior `bundle_sha256_pending`
  no longer equal to SHA-256 of lines 1–13016).
- An unauthorized edit to the spec/matrix outside §16 change control — the
  allowlist extension makes such edits visible to the gate only as
  planned-surface changes, so the §16 review trail is the controlling record.
- Any claim drawn from this file beyond OC-01 artifact integrity/provenance
  (overreach invalidates the evidence standard itself).
- The P1 preregistration record failing or diverging from the priority plan
  narrows only the OC-02 unblocking conclusion, not the OC-01 results.

## 5. Preserved Stage 1B interim record (historical)

The following is the interim Stage 1B acceptance evidence committed at
`67a27be`, preserved verbatim for audit continuity. Statements below about
pending stages and about `--planned-surface-only` behavior at later HEADs
were true when written and are superseded by the final record above (the
gate amendment described in §2/§6 restored executability).

> # OC-01 Stage 1B Historical-Chain Acceptance Evidence (Interim)
>
> **Status:** Stage 1A (workspace boundary) and Stage 1B (heavy historical
> chains) accepted. Implementation Stages 2A–2E (primitives, schema,
> protocol, DAG, I/O, vectors) and the full 8-stage gate are **pending**.
> This is the implementation-time four-layer record for the Stage 1B
> acceptance only; it asserts nothing beyond the artifact integrity and
> provenance observations listed below.
>
> ## 1. Sources
>
> | Item | Value | Authority |
> |---|---|---|
> | Baseline (spec/matrix freeze) | `0cf192b625384283d10c008d4a0e984ae9d0be08` | git object |
> | Stage 1A completion | `2edbe8d86dea645496dde44db7e3a2e4f6cc404e` | git object |
> | Delegation kit | `5ef6be91246e83cad754a2639cd61380accf7dce` | git object |
> | Allowlist fix | `59fd165bc850187d3983b4ccda7ddb90f180e81e` | git object |
> | Evidence bundle commit | `9d363bd6a10ef29aa1a8f7af0d9e532d7ba625ab` | git object |
> | OA-07 historical completion | `9c275f0f83b320d697dc9ccccc2b51ee60a05114` | git object |
> | OB-13 historical completion | `1df53344afc29ac7730e373de1fb4a46def3a9f5` | git object |
> | Bundle | `oc-01-heavy-chain-bundle/bundle.txt` (13,043 lines) | committed at `9d363bd` |
> | Bundle final SHA-256 | `08edeb93271d01cab3dee9d45d6f7d9295ee10c2fc196b3858d636353bcf6836` | reviewer-recomputed |
> | Bundle pre-manifest SHA-256 | `77f9d9ceae49168d0cc051689c4fa05e4f6fac17fc7e20e3c636c6787bb7c8c6` (recorded as `bundle_sha256_pending` at bundle line 13043; the hash covers lines 1–13016) | reviewer-recomputed over lines 1–13016 |
> | Toolchain | rustc/cargo 1.97.0 pinned by the repo's `rust-toolchain.toml`; run performed offline per the kit contract | bundle line 2 + `rust-toolchain.toml` |
> | Execution host | delegation host (capable machine); acceptance review on the agent host | — |
> | Operator approvals | bundle submission Discord `1541063227675508817`; evidence-write instruction Discord `1541064884693893161`; matrix approval Discord `1540352346364842105` | Discord records |
>
> Reviewer commands (agent host, offline): `sha256sum` over the bundle and each
> `scripts/verify-o*.sh`; `git show <commit>:<path> | sha256sum`; `git diff
> --stat 59fd165..9d363bd`; `bash scripts/verify-oc01.sh --self-test`;
> `git grep -E 'token1_[A-Za-z0-9_-]{43}'`.
>
> ## 2. Reasoning
>
> - **Why delegation:** the OA-07/OB-13 chains each require a detached-worktree
>   full build (60–120+ min, multi-GB rustc peak). The agent host previously
>   hit memory-cgroup OOM at rustc peak, so the frozen chains were executed on
>   the delegation host via the committed kit (`5ef6be9`), exactly as designed.
> - **Why acceptance by bundle instead of re-running the strict gate at HEAD:**
>   `verify-oc01.sh::planned_surface_only` freezes
>   `_bmad-output/verification-artifacts` against baseline `0cf192b`. Once the
>   bundle (and later this file) is committed, `git diff <baseline> --
>   _bmad-output/verification-artifacts` is non-empty by construction, so the
>   strict gate is not re-runnable at such HEADs. This is the accepted
>   convention: the gate runs pre-commit on the working tree; afterwards
>   acceptance rests on recorded hashes plus PASS lines inside the hash-bound
>   bundle. Re-running the 90+ minute chains on the OOM-limited agent host
>   would add no information — script identity is already proven
>   cryptographically.
> - **Why the allowlist fix (`59fd165`) is legitimate:** the kit commit
>   `5ef6be9` added `scripts/run-oc01-historical-chains.sh` and the delegation
>   task doc but omitted both from its own allowlist, making
>   `--planned-surface-only` deterministically unpassable at that HEAD. The fix
>   adds exactly those two paths to `allowed_path()`; no frozen surface
>   changed — all 22 OA/OB verifier scripts remain byte-identical.
> - **Alternatives rejected:** weakening the baseline freeze (forbidden by W13
>   and change control); treating runs #1/#2 partial bundles as acceptance;
>   re-running chains on the agent host (OOM risk, redundant with hash
>   identity).
>
> ## 3. Conclusion derivation
>
> | Requirement | Observation (exact) | Conclusion (bounded) |
> |---|---|---|
> | OC01-W09 (OA chain at `9c275f0`) | bundle L492 `[oc01-heavy] chain oa07 PASS`; `scripts/verify-oa07.sh` SHA-256 equals both bundle manifest entry and the git blob at `9c275f0` | The unchanged OA-07 completion chain passed offline in a detached clean worktree on the delegation host |
> | OC01-W10 (OB chain at `1df5334`) | bundle L12739 `[oc01-heavy] chain ob13 PASS`; `scripts/verify-ob13.sh` SHA-256 equals both manifest entry and the git blob at `1df5334` | The unchanged OB-13 completion chain passed offline in a detached clean worktree on the delegation host |
> | OC01-W11 (package-scoped current checks) | bundle tail `ok: planned_surface_only`; `current_workspace_checks_are_package_scoped_and_legacy_scripts_immutable … ok`; bundle head `ok:` lines for clean tree, pinned toolchain, exact pins/closure 320, OA-00..06 checkpoints (155), rustfmt | Current-tree checks passed package-scoped at run HEAD `59fd165` without invoking obsolete historical HEAD assertions |
> | OC01-W12 (stage order) | `bash scripts/verify-oc01.sh --self-test` → `ok: stages_execute_in_dependency_order` | The 8 stages execute fail-fast in dependency order and stop after an injected failure |
> | OC01-W13 (planned boundary) | manifest re-hash: 22/22 identical, 0 mismatch; `git diff --stat 59fd165..9d363bd` touches only `bundle.txt`; runner SHA-256 equals the `5ef6be9` blob | The bundle commit changed no frozen surface; the committed boundary matches the approved OC-01 allowlist |
> | OC01-R08 (privacy, partial) | reviewer `git grep` token-pattern scan clean; bundle contains zero FAIL verdicts, zero panic events, and zero `^error` lines | No secret-token pattern or failure verdict is present in the recorded artifacts reviewed |
> | OC01-R09 (claim limits) | conclusions above are limited to chain execution, script identity, and boundary integrity | No C2–C5, OC-02, completion, cost, or causal-attribution claim is made |
>
> ## 4. Invalidators
>
> - Any `scripts/verify-oa*.sh` / `verify-ob*.sh` byte differs from the bundle
>   manifest hash or from its historical completion-commit blob.
> - A FAIL verdict, panic event, or `^error` line inside the bundle, or
>   `bundle_sha256_pending` no longer equal to the SHA-256 of bundle lines
>   1–13016.
> - Closure drift changing 320 or adding a registry identity.
> - A later re-run of either chain on any capable host failing at the same
>   commits.
> - Any claim drawn beyond Stage 1A/1B artifact integrity.
> - Structural note (superseded by the final record's gate amendment): at
>   HEADs ≥ `9d363bd` the strict `--planned-surface-only` check failed on the
>   artifacts-directory baseline freeze by design; acceptance was the recorded
>   hashes and PASS lines above.
>
> ## 5. Deviations and operator declarations (caller-supplied)
>
> - Free disk during run #3 was ~20 GB versus the kit's stated ≥25 GB
>   (declaration, not independently re-verified).
> - Runs #1/#2 produced partial bundles (chains passed 2-for-2 in each;
>   current checks failed on the pre-fix allowlist); partial outputs are
>   preserved on the delegation host (declaration).
> - Runner argument-handling quirk: a stray space silently redirects the
>   bundle output path; hardening recommended for any future kit; the frozen
>   runner was not modified (declaration).
> - The allowlist fix `59fd165` was applied on the delegation host per
>   operator decision, then produced the accepted run #3 (declaration).

## 6. Deviations and change control (final record)

- **Gate amendment (this commit), founder-approved Discord
  `1541805893144813651`:** added matrix-named machine audits R06–R10 to
  `verify-oc01.sh`; extended `allowed_path()` with the spec, matrix, and
  bundle paths; narrowed the immutable check to exclude exactly OC-01's own
  two evidence outputs. Rationale and rejected alternatives in §2. The
  spec/matrix texts themselves are unchanged by this commit.
- **R01–R05 row-name mapping:** descriptive roll-up names map to executing
  checks as noted in §3; no result depends on a rename.
- **Historical chains:** covered by the preserved hash-bound bundle and the
  two `oc01_workspace.rs` chain tests (skipped in ordinary runs via the inner
  gate env var; directly runnable via `--historical-release-chains`).
- **Caller declarations vs verified facts:** bundle interior lines, delegation
  host disk levels, and partial-run preservation are caller-supplied
  declarations (§5); commit identities, fixture hashes, closure JSON, test and
  gate outputs cited in §3 are independently recomputed facts.

## 7. Non-claims

This record does not claim OC-01 usefulness, outcome quality, cost accuracy,
causal attribution (C2), prior grounding (C3), selection utility (C4),
comprehension (C5), or any Option C capability beyond the OC-01 package's
artifact integrity and provenance recording. Attribution marks remain
caller-supplied candidates, never causal evidence. No secret keys,
credentials, raw transcripts, chain-of-thought, or private host paths are
contained here. OC-02 implementation and test-label inspection remain blocked
until both the OC-01 gate and the separate P1 preregistration-hash gate pass.
