# OC-0.5 private real-data replay harness

This directory contains the reproducible, privacy-preserving replay used by:

- `_bmad-output/verification-artifacts/oc-00-5-real-data-replay.md`
- `results-aggregate.json`

## Privacy contract

`replay.py` reads a local Hermes `state.db` snapshot, but its outputs contain aggregate numbers only. It must not emit message text, tasks, answers, session/user/chat IDs, paths, URLs, tool arguments, reasoning, or credentials. It performs no network calls and uses Python stdlib only.

The private SQLite snapshot is covered by the repository `*.db` ignore rule. Do not force-add it.

## Reproduction

From the repository root:

```bash
H=_bmad-output/implementation-artifacts/oc-real-data-replay/replay.py
D=_bmad-output/implementation-artifacts/oc-real-data-replay/private/state.db
O=_bmad-output/implementation-artifacts/oc-real-data-replay/results-aggregate.json

python3 "$H" snapshot \
  --source /home/cosmo/.hermes/state.db \
  --dest "$D"

python3 "$H" run --db "$D" --out "$O" --repo .
python3 "$H" verify --db "$D" --results "$O"
```

To check determinism, run twice against the same snapshot and compare `manifest.aggregate_fingerprint`. `generated_at_utc` is intentionally excluded from the fingerprint.

## Frozen research configuration

- seed: `20260820`
- split: temporal parent-family 60/20/20
- PPR damping: `0.85`
- PPR iterations: `20`
- alpha: `2.0`
- beta: `0.6`
- budgets: `6`, `12`
- M0/M1n: prototype-compatible value extraction

## Interpretation boundary

The replay uses silver proxies. It does not prove causal attribution, recipient comprehension, task success, or C1–C5 completion. See the verification report for the full validity analysis.
