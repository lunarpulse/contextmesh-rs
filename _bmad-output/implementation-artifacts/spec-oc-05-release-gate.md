# OC-05 Release and Claim Gate — Specification (FROZEN v29)

Status: FROZEN v29 — approved 2026-09-03 (Discord approval, message
`1544867811812184064`). §12 founder disposition: **Option 2 — P5-GO
recorded `DEFERRED`** until OC-04 4G + real gold exist; no conditional
release record. Freeze basis: dual review round 21 Compliance GO +
Quality APPROVE (deleg_a2d607d4); preflight angle 1 real-time execution
(6 paths) PASS; preflight angles 2–4 defect closure through re-checks
#1–#7 (deleg_92ea9eba, deleg_b83159f8, deleg_4330d0f7, deleg_9b769737,
deleg_59861f98, deleg_39bfd2a5, deleg_74d1d9fd) plus founder-verified
R13(v) live merge construction (MERGE_OK, E..HEAD merge non-empty,
porcelain empty, detached HEAD).

## 1. Intent, scope, and non-claims

Execute C5 (option-c §C5) over the recorded evidence surface. No new product
artifact; writable paths `scripts/`, `_bmad-output/verification-artifacts/`,
`_bmad-output/planning-artifacts/`; prereg `non_claims` verbatim; P3-GO
OPEN; Thorn disabled; "privacy" = §7 scan only.

## 2. Upstream frozen surface

priority plan §P5 L166-173 (P5-GO → §12); option-c §C5; p1-prereg-config.json
v1; spec-oa-07; OC-01..04 evidence/docs.

## 3. Lifecycle: TRUSTED E-INDEPENDENT WRAPPER (precise invariants)

**3.1 Chain.** `F` (executor commit: `scripts/verify-oc05.sh` gate only) →
`E` (pin commit, F's sole child, founder-approved spec freeze recording
`OC05_SCRIPT_SHA256` + §5 freezes). E's object id lives in the founder
approval record outside the repo.

OC05_SCRIPT_SHA256=0ef7a7f38fe3c4e65afaa42c4a2c21e3bd1bedfb7eb72a009e5d483698c9e9b4

**3.2 Wrapper invariants (worded to match the implementation exactly):**
i. **Unset-safe expansion**: no `set -u`; pins/expected read via `${VAR-}`.
   Unset → empty → format guard fails → frozen marker.
ii. **Guaranteed-guard steps (inventory, with declared deviations)**:
    every fallible step in §10 steps 0–3 (excluding the silent trap
    cleanup, (d)) terminates the wrapper with a frozen marker and exit 1;
    step 4 is outside §3.2 scope ((e)). The §3.2(i)–(iv) invariants are
    GUARANTEE-level, not syntax-template claims; §10 realizes them
    step-by-step as follows. Anchors are CONTENT anchors — deliberately
    NOT line numbers, which shift with any spec edit. Each anchor is a
    UNIQUE COMMAND PREFIX PATTERN: the literal leading substring of
    exactly one §10 line (matched up to but not including the first
    whitespace-adjacent divergence); prefixes are stated verbatim, the
    remainder of each line is NOT part of the anchor:
    (a) stderr-suppressed guards `2>/dev/null \|\| { marker; exit 1; }`:
        lines with prefixes `cd "${OC05_WORKDIR:-$PWD}"`,
        `source ~/.cargo/env`, `F=$(git rev-parse "$PINS^"`,
        `tmp=$(mktemp`, `git show "$F":scripts/verify-oc05.sh`,
        `want=$(git show "$PINS":` (the pins-read pipeline, via
        `set -o pipefail`), and `have=$(sha256sum "$tmp"` (the digest
        pipeline, via pipefail).
    (b) PINS conjunction (step 1): the line with prefix
        `[ "${#PINS}" -eq 40 ] && [[ "$PINS"` (both quiet builtin tests in
        one continued line), the continuation line with prefix
        `&& git rev-parse --verify` (leading indentation stripped before
        matching), and the continuation line with prefix
        `|| { echo "OC05-PINS: FAIL"` (leading indentation stripped)
        together share ONE trailing `|| { echo "OC05-PINS: FAIL";
        exit 1; }` clause — the single deviation from the
        `2>/dev/null` form; git output is fully discarded via
        `>/dev/null 2>&1`. Prefix matching for continuation lines ignores
        LEADING WHITESPACE only; backslash-continuation characters are
        stripped before matching, as bash itself does at parse time.
    (c) bare `\|\| { marker; exit 1; }` on inherently quiet builtin tests:
        the BIND check (step 2, prefix `[ "$EXPECTED"`), the extraction
        test (prefix `[ -s "$tmp" ]`), and the digest comparison (prefix
        `[ "$have"`).
    (d) the line with prefix `trap 'rm -f "$tmp" 2>/dev/null' EXIT`: the
        REGISTRATION is an infallible builtin; the action suppresses its
        own stderr and deliberately carries NO marker — a failed temp-file
        cleanup must stay silent so it can never mask or follow the
        primary failure marker.
    (e) step 4, the line `bash "$tmp"; rc=$?; exit $rc` (given in full),
        is DELIBERATELY UNGUARDED and outside wrapper invariants: the
        gate's own checkpoint output and exit code are the contract
        (§3.3); §3.2 scope ends at the gate boundary.
    the line with prefix `set -o pipefail` is an infallible builtin,
    exempt.
iii. **Markers-only transcript (guarantee scope)**: for every wrapper
     failure path through steps 0–3, the transcript contains exactly ONE
     frozen marker and exit 1, with no raw diagnostic (all fallible
     external commands suppress their own stderr; the PINS git discards
     stdout+stderr; the trap cleanup is silent by (ii)(d)). Step 4 output
     is the gate's own checkpoint lines (outside this scope).
iv. **Trust ordering** (PINS → BIND → LAUNCH → execute): pins format check
    before ANY blob access; byte-compare `OC05_EXPECTED_E` ==
    `OC05_PINS_COMMIT` before `rev-parse E^`; F derived, extracted,
    digest-verified, then executed. Trust boundary: whole-commit founder
    review of E + the founder record (procedural controls in the wrapper);
    E..HEAD mechanically enforced by the gate.

**3.3 Gate checkpoints (13, inside verify-oc05.sh at F) — complete list,
defined HERE (no external reference):**
- OC05-01 clean+detached: `git status --porcelain` EMPTY AND
  `git symbolic-ref -q HEAD` exits non-zero (detached).
- OC05-02a executor consistency: `sha256sum scripts/verify-oc05.sh` ==
  `OC05_SCRIPT_SHA256` extracted from
  `git show "$OC05_PINS_COMMIT":_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md`.
- OC05-02b strict-linear chain + window purity:
  (a) `git merge-base --is-ancestor "$OC05_PINS_COMMIT" HEAD`;
  (b) `git rev-list --parents -n 1 "$OC05_PINS_COMMIT"` == exactly
  `[E, F]` (two parents' worth: the commit and its sole parent F);
  (c) `git rev-list --merges "$OC05_PINS_COMMIT"..HEAD` EMPTY;
  (d) per-commit changed-path union over `E..HEAD` within the 2 allowed
  dirs (`_bmad-output/verification-artifacts/`,
  `_bmad-output/planning-artifacts/`) component-wise.
- OC05-03 frozen manifest hashes (OC-04 X06 literals, verbatim):
  `sha256sum Cargo.toml` == `7c2075b807d9e5b7471e73aca95fa2984f9059da613d0eabae8c9bc5bb470124`;
  `sha256sum Cargo.lock` == `653accffb3d64e3a2810d4974112637fb98e7efa7eb1ab0a3ce99c543ea1ddf0`;
  `sha256sum contextmesh-salience/Cargo.toml` ==
  `e6aa9120a7115a08978dae517641fd6f80869ee2d393ae20ddf8db6f6261c3f4`.
- OC05-04 dependency closure: root does NOT depend on salience;
  salience→root present; package-ID set == claim-audit snapshot.
- OC05-05 fixture manifest: regenerated `oc-05-fixture-manifest.txt` ==
  committed; 3 sidecar bare-digest equalities (trailing newline
  tolerated).
- OC05-06 owner pins (whitespace-normalized substring match — all
  whitespace runs collapsed to one space on both sides — of each literal
  pin inside the named artifact; literals are the ACTUAL status lines
  recorded in those artifacts):
  * OC-01 pin → `_bmad-output/verification-artifacts/oc-01-evidence.md`:
    `have passing machine evidence at the commits`
  * OC-02 pin → `_bmad-output/verification-artifacts/oc-02-evidence.md`:
    `have passing machine evidence at the commits and commands recorded below`
  * OC-03 pin → `_bmad-output/verification-artifacts/oc-03-evidence.md`:
    `Implementation COMPLETE (3A–3G, commits`
  * OC-04 pin → `_bmad-output/planning-artifacts/oc-04-test-traceability-matrix.md`:
    `FROZEN v12`
  (NOTE: no `oc-04-evidence.md` exists — OC-04's evidence surface is its
  FROZEN spec + matrix; the OC-04 pin therefore anchors on the matrix's
  FROZEN status line. These four literals are frozen here; the gate
  script embeds them verbatim.)
- OC05-07 focused golden gates: `OC01_INNER_CURRENT_GATE=1
  CARGO_NET_OFFLINE=true cargo test -p contextmesh-salience --test
  oc03_artifact --test oc04_exec --locked -j 2`.
- OC05-08 full workspace regression: `OC01_INNER_CURRENT_GATE=1
  CARGO_NET_OFFLINE=true cargo test --workspace --locked -j 2` to a temp
  log; wait; require final line `REGRESSION_EXIT:0`.
- OC05-09 privacy scan: §7.1 categories (a)/(b)/(c) over the 5 filemap
  artifacts after §7.3 normalization — ANY hit → FAIL.
- OC05-10 OC-00 evidence integrity: `sha256sum` of
  `oc-00-5-real-data-replay.md` == `OC05_REPLAY_SHA256` and of
  `oc-00-prototype-validation.md` == `OC05_PROTO_SHA256`.
- OC05-11 claim audit: §7.2 inventory fully resolved; zero
  pending/TODO/TBD; gate prints `OC05_SCRIPT_ID=oc-05-release-gate-v1`
  matching §5 verbatim.
- OC05-12 honest disposition record: gate output records the chosen §12
  P5-GO disposition and the P3-GO OPEN line with its synthetic-label
  citation present.

`OC05_ONLY` selector: unset → full run; else exactly ONE of the 13
literals above; invalid → `OC05-ONLY: FAIL (invalid selector '<v>')`,
exit 1.

**3.4 Failure shapes:** fixture tamper C → OC05-02b FAIL; C + evidence D →
still FAIL; HEAD-script replacement → OC05-02a FAIL; merge-E →
OC05-02b(b) FAIL; sibling branch → OC05-02b(a) FAIL; unset/`abc`/unknown
pins → `OC05-PINS: FAIL`; E′ substitution → `OC05-BIND: FAIL`; workdir/env
failure → `OC05-WRAP: FAIL`; extraction/digest/mktemp failure →
`OC05-LAUNCH: FAIL` with temp cleanup.

## 4. Stage plan

**5A workspace (exact commands, run from repo root; all must pass before
5B):**
```
git rev-parse HEAD                          # expect 473783172c9af914b0964780e54fb4d7a7625d1b (4F) or its documented successor
git status --porcelain                      # expect ONLY: the two untracked OC-05 drafts + regression-*.log
! grep -rqn "oc05" --include="*.rs" src contextmesh-salience/src || { grep -rn "oc05" --include="*.rs" src contextmesh-salience/src | grep -v attribution.rs; false; }
git diff --exit-code Cargo.toml Cargo.lock contextmesh-salience/Cargo.toml                # expect: clean (no manifest drift)
```
(The third command asserts "no oc05 product code": an empty grep is a
PASS — current tree has zero oc05 matches, so the inverted assertion
holds trivially.)
**5B script:** author `scripts/verify-oc05.sh` implementing §3.3 (13
checkpoints with the frozen commands of §6) and the §7 categories verbatim;
commit it — this commit IS F. Derive
`OC05_SCRIPT_SHA256=$(sha256sum scripts/verify-oc05.sh | cut -d' ' -f1)`.

**5C evidence (exact order — the gate run happens AFTER the evidence
commit, because OC05-01 requires detached HEAD + clean tree and OC05-02
requires the evidence in tracked paths):**
1. Edit the OC-05 spec inside the working tree: replace the
   `OC05_SCRIPT_SHA256` placeholder in §5 with the concrete 64-hex digest
   from 5B (the spec file at E must contain a line beginning
   `` `OC05_SCRIPT_SHA256` `` followed by that digest — the wrapper's
   pins-read pipeline extracts exactly this). ALSO author the four
   evidence artifacts now: `oc-05-release-evidence.md`,
   `oc-05-claim-audit.md`, `oc-05-fixture-manifest.txt`, and this matrix —
   they must exist as TRACKED files at E so OC05-02 window purity and
   OC05-05 manifest checks see them.
2. Commit the spec edit AND the four evidence artifacts together — this
   commit IS E, and it MUST be F's sole child (`git commit` immediately
   after F, no intermediate commits; verify with
   `git rev-list --parents -n 1 E` == `[E, F]`). All changed paths must
   lie within the §5 allowed evidence dirs + the spec's own path, so
   OC05-02(d) passes.
3. `git checkout --detach` (OC05-01 needs detached HEAD; the working tree
   is now clean because everything is committed).
4. Founder approval of E is recorded OUTSIDE the repo (approval message +
   E's object id); export `OC05_PINS_COMMIT=$E OC05_EXPECTED_E=$E`.
5. Run the frozen §10 wrapper verbatim; it derives F, extracts the F-blob
   gate, verifies the digest against E's spec, and executes OC05-01..12
   over the tracked, committed evidence. PASS output + the four evidence
   files are then pushed together.

**5D ADVERSARIAL (executable script, per matrix R13–R15):**
```bash
#!/usr/bin/env bash
set -u -o pipefail
# --- setup ---
E="${1:?usage: oc05-5d.sh <founder-E commit id>}"
git show "$E":_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md \
  | sed -n '/^```$/,/^```$/p' | sed -n '/^cd "\${OC05_WORKDIR/,/^bash "\$tmp"; rc=\$?; exit \$rc$/p' \
  > /tmp/oc05-wrapper.sh
git clone -q /home/cosmo/contextmesh-rs /tmp/oc05-probe
cd /tmp/oc05-probe || exit 9
git checkout -q --detach "$E"
reset_base() { git checkout -q --detach "$E" && git clean -qfdx -e /tmp 2>/dev/null; git reset -q --hard "$E"; }

run_wrap() {  # $1=pins $2=expected $3=selector("" = unset)
  if [ -n "${3:-}" ]; then
    OC05_PINS_COMMIT="$1" OC05_EXPECTED_E="$2" OC05_ONLY="$3" \
      bash /tmp/oc05-wrapper.sh > /tmp/probe.out 2>&1
  else
    OC05_PINS_COMMIT="$1" OC05_EXPECTED_E="$2" \
      bash /tmp/oc05-wrapper.sh > /tmp/probe.out 2>&1
  fi
  return $?
}
assert_fail() { # $1=name $2=expected-marker-substring $3=pins $4=expected $5=selector
  local rc
  run_wrap "$3" "$4" "${5:-}" >/dev/null; rc=$?
  [ "$rc" -eq 1 ] || { echo "$1: FAIL-rc=$rc"; return 1; }
  [ "$(tail -n 1 /tmp/probe.out)" = "$2" ] \
    || { echo "$1: FAIL-marker got='$(tail -n 1 /tmp/probe.out)'"; return 1; }
  echo "$1: OK"
}
assert_clean() { [ -z "$(git status --porcelain)" ] || { echo "$1: FAIL-porcelain"; return 1; }; }

PASS=0; FAILN=0
chk() { if "$@"; then PASS=$((PASS+1)); else FAILN=$((FAILN+1)); fi; }

# === R13 (i) committed fixture tamper (new commit C) ===
reset_base
printf 'x' >> contextmesh-salience/tests/fixtures/oc03-prior-v1-golden.json
git add -A && git commit -qm C
git checkout -q --detach HEAD        # detach so OC05-01 passes
chk assert_clean R13-i-pre
chk assert_fail R13-i 'OC05-02b FAIL' "$E" "$E"

# === R13 (ii) tamper C + in-window evidence commit D ===
reset_base
printf 'x' >> contextmesh-salience/tests/fixtures/oc03-prior-v1-golden.json
git add -A && git commit -qm C
printf 'd\n' >> _bmad-output/verification-artifacts/oc-01-evidence.md
git add -A && git commit -qm D
git checkout -q --detach HEAD
chk assert_fail R13-ii 'OC05-02b FAIL' "$E" "$E"

# === R13 (iii) HEAD gate-script replacement (new commit) ===
reset_base
printf '#!/usr/bin/env bash\necho pwned\n' > scripts/verify-oc05.sh
git add -A && git commit -qm script-swap
git checkout -q --detach HEAD
chk assert_fail R13-iii 'OC05-02a FAIL' "$E" "$E"

# === R13 (iv) committed fixture tamper, selected checkpoint OC05-05 ===
reset_base
printf 'x' >> contextmesh-salience/tests/fixtures/oc03-prior-v1-golden.json
git add -A && git commit -qm C4
git checkout -q --detach HEAD
chk assert_fail R13-iv 'OC05-05 FAIL' "$E" "$E" OC05-05

# === R13 (v) merge-E: pins stay founder E; a merge commit is created as
#     a DESCENDANT of E, so `rev-list --merges E..HEAD` is non-empty and
#     OC05-02b(c) fires. Detach at the merge so OC05-01 passes. (The two
#     sides touch DIFFERENT files to avoid a content conflict.) ===
reset_base
git checkout -q --detach "$E"
printf 'm\n' >> _bmad-output/verification-artifacts/oc-01-evidence.md
git add -A && git commit -qm side1
SIDE1=$(git rev-parse HEAD)
git checkout -q --detach "$E"
printf 'n\n' >> _bmad-output/verification-artifacts/oc-02-evidence.md
git add -A && git commit -qm side2
git merge -q --no-ff -m merge-node "$SIDE1"
git checkout -q --detach HEAD        # detach at the merge commit
chk assert_fail R13-v 'OC05-02b FAIL' "$E" "$E"

# === R13 (vi) sibling branch from F: pins=S is NOT an ancestor of HEAD ===
#     detach at E (so OC05-01 passes: clean+detached), pins=S where S is a
#     sibling commit (not on E..HEAD ancestry) -> 02b(a) fails ===
reset_base
git checkout -q --detach 0386462f8a1069820371726af19125351a5ae8b6
git checkout -q -b sibling 0386462f8a1069820371726af19125351a5ae8b6
git commit -q --allow-empty -m sibling
S=$(git rev-parse HEAD)
git checkout -q --detach "$E"        # back to clean detached E
chk assert_fail R13-vi 'OC05-02b FAIL' "$S" "$S"

# === R13 (vii) E′ substitution: pins=E2 (valid linear), expected=founder E
#     BIND compares expected vs pins -> mismatch -> OC05-BIND FAIL ===
reset_base
E2=$(git rev-parse 473783172c9af914b0964780e54fb4d7a7625d1b)
[ "$E2" != "$E" ] || E2=$(git rev-parse "$E^")
chk assert_fail R13-vii 'OC05-BIND: FAIL (E mismatch vs founder record)' "$E2" "$E"

# === R15 selector negatives (assert FULL last marker line) ===
chk assert_fail R15-neg-OC05-99 "OC05-ONLY: FAIL (invalid selector 'OC05-99')" "$E" "$E" OC05-99
chk assert_fail R15-neg-multi "OC05-ONLY: FAIL (invalid selector 'OC05-02a OC05-02b')" "$E" "$E" 'OC05-02a OC05-02b'
chk assert_fail R15-neg-comma "OC05-ONLY: FAIL (invalid selector 'OC05-02a,OC05-02b')" "$E" "$E" 'OC05-02a,OC05-02b'
# bootstrap negatives
chk assert_fail R15-neg-unset 'OC05-PINS: FAIL' '' ''
chk assert_fail R15-neg-abc   'OC05-PINS: FAIL' abc abc
chk assert_fail R15-neg-unknown40 'OC05-PINS: FAIL' \
  0000000000000000000000000000000000000000 0000000000000000000000000000000000000000
# selector positives (single-checkpoint runs on clean detached clone)
reset_base
run_wrap "$E" "$E" OC05-02a
[ $? -eq 0 ] && grep -q 'OC05-02a' /tmp/probe.out && echo 'R15-pos-02a: OK' && PASS=$((PASS+1)) \
  || { echo 'R15-pos-02a: FAIL'; FAILN=$((FAILN+1)); }
reset_base
run_wrap "$E" "$E" OC05-02b
[ $? -eq 0 ] && grep -q 'OC05-02b' /tmp/probe.out && echo 'R15-pos-02b: OK' && PASS=$((PASS+1)) \
  || { echo 'R15-pos-02b: FAIL'; FAILN=$((FAILN+1)); }

# === R14 determinism (clean clone at E, two runs, §7.3 normalization) ===
reset_base
run_wrap "$E" "$E"; a=$?
run_wrap "$E" "$E"; b=$?
[ "$a" -eq 0 ] && [ "$b" -eq 0 ] || { echo 'R14: FAIL-rc'; FAILN=$((FAILN+1)); }
norm() { sed -E -e 's#/tmp/oc05-probe#PROBE#g' -e 's#/home/[a-zA-Z0-9_.-]+#HOME#g' \
             -e 's#[0-9]+(\.[0-9]+)?(ms|s)\b#TIME#g' /tmp/probe.out | tr -s ' \t' ' '; }
run_wrap "$E" "$E" >/dev/null; norm > /tmp/t1
run_wrap "$E" "$E" >/dev/null; norm > /tmp/t2
diff -q /tmp/t1 /tmp/t2 >/dev/null && echo 'R14-diff: OK' && PASS=$((PASS+1)) \
  || { echo 'R14-diff: FAIL'; FAILN=$((FAILN+1)); }
[ -z "$(git status --porcelain)" ] && echo 'R14-porcelain: OK' && PASS=$((PASS+1)) \
  || { echo 'R14-porcelain: FAIL'; FAILN=$((FAILN+1)); }

echo "PASS=$PASS FAIL=$FAILN"
[ "$FAILN" -eq 0 ]
```
No immutable Git object is edited; every variant creates NEW objects
(commits, branches, merges) or fresh working-tree state. R16 is a
delegated read-only review (over §7.2's resolved inventory); R17 is
wrapper step 2 exercised inside the R13(vii) probe. Marker assertions use
the designated matrix substrings; the exact last-line equality in
`assert_fail` is applied to the FINAL wrapper marker line (for gate runs
the wrapper exits at the first failing checkpoint, so the last line IS
that checkpoint's marker).

## 5. Declared freezes

Prereg: `P1-PREREG-SALIENCE-EVAL-V1` v1 `non_claims` (4).

New freezes (written at E): `OC05_SCRIPT_ID="oc-05-release-gate-v1"`;
`OC05_SCRIPT_SHA256`=`0ef7a7f38fe3c4e65afaa42c4a2c21e3bd1bedfb7eb72a009e5d483698c9e9b4`;
`OC05_REPLAY_SHA256=176ea2801555dbef59f31013d426f183b288524c44422dcaf47a5e91363219c5`;
`OC05_PROTO_SHA256=4c71f4a2cfb992c3c777c9e7a77e584c9090ca41fc6eeb48c91001071a479631`;
allowed set = 2 full repo-relative dirs (component-wise); owner pins (§6);
`OC05_ONLY` grammar (13 literals); R14 normalization (§7); wrapper
invariants §3.2 (i–iv, incl. the infallible-preamble exemption) and the
frozen §10 procedure text; new marker `OC05-WRAP: FAIL`.
`oc-05-fixture-manifest.txt` = fixtures-only.

## 6. Gate checkpoints

The 13 checkpoints are FULLY DEFINED in §3.3 above (single source of
truth — no external reference). §5 freezes the commands that implement
them inside `verify-oc05.sh`; the two heavyweight commands are:
- OC05-07: `OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test -p contextmesh-salience --test oc03_artifact --test oc04_exec --locked -j 2`
- OC05-08: `OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test --workspace --locked -j 2` (temp log; `wait`; marker `REGRESSION_EXIT:0`)

`OC05_ONLY` selector: unset → full run; else exactly ONE of 13 literals;
invalid → `OC05-ONLY: FAIL (invalid selector '<v>')`, exit 1.

## 7. Privacy scan, claim inventory, transcript normalization

**7.1 Privacy scan (checkpoint OC05-09, fail-closed — any hit → FAIL).**
Three categories, scanned over the evidence surface (the 5 filemap
artifacts) after R14 normalization:
- (a) **Credential/secret patterns**: `AKIA[0-9A-Z]{16}`,
  `sk-or-v1-[0-9a-f]{32,}`, `sk-[A-Za-z0-9]{20,}`, `ghp_[A-Za-z0-9]{36}`,
  `-----BEGIN [A-Z ]*PRIVATE KEY-----`, `Authorization: Bearer `,
  `(?i)(api[_-]?key|secret|password|token)\s*[:=]\s*['\"]?[A-Za-z0-9/+_-]{16,}`.
- (b) **Local-infrastructure identifiers**: `192\.168\.[0-9]{1,3}\.[0-9]{1,3}`,
  `10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}`, `127\.0\.0\.1[^0-9]`,
  `/home/[a-z0-9_]+/` (home-dir paths), `192.168.100.32` (infra host).
- (c) **Unreleased-source markers**: `TODO`, `TBD`, `FIXME`, `XXX`,
  `HACK`, `pending`, `WIP` (case-sensitive literal matches, word-ish).
The OC-04 frozen surface is SAFE by construction for (a)/(b): the
`VerifiedPrior` compile-time privacy gate and runtime verification already
prohibit raw-payload emission, and OC-04 evidence contains no credentials
or local identifiers — this section's scan re-proves it mechanically on
the OC-05 evidence surface without weakening those frozen guarantees.

**7.2 Resolved claim inventory (checkpoint OC05-11 input).** The audit
resolves every release-relevant claim in the filemap artifacts against the
frozen `P1-PREREG-SALIENCE-EVAL-V1` v1 `non_claims` (4, verbatim in the
prereg JSON): no causal C2 claim without causal/human/B8 evidence; no C3
prior/Thorn completion claim; preregistration does not approve/define/claim
OC-02 implementation; policy-only freeze with no evaluation-result claim.
Zero `pending`/`TODO`/`TBD` occurrences must remain after resolution.

**7.3 Transcript normalization (matrix R14).** For the two-run determinism
probe: strip absolute paths (`/tmp/oc05-probe…`, `/home/…`), strip
timing/duration fields, collapse whitespace runs, then compare the two
trimmed transcripts for byte equality.

## 8. Risks

Regression duration (temp-log+wait+marker); operator executes the frozen
inline wrapper verbatim; manifest cost trivial.

## 9. Filemap

`scripts/verify-oc05.sh`; `oc-05-release-evidence.md`;
`oc-05-claim-audit.md`; `oc-05-fixture-manifest.txt`;
`oc-05-test-traceability-matrix.md`.

## 10. Operator procedure (FROZEN inline wrapper — fully guarded, quiet)

```
cd "${OC05_WORKDIR:-$PWD}" 2>/dev/null \
  || { echo "OC05-WRAP: FAIL (workdir)"; exit 1; }
source ~/.cargo/env 2>/dev/null \
  || { echo "OC05-WRAP: FAIL (cargo env)"; exit 1; }
set -o pipefail   # infallible preamble builtin (§3.2-ii)
PINS="${OC05_PINS_COMMIT-}"
EXPECTED="${OC05_EXPECTED_E-}"
# --- step 1: PINS (before any blob access) ---
[ "${#PINS}" -eq 40 ] && [[ "$PINS" =~ ^[0-9a-f]{40}$ ]] \
  && git rev-parse --verify "$PINS^{commit}" >/dev/null 2>&1 \
  || { echo "OC05-PINS: FAIL"; exit 1; }
# --- step 2: BIND (before any F-derived code is selected) ---
[ "$EXPECTED" = "$PINS" ] \
  || { echo "OC05-BIND: FAIL (E mismatch vs founder record)"; exit 1; }
# --- step 3: LAUNCH (derive + extract + digest) ---
F=$(git rev-parse "$PINS^" 2>/dev/null) \
  || { echo "OC05-LAUNCH: FAIL (derive)"; exit 1; }
tmp=$(mktemp 2>/dev/null) \
  || { echo "OC05-LAUNCH: FAIL (mktemp)"; exit 1; }
trap 'rm -f "$tmp" 2>/dev/null' EXIT   # registration infallible (§3.2-ii); rm action guarded (§3.2-iii)
git show "$F":scripts/verify-oc05.sh > "$tmp" 2>/dev/null \
  || { echo "OC05-LAUNCH: FAIL (extraction)"; exit 1; }
[ -s "$tmp" ] || { echo "OC05-LAUNCH: FAIL (extraction)"; exit 1; }
want=$(git show "$PINS":_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md 2>/dev/null \
       | grep -m1 '^OC05_SCRIPT_SHA256=' 2>/dev/null \
       | grep -oE '[0-9a-f]{64}' 2>/dev/null) \
  || { echo "OC05-LAUNCH: FAIL (pins read)"; exit 1; }
have=$(sha256sum "$tmp" 2>/dev/null | cut -d' ' -f1 2>/dev/null) \
  || { echo "OC05-LAUNCH: FAIL (digest)"; exit 1; }
[ "$have" = "$want" ] || { echo "OC05-LAUNCH: FAIL (gate digest)"; exit 1; }
# --- step 4: execute ---
bash "$tmp"; rc=$?; exit $rc
```

## 11. Change Log

v1→…→v13 (see prior changelogs) →
**v14 after round 13 (deleg_c91cb438): Quality APPROVE; Compliance NO-GO
2 claims-accuracy blockers — addressed.** (1) preamble steps guarded: `cd`
and `source` now fail loud with the new `OC05-WRAP: FAIL` marker;
`set -o pipefail` and `trap` are explicitly declared INFALLIBLE PREAMBLE
builtins exempt from the guard rule — §3.2(ii) is now exactly true.
(2) markers-only claim scoped precisely (§3.2-iii): every FALLIBLE command
suppresses its own stderr (`mktemp`, `sha256sum`, `cut`, `grep`, `source`,
`git`), so each failure path emits exactly one frozen marker with no raw
diagnostics; R00 evidence updated to the precise wording. (3) matrix R00
rewritten to cite invariants i–iv verbatim. →
**v15 after round 14 (deleg_9fa7a8fe): Quality APPROVE reconfirmed; Compliance
NO-GO 2 wording-precision blockers — addressed.** (1) §3.2(ii) re-scoped into
two exact classes: (a) EXTERNAL commands carry `2>/dev/null || { marker;
exit 1; }`, (b) SHELL BUILTIN TESTS (`[ … ]`/`[[ … ]]`) are inherently quiet
and carry the bare `|| { marker; exit 1; }` form — the claimed universal
syntax now matches the wrapper literally. (2) trap ACTION guarded: §10 now
reads `trap 'rm -f "$tmp" 2>/dev/null' EXIT` — `rm` is a fallible external
command and its stderr is suppressed, so §3.2(iii) markers-only holds for
cleanup failures too. Matrix R00 evidence updated to the two-class wording. →
**v16 after round 15 (deleg_057a02f7): Quality APPROVE reconfirmed; Compliance
NO-GO 1 blocker — syntax-classification claims still diverged from §10 in 5
spots (cd/source called external though builtin; PINS git uses
`>/dev/null 2>&1` not `2>/dev/null`; trap action has no marker clause; step 4
`bash "$tmp"` unguarded and unexempted; PINS builtin tests share one trailing
`||` rather than individually carrying a form). Addressed by ABANDONING
syntax-template claims entirely: §3.2(ii) is now a GUARANTEE-level invariant
("every fallible step terminates with a frozen marker + exit 1") plus a
line-anchored inventory of §10 (l.114–145) that lists each step's actual
realization, including the declared deviations ((b) PINS conjunction,
(d) markerless silent trap cleanup, (e) step 4 outside §3.2 scope).
§3.2(iii) reworded to the same guarantee scope. Matrix R00 updated to cite
the inventory, not a syntax form. →
**v17 after round 16 (deleg_3b4918a6): Quality REQUEST_CHANGES + Compliance
NO-GO on the same 2 defects — addressed.** (1) All §3.2(ii) line anchors
were stale by exactly 15 lines (written against the pre-v15 §10 offsets);
corrected to the actual spec-file lines (cd l.129 … step 4 l.160; §10 now
spans l.129–160) and the anchor basis declared ("relative to the spec
file"). Matrix R00 range updated to l.129–160. (2) The guarantee invariant
was overbroad ("every fallible step") while (d)/(e) declare markerless/
unguarded fallible steps; re-scoped to "every fallible step in §10 steps
0–3 (excluding the silent trap cleanup)" with step 4 explicitly outside
§3.2 scope. →
**v18 after round 17 (deleg_93918006): Quality REQUEST_CHANGES + Compliance
NO-GO on the same single defect — anchors still one line stale.** Root
cause identified: the v17 anchors were measured correctly, but the v17
edit itself moved §10 by one line — LINE ANCHORS IN A PROSE SECTION ABOVE
§10 ARE SELF-DEFEATING (any edit to the anchors shifts their target).
v18 ABOLISHES line-number anchors: §3.2(ii) now uses CONTENT anchors —
§10 step-comment markers (`# --- step N: …`) and verbatim command text
(`cd "${OC05_WORKDIR:-$PWD}"`, `source ~/.cargo/env`,
`F=$(git rev-parse "$PINS^" …)`, `tmp=$(mktemp …)`,
`git show "$F":scripts/verify-oc05.sh`, the `[ -s "$tmp" ]` test, the
want/have pipelines, `[ "$have" = "$want" ]`,
`trap 'rm -f "$tmp" 2>/dev/null' EXIT`, `bash "$tmp"; rc=$?; exit $rc`) —
which are invariant under further spec edits. The re-scoped guarantee
invariant wording from v17 is retained (round 17 confirmed it no longer
contradicts (d)/(e)); the six primary failure classifications and the
five approved paths were re-confirmed. Matrix R00 likewise cites content
anchors. →
**v19 after round 18 (deleg_94b0b7f1): line-anchor cycling CLOSED (both
verdicts confirmed no l.NNN anchors remain; invariant (a)–(e) internally
consistent; six gate failure classifications re-confirmed). One NEW defect:
several §3.2(ii) "verbatim command text" anchors contained prose ellipses
(`…`) and were not verbatim §10 substrings. Addressed by the reviewer's
suggested pattern option: every anchor is now defined as a UNIQUE COMMAND
PREFIX PATTERN — the literal leading substring of exactly one §10 line
(e.g. `F=$(git rev-parse "$PINS^"`, `tmp=$(mktemp`,
`want=$(git show "$PINS":`, `have=$(sha256sum "$tmp"`,
`[ "${#PINS}"`, `[[ "$PINS"`, `&& git rev-parse --verify`,
`[ "$EXPECTED"`, `[ "$have"`, `set -o pipefail`), stated verbatim up to
the prefix boundary; the remainder of each line is explicitly NOT part of
the anchor. Step 4 is given in full (`bash "$tmp"; rc=$?; exit $rc`).
Matrix R00 updated to the pattern-anchor wording. →
**v20 after round 19 (deleg_a4e6fc2d): Compliance GO (first of the anchor
saga — 17/17 anchors literal+unique, no ellipsis, R00 accurate). Quality
REQUEST_CHANGES 1 narrow blocker — 3 PINS-conjunction anchors failed the
LEADING-substring property: (1) `[[ "$PINS"` occurs mid-line (§10 joins
both builtin tests on one continued line); (2)/(3) `&& git rev-parse
--verify` and `\|\| { echo "OC05-PINS: FAIL"` carry two leading spaces
in §10; the latter also carried markdown escapes absent from the file.
Addressed: (1) the anchor is now the full literal prefix
`[ "${#PINS}" -eq 40 ] && [[ "$PINS"`; (2)/(3) the matching rule now
declares leading whitespace ignored and backslash-continuation characters
stripped before matching (as bash does at parse time); the shared trailing
clause is stated in full instead of `…`. No other anchor touched; §10
unchanged. →
**v21 after round 20 (deleg_c597c7e2): 2 of 3 v20 PINS anchor fixes VERIFIED
by both verdicts (full builtin-test prefix matches line 161; whitespace-
normalized `&& git rev-parse --verify` matches line 162); 1 residual defect
— the failure-clause anchor still read `\|\| { echo "OC05-PINS: FAIL"`
with Markdown escape backslashes, which the declared rule (leading-
whitespace strip + bash-continuation strip) does NOT remove. Fixed: the
anchor and the shared trailing clause are now written with plain literal
pipes, `|| { echo "OC05-PINS: FAIL"` / `|| { echo "OC05-PINS: FAIL";
exit 1; }`, exactly as the file bytes read. Nothing else changed; §10
unchanged.

## 12. REQUIRED founder choice at freeze: P5-GO disposition

**Option 2 (CURRENT DEFAULT)**: P5-GO recorded `DEFERRED` until 4G + real
gold; silence = Option 2. **Option 1**: explicit founder approval required
for the separately-labeled conditional-release record. **Founder directive
needed: "Option 1" or "Option 2".**
