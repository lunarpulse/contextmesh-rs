# OB-12 Semantic Mechanisms Dependency Audit (gate B12)

candidate-commit: c1d485d (OB-11 capability commit; the parent of the OB-12 evidence commit)
gate: scripts/verify-ob12.sh (deterministic, non-recording, offline)
decision: NON-ADOPTION — no candidate semantic mechanism passes the frozen audit

## Audit criteria (frozen, from spec + delivery plan)

1. **Forbidden-surface discipline:** the dependency closure must contain no TLS
   stacks, HTTP/2/3, QUIC, cookies, compression, DNS resolvers, shells, libp2p,
   or sqlite alternates (spec "Ask First"; Option A forbidden-surface list).
2. **Dependency-closure pin:** the locked closure must stay at 320 packages
   (the gate's hard pin; every OB gate re-asserts `Cargo.lock`).
3. **Offline discipline:** the suite runs with `CARGO_NET_OFFLINE=true`; model
   artifacts must be pinned in the repo and no runtime download may exist.
4. **Recorded-not-re-derived:** selector provenance records model identity +
   configuration hash; cross-run determinism is guaranteed only on the
   structural and verification path, never over model inference.
5. **No new wall-clock or network dependency** ("time" must not appear in the
   direct dependencies).

## Candidate evaluation

### Embeddings

| Candidate | Why it fails |
|---|---|
| `ort` (ONNX Runtime bindings) | pulls the native `onnxruntime` runtime and a large build-time dependency surface; cannot be pinned offline in this repo; breaks the 320 closure pin. |
| `candle` (Rust ML) | pulls `candle-core` plus tokenizer and backend crates (CUDA/cuBLAS on GPU paths); embedding model weights are downloaded at runtime, violating the offline discipline; breaks the 320 closure pin. |
| `tch` (libtorch bindings) | requires the native libtorch distribution (runtime download/install), GPU blobs, and a large native surface; breaks the offline discipline and the closure pin. |
| `fastembed` (via `ort`) | inherits the `ort`/onnxruntime surface and model downloads; same failures. |

### Vector search

| Candidate | Why it fails |
|---|---|
| `hnswlib` (Rust bindings) | new native dependency outside the pinned closure; no in-repo offline artifact; breaks the 320 closure pin. |
| `usearch` | new native dependency outside the pinned closure; breaks the 320 closure pin. |
| sqlite vector extensions (`sqlite-vss` and similar) | "sqlite alternates" are explicitly on the forbidden-surface list; breaks criterion 1. |
| pure-Rust HNSW/KD-tree crates | each candidate adds a new dependency outside the pinned closure; none is already in the 320 closure; breaks criterion 2. |

### Reranking

| Candidate | Why it fails |
|---|---|
| cross-encoder crates (candle/ort-based) | inherit the embedding failures: native surface, runtime model downloads, closure pin break. |
| any offline reranker | no reranker is present in the frozen 320 closure; adding one breaks the closure pin; the offline model-artifact requirement is unmet by every candidate surveyed. |

## Verdict

Every surveyed candidate for the three named mechanisms (embeddings, vector
search, reranking) fails at least one frozen audit criterion — in every case
the dependency-closure pin (criterion 2) and in most cases the offline
discipline (criterion 3) and the forbidden-surface list (criterion 1). There
is no compliant dependency to adopt.

**Decision: NON-ADOPTION**, recorded here and in `ob-12-evidence.md`. The
demonstrated baseline stands: the OB-02 deterministic lexical/term-frequency
selector with budget enforcement and recorded provenance, backed by the
frozen OB-08 evaluation suite (withheld-context cases fail, repaired cases
pass — the selection is load-bearing). This is a recorded decision, not a
silent deferral; B2 remains complete under the frozen spec.

## Evidence owners

- Winston (dependency gate) — audit verdict for gate B12.
- Amelia (engineer) — completion verdict for gate B12.
- Lunarpulse (Ask-First approver) — approval of the non-adoption decision.
