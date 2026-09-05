#!/usr/bin/env python3
"""sample.py — OC-04 4G+ deterministic family-cluster sampling manifest.

Plan: _bmad-output/planning-artifacts/oc04-gold-preparation-plan.md (v5.4 §3)

Committed, stdlib-only, deterministic given seed + snapshot. Emits the
frozen sampling manifest. No manual picking (BLOCKER-10 fix).

v5.4 strata (founder-approved Option A, PRE-LABEL, §2.4 clean):
  - terminal_high_tokens : terminated, input_tokens >  frame median
  - terminal_low_tokens  : terminated, input_tokens <= frame median
  - unterminated         : session did not terminate
Rationale: the real post-cutoff frame has ZERO sessions with recorded
cost > 0 (subscription-included API) — the v5.3 cost strata have no
separating power; input_tokens (pre-label state) preserves the §3
usage-intensity-proxy intent. The median is computed deterministically
from the snapshot itself and recorded in the manifest.

The frame is post-cutoff (temporal holdout): sessions with
started_at >= cutoff_epoch are eligible; the cutoff is the OC-00.5
replay snapshot date (2026-08-21, epoch 1787280000.0).

`strict_all_gold_tf0` CANNOT be known pre-label: the oversupply pool
size is pre-declared by formula (§3):
    oversupply_N = ceil(target_stratum_N / base_rate_estimate - target_stratum_N)
Labeled blind, then assigned by the frozen rule AFTER labeling. No
opportunistic replacement; short yield is recorded SHORT, never swapped.

Families are REAL parent families: sessions sharing parent_session_id
form one cluster (§3 BLOCKER-2); orphans cluster by their own id.
The achieved per-stratum family count is reported; < 12 families in a
stratum is recorded SHORT (underpowered-for-stratum, disclosed).

Inputs
------
--snapshot PATH : JSON {"cutoff_epoch": float, "sessions": [{id, terminated,
                  input_tokens, message_count, parent_session_id}]}
                  (pre-label redacted state; LOCAL-ONLY, never committed)
--size N        : target corpus size from Stage-0 (48/72/96/192/384)
--pilot N       : optional pilot subset size (§5.2b), stratum-proportional
--seed N        : default 20260820 (D-C-10 / §2)
--key-hex       : HMAC key (hex, LOCAL-ONLY — never committed, §6)
--out PATH      : manifest output path
--pilot-out PATH: optional pilot manifest output path

Output manifest (committed artifact) contains ONLY: session HMAC-ids
(keyed, §6 — raw ids stay local), stratum, family HMAC, alternates,
exclusion reasons, and seed/snapshot-digest provenance. This script
never writes transcripts.
"""

import argparse
import hashlib
import hmac
import json
import math
import random
from typing import Dict, List, Optional, Tuple

SEED_DEFAULT = 20260820
CUTOFF_EPOCH = 1787280000.0  # 2026-08-21 (OC-00.5 snapshot date)
MIN_MESSAGES = 4

# §3 v5.4 strata. Proportions are NOT fixed constants: allocation is
# proportional to the measured frame (near 50/50 in the real frame).
STRATA = ("terminal_high_tokens", "terminal_low_tokens", "unterminated")

# Base-rate estimate for the strict-all-gold-TF0 stratum (§3): the Stage-0
# simulation's strict-stratum yield estimate. Declared pre-label, part of
# the frozen manifest; revision restarts Stage 0 (§2.4).
STRICT_BASE_RATE_ESTIMATE = 0.30
STRICT_SOURCE = "Stage-0 power-sim-v2 strict-stratum yield estimate (declared pre-label)"

FAMILY_MIN = 12  # §3 BLOCKER-2: >= 12 independent families per stratum

# Keyed-ID derivation (§6): HMAC-SHA256 with an application-separated key.
# The KEY ships via --key-hex; raw ids and the key stay local-only.
FAMILY_DOMAIN = b"oc04-gold/family/v1"
SESSION_DOMAIN = b"oc04-gold/session/v1"


def eligible(s: dict, cutoff: float) -> bool:
    return (
        s.get("started_at", 0.0) >= cutoff
        and int(s.get("message_count", 0)) >= MIN_MESSAGES
    )


def frame_median(sessions: List[dict]) -> float:
    """Deterministic median input_tokens over eligible terminated sessions."""
    toks = sorted(
        float(s["input_tokens"])
        for s in sessions
        if eligible(s, CUTOFF_EPOCH) and s.get("terminated") and float(s.get("input_tokens", 0)) > 0
    )
    if not toks:
        raise SystemExit("empty frame: no eligible terminated sessions with tokens")
    n = len(toks)
    return toks[n // 2] if n % 2 else (toks[n // 2 - 1] + toks[n // 2]) / 2.0


def assign_stratum(s: dict, median: float) -> str:
    """Deterministic pre-label stratum from transcript state (§3, v5.4)."""
    if not s.get("terminated", False):
        return "unterminated"
    return "terminal_low_tokens" if float(s.get("input_tokens", 0)) <= median \
        else "terminal_high_tokens"


def assign_stratum_sessions(sessions: List[dict], median: float) -> str:
    """Stratum of a whole family: majority over members, ties broken by
    the family-level token ratio (high share > 0.5 → high, else low),
    unterminated only when EVERY member is unterminated. Deterministic.
    Mixed-stratum families are common in the real frame (10/19), so
    per-member classification undercounts one side; the family is ONE
    cluster (§3 BLOCKER-2) and must have exactly one stratum."""
    if all(not s.get("terminated", False) for s in sessions):
        return "unterminated"
    term = [s for s in sessions if s.get("terminated", False)]
    high = sum(1 for s in term if assign_stratum(s, median) == "terminal_high_tokens")
    return "terminal_high_tokens" if high * 2 >= len(term) else "terminal_low_tokens"


def family_of(s: dict) -> str:
    return s.get("parent_session_id") or s["id"]


def proportional_quotas(pool: List[dict], size: int, median: float) -> Dict[str, int]:
    """Largest-remainder allocation so the total is exact."""
    counts = {st: 0 for st in STRATA}
    for s in pool:
        counts[assign_stratum(s, median)] += 1
    total = sum(counts.values())
    raw = {st: size * c / total for st, c in counts.items()}
    base = {st: int(math.floor(raw[st])) for st in STRATA}
    # distribute remaining seats by largest fractional remainder (stable order)
    rem = size - sum(base.values())
    order = sorted(STRATA, key=lambda st: (-(raw[st] - base[st]), STRATA.index(st)))
    for st in order[:rem]:
        base[st] += 1
    return base


def stratified_sample(
    pool: List[dict], size: int, median: float, rng: random.Random
) -> List[dict]:
    """Proportional allocation without replacement, deterministic order.

    v5.4: within each stratum, sessions are drawn FAMILY-ROUND-ROBIN
    (deterministic shuffled family order) so the sample spreads across the
    maximum number of real parent families. Without this, proportional
    sampling concentrates large low-token mega-families into a single
    cluster and destroys the family-bootstrap effective-N.
    """
    quotas = proportional_quotas(pool, size, median)
    chosen: List[dict] = []
    remaining = list(pool)
    for st in STRATA:
        members = [s for s in remaining if assign_stratum(s, median) == st]
        # group by real parent family
        fams: Dict[str, List[dict]] = {}
        for s in members:
            fams.setdefault(family_of(s), []).append(s)
        fam_keys = sorted(fams.keys())
        rng.shuffle(fam_keys)
        # round-robin one session per family per pass (deterministic order)
        queues = {f: fams[f][:] for f in fam_keys}
        for f in fam_keys:
            rng.shuffle(queues[f])
        take = min(quotas[st], len(members))
        picked = 0
        while picked < take:
            progressed = False
            for f in fam_keys:
                if picked >= take:
                    break
                if queues[f]:
                    chosen.append(queues[f].pop())
                    remaining.remove(chosen[-1])
                    picked += 1
                    progressed = True
            if not progressed:
                break
    if len(chosen) < size:
        raise SystemExit(
            f"frame too small: requested {size}, allocated {len(chosen)}"
        )
    return chosen


def select_pilot(
    sample: List[dict], size: int, median: float, rng: random.Random
) -> List[dict]:
    """Pilot subset (§5.2b): stratum-proportional, drawn FROM the corpus."""
    quotas = proportional_quotas(sample, size, median)
    chosen: List[dict] = []
    remaining = list(sample)
    for st in STRATA:
        members = [s for s in remaining if assign_stratum(s, median) == st]
        rng.shuffle(members)
        take = min(quotas[st], len(members))
        chosen.extend(members[:take])
        for m in members[:take]:
            remaining.remove(m)
    return chosen


def family_clusters(sample: List[dict]) -> Dict[str, List[dict]]:
    fams: Dict[str, List[dict]] = {}
    for s in sample:
        fams.setdefault(family_of(s), []).append(s)
    return fams


def keyed_id(key: bytes, domain: bytes, raw_id: str) -> str:
    return hmac.new(key, domain + raw_id.encode("utf-8"), hashlib.sha256).hexdigest()


def oversupply_formula(target_n: int) -> int:
    """§3: oversupply_N = ceil(target/base_rate - target)."""
    return math.ceil(target_n / STRICT_BASE_RATE_ESTIMATE - target_n)


def build_manifest(
    pool: List[dict], size: int, seed: int, key: bytes,
    pilot_size: Optional[int] = None,
) -> Tuple[dict, Optional[dict]]:
    rng = random.Random(seed)
    median = frame_median(pool)

    eligible_pool = [s for s in pool if eligible(s, CUTOFF_EPOCH)]
    excluded = [
        {
            "session_hmac": keyed_id(key, SESSION_DOMAIN, s["id"]),
            "reason": (
                "pre_cutoff" if s.get("started_at", 0.0) < CUTOFF_EPOCH
                else "under_min_messages"
            ),
        }
        for s in pool if not eligible(s, CUTOFF_EPOCH)
    ]

    sample = stratified_sample(eligible_pool, size, median, rng)
    fams = family_clusters(sample)

    # stratum family counts (§3: >= FAMILY_MIN, else SHORT disclosed)
    # A family is ONE cluster: its stratum is the deterministic family-level
    # majority rule (assign_stratum_sessions), not per-member counts.
    fam_strata: Dict[str, set] = {st: set() for st in STRATA}
    for fraw, members in fams.items():
        st = assign_stratum_sessions(members, median)
        fam_strata[st].add(fraw)
    strata_counts = {st: 0 for st in STRATA}
    for s in sample:
        strata_counts[assign_stratum(s, median)] += 1

    # alternates: 2 per stratum from the unchosen eligible pool (§3)
    chosen_ids = {s["id"] for s in sample}
    alternates: Dict[str, List[str]] = {st: [] for st in STRATA}
    for st in STRATA:
        cand = [
            s for s in eligible_pool
            if s["id"] not in chosen_ids and assign_stratum(s, median) == st
        ]
        rng.shuffle(cand)
        alternates[st] = [
            keyed_id(key, SESSION_DOMAIN, s["id"]) for s in cand[:2]
        ]

    snapshot_digest = hashlib.sha256(
        json.dumps(sorted(s["id"] for s in pool)).encode("utf-8")
    ).hexdigest()
    script_sha = hashlib.sha256(
        open(__file__, "rb").read()
    ).hexdigest()

    target_stratum_n = max(strata_counts.values()) if strata_counts else 0
    manifest = {
        "version": 2,
        "plan_version": "v5.4",
        "seed": seed,
        "corpus_size": size,
        "cutoff_epoch": CUTOFF_EPOCH,
        "cutoff_iso": "2026-08-21T02:40:00Z",
        "eligibility": {"min_messages": MIN_MESSAGES, "temporal_holdout": "post_OC00.5_snapshot"},
        "snapshot_digest": snapshot_digest,
        "selection_script_sha256": script_sha,
        "frame_median_input_tokens": median,
        "frame_counts": {
            "total": len(pool),
            "eligible": len(eligible_pool),
            "excluded": len(excluded),
        },
        "strata": strata_counts,
        "strata_families": {
            st: {
                "families": len(fam_strata[st]),
                "family_min_required": FAMILY_MIN,
                "status": "OK" if len(fam_strata[st]) >= FAMILY_MIN
                else f"SHORT({FAMILY_MIN - len(fam_strata[st])})",
            }
            for st in STRATA
        },
        "alternates_per_stratum": alternates,
        "oversupply": {
            "formula": "ceil(target_stratum_N / base_rate_estimate - target_stratum_N)",
            "base_rate_estimate": STRICT_BASE_RATE_ESTIMATE,
            "base_rate_source": STRICT_SOURCE,
            "oversupply_N": oversupply_formula(target_stratum_n),
        },
        "sessions": [
            {
                "session_hmac": keyed_id(key, SESSION_DOMAIN, s["id"]),
                "family_hmac": keyed_id(key, FAMILY_DOMAIN, family_of(s)),
                "stratum": assign_stratum(s, median),
            }
            for s in sorted(sample, key=lambda x: x["id"])
        ],
        "excluded_sessions": excluded,
    }

    pilot_manifest = None
    if pilot_size:
        prng = random.Random(seed + 1)
        pilot = select_pilot(sample, pilot_size, median, prng)
        pilot_manifest = {
            "version": 2,
            "plan_version": "v5.4",
            "kind": "pilot",
            "parent_manifest_snapshot_digest": snapshot_digest,
            "seed": seed + 1,
            "corpus_size": pilot_size,
            "strata": {
                st: sum(1 for s in pilot if assign_stratum(s, median) == st)
                for st in STRATA
            },
            "sessions": [
                {
                    "session_hmac": keyed_id(key, SESSION_DOMAIN, s["id"]),
                    "family_hmac": keyed_id(key, FAMILY_DOMAIN, family_of(s)),
                    "stratum": assign_stratum(s, median),
                }
                for s in sorted(pilot, key=lambda x: x["id"])
            ],
        }
    return manifest, pilot_manifest


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--snapshot", required=True, help="pre-label redacted state JSON")
    ap.add_argument("--size", type=int, required=True, choices=[48, 72, 96, 192, 384])
    ap.add_argument("--pilot", type=int, default=None, help="pilot subset size (§5.2b)")
    ap.add_argument("--pilot-out", default=None)
    ap.add_argument("--seed", type=int, default=SEED_DEFAULT)
    ap.add_argument("--key-hex", required=True, help="HMAC key (hex, LOCAL-ONLY)")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    with open(args.snapshot, "r", encoding="utf-8") as fh:
        pool = json.load(fh)["sessions"]
    manifest, pilot = build_manifest(
        pool, args.size, args.seed, bytes.fromhex(args.key_hex), args.pilot
    )

    text = json.dumps(manifest, indent=2, sort_keys=True)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(text)
        print(f"manifest written: {args.out} (sessions={len(manifest['sessions'])})")
    else:
        print(text)

    if pilot is not None:
        ptext = json.dumps(pilot, indent=2, sort_keys=True)
        if args.pilot_out:
            with open(args.pilot_out, "w", encoding="utf-8") as fh:
                fh.write(ptext)
            print(f"pilot manifest written: {args.pilot_out} (sessions={len(pilot['sessions'])})")
        else:
            print(ptext)


if __name__ == "__main__":
    main()
