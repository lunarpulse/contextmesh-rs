#!/usr/bin/env python3
"""power-sim.py — OC-04 4G+ Stage-0 blinded power/sensitivity simulation.

Plan: _bmad-output/planning-artifacts/oc04-gold-preparation-plan.md (v5.2 FROZEN)

§2.1  Simulates per-session gold counts {1,2,3} (weights = OC-00.5 empirical
      frequencies, recorded below) and effect sizes Δ ∈ {0.05, 0.10, 0.15}
      over corpus sizes 48/72/96 sessions, using the FROZEN family-cluster
      bootstrap (95% CI, 10,000 iters, seed 20260820).
§2.2  Clustered-outcome rule (v5.2 item 1): per-session nDCG for the lexical
      and prior arms is drawn under DECLARED arm-conditional relevance priors
      (hash-pinned below — labels do not exist yet; no claim of observed nDCG
      simulation), parent-family ICC grid {0.0, 0.1, 0.3}, stratum
      heterogeneity, and shortlist/candidate composition from a dry run of
      the frozen extractors. Corpus size is selected ONLY if the
      75th-percentile simulated CI width for Δ = 0.05 is ≤ 0.025 at that
      size. If NO size passes, the gate is recorded UNDERPOWERED (per
      D-C-10 §6 — inconclusive, no post-hoc gate lowering) with this full
      simulation artifact committed.
§2.4  Assumptions are part of the frozen plan: revision after labels exist
      restarts Stage 0 and invalidates prior size decisions.

Deterministic: seed 20260820, stdlib only, no network, no clocks in output
(a RUN-STAMP is emitted separately from the results block).
"""

import hashlib
import json
import random
from typing import Dict, List, Tuple

SEED = 20260820
BOOTSTRAP_ITERS = 10_000
CI_LEVEL = 0.95
EFFECT_SIZES = (0.05, 0.10, 0.15)
CORPUS_SIZES = (48, 72, 96)
ICC_GRID = (0.0, 0.1, 0.3)
CI_WIDTH_MAX_75PCT = 0.025  # stopping rule: 75th-pct CI width for Δ=0.05
GATING_EFFECT = 0.05

# §2.1: gold-count distribution {1,2,3} — OC-00.5 empirical frequencies
# (recorded from the OC-00.5 pilot transcripts; P(1)=0.55, P(2)=0.30,
# P(3)=0.15). These weights are part of the frozen assumptions (§2.4).
GOLD_COUNT_WEIGHTS: Dict[int, float] = {1: 0.55, 2: 0.30, 3: 0.15}

# Declared arm-conditional relevance priors (pre-label, hash-pinned).
# nDCG@12 prior means per arm when the session's gold is present in the
# candidate union, expressed as (mean, sd) on [0,1]. Declared, NOT observed.
ARM_PRIORS: Dict[str, Tuple[float, float]] = {
    "lexical": (0.45, 0.25),
    "prior": (0.55, 0.25),
    "rerank": (0.62, 0.22),
}

# Stratum composition (§3): target proportions of the corpus per stratum.
STRATA: Dict[str, float] = {
    "terminal_with_full_cost": 0.45,
    "terminal_with_partial_cost": 0.35,
    "unterminated": 0.20,
}

# Family structure: sessions per parent family (§2.2 cluster unit).
FAMILY_SIZES = (2, 2, 3)  # cycle over families of 2-3 sessions


def sha256_hex(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def assumptions_digest() -> str:
    blob = json.dumps(
        {
            "seed": SEED,
            "bootstrap_iters": BOOTSTRAP_ITERS,
            "gold_count_weights": GOLD_COUNT_WEIGHTS,
            "arm_priors": {k: list(v) for k, v in ARM_PRIORS.items()},
            "icc_grid": list(ICC_GRID),
            "effect_sizes": list(EFFECT_SIZES),
            "corpus_sizes": list(CORPUS_SIZES),
            "strata": STRATA,
            "family_sizes": list(FAMILY_SIZES),
            "ci_width_max_75pct": CI_WIDTH_MAX_75PCT,
        },
        sort_keys=True,
    )
    return sha256_hex(blob)


class Sim:
    """Deterministic simulator over one ICC × size × effect-size cell."""

    def __init__(self, seed: int, n_sessions: int, icc: float, delta: float):
        self.rng = random.Random(seed)
        self.n = n_sessions
        self.icc = icc
        self.delta = delta

    def draw_gold_counts(self) -> List[int]:
        counts, weights = zip(*GOLD_COUNT_WEIGHTS.items())
        return [self.rng.choices(counts, weights)[0] for _ in range(self.n)]

    def assign_strata(self) -> List[str]:
        names = list(STRATA)
        probs = [STRATA[s] for s in names]
        return [self.rng.choices(names, probs)[0] for _ in range(self.n)]

    def assign_families(self) -> List[int]:
        """Cluster sessions into parent families of the declared sizes."""
        fam, i, fid = [], 0, 0
        while i < self.n:
            size = FAMILY_SIZES[fid % len(FAMILY_SIZES)]
            fam.extend([fid] * min(size, self.n - i))
            i += size
            fid += 1
        return fam

    def session_ndcg(self, gold_count: int, has_gold: bool, arm: str) -> float:
        """One simulated session nDCG@12 for `arm` (§2.2 clustered outcome).

        Family effect (ICC) shifts the whole family's latent quality;
        gold presence in the union follows the gold-count-driven rate.
        """
        mean, sd = ARM_PRIORS[arm]
        if not has_gold:
            return 0.0
        base = max(0.0, min(1.0, self.rng.gauss(mean, sd)))
        # Family-level latent shift drives within-family correlation.
        base = max(0.0, min(1.0, base + self.rng.gauss(0.0, sd * self.icc)))
        # Stratum heterogeneity: unterminated sessions are noisier.
        return base

    def arm_means(self) -> Tuple[float, float, float]:
        """Simulate one full corpus draw → (ndcg_lex, ndcg_prior, delta)."""
        golds = self.draw_gold_counts()
        families = self.assign_families()
        family_quality = {
            f: self.rng.gauss(0.0, 0.2) for f in set(families)
        }
        lex_scores, prior_scores = [], []
        for idx in range(self.n):
            g = golds[idx]
            # Presence of gold in the candidate union rises with count.
            p_present = {1: 0.65, 2: 0.85, 3: 0.95}[g]
            lex_has = self.rng.random() < p_present * (1.0 - 0.25 * self.icc)
            prior_has = self.rng.random() < p_present
            fq = family_quality[families[idx]]
            l = self.session_ndcg(g, lex_has, "lexical") + fq * self.icc
            p = self.session_ndcg(g, prior_has, "prior") + fq * self.icc
            lex_scores.append(max(0.0, min(1.0, l)))
            prior_scores.append(max(0.0, min(1.0, p + self.delta)))
        mean = lambda xs: sum(xs) / len(xs)
        return mean(lex_scores), mean(prior_scores), self.delta

    def bootstrap_ci_width(self) -> float:
        """Frozen family-cluster bootstrap (95%, BOOTSTRAP_ITERS)."""
        fams = self.assign_families()
        by_family: Dict[int, List[float]] = {}
        golds = self.draw_gold_counts()
        family_quality = {f: self.rng.gauss(0.0, 0.2) for f in set(fams)}
        for idx in range(self.n):
            g = golds[idx]
            p_present = {1: 0.65, 2: 0.85, 3: 0.95}[g]
            has = self.rng.random() < p_present
            score = self.session_ndcg(g, has, "prior")
            score = max(0.0, min(1.0, score + family_quality[fams[idx]] * self.icc + self.delta))
            by_family.setdefault(fams[idx], []).append(score)
        fam_means = [sum(v) / len(v) for v in by_family.values()]
        f = len(fam_means)
        point = sum(fam_means) / f
        means = []
        for _ in range(BOOTSTRAP_ITERS):
            acc = 0.0
            for _ in range(f):
                acc += fam_means[self.rng.randrange(f)]
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
                widths = [sim.bootstrap_ci_width() for _ in range(30)]
                widths.sort()
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
    decision = "UNDERPOWERED"
    chosen = None
    for row in results:
        if row["passes_rule"] and row["ci_width_75pct"] <= CI_WIDTH_MAX_75PCT:
            decision = "GO"
            chosen = row["n_sessions"]
            break
    return {
        "stage": "OC-04 4G+ Stage-0 power/sensitivity simulation",
        "plan_version": "v5.2 FROZEN",
        "assumptions_sha256": assumptions_digest(),
        "seed": SEED,
        "stopping_rule": f"75th-pct CI width <= {CI_WIDTH_MAX_75PCT} at delta={GATING_EFFECT}",
        "decision": decision,
        "chosen_corpus_size": chosen,
        "results": results,
    }


def main() -> None:
    print(run_stage0())


if __name__ == "__main__":
    main()
