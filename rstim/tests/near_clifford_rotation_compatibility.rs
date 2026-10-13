use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordExecutor,
};
use rstim::parser::parse_lines;

#[test]
fn repeated_targets_apply_complete_axis_rotations_in_sequence() {
    for gate in [
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "R_X(0.5)",
        "R_Y(-0.5)",
    ] {
        let text = format!("{gate} 0 0 1 1 2 2\nM 0 1 2");
        let scalar =
            NearCliffordExecutor::compile_with_limit(parse_lines(&text).unwrap(), 0).unwrap();
        assert!(
            scalar
                .sample(65, &mut StdRng::seed_from_u64(717))
                .unwrap()
                .iter()
                .all(|s| s.measurements == [true, true, true]),
            "{text}"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
                parse_lines(&text).unwrap(),
                0,
                arithmetic,
            )
            .unwrap();
            for budget in [0, 64 * 1024 * 1024] {
                let shots = plan
                    .prepare_sampler_with_cache_budget(budget)
                    .unwrap()
                    .sample(65, &mut StdRng::seed_from_u64(717))
                    .unwrap();
                assert!(
                    shots.iter().all(|s| s.measurements == [true, true, true]),
                    "{arithmetic:?}: {text}"
                );
            }
        }
    }
}

#[test]
fn neighboring_clifford_angles_still_need_non_clifford_coordinates() {
    for angle in [
        -0.5000000000000001,
        -0.49999999999999994,
        0.5000000000000001,
        0.49999999999999994,
    ] {
        for gate in ["R_X", "R_Y", "R_Z"] {
            // Z rotation needs a superposition and a noncommuting measurement;
            // otherwise the planner can legitimately project before rotating.
            let (prefix, measure) = if gate == "R_Z" {
                ("H 0\n", "MX")
            } else {
                ("", "M")
            };
            let text = format!("{prefix}{gate}({angle}) 0\n{measure} 0");
            let scalar =
                NearCliffordExecutor::compile_with_limit(parse_lines(&text).unwrap(), 0).unwrap();
            assert!(
                scalar.run(&mut StdRng::seed_from_u64(713)).is_err(),
                "{text}"
            );
            for arithmetic in [
                CompiledRotationArithmetic::Strict,
                CompiledRotationArithmetic::Fused,
            ] {
                assert!(
                    CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
                        parse_lines(&text).unwrap(),
                        0,
                        arithmetic,
                    )
                    .is_err(),
                    "{text}"
                );
                let samples =
                    CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                        .unwrap()
                        .sample(65, &mut StdRng::seed_from_u64(713))
                        .unwrap();
                let ones = samples.iter().filter(|s| s.measurements[0]).count();
                assert!(
                    (8..=57).contains(&ones),
                    "angle-sensitive half-turn result: {text}, ones={ones}"
                );
            }
        }
    }
}

#[test]
fn original_distillation_counts_match_records_and_rng_continuation() {
    let text = include_str!(
        "../../benchmarks/near_clifford/application_counts/fixtures/distillation.stim"
    );
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        for budget in [0, 64 * 1024 * 1024] {
            let mut records_rng = StdRng::seed_from_u64(711);
            let mut counts_rng = records_rng.clone();
            let records = plan
                .prepare_sampler_with_cache_budget(budget)
                .unwrap()
                .sample(129, &mut records_rng)
                .unwrap();
            let counts = plan
                .prepare_sampler_with_cache_budget(budget)
                .unwrap()
                .sample_postselected_counts(129, 0, &mut counts_rng)
                .unwrap();
            let accepted: Vec<_> = records
                .iter()
                .filter(|s| s.detectors.iter().all(|d| !d))
                .collect();
            let logical_errors = accepted
                .iter()
                .filter(|s| {
                    s.observables
                        .iter()
                        .filter(|(index, _)| *index == 0)
                        .fold(false, |p, (_, bit)| p ^ bit)
                })
                .count();
            assert_eq!(counts.attempted, records.len());
            assert_eq!(counts.accepted, accepted.len());
            assert_eq!(counts.logical_errors, logical_errors);
            assert!(
                counts.accepted > 0 && counts.accepted < counts.attempted,
                "both accepted and rejected witnesses"
            );
            assert!(
                logical_errors > 0 && logical_errors < accepted.len(),
                "both logical parity outcomes witnessed"
            );
            assert_eq!(records_rng.next_u64(), counts_rng.next_u64());
        }
    }
}
