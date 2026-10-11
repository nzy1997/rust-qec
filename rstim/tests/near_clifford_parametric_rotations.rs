use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordExecutor,
};

// Independent dense matrices in the computational basis, without production
// tableau, Pauli, coordinate, scheduling or rotation helpers.
type Amp = (f64, f64);
fn product(a: Amp, b: Amp) -> Amp {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn sum(a: Amp, b: Amp) -> Amp {
    (a.0 + b.0, a.1 + b.1)
}
fn single(state: &mut [Amp; 8], q: usize, m: [[Amp; 2]; 2]) {
    for a in 0..8 {
        if a & (1 << q) != 0 {
            continue;
        }
        let b = a | (1 << q);
        let old = [state[a], state[b]];
        state[a] = sum(product(m[0][0], old[0]), product(m[0][1], old[1]));
        state[b] = sum(product(m[1][0], old[0]), product(m[1][1], old[1]));
    }
}
fn matrix(axis: char, alpha: f64) -> [[Amp; 2]; 2] {
    let theta = alpha * std::f64::consts::PI / 2.0;
    let c = theta.cos();
    let s = theta.sin();
    let zero = (0.0, 0.0);
    match axis {
        'X' => [[(c, 0.0), (0.0, -s)], [(0.0, -s), (c, 0.0)]],
        'Y' => [[(c, 0.0), (-s, 0.0)], [(s, 0.0), (c, 0.0)]],
        'Z' => [[(c, -s), zero], [zero, (c, s)]],
        _ => unreachable!(),
    }
}
fn cx(state: &mut [Amp; 8], control: usize, target: usize) {
    for i in 0..8 {
        if i & (1 << control) != 0 && i & (1 << target) == 0 {
            state.swap(i, i | (1 << target));
        }
    }
}

#[test]
fn entangled_parametric_circuits_match_independent_dense_joint_distribution() {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let h = [[(w, 0.0), (w, 0.0)], [(w, 0.0), (-w, 0.0)]];
    // Qubit 0 begins in |0> and is measured in Z, so generic X angles
    // change joint probabilities instead of contributing only common phase.
    let mut distributions = Vec::new();
    for alpha in [-0.3040867239846964, 0.17, 0.31] {
        let mut state = [(0.0, 0.0); 8];
        state[0] = (1.0, 0.0);
        single(&mut state, 1, h);
        single(&mut state, 0, matrix('X', alpha));
        cx(&mut state, 0, 2);
        single(&mut state, 1, matrix('Y', 0.31));
        single(&mut state, 2, matrix('Y', 0.5));
        single(&mut state, 2, matrix('Z', 0.02));
        cx(&mut state, 1, 2);
        single(&mut state, 1, matrix('X', -0.5));
        let probabilities = state.map(|a| a.0 * a.0 + a.1 * a.1);
        distributions.push(probabilities);
        assert!((probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let text = format!(
            "H 1\nR_X({alpha}) 0\nCX 0 2\nR_Y(0.31) 1\nSQRT_Y 2\nR_Z(0.02) 2\nCX 1 2\nSQRT_X_DAG 1\nM 0 1 2\n"
        );
        let incumbent = NearCliffordExecutor::compile_text(&text).unwrap();
        let n = 16384;
        let mut rng = StdRng::seed_from_u64(9141);
        let mut samples = vec![incumbent.sample(n, &mut rng).unwrap()];
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            let mut rng = StdRng::seed_from_u64(9141);
            samples.push(plan.prepare_sampler().unwrap().sample(n, &mut rng).unwrap());
        }
        for records in samples {
            let mut observed = [0usize; 8];
            for row in records {
                let i = row
                    .measurements
                    .iter()
                    .enumerate()
                    .fold(0, |v, (j, b)| v | (usize::from(*b) << j));
                observed[i] += 1;
            }
            for i in 0..8 {
                assert!(
                    (observed[i] as f64 / n as f64 - probabilities[i]).abs() < 0.025,
                    "alpha={alpha}, bin={i}, observed={}, expected={}",
                    observed[i],
                    probabilities[i]
                );
            }
        }
    }
    let difference = distributions[0]
        .iter()
        .zip(distributions[1])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(
        difference > 0.02,
        "X-angle oracle must distinguish the tested magnitudes"
    );
}

#[test]
fn half_turn_rotation_probabilities_match_analytic_single_qubit_answers() {
    for (text, expected) in [
        (
            "R_X(0.25) 0\nM 0\n",
            (1.0 - std::f64::consts::FRAC_1_SQRT_2) / 2.0,
        ),
        (
            "R_X(-0.25) 0\nMY 0\n",
            (1.0 - std::f64::consts::FRAC_1_SQRT_2) / 2.0,
        ),
        (
            "R_Y(0.25) 0\nMX 0\n",
            (1.0 - std::f64::consts::FRAC_1_SQRT_2) / 2.0,
        ),
        (
            "H 0\nR_Z(0.25) 0\nMY 0\n",
            (1.0 - std::f64::consts::FRAC_1_SQRT_2) / 2.0,
        ),
        ("R_X(1) 0\nM 0\n", 1.0),
        ("R_Y(0) 0\nM 0\n", 0.0),
        ("R_X(2) 0\nM 0\n", 0.0),
    ] {
        let incumbent = NearCliffordExecutor::compile_text(text).unwrap();
        let mut rng = StdRng::seed_from_u64(1947);
        let n = 16_384;
        let records = incumbent.sample(n, &mut rng).unwrap();
        let actual = records.iter().filter(|r| r.measurements[0]).count() as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.02,
            "{text}: {actual} != {expected}"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut rng = StdRng::seed_from_u64(1947);
            let actual = sampler
                .sample(n, &mut rng)
                .unwrap()
                .iter()
                .filter(|r| r.measurements[0])
                .count() as f64
                / n as f64;
            assert!(
                (actual - expected).abs() < 0.02,
                "{text}: compiled {actual} != {expected}"
            );
        }
    }
}

#[test]
fn parametric_noise_feedback_and_joint_projection_preserve_counts_and_rng() {
    let text = "H 0 1 2 3\nR_X(-0.3040867239846964) 0\nR_Y(0.31) 1\nR_Z(0.02) 2\nT 3\nCX 0 1 2 3\nDEPOLARIZE2(0.17) 0 2\nMPP(0.09) X0*Y1 X2*Y3\nCX rec[-1] 1\nR_Z(-0.21) 1\nMRX(0.13) 0\nCY rec[-1] 2\nR_Y(-0.17) 2\nSQRT_X 3\nSQRT_Y_DAG 2\nM(0.07) 0 1 2 3\nDETECTOR rec[-4] rec[-3]\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        for seed in [1739, 583] {
            let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut cached = plan.prepare_sampler().unwrap();
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            let mut errors = 0;
            for shots in [1, 31, 32, 63, 64, 65, 129, 1024, 64] {
                let records = reference.sample(shots, &mut a).unwrap();
                assert_eq!(cached.sample(shots, &mut b).unwrap(), records);
                let records = reference.sample(shots, &mut a).unwrap();
                let accepted = records
                    .iter()
                    .filter(|r| r.detectors.iter().all(|v| !v))
                    .collect::<Vec<_>>();
                let logical = accepted
                    .iter()
                    .filter(|r| {
                        r.observables
                            .iter()
                            .filter(|(i, _)| *i == 0)
                            .fold(false, |v, (_, bit)| v ^ bit)
                    })
                    .count();
                let counts = cached.sample_postselected_counts(shots, 0, &mut b).unwrap();
                assert_eq!(
                    (counts.attempted, counts.accepted, counts.logical_errors),
                    (shots, accepted.len(), logical)
                );
                errors += logical;
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert!(errors > 0);
        }
    }
}

#[test]
fn inverse_and_clifford_rotation_circuits_restore_deterministic_outcomes() {
    for text in [
        "R_X(0.17) 0\nR_X(-0.17) 0\nM 0\n",
        "R_Y(-0.3040867239846964) 0\nR_Y(0.3040867239846964) 0\nM 0\n",
        "H 0\nR_Z(5.25) 0\nR_Z(-1.25) 0\nH 0\nM 0\n",
        "SQRT_X 0\nSQRT_X_DAG 0\nSQRT_Y 0\nSQRT_Y_DAG 0\nM 0\n",
        "R_Z(1e300) 0\nM 0\n",
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let mut sampler = plan.prepare_sampler().unwrap();
            let mut rng = StdRng::seed_from_u64(77);
            assert!(
                sampler
                    .sample(1024, &mut rng)
                    .unwrap()
                    .iter()
                    .all(|r| !r.measurements[0]),
                "{text}"
            );
        }
    }
}

#[test]
fn invalid_parametric_arguments_and_targets_fail_before_sampling() {
    for text in [
        "R_X 0",
        "R_Y(0.1,0.2) 0",
        "R_Z(0.1)",
        "R_X(0.1) !0",
        "R_Y(0.1) rec[-1]",
        "R_Z(inf) 0",
        "R_X(NaN) 0",
    ] {
        assert!(NearCliffordExecutor::compile_text(text).is_err(), "{text}");
        assert!(
            CompiledNearCliffordExecutor::compile_text(text).is_err(),
            "{text}"
        );
    }
}

#[test]
fn quarter_turn_rotations_require_no_active_coordinates() {
    let text =
        "R_X(0.5) 0\nR_Y(-0.5) 1\nR_Z(2.5) 2\nR_X(1) 3\nR_Y(1.5) 4\nR_Z(-2) 5\nM 0 1 2 3 4 5\n";
    let instructions = rstim::parser::parse_lines(text).unwrap();
    let incumbent = NearCliffordExecutor::compile_with_limit(instructions.clone(), 0).unwrap();
    let mut rng = StdRng::seed_from_u64(77);
    assert_eq!(incumbent.sample(129, &mut rng).unwrap().len(), 129);
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan = CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
            instructions.clone(),
            0,
            arithmetic,
        )
        .unwrap();
        assert_eq!(plan.peak_active_rank(), 0);
        let mut rng = StdRng::seed_from_u64(77);
        assert_eq!(
            plan.prepare_sampler()
                .unwrap()
                .sample(129, &mut rng)
                .unwrap()
                .len(),
            129
        );
    }
}
