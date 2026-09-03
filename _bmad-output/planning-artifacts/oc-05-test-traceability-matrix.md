# OC-05 Test Traceability Matrix — FROZEN v29

Convention: one RULE = one executable assertion; names verbatim; evidence
cells describe the actual assertion. Rows: R00, R01, R02a, R02b, R03–R17 =
**19 rows**.

## Rows

| ID | Requirement | Script/checkpoint or procedure | Location | Evidence |
|---|---|---|---|---|
| OC05-R00 | Trusted wrapper pre-execution checks (PINS → BIND → LAUNCH order; unset-safe; guarded fallible steps; markers-only failures) | frozen §10 wrapper (spec §3.2 invariants i–iv) | spec §10 | unset pins → `OC05-PINS: FAIL` exit 1 (via `${VAR-}` — no `set -u`); `abc`/unknown 40-hex → same; unset/mismatched EXPECTED → `OC05-BIND: FAIL`; workdir/env failure → `OC05-WRAP: FAIL`; extraction/digest/mktemp failure → `OC05-LAUNCH: FAIL` with silent trap cleanup. §3.2(ii) is a GUARANTEE-level invariant — every fallible step in §10 steps 0–3 (excluding the silent trap cleanup) terminates with a frozen marker + exit 1, step 4 outside §3.2 scope — realized per the §3.2(ii) content-anchored inventory — each anchor a UNIQUE COMMAND PREFIX PATTERN (literal leading substring of exactly one §10 line, NOT line numbers and NOT full-line verbatim claims): stderr-suppressed guards (a), PINS conjunction single-`\|\|` deviation with `>/dev/null 2>&1` (b), bare `\|\|` on quiet builtin tests (c), markerless silent trap cleanup (d), step 4 outside §3.2 scope (e); each failure path emits exactly one frozen marker, no raw diagnostics |
| OC05-R01 | Clean tree + detached HEAD | verify-oc05.sh OC05-01 | scripts/verify-oc05.sh | `git status --porcelain` empty AND `git symbolic-ref -q HEAD` exits non-zero |
| OC05-R02a | Executor consistency (HEAD gate digest vs pins) | verify-oc05.sh OC05-02a | scripts/verify-oc05.sh | sha256(HEAD `scripts/verify-oc05.sh`) == `OC05_SCRIPT_SHA256` from `git show $OC05_PINS_COMMIT:<spec>` |
| OC05-R02b | Strict-linear chain + E..HEAD purity | verify-oc05.sh OC05-02b | scripts/verify-oc05.sh | (a) `merge-base --is-ancestor E HEAD`; (b) `git rev-list --parents -n 1 E` == exactly `[E, F]`; (c) `rev-list --merges E..HEAD` EMPTY; (d) per-commit changed-path union over E..HEAD within `_bmad-output/verification-artifacts/` or `_bmad-output/planning-artifacts/` (component-wise, full repo-relative) |
| OC05-R03 | Frozen manifest hash literals | verify-oc05.sh OC05-03 | scripts/verify-oc05.sh | root+salience Cargo.toml / Cargo.lock sha256 == OC-04 X06 literals |
| OC05-R04 | Dependency closure metadata graph | verify-oc05.sh OC05-04 | scripts/verify-oc05.sh | root NOT→salience; salience→root; package-ID set == claim-audit snapshot |
| OC05-R05 | Fixtures-only manifest + real sidecar digests | verify-oc05.sh OC05-05 | scripts/verify-oc05.sh | regenerated manifest == committed; 3 sidecar bare-digest equalities (trailing newline tolerated) |
| OC05-R06 | Owner pins via whitespace-normalized matching | verify-oc05.sh OC05-06 | scripts/verify-oc05.sh | normalized substrings (OC-01/02/03/04 per spec §6) |
| OC05-R07 | Golden verification via production verifiers | verify-oc05.sh OC05-07 | scripts/verify-oc05.sh | frozen command all GREEN |
| OC05-R08 | Full workspace regression incl. B8 | verify-oc05.sh OC05-08 | scripts/verify-oc05.sh | frozen command; temp log; marker `REGRESSION_EXIT:0` |
| OC05-R09 | Bounded privacy scan fail-closed | verify-oc05.sh OC05-09 | scripts/verify-oc05.sh | §7 categories; zero hits |
| OC05-R10 | OC-00 evidence integrity | verify-oc05.sh OC05-10 | scripts/verify-oc05.sh | sha256 == frozen digests |
| OC05-R11 | Claim audit over frozen inventory + freeze identity | verify-oc05.sh OC05-11 | scripts/verify-oc05.sh | covers §7 list; zero pending/TODO/TBD; §5 freeze identity asserted: gate output line `OC05_SCRIPT_ID=oc-05-release-gate-v1` matches the §5 value verbatim |
| OC05-R12 | P3-GO OPEN honestly recorded | verify-oc05.sh OC05-12 | scripts/verify-oc05.sh | synthetic-label citation present |
| OC05-R13 | Tamper rejected at multiple gates | 5D probe (7 variants, /tmp clone): (i) fixture tamper C → OC05-02b FAIL; (ii) C + evidence D → still FAIL; (iii) complete HEAD-script replacement → OC05-02a FAIL; (iv) `OC05_ONLY=OC05-05` tampered fixture → OC05-05 FAIL; (v) merge in E..HEAD → OC05-02b(c) FAIL; (vi) sibling branch → OC05-02b FAIL; (vii) E′ substitution (valid linear E′ ≠ founder E) → `OC05-BIND: FAIL` | /tmp clone probe | each prints designated FAIL, exit 1 |
| OC05-R14 | Deterministic, non-recording, PASSING runs | 5D probe: /tmp clone → detach at clean E-descendant → two consecutive F-blob runs | /tmp clone probe | both EXIT 0; every checkpoint PASS once; §7-normalized transcripts identical; porcelain unchanged |
| OC05-R15 | Selector grammar + bootstrap negatives | 5D probe: selector negatives (`""`, `OC05-99`, multi, comma) + positives (`OC05-02a`, `OC05-02b`); bootstrap negatives (unset pins, `abc`, unknown 40-hex) | /tmp clone probe | selector negatives → `OC05-ONLY: FAIL` exit 1; positives → single checkpoint runs; bootstrap negatives → `OC05-PINS: FAIL` exit 1 (never silent exit 0) |
| OC05-R16 | Claim completeness + overclaim hunt | 5D delegated review over §7 resolved inventory | delegated gpt-5.6-sol | classifications complete; contradictions resolved; verdict committed |
| OC05-R17 | Founder-record binding BEFORE any F-derived code runs | §10 wrapper step 2 (spec §3.2) | spec §10 (trusted inline wrapper) | `EXPECTED="${OC05_EXPECTED_E-}"` and `PINS="${OC05_PINS_COMMIT-}"` are assigned verbatim from those env vars (§10 preamble), then byte-compared by `[ "$EXPECTED" = "$PINS" ]` ELSE `OC05-BIND: FAIL (E mismatch vs founder record)` exit 1 — executes before `rev-parse E^`/extraction, so a valid linear E′ with malicious F′ launcher is rejected before any attacker code exists; then F-blob digest verified in wrapper step 3 before `bash` |

Row count: **19** (R00, R01, R02a, R02b, R03–R17).

Non-matrix governance notes (not gate rows, founder-process obligations):
- **Prereg identity freeze**: the gate asserts via OC05-10's digest-pinning
  mechanism at the artifact level and OC05-11 claims the frozen inventory;
  the 4 `non_claims` of `P1-PREREG-SALIENCE-EVAL-V1` v1 are binding on
  R16's claim audit — R16 must classify every release claim against the
  four verbatim texts held in `_bmad-output/implementation-artifacts/p1-prereg-config.json`
  (`non_claims` array, the single authoritative copy; this note cites it
  by path rather than re-quoting, so drift is impossible) and reject any
  claim outside the measured regimes.
- **§12 P5-GO disposition**: Option 1/Option 2 selection is a founder
  process decision recorded OUTSIDE the repo (like E's object id); it is
  deliberately not a gate row — the gate records the CHOSEN disposition
  honestly via OC05-12's disposition-record mechanism, but cannot
  self-select it. Silence = Option 2 (`DEFERRED`).
