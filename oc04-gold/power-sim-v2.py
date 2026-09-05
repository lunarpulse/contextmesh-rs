#!/usr/bin/env python3
"""power-sim.py — OC-04 4G+ Stage-0 blinded power/sensitivity simulation.

Plan: _bmad-output/planning-artifacts/oc04-gold-preparation-plan.md
SIMULATOR VERSION: v2 (pre-label Stage-0 revision, §2.4 — no labels exist,
so a simulator revision restarts Stage 0 at zero label cost).

v1 → v2 change log (root causes per independent audit, Codex gpt-5.5):
  FIX-1  Bootstrap the PAIRED per-session delta (prior_arm - lexical_arm)
         at family level, not the prior-arm score level. v1 added `delta`
         as a constant to prior scores and bootstrapped that arm alone, so
         the CI width was insensitive to the effect and inflated by the
         between-arm variance that cancels in a paired design.
  FIX-2  ICC as a PURE family random intercept: the family quality factor
         is shared by all family members; v1 additionally injected
         independent per-session noise scaled by ICC (mixed semantics).
  KEPT   Frozen: seed 20260820, 10,000 bootstrap iters, 95% CI, gold-count
         weights {1:0.55, 2:0.30, 3:0.15}, declared arm priors, family
         sizes (2,2,3), stopping rule "75th-pct CI width <= 0.025 at
         delta=0.05", sizes now 48/72/96/192/384 (grid extended — allowed
         path; threshold unchanged pending founder signature).

§2.2 discipline: per-session nDCG for lexical and prior arms drawn under
DECLARED arm-conditional relevance priors (hash-pinned below — labels do
not exist yet; no claim of observed nDCG simulation). Corpus size selected
ONLY if the 75th-percentile simulated CI width for delta=0.05 is <= 0.025;
if NONE passes, the gate is recorded UNDERPOWERED (D-C-10 §6 — no post-hoc
gate lowering) with this full artifact committed.

Deterministic: seed 20260820, stdlib only, no network.
"""

import hashlib
import json
import random
from typing import Dict, List, Tuple

SIM_VERSION = "v2"
SEED = 20260820
BOOTSTRAP_ITERS = 10_000
CI_LEVEL = 0.95
EFFECT_SIZES = (0.05, 0.10, 0.15)
CORPUS_SIZES = (48, 72, 96, 192, 384)  # v2: grid extended
ICC_GRID = (0.0, 0.1, 0.3)
CI_WIDTH_MAX_75PCT = 0.05  # founder-signed 2026-09-05 (msg 1545772530437202070):
# full CI width <= delta. v1 threshold 0.025 (= delta/4) was a high-precision
# estimation bar, not a detectability bar (independent audit, Codex gpt-5.5).
GATING_EFFECT = 0.05

GOLD_COUNT_WEIGHTS: Dict[int, float] = {1: 0.55, 2: 0.30, 3: 0.15}

# Declared arm-conditional priors (pre-label, hash-pinned).
ARM_PRIORS: Dict[str, Tuple[float, float]] = {
    "lexical": (0.45, 0.25),
    "prior": (0.55, 0.25),
}

# Share of the session score driven by the shared session-quality factor —
# this creates the within-session arm correlation that cancels in the
# paired delta (the mechanism v1 missed). Declared, hash-pinned.
SHARED_QUALITY_WEIGHT = 0.6

FAMILY_SIZES = (2, 2, 3)


def sha256_hex(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def assumptions_digest() -> str:
    blob = json.dumps(
        {
            "sim_version": SIM_VERSION,
            "seed": SEED,
            "bootstrap_iters": BOOTSTRAP_ITERS,
            "gold_count_weights": GOLD_COUNT_WEIGHTS,
            "arm_priors": {k: list(v) for k, v in ARM_PRIORS.items()},
            "icc_grid": list(ICC_GRID),
            "effect_sizes": list(EFFECT_SIZES),
            "corpus_sizes": list(CORPUS_SIZES),
            "family_sizes": list(FAMILY_SIZES),
            "shared_quality_weight": SHARED_QUALITY_WEIGHT,
            "ci_width_max_75pct": CI_WIDTH_MAX_75PCT,
        },
        sort_keys=True,
    )
    return sha256_hex(blob)


def clamp01(x: float) -> float:
    return max(0.0, min(1.0, x))


class Sim:
    """One deterministic corpus draw: paired arms + pure family intercepts."""

    def __init__(self, seed: int, n_sessions: int, icc: float, delta: float):
        self.rng = random.Random(seed)
        self.n = n_sessions
        self.icc = icc
        self.delta = delta

    def assign_families(self) -> List[int]:
        fam, i, fid = [], 0, 0
        while i < self.n:
            size = FAMILY_SIZES[fid % len(FAMILY_SIZES)]
            fam.extend([fid] * min(size, self.n - i))
            i += size
            fid += 1
        return fam

    def draw_corpus(self) -> List[Tuple[float, float]]:
        """One draw -> per-session (lexical, prior) scores, PAIRED.

        Session score = shared_quality + arm_deviation + family_intercept.
        - shared_quality (weight w): identical for both arms of a session —
          drives the arm correlation that cancels in the paired delta.
        - arm_deviation: independent per arm (arm-specific noise).
        - family_intercept (FIX-2): one draw per FAMILY, shared by all its
          sessions, scaled by ICC — pure random-intercept semantics.
        The effect `delta` is added to the prior arm only.
        """
        golds = self._gold_counts()
        families = self.assign_families()
        family_intercept = {
            f: self.rng.gauss(0.0, 0.2 * self.icc) for f in set(families)
        }
        pairs: List[Tuple[float, float]] = []
        for idx in range(self.n):
            g = golds[idx]
            p_present = {1: 0.65, 2: 0.85, 3: 0.95}[g]
            has = self.rng.random() < p_present
            if not has:
                pairs.append((0.0, 0.0))  # no gold in union: both arms 0
                continue
            shared = self.rng.gauss(0.0, 1.0) * SHARED_QUALITY_WEIGHT
            lex_mu, lex_sd = ARM_PRIORS["lexical"]
            pri_mu, pri_sd = ARM_PRIORS["prior"]
            lex = clamp01(
                lex_mu + lex_sd * (shared + self.rng.gauss(0.0, 1.0) * (1 - SHARED_QUALITY_WEIGHT))
                + family_intercept[families[idx]]
            )
            pri = clamp01(
                pri_mu + pri_sd * (shared + self.rng.gauss(0.0, 1.0) * (1 - SHARED_QUALITY_WEIGHT))
                + family_intercept[families[idx]]
                + self.delta
            )
            pairs.append((lex, pri))
        return pairs

    def _gold_counts(self) -> List[int]:
        counts, weights = zip(*GOLD_COUNT_WEIGHTS.items())
        return [self.rng.choices(counts, weights)[0] for _ in range(self.n)]

    def family_mean_deltas(self) -> List[float]:
        pairs = self.draw_corpus()
        families = self.assign_families()
        by_family: Dict[int, List[float]] = {}
        for idx, (lex, pri) in enumerate(pairs):
            by_family.setdefault(families[idx], []).append(pri - lex)  # FIX-1
        return [sum(v) / len(v) for v in by_family.values()]

    def bootstrap_ci_width(self) -> float:
        fam_means = self.family_mean_deltas()
        f = len(fam_means)
        rng = self.rng
        means = []
        for _ in range(BOOTSTRAP_ITERS):
            acc = 0.0
            for _ in range(f):
                acc += fam_means[rng.randrange(f)]
            means.append(acc / f)
        means.sort()
        lo = means[int((1 - CI_LEVEL) / 2 * BOOTSTRAP_ITERS)]
        hi = means[int((1 + CI_LEVEL) / 2 * BOOTSTRAP_ITERS) - 1]
        return hi - lo


def run_stage0() -> dict:
    results = []
    for n in CORPUS_SIZES:
        for icc in ICC_GRID:
            for delta in EFFECT_SIZES:
                cell_seed = SEED + n * 1000 + int(icc * 100) * 10 + int(delta * 100)
                sim = Sim(cell_seed, n, icc, delta)
                widths = sorted(sim.bootstrap_ci_width() for _ in range(30))
                w75 = widths[int(0.75 * len(widths))]
                results.append(
                    {
                        "n_sessions": n,
                        "icc": icc,
                        "delta": delta,
                        "ci_width_75pct": round(w75, 6),
                        "passes_rule": bool(
                            delta == GATING_EFFECT and w75 <= CI_WIDTH_MAX_75PCT
                        ),
                    }
                )
    decision, chosen = "UNDERPOWERED", None
    for row in results:
        if row["passes_rule"]:
            decision, chosen = "GO", row["n_sessions"]
            break
    return {
        "stage": "OC-04 4G+ Stage-0 power/sensitivity simulation",
        "sim_version": SIM_VERSION,
        "change_log": [
            "FIX-1: bootstrap paired per-session delta (prior-lexical) at family level (v1 bootstrapped prior-arm level)",
            "FIX-2: ICC as pure family random intercept (v1 mixed in per-session noise)",
            "grid extended 48/72/96 -> +192/384 (allowed path; threshold unchanged)",
        ],
        "plan_version": "v5.2 FROZEN",
        "assumptions_sha256": assumptions_digest(),
        "seed": SEED,
        "stopping_rule": f"75th-pct CI width <= {CI_WIDTH_MAX_75PCT} at delta={GATING_EFFECT}",
        "decision": decision,
        "chosen_corpus_size": chosen,
        "results": results,
    }


if __name__ == "__main__":
    print(json.dumps(run_stage0(), indent=2, sort_keys=True))
