#!/usr/bin/env python3
"""sample.py — OC-04 4G+ deterministic family-cluster sampling manifest.

Plan: _bmad-output/planning-artifacts/oc04-gold-preparation-plan.md (v5.2 §3)

Committed, stdlib-only, deterministic given seed + snapshot. Emits the
frozen sampling manifest. No manual picking (BLOCKER-10 fix).

Strata are assigned deterministically from PRE-LABEL transcript state only
(BLOCKER-11 fix): terminal status + cost fields.
  - terminal_with_full_cost   : session terminated, cost >= full threshold
  - terminal_with_partial_cost: session terminated, cost <  full threshold
  - unterminated              : session did not terminate
`strict_all_gold_tf0` CANNOT be known pre-label: the oversupply pool size
is pre-declared by formula (§3):
    oversupply_N = ceil(target_stratum_N / base_rate_estimate - target_stratum_N)
Labeled blind, then assigned by the frozen rule AFTER labeling. No
opportunistic replacement; short yield is recorded SHORT, never swapped.

Inputs
------
--snapshot PATH : JSON {sessions: [{id, terminated, cost}] } (pre-label
                  redacted transcript state; local-only, never committed)
--size N        : target corpus size from Stage-0 (48/72/96)
--seed N        : default 20260820 (D-C-10 / §2)
--out PATH      : manifest output path (default stdout)

Output manifest (committed artifact) contains ONLY: session HMAC-ids
(keyed, §6 — raw ids stay local), stratum, family assignment, and the
seed/snapshot-digest provenance. This script never writes transcripts.
"""

import argparse
import hashlib
import hmac
import json
import math
import random
from typing import Dict, List

SEED_DEFAULT = 20260820

# §3 target proportions (match power-sim.py STRATA — frozen).
STRATA: Dict[str, float] = {
    "terminal_with_full_cost": 0.45,
    "terminal_with_partial_cost": 0.35,
    "unterminated": 0.20,
}

# Base-rate estimate for the strict-all-gold-TF0 stratum (§3): the Stage-0
# simulation's strict-stratum yield estimate. Declared pre-label, part of
# the frozen manifest; revision restarts Stage 0 (§2.4).
STRICT_BASE_RATE_ESTIMATE = 0.30
STRICT_SOURCE = "Stage-0 power-sim.py strict-stratum yield estimate (declared pre-label)"

# Keyed-ID derivation (§6): HMAC-SHA256 over the raw session id with this
# application-separated key. The KEY ITSELF ships via --key-hex; raw ids
# and keys stay local-only (BLOCKER from v5.1 cross-validation: the HMAC
# key must NOT be committed).
FAMILY_DOMAIN = b"oc04-gold/family/v1"
SESSION_DOMAIN = b"oc04-gold/session/v1"


def assign_stratum(session: dict) -> str:
    """Deterministic pre-label stratum from transcript state (§3)."""
    if not session.get("terminated", False):
        return "unterminated"
    cost = float(session.get("cost", 0.0))
    return "terminal_with_full_cost" if cost >= 1.0 else "terminal_with_partial_cost"


def stratified_sample(pool: List[dict], size: int, rng: random.Random) -> List[dict]:
    """Proportional allocation without replacement, deterministic order."""
    chosen: List[dict] = []
    remaining = list(pool)
    for i, (name, prop) in enumerate(STRATA.items()):
        # Last stratum absorbs rounding so the total is exact.
        if i == len(STRATA) - 1:
            quota = size - len(chosen)
        else:
            quota = round(size * prop)
        members = [s for s in remaining if assign_stratum(s) == name]
        rng.shuffle(members)
        take = min(quota, len(members))
        chosen.extend(members[:take])
        for m in members[:take]:
            remaining.remove(m)
    return chosen


def assign_families(sessions: List[dict], rng: random.Random) -> Dict[str, int]:
    """Parent-family assignment: group sibling sessions deterministically."""
    fams: Dict[str, int] = {}
    fid, i = 0, 0
    ids = [s["id"] for s in sessions]
    rng.shuffle(ids)
    while i < len(ids):
        size = 2 if (fid % 3) != 2 else 3  # 2,2,3 cycle (matches power-sim)
        for sid in ids[i : i + size]:
            fams[sid] = fid
        i += size
        fid += 1
    return fams


def keyed_id(key: bytes, domain: bytes, raw_id: str) -> str:
    return hmac.new(key, domain + raw_id.encode("utf-8"), hashlib.sha256).hexdigest()


def oversupply_formula(target_n: int) -> int:
    """§3: oversupply_N = ceil(target/base_rate - target)."""
    base = STRICT_BASE_RATE_ESTIMATE
    return math.ceil(target_n / base - target_n)


def build_manifest(pool: List[dict], size: int, seed: int, key: bytes) -> dict:
    rng = random.Random(seed)
    sample = stratified_sample(pool, size, rng)
    families = assign_families(sample, rng)
    strata_counts: Dict[str, int] = {}
    for s in sample:
        st = assign_stratum(s)
        strata_counts[st] = strata_counts.get(st, 0) + 1
    target_stratum_n = max(strata_counts.values()) if strata_counts else 0
    snapshot_digest = hashlib.sha256(
        json.dumps(sorted(s["id"] for s in pool)).encode("utf-8")
    ).hexdigest()
    return {
        "version": 1,
        "seed": seed,
        "corpus_size": size,
        "snapshot_digest": snapshot_digest,
        "strata": strata_counts,
        "oversupply": {
            "formula": "ceil(target_stratum_N / base_rate_estimate - target_stratum_N)",
            "base_rate_estimate": STRICT_BASE_RATE_ESTIMATE,
            "base_rate_source": STRICT_SOURCE,
            "oversupply_N": oversupply_formula(target_stratum_n),
        },
        "sessions": [
            {
                "session_hmac": keyed_id(key, SESSION_DOMAIN, s["id"]),
                "family_hmac": keyed_id(
                    key, FAMILY_DOMAIN, f"fam{families[s['id']]}"
                ),
                "stratum": assign_stratum(s),
            }
            for s in sorted(sample, key=lambda x: x["id"])
        ],
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--snapshot", required=True, help="pre-label redacted state JSON")
    ap.add_argument("--size", type=int, required=True, choices=[48, 72, 96])
    ap.add_argument("--seed", type=int, default=SEED_DEFAULT)
    ap.add_argument("--key-hex", required=True, help="HMAC key (hex, LOCAL-ONLY)")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    with open(args.snapshot, "r", encoding="utf-8") as fh:
        pool = json.load(fh)["sessions"]
    manifest = build_manifest(pool, args.size, args.seed, bytes.fromhex(args.key_hex))

    text = json.dumps(manifest, indent=2, sort_keys=True)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(text)
        print(f"manifest written: {args.out} (sessions={len(manifest['sessions'])})")
    else:
        print(text)


if __name__ == "__main__":
    main()
