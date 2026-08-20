# OB-06 Option B Omission Challenge and Uncertainty Evidence (gate B6)

candidate-commit: a7ad623 (OB-05 handoff commit; the parent of the OB-06 evidence commit)
procedure-tree: 6f885dd3f1ea8f9235d836224532238bbb7393cd (tree of the candidate commit; the OB-06 commit extends the handoff module, adds the omission matrix, the verifier, and this evidence)
gate: scripts/verify-ob06.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01, OB-02, OB-03, OB-04, and OB-05 complete; OB-06 is the B6 omission challenge and uncertainty package of the Option B delivery plan)

## Scope of this evidence

OB-06 implements gate B6 (omission challenge and uncertainty) from the frozen
spec `spec-option-b-source-grounded-context-handoff.md` and package OB-06 from
`option-b-delivery-plan.md`. It is purely additive over Option A and
OB-01..OB-05, and it is the "handoff negotiation" entry point from plan §3.2:

- `src/handoff.rs` extended (the OB-06 work module) with the negotiation
  portion: the explicit `Omission` list with typed `OmissionReason`, the
  `uncertainty` markers, the typed `OmissionChallenge`, the recipient-facing
  `Handoff::challenge` API, and the re-inclusion half
  `Handoff::follow_up` that lands a challenged source in a follow-up handoff
  with the challenge recorded (`ReIncluded`);
- additive doc note in `src/lib.rs` only;
- no change to `src/delta.rs` (OB-04), `src/handoff.rs` unchanged parts are
  untouched, no change to `src/receipt.rs` (OB-01), no change to
  `src/selection.rs` / `src/compiler.rs` (OB-02), no change to `src/closure.rs`
  (OB-03), no change to any Option A module (verified by the gate's
  additive-only diff check), no new dependency, no CLI change.

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
- dependency-closure: 320 (unchanged; OB-06 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. Negotiation is self-contained in
  handoff.rs.

## Design notes

**Omissions are first-class, challengeable data.** Every handoff carries an
explicit omission list and uncertainty markers from construction (`from_delta`
starts them empty, so the list is present even when nothing is withheld — no
omission is hidden). `Handoff::with_omission` records a withheld source with a
typed, deterministic `OmissionReason` (`NotSelected`, `Deliberate`, or
`CapabilityMismatch` for the B11 capability-mismatch flags that wire in during
OB-11). The list is kept in canonical event order, and listing an event the
handoff already carries fails closed — an omission never names a delivered
source, so the record stays honest.

**Challenge API.** `Handoff::challenge(event, note)` is the recipient-facing
negotiation entry point: it produces the typed `OmissionChallenge` naming the
omitted event and recording the recipient's stated reason, and fails closed
with `UnknownOmission` when the event is not a listed omission and with
`InvalidState` for an empty or oversized note.

**Re-inclusion with the challenge recorded.** `Handoff::follow_up` builds the
follow-up handoff that re-includes a challenged omission. B5 composes into B6:
the original handoff must still be valid against the recipient's current head,
so a stale handoff is never negotiated. The re-inclusion must be real — the
supplied closed selection must contain the challenged source and the recomputed
B4 delta must land it in the follow-up handoff's events, or the negotiation
fails closed with `InvalidState`. The follow-up handoff carries the challenge
recorded on its `ReIncluded` list, drops the re-included omission, carries every
other listed omission and uncertainty marker forward, and leaves the original
handoff record intact (verified byte-for-byte).

**Uncertainty and the I/O matrix.** Selection uncertainty notes (for example
the no-match marker from the I/O & edge-case matrix — "no source matches the
task" is an uncertainty marker, never a hallucinated mapping) are carried onto
the handoff through `Handoff::with_uncertainty`; from OB-11 onward
capability-mismatch flags surface through the same channel.

## B6 success evidence

- The omission list, the uncertainty markers, and the re-inclusion list are
  present on every handoff, including empty ones (`every_handoff_carries_an_explicit_omission_list`).
- A handoff records explicit omissions and uncertainty markers, never lists a
  carried source as omitted, and the canonical wire carries the negotiation
  fields (`omissions_and_uncertainty_are_recorded_explicitly`,
  `with_omission_fails_closed_for_an_included_event`).
- A recipient challenges a listed omission and gets the typed challenge record;
  challenging an unlisted omission fails closed with `UnknownOmission`, and an
  empty note fails closed (`a_listed_omission_can_be_challenged_and_an_unlisted_one_cannot`).
- A challenged omission is re-included in a follow-up handoff with the challenge
  recorded, the other omissions stay explicitly listed, the follow-up is still
  state-bound (B5), and the original handoff is byte-identical afterward
  (`challenged_omission_is_re_included_in_the_follow_up_handoff_with_the_challenge_recorded`).
- A negotiation whose re-inclusion does not actually land in the delta fails
  closed, and a challenge that never came from this handoff's omission list
  fails closed with `UnknownOmission`
  (`follow_up_fails_closed_when_the_re_inclusion_does_not_land_in_the_delta`).
- A stale handoff is never negotiated: when the recipient advances, `follow_up`
  is rejected with the typed `Stale` error
  (`follow_up_fails_closed_when_the_original_handoff_is_stale`).
- A no-match selection yields an empty selection plus the explicit uncertainty
  marker, which the handoff carries — never a fabricated mapping
  (`uncertainty_markers_flow_from_a_no_match_selection`).
- Composition with OB-01/OB-02/OB-03/OB-04/OB-05: selection → closure → delta →
  handoff with an explicit omission + uncertainty → receipt whose
  omission/uncertainty notes are populated from the handoff's negotiation
  fields, verifying against the DAG with `checked_events == 3` (delta events
  plus the recipient head) (`omission_notes_and_uncertainty_feed_the_receipt`).
- Identical inputs produce byte-identical canonical handoff wires across the
  full negotiation, including the re-included history
  (`negotiation_fields_are_deterministic_on_the_wire`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/handoff.rs | negotiation portion: `Omission`/`OmissionReason`, `uncertainty`, `OmissionChallenge`, `ReIncluded`, `Handoff::with_omission`, `with_uncertainty`, `omissions`, `uncertainty`, `re_included`, `challenge`, `follow_up`; `UnknownOmission`/`Delta` error variants; bounds `MAX_OMISSIONS`, `MAX_UNCERTAINTY_NOTES`, `MAX_NOTE_BYTES` | extend the OB-05 handoff module (the OB-06 work module) with the omission-challenge and uncertainty negotiation |
| src/lib.rs | doc note for OB-06 | record the gate in the crate docs (additive registration only) |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs, zero
changes to delta.rs, receipt.rs, selection.rs, compiler.rs, closure.rs,
crypto.rs, cli.rs, model/store/error/sync/provider/http modules, and that
src/handoff.rs + src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- A challenged omission is re-included in a follow-up handoff with the
  challenge recorded: yes (the re-included source lands in the follow-up
  delta; the challenge is recorded on `ReIncluded`; the original handoff is
  left intact).
- No omission is hidden: the omission list is present on every handoff: yes
  (`from_delta` starts an explicit list, `with_omission` appends, `follow_up`
  carries the still-withheld omissions forward).

## Regression

- OB-05 validity matrix green (`cargo test --test ob05_validity`).
- OB-04 delta matrix green (`cargo test --test ob04_delta`).
- OB-03 closure matrix green (`cargo test --test ob03_closure`).
- OB-02 selection matrix green (`cargo test --test ob02_selection`).
- OB-01 receipt matrix green (`cargo test --test ob01_receipts`).
- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA suites pass).
- OB-02/OB-01/OA-01/OA-03/OA-04 golden fixtures byte-identical (sha256
  asserted).

## Evidence owners

- Amelia (engineer) — completion verdict for gate B6.
- Sally (UX) — challenge ergonomics review of the negotiation entry point.
- Lunarpulse — final approval.
