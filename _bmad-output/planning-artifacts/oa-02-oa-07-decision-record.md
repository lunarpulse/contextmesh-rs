---
title: 'OA-02 through OA-07 Approved Decision Record'
type: 'decision-record'
created: '2026-08-16'
status: 'approved'
approved: '2026-08-16'
approved_by: 'Lunarpulse'
baseline_commit: 'f61c4f0d147544c4011b2bb8b8094943e196c883'
execution_plan: './oa-02-oa-07-detailed-execution-plan.md'
test_matrix: './oa-02-oa-07-test-traceability-matrix.md'
---

# OA-02 through OA-07 Approved Decision Record

## Scope of approval

On 2026-08-16, Lunarpulse approved the detailed OA-02 through OA-07 execution plan, its minimum test traceability matrix, and decisions D-02-01, D-05-01, and D-04-01.

The approval authorizes preparation and execution of OA-02 followed by the recorded package sequence and gates. It does not change OA-01, declare unexecuted probes successful, mark OA-02 through OA-07 complete, weaken A1-A8, or unlock Option B.

## D-02-01 — Context bootstrap and append-only authorization

**Status:** Approved.

1. Each context has exactly one signed zero-parent context.genesis event.
2. Context IDs use 32 bytes of fallible OS entropy.
3. Context membership and author authorization are explicit local operator policy, not synchronized global consensus.
4. A joining node is provisioned with ContextId, expected genesis EventId, and a sorted unique initial author allowlist.
5. The first admitted event must exactly match the provisioned genesis; every other zero-parent event is rejected.
6. The Option A allowlist is append-only. Revocation, trusted policy chronology, and broader multi-workspace authorization are deferred and require approval.
7. Author admission permission proves neither truth, trustworthiness, nor semantic relevance.

**Consequence:** OA-02 may freeze and implement schema, provisioning, genesis, authorization, and admission semantics using this policy.

## D-05-01 — Opaque local signing-key custody

**Status:** Approved.

1. OA-05 may persist a 32-byte Ed25519 seed in a caller-selected local file solely for restart-safe signing.
2. Private material is never returned through public values, printed, logged, command-JSON encoded, stored in Turso, bundled, or synchronized.
3. Creation is atomic using a same-directory temporary file, file sync, rename, and directory sync where supported.
4. Unix files use mode 0600; group/other-accessible files are rejected unless an explicit repair operation is invoked.
5. Symlink targets are rejected. Key and token paths are local runtime configuration and ignored by source control.
6. ContextMesh makes no encryption-at-rest claim; OS or disk protection is the user's responsibility.
7. Filesystem races, permissions, disclosure, crash consistency, and platform limits require independent review.

**OA-01 interpretation:** Private material remains forbidden in OA-01 public and wire representations. This opaque local secret-at-rest mechanism is outside the signed-event contract and changes no canonical bytes, IDs, signatures, fixtures, or verification behavior.

## D-04-01 — Exact dependencies through mandatory preflight

**Status:** Approved for mandatory preflight. Pins become effective only after successful recorded probes.

Approved target candidates:

- Tokio 1.53.1 with only features proven necessary by completed packages;
- Axum 0.8.9 with defaults disabled and only HTTP/1, JSON, and Tokio support;
- Reqwest 0.13.4 with defaults disabled and only plain HTTP/1 and JSON support;
- Clap 4.6.6 with minimized derive/std/help/usage/error-context features;
- a direct constant-time comparison dependency only if no existing audited primitive is suitable.

Required selection process:

1. Probe each target on pinned Rust 1.97 in a disposable project.
2. Inspect MSRV and compile representative server, client, runtime, process, signal, and CLI paths.
3. Record exact Cargo metadata, direct features, locked transitive graph, and forbidden-feature checks.
4. Reject TLS backends, cookies, compression, HTTP/2, multipart, unnecessary proxy discovery, and agent-protocol dependencies.
5. Passing targets become exact minimal pins recorded in the OA-04 dependency plan before manifest changes.
6. Any failed target or substitute version/feature set requires approval; silent substitution is forbidden.

## Approved next sequence

1. Freeze the OA-02 implementation spec using D-02-01.
2. Run the Turso transaction, foreign-key, trigger, multi-connection, and restart probe.
3. Implement, review, verify, and commit OA-02 before OA-03.
4. Run D-04-01 probes before OA-04/OA-05 dependency changes.
5. Record D-05-01 in the OA-05 spec before key-custody implementation.
6. Preserve the OA-07 Option B gate.

## Change control

New approval is required for changes to these decisions, author revocation or broader authorization, failed/substitute dependencies, private-material exposure, weaker secret-file rules, OA-01 wire behavior, or the OA-07 Option B gate.
