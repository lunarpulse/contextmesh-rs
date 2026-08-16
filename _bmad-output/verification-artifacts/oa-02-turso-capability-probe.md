---
title: 'OA-02 Turso 0.7.2 Capability Probe'
type: 'technical-evidence'
created: '2026-08-16'
status: 'passed'
executed_on: '2026-08-16'
baseline_commit: 'e82b386'
rust: '1.97'
turso: '0.7.2, default-features=false'
tokio_probe: '1.53.1, macros+rt'
---

# OA-02 Turso Capability Probe

## Purpose

Before freezing OA-02 implementation details, test the pinned database API behaviors required by the approved plan in a disposable Rust 1.97 project.

## Probe construction

The disposable project pinned Turso 0.7.2 with defaults disabled and Tokio 1.53.1 with only macros and current-thread runtime support. It opened a file-backed local database and exercised multiple connections and independently built database handles.

## Passed assertions

1. An IMMEDIATE transaction can insert and explicitly roll back without retaining a row.
2. An IMMEDIATE transaction can explicitly commit and become visible to another connection.
3. PRAGMA foreign_keys must be enabled and queried back on each connection.
4. A missing-parent insert is rejected after foreign-key enablement.
5. A BEFORE UPDATE trigger rejects mutation of an immutable table.
6. Trigger enforcement survives close and reopen.
7. A second connection sees committed writes.
8. A separately constructed Turso Database for the same file sees existing rows and can commit a write visible to the first handle.
9. Committed parent and child rows survive complete handle drop and reopen.

Observed output:

    ok: IMMEDIATE transaction commit and explicit rollback
    ok: per-connection foreign-key enablement and enforcement
    ok: immutability trigger enforcement survives restart
    ok: multiple connections and independent database handles interoperate

## Implementation consequences

- OA-02 will use transaction_with_behavior(Immediate) for every mutation transaction.
- OA-02 will explicitly commit or rollback; it will not depend on dropped-transaction recovery.
- OA-02 will enable and verify foreign keys for every connection it creates.
- OA-02 may use database triggers as defense in depth for immutable event/edge/policy tables.
- OA-02 may test independent Store instances against one local file, but will claim only the concurrency semantics demonstrated by integration tests.

## Limitations

This probe did not prove high-contention fairness, process crash durability at every filesystem boundary, all platform behavior, or support for unbounded multi-process writers. Those remain implementation tests and documented limits. The disposable probe source/database were not committed and contained no project secret or production data.
