//! OC-03 Stage 3E propagation tests (matrix rows OC03-P01..P14).
//!
//! The §7.6 recurrence is integer-only (u128 checked, no floats); its math
//! was independently verified (algebra + 200k brute-force cases) before
//! this implementation.

use contextmesh_salience::prior::{
    EntityGraphV1, PriorConfigV1, PriorSeedSetV1, SessionPayloads, build_entity_graph,
    derive_entity_keys, run_ppr,
};

/// Two-entity helper graph: `alpha` — `beta` (mutually adjacent).
fn two_entity_graph() -> EntityGraphV1 {
    let session = SessionPayloads::from_payloads(vec!["alpha beta"]);
    build_entity_graph(std::slice::from_ref(&session), &PriorConfigV1::default()).unwrap()
}

fn seeds_of(pairs: &[(&str, u128)]) -> PriorSeedSetV1 {
    PriorSeedSetV1::new_for_test(
        pairs
            .iter()
            .map(|(e, p)| contextmesh_salience::prior::PriorSeedV1::new_for_test(e, *p))
            .collect(),
        pairs.iter().map(|(e, _)| (*e).to_owned()).collect(),
        0,
        PriorConfigV1::default().config_hash().unwrap(),
    )
}

#[test]
fn ppr_teleport_floor() {
    // P01: teleport = floor(seed × 850000/1e6). Seed 1,000,000,000 ppb →
    // 850,000,000 ppb exactly; seed 1,999,999 ppb floors to 1,699,999 ppb
    // (1,999,999 × 0.85 = 1,699,999.15).
    let graph = two_entity_graph();
    let seeds = seeds_of(&[("alpha", 1_000_000_000), ("beta", 1_999_999)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    let alpha = out.vector().iter().find(|(e, _)| e == "alpha").unwrap();
    assert!(
        alpha.1 >= 850_000_000,
        "alpha mass ≥ teleport floor: {alpha:?}"
    );
    // Isolated check via a graph where beta has degree 1 and alpha 1: the
    // exact m_0 is pinned by P06 instead; here assert the floor relation.
    let beta0 = 1_999_999u128 * 850_000 / 1_000_000;
    let beta = out.vector().iter().find(|(e, _)| e == "beta").unwrap();
    assert!(
        beta.1 >= beta0,
        "beta mass ≥ teleport floor {beta0}: {beta:?}"
    );
}

#[test]
fn ppr_neighbor_floor() {
    // P02: hand-computed propagation. An independent test-side simulation
    // of the §7.6 recurrence (teleport once, per-neighbor floor, iterate to
    // the L∞ stop) must reproduce run_ppr's final vector exactly; the
    // first-step neighbor term for beta is floor(m_0(alpha)·C/1e12) with
    // out=1 → floor(850_000_000 × 0.15) = 127_500_000, and the fixed point
    // strictly exceeds it (mass keeps flowing while the delta budget lasts).
    let graph = two_entity_graph();
    let seeds = seeds_of(&[("alpha", 1_000_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();

    let c = 1_000_000_000_000u128 - 850_000u128 * 1_000_000;
    let t_alpha = 1_000_000_000u128 * 850_000 / 1_000_000;
    let first_term = t_alpha * c / 1_000_000_000_000; // one step, out=1
    assert_eq!(first_term, 127_500_000, "hand-computed first neighbor term");

    // Independent recurrence to convergence.
    let (mut ma, mut mb) = (t_alpha, 0u128);
    loop {
        let na = t_alpha + mb * c / 1_000_000_000_000;
        let nb = ma * c / 1_000_000_000_000;
        let d = (na.abs_diff(ma)).max(nb.abs_diff(mb));
        ma = na;
        mb = nb;
        if d <= 1_000_000 {
            break;
        }
    }
    let alpha = out.vector().iter().find(|(e, _)| e == "alpha").unwrap();
    let beta = out.vector().iter().find(|(e, _)| e == "beta").unwrap();
    assert_eq!((alpha.1, beta.1), (ma, mb), "fixed point reproduced");
    assert!(beta.1 > first_term, "converged mass exceeds one-step term");
}

#[test]
fn ppr_summation_order() {
    // P03: permuted input sessions produce identical canonical output.
    let s1 = SessionPayloads::from_payloads(vec!["a b", "b c", "c d"]);
    let s2 = SessionPayloads::from_payloads(vec!["c d", "a b", "b c"]);
    let g1 = build_entity_graph(&[s1], &PriorConfigV1::default()).unwrap();
    let g2 = build_entity_graph(&[s2], &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("a", 500_000_000)]);
    let o1 = run_ppr(&g1, &seeds, &PriorConfigV1::default()).unwrap();
    let o2 = run_ppr(&g2, &seeds, &PriorConfigV1::default()).unwrap();
    assert_eq!(o1.vector(), o2.vector(), "order-independent bytes");
    assert_eq!(o1.residual_ppb(), o2.residual_ppb());
}

#[test]
fn ppr_convergence_stop() {
    // P04: a small graph converges with converged=true and the recorded
    // iteration count respects the cap.
    let graph = two_entity_graph();
    let seeds = seeds_of(&[("alpha", 800_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    assert!(out.converged(), "two-node graph converges");
    assert!(out.iterations() <= 64);
}

#[test]
fn ppr_iteration_cap() {
    // P05: bound contract — iterations never exceed 64 and the converged
    // flag is recorded honestly. Within the frozen caps there is no known
    // non-converging fixture (masses are monotone non-decreasing and the
    // per-hop floor damping decays distant mass to zero), so the cap branch
    // is asserted as a bound, not exercised by a diverging fixture.
    let mut payloads = Vec::new();
    for i in 0..40u32 {
        let next = (i + 1) % 40;
        payloads.push(format!("n{i:02} n{next:02}"));
    }
    let refs: Vec<&str> = payloads.iter().map(String::as_str).collect();
    let session = SessionPayloads::from_payloads(refs);
    let graph =
        build_entity_graph(std::slice::from_ref(&session), &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("n00", 1_000_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    assert!(out.iterations() <= 64, "cap bound holds");
    // The fixture converges early; converged=true is the honest record.
    assert!(out.converged());
}

#[test]
fn ppr_isolated_entity() {
    // P06: degree-0 entity retains exactly its teleport mass. The lone
    // entity sits in its OWN session (co-occurrence is per session).
    let pair = SessionPayloads::from_payloads(vec!["alpha beta"]);
    let alone = SessionPayloads::from_payloads(vec!["lone"]);
    let graph = build_entity_graph(&[pair, alone], &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("lone", 700_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    let lone = out.vector().iter().find(|(e, _)| e == "lone").unwrap();
    assert_eq!(lone.1, 595_000_000, "mass = teleport exactly");
    assert!(!out.vector().iter().any(|(e, _)| e == "alpha"));
}

#[test]
fn ppr_isolated_no_outflow() {
    // P07: removing an isolated entity leaves all other masses byte-identical.
    let pair = SessionPayloads::from_payloads(vec!["alpha beta"]);
    let alone = SessionPayloads::from_payloads(vec!["lone"]);
    let g1 = build_entity_graph(&[pair.clone(), alone], &PriorConfigV1::default()).unwrap();
    let g2 = build_entity_graph(&[pair], &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("alpha", 900_000_000), ("lone", 123_456_789)]);
    let o1 = run_ppr(&g1, &seeds, &PriorConfigV1::default()).unwrap();
    let o2 = run_ppr(&g2, &seeds, &PriorConfigV1::default()).unwrap();
    let v1: Vec<&(String, u128)> = o1.vector().iter().filter(|(e, _)| e != "lone").collect();
    let v1_owned: Vec<(String, u128)> = v1.into_iter().cloned().collect();
    assert_eq!(
        v1_owned,
        o2.vector(),
        "isolated entity contributes no outflow"
    );
}

#[test]
fn ppr_overflow_fail_closed() {
    // P08: hub concentration from fully-legal inputs drives one vector
    // entry above PRIOR_MAX_PPB → Err(Malformed), never a silent clamp.
    // Reviewer-verified counterexample: hub (degree 32 = cap) + 32 leaves,
    // every leaf seeded at the per-seed clamp 1e9 → hub ≈ 4.17e9 > 1e9.
    let hub = "a_hub"; // sorts first: hub edges lead canonical order
    // A STAR needs one session per hub-leaf pair — a single session would
    // union into complete K33 where symmetry caps every mass at 1e9.
    let mut sessions = Vec::new();
    for i in 0..32u32 {
        sessions.push(SessionPayloads::from_payloads(vec![Box::leak(
            format!("a_hub l{i:02}").into_boxed_str(),
        )]));
    }
    let graph = build_entity_graph(&sessions, &PriorConfigV1::default()).unwrap();
    let mut seed_pairs: Vec<(&str, u128)> = vec![(hub, 1_000_000_000)];
    for i in 0..32u32 {
        seed_pairs.push((
            Box::leak(format!("l{i:02}").into_boxed_str()),
            1_000_000_000,
        ));
    }
    let seeds = seeds_of(&seed_pairs);
    let result = run_ppr(&graph, &seeds, &PriorConfigV1::default());
    assert!(result.is_err(), "hub concentration must fail closed");
    // And the error is Malformed, with no partial artifact returned.
    assert!(matches!(
        result,
        Err(contextmesh_salience::error::OutcomeError::Malformed)
    ));
    // Sanity: a mild star (hub-only seed) stays in range → Ok.
    let mild = seeds_of(&[(hub, 1_000_000_000)]);
    let out = run_ppr(&graph, &mild, &PriorConfigV1::default()).unwrap();
    for (_, m) in out.vector() {
        assert!(*m <= 1_000_000_000, "range holds for well-formed input");
    }
}

#[test]
fn ppr_no_float() {
    // P09: no f32/f64 tokens in non-comment lines of src/prior.rs.
    let src = include_str!("../src/prior.rs");
    for (n, line) in src.lines().enumerate() {
        let code = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        assert!(
            !code.contains("f32")
                && !code.contains("f64")
                && !code.contains("0.0")
                && !code.contains("1.0"),
            "float token on line {}: {}",
            n + 1,
            line
        );
    }
}

#[test]
fn ppr_vector_ordering() {
    // P10: vector lists ppb>0 only, entity byte order.
    let session = SessionPayloads::from_payloads(vec!["delta alpha", "charlie bravo"]);
    let graph =
        build_entity_graph(std::slice::from_ref(&session), &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("alpha", 600_000_000), ("bravo", 400_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    let entities: Vec<&str> = out.vector().iter().map(|(e, _)| e.as_str()).collect();
    let mut sorted = entities.clone();
    sorted.sort_unstable();
    assert_eq!(entities, sorted, "byte order");
    assert!(out.vector().iter().all(|(_, m)| *m > 0), "only >0 entries");
}

#[test]
fn ppr_range_assert() {
    // P11: every vector value ≤ PRIOR_MAX_PPB (impossible-by-construction,
    // asserted anyway — run_ppr fails closed above; here assert outputs).
    let graph = two_entity_graph();
    let seeds = seeds_of(&[("alpha", 1_000_000_000)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    for (_, m) in out.vector() {
        assert!(*m <= 1_000_000_000);
    }
}

#[test]
fn ppr_empty_seeds() {
    // P12: empty seed set → empty vector, valid computation.
    let graph = two_entity_graph();
    let seeds = seeds_of(&[]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    assert!(out.vector().is_empty(), "no seeds → no mass");
}

#[test]
fn ppr_determinism() {
    // P13: 20 reruns → identical outcomes (vector, iterations, residual).
    let session = SessionPayloads::from_payloads(vec!["a b", "b c", "c a"]);
    let graph =
        build_entity_graph(std::slice::from_ref(&session), &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("a", 650_000_000)]);
    let first = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
    for _ in 0..20 {
        let again = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();
        assert_eq!(first.vector(), again.vector());
        assert_eq!(first.iterations(), again.iterations());
        assert_eq!(first.residual_ppb(), again.residual_ppb());
    }
}

#[test]
fn ppr_residual_recorded() {
    // P14: residual_ppb equals hand-computed ⌊Σ r_u / 1e12⌋ over the final
    // iteration, with n_u = m_final(u)·C, d_u = 1e12·out(u).
    let session = SessionPayloads::from_payloads(vec!["alpha beta", "beta charlie"]);
    let graph =
        build_entity_graph(std::slice::from_ref(&session), &PriorConfigV1::default()).unwrap();
    let seeds = seeds_of(&[("alpha", 731_923_456)]);
    let out = run_ppr(&graph, &seeds, &PriorConfigV1::default()).unwrap();

    // Independent recomputation from the final masses.
    let c = 1_000_000_000_000u128 - 850_000u128 * 1_000_000;
    let degrees: Vec<(&str, usize)> = vec![("alpha", 1), ("beta", 2), ("charlie", 1)];
    let mut sum = 0u128;
    for (entity, deg) in degrees {
        let m = out
            .vector()
            .iter()
            .find(|(e, _)| e == entity)
            .map_or(0u128, |(_, m)| *m);
        let n_u = m * c;
        let d_u = 1_000_000_000_000u128 * u128::try_from(deg).unwrap();
        sum += n_u % d_u;
    }
    let expected = sum / 1_000_000_000_000;
    assert_eq!(out.residual_ppb(), expected, "exact residual identity");
}

// Silence unused-import churn for helper used by later stages.
#[allow(dead_code)]
fn _keep(e: &str) -> Vec<String> {
    derive_entity_keys(e)
}
