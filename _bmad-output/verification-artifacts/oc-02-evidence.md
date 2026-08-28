# OC-02 Implementation Evidence

**Status:** Final four-layer record for the OC-02 implementation stages. Nine
acceptance gates (OC02-SETUP, OC02-SCHEMA, OC02-MECHANISMS, OC02-SHORTLIST,
OC02-JUDGES, OC02-REPORTS, OC02-ADVERSARIAL, OC02-REGRESSION, OC02-EVIDENCE)
have passing machine evidence at the commits and commands recorded below. The
verdict is limited to attribution-mechanism correctness and artifact
integrity/provenance recording; it asserts nothing about measured retrieval
utility — that is exclusively the OC02-EVALUATION gate (V01–V05 + E1 rerun),
which remains open and is the C2 completion prerequisite (D-C-06 #4).

## 1. Sources

| Item | Value | Authority |
|---|---|---|
| P1 preregistration config | `c080722` (SHA-256 `be20d8fc…eae784c9`, blob `cd51a188…`) | git object + sha256sum |
| P1 preregistration freeze | `65f7e4f` (`p1-prereg-record.md`, sealing values + coverage map) | git object |
| Spec + matrix freeze | `e28cacb` (tree `dc094497ec4f`; founder approval Discord `1541934459069145168`, recorded in spec header) | git object |
| Stage 2A tags/config | `c0570b6` (tree `52c5e8f41e9a`) | git object |
| Stage 2B M0 overlap core | `109470e` (tree `63077706bf98`) | git object |
| Stage 2C M1 normalized core | `c8086f9` (tree `85bbf026f99d`) | git object |
| Stage 2D M2 structural extractor | `474c9bd` (tree `5db054301175`) | git object |
| Stage 2E deterministic shortlist | `563b86e` (tree `4ff8e0c7f92e`; freeze clarification Discord `1542499082533343264`) | git object |
| Stage 2F M3 adapter | `c768c8c` (tree `cbf2b199ade0`; freeze clarification Discord `1542525263240364093`) | git object |
| Stage 2G M4 coalition adapter | `1b25907` (tree `a41de5fb98fa`; founder freeze Discord `1542555983090557129`) | git object |
| Stage 2H report assembly | `8f663e1` (tree `9515aef95a49`; approval Discord `1542724072700387388`) | git object |
| Stage 2I adversarial vectors | `eba598e` (tree `b50653b59eeb`; approval Discord `1542759823211364383`) | git object |
| Evidence stage | this commit | git object |
| Toolchain | rustc/cargo pinned by `rust-toolchain.toml`; all runs offline (`CARGO_NET_OFFLINE=true`, `--locked`, `OC01_INNER_CURRENT_GATE=1`) | repo pin + run output |
| Package surface | `contextmesh-salience`: `src/{attribution,judge,attribution_report}.rs` added; `tests/{oc02_schema,oc02_mechanisms,oc02_shortlist_judges,oc02_reports,oc02_adversarial}.rs` + `tests/fixtures/oc02-attribution-report-v1-golden.{json,sha256}`; zero new dependencies | spec §10 file map + Cargo.lock diff |

## 2. Reasoning (gate justifications)

Each stage followed TDD (matrix rows as tests → RED → implementation → GREEN)
plus dual independent review; every commit required founder approval.

| Gate | Evidence basis | Rejected alternatives |
|---|---|---|
| OC02-SETUP | Stage 1A + per-stage `git status` checks: OC-02 work confined to `contextmesh-salience`, zero new deps (Cargo.lock diff reviewed each stage) | Separate crate rejected — one-way path dependency preserved |
| OC02-SCHEMA | `oc02_schema` 5/5 at `c0570b6` (T01–T04 + T06 half; matrix amendment log records the re-scope of T05–T10 to mechanisms/reports suites, covered by the 2A dual review) | In-module unit tests rejected — matrix required `tests/oc02_schema.rs` |
| OC02-MECHANISMS | `oc02_mechanisms` 17/17 at `474c9bd` (A01–A17; A18–A26 re-scoped to later suites per matrix amendment) | Direct subagent implementation in 2G rejected after 2 API timeouts — parent did TDD directly |
| OC02-SHORTLIST | `oc02_shortlist_judges` 22/22 at `563b86e`→`1b25907` (S01–S08 + J-rows) | Per-candidate share normalization rejected — violated J10 invariant; section-wide normalization chosen |
| OC02-JUDGES | J01–J14 in the same suite (M3 8 + M4 5 + boundary rows) | Network-calling judge rejected — trait-only isolation frozen at spec |
| OC02-REPORTS | `oc02_reports` 8/8 + golden generator at `8f663e1`; fixture SHA-256 verified via system `sha256sum` | BLAKE3-labeled-as-SHA256 rejected (Quality blocker, fixed) |
| OC02-ADVERSARIAL | `oc02_adversarial` 10/10 at `eba598e`; canonical-only verify gate added in `parse_report_bytes` (X10) | BTreeMap re-insertion key-reorder test rejected as vacuous (Compliance blocker — raw byte splice used) |
| OC02-REGRESSION | Full workspace `cargo test --workspace --locked` EXIT 0 at each stage close; final 365 passed / 0 failed | — |
| OC02-EVIDENCE | This document + claim audit below | — |

Notable dual-review history (all resolved before commit): 2E compliance
NO-GO→freeze clarification; 2H compliance NO-GO (M4 provenance, §9.4
transcript) + quality REJECT (verify circularity) → `verify_report`
transcript-replay redesign; 2I quality APPROVE w/7 warnings (5 fixed in code)
and compliance NO-GO (X10 vacuous branch) → re-check GO 0/0. Verdicts are
recorded in the delegation transcripts under
`~/.hermes/cache/delegation/live/` and in this chat's history; they are not
independently reproducible from the repo alone — the repo-verifiable residue
is the code changes each verdict gated (e.g., `split_top_level_members` in
`tests/oc02_adversarial.rs` closing X10).

## 3. Conclusion derivation

The nine gates above close in dependency order: schema constants must exist
before mechanisms; mechanisms before shortlist; shortlist before judges;
both adapters before report assembly; assembly before adversarial vectors
could exercise the canonical-only verify gate. The final state is: every
matrix row T01–T10(half), A01–A17, S01–S08, J01–J14, R01–R08, X01–X10 has a
1:1 named test, all green, with workspace regression 365/0. Therefore OC-02
implementation is complete through the ADVERSARIAL and EVIDENCE gates, with
attribution provenance (C2's OC-02 portion) implemented and
integrity-verified. The claim stops at the EVALUATION gate boundary: V01–V05
and the E1 deterministic rerun are not executed here and C2 completion is
explicitly not asserted.

## 4. Invalidators

This evidence record is invalid if any of the following becomes true:

1. Any OC-02 suite reports a failure or flake on a clean checkout of
   `eba598e` with the recorded toolchain and env vars.
2. A mechanism is found that mutates ledger state or reads outside the
   caller-supplied universe (violating the one-way nomination direction).
3. The golden fixture's SHA-256 stops matching the committed `.sha256` file.
4. `verify_report` is shown to accept non-canonical bytes (whitespace,
   reordered keys) on any input.
5. A judge implementation can produce a `Computed` causal status without
   carrying §9.3 judge provenance in every m3/m4 record.
6. A matrix row is found whose named test does not exist or does not assert
   the row's claim (claim-audit failure).
7. The E1 rerun (when executed) diverges from the recorded V-row expectations
   under identical replay inputs.

## 5. Reviewer commands

A reviewer can re-derive the machine evidence for the nine gates above on a
clean checkout of the cited commits:

```sh
# All five OC-02 suites (SCHEMA+MECHANISMS+SHORTLIST/JUDGES+REPORTS+ADVERSARIAL):
source ~/.cargo/env
OC01_INNER_CURRENT_GATE=1 TMPDIR=${TMPDIR:-/tmp} CARGO_NET_OFFLINE=true \
  cargo test -p contextmesh-salience --locked
# Full workspace regression (REGRESSION gate):
OC01_INNER_CURRENT_GATE=1 TMPDIR=${TMPDIR:-/tmp} CARGO_NET_OFFLINE=true \
  cargo test --workspace --locked
# Golden fixture integrity (REPORTS gate, R07):
sha256sum contextmesh-salience/tests/fixtures/oc02-attribution-report-v1-golden.json \
  && cat contextmesh-salience/tests/fixtures/oc02-attribution-report-v1-golden.sha256
# Commit chain integrity (Sources):
git log --oneline e28cacb..eba598e
```

Test counts asserted per gate: schema 5, mechanisms 17, shortlist+judges 22,
reports 8 (+1 ignored golden generator), adversarial 10, workspace total 365
passed / 0 failed at `eba598e`. Those counts are this document's recorded run
output; a reviewer recomputes them with the commands above rather than
trusting the record.

## 6. Change control

| Approval | Discord message ID | Scope |
|---|---|---|
| 2E minimal freeze (binary score + S04 timing) | `1542499082533343264` | Score = exactly 1,000,000 ppm; Stage 2E emits marker, 2H serializes |
| 2F minimal freeze (contract clarification) | `1542525263240364093` | M3 adapter ownership of typed partials |
| 2G founder freeze (5 items) | `1542555983090557129` | Coalition types, share normalization, schedule, cap marker, partial-section ownership |
| 2H commit approval | `1542724072700387388` | Attribution report assembly + verification |
| 2I commit approval | `1542759823211364383` | Adversarial boundary + privacy vectors |
| 2J evidence-stage approval | pending (this approval) | This document — self-referential until this commit is approved; the Discord ID will be quoted in the completion report |

Open change-control items (§6, non-blocking, recorded for honesty):
1. `report_id` is derived over placeholder-normalized bytes
   ("report_id" literal in the ID position) rather than the §9.2 wording
   "canonical full report bytes" — a spec-wording deviation accepted at 2H;
   either a spec amendment or implementation change is owed before OC-03.
2. R02 matrix note references fixture comparison that actually lives in R07
   (wording only).

## 7. Claims and non-claims

**Claims:** attribution mechanisms M0/M1/M2, shortlist policy, M3/M4 adapters,
report assembly/verification, and adversarial/privacy vectors are implemented,
deterministic, fail-closed, and dual-reviewed; all named matrix rows pass;
regression is clean.

**Non-claims:** no measured attribution quality (nDCG@12 or otherwise); no
live-judge behavior; no claim that the system improves retrieval; no C2
completion. Evaluation (V01–V05 + E1) remains the sole path to those claims.
