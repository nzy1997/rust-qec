use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordPostselectedCounts,
    NearCliffordShot,
};

fn count_records(shots: &[NearCliffordShot], observable: u32) -> NearCliffordPostselectedCounts {
    let accepted: Vec<_> = shots
        .iter()
        .filter(|shot| shot.detectors.iter().all(|&bit| !bit))
        .collect();
    NearCliffordPostselectedCounts {
        attempted: shots.len(),
        accepted: accepted.len(),
        logical_errors: accepted
            .iter()
            .filter(|shot| {
                shot.observables
                    .iter()
                    .filter(|(index, _)| *index == observable)
                    .fold(false, |parity, (_, bit)| parity ^ bit)
            })
            .count(),
    }
}

#[test]
fn raw_postselection_folds_annotations_without_reference_normalization() {
    // The deterministic raw one must be rejected despite its zero normalized value.
    let plan = CompiledNearCliffordExecutor::compile_text(
        "X 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\n",
    )
    .unwrap();
    let mut sampler = plan.prepare_sampler().unwrap();
    assert_eq!(
        sampler
            .sample_postselected_counts(65, 7, &mut StdRng::seed_from_u64(1))
            .unwrap(),
        NearCliffordPostselectedCounts {
            attempted: 65,
            accepted: 0,
            logical_errors: 0
        }
    );
    // Detector and observable duplicate record targets cancel; unrelated observables
    // do not enter the chosen parity, and no detector means every shot is accepted.
    for text in [
        "X 0\nM 0\nDETECTOR rec[-1] rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(3) rec[-1]\n",
        "X 0\nM 0\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(3) rec[-1]\n",
        "OBSERVABLE_INCLUDE(7)\nDETECTOR\n",
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        for shots in [0, 1, 31, 32, 33, 63, 64, 65, 129] {
            let mut a = StdRng::seed_from_u64(739);
            let mut b = a.clone();
            let records = plan
                .prepare_sampler()
                .unwrap()
                .sample(shots, &mut a)
                .unwrap();
            let counts = plan
                .prepare_sampler()
                .unwrap()
                .sample_postselected_counts(shots, 7, &mut b)
                .unwrap();
            assert_eq!(counts, count_records(&records, 7));
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}

#[test]
fn postselected_counts_preserve_noise_projection_feedback_and_rng_across_routes() {
    for text in [
        "H 0 64\nT 0 64\nDEPOLARIZE2(0.43) 0 64\nMY(0.2) !64\nCX rec[-1] 0\nCX sweep[1] 64\nT_DAG 0\nMRX 0\nM 0 64\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\n",
        "H 0 1 2 3\nT 0 1 2 3\nDEPOLARIZE2(0.23) 0 1\nMPP(0.37) !X0*Y1*X2*Y3\nDETECTOR rec[-1]\nMRX(0.41) !0\nCX rec[-1] 3\nT_DAG 3\nMRY 1\nMY 2\nMX 3\nM 0 1 2 3\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\nOBSERVABLE_INCLUDE(3) rec[-4]\n",
        "H 0 1\nT 0 1\nREPEAT 3 {\nDEPOLARIZE1(0.1) 0 1\nMPP X0*X1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\n}\n",
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for budget in [0, initial + 288, 64 * 1024 * 1024] {
                for seed in [739, 1739, 2739] {
                    let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                    let mut counts = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                    let mut a = StdRng::seed_from_u64(seed);
                    let mut b = a.clone();
                    for shots in [0, 1, 31, 32, 33, 63, 64, 65, 127, 129, 64] {
                        let records = scalar
                            .sample_with_sweep(shots, &[false, true], &mut a)
                            .unwrap();
                        let actual = counts
                            .sample_postselected_counts_with_sweep(shots, 7, &[false, true], &mut b)
                            .unwrap();
                        assert_eq!(
                            actual,
                            count_records(&records, 7),
                            "shots={shots} budget={budget} seed={seed} {arithmetic:?}"
                        );
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                    }
                    // An existing measurements call after counts must keep sampler state
                    // and dispatch adaptation compatible with the scalar record stream.
                    let records = scalar
                        .sample_with_sweep(65, &[false, true], &mut a)
                        .unwrap();
                    let flat = counts
                        .sample_measurements_u8_with_sweep(65, &[false, true], &mut b)
                        .unwrap();
                    assert_eq!(
                        flat.measurements,
                        records
                            .iter()
                            .flat_map(|s| s.measurements.iter().copied().map(u8::from))
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }
}

#[test]
fn missing_observable_fails_before_consuming_rng_even_for_zero_shots() {
    let plan =
        CompiledNearCliffordExecutor::compile_text("H 0\nM 0\nOBSERVABLE_INCLUDE(7) rec[-1]\n")
            .unwrap();
    let mut sampler = plan.prepare_sampler().unwrap();
    let mut a = StdRng::seed_from_u64(1);
    let mut b = a.clone();
    for shots in [0, 65] {
        assert!(
            sampler
                .sample_postselected_counts(shots, 0, &mut a)
                .unwrap_err()
                .contains("observable 0")
        );
    }
    assert_eq!(a.next_u64(), b.next_u64());
    assert_eq!(
        sampler.sample_postselected_counts(65, 7, &mut a).unwrap(),
        count_records(
            &plan.prepare_sampler().unwrap().sample(65, &mut b).unwrap(),
            7
        )
    );
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn rejected_scalar_rows_consume_sparse_noise_and_independent_draws_before_next_call() {
    for repeats in [2, 70] {
        let text = format!(
            "H 0\nM 0\nDETECTOR rec[-1]\nH 1 2 3 4\nT 1 2 3 4\nREPEAT {repeats} {{\nDEPOLARIZE1(0.001) 1 2 3 4\nX_ERROR(0.003) 1\nH 5\nM(0.002) 5\nR 5\n}}\nMPP(0.004) X1*Y2*X3*Y4\nCX rec[-1] 1\nMRX 1\nM 1 2 3 4\nOBSERVABLE_INCLUDE(7) rec[-1]\n"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            for budget in [0, 64 * 1024 * 1024] {
                let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut native = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut a = StdRng::seed_from_u64(719);
                let mut b = a.clone();
                for shots in [1, 31, 32, 64, 65, 127, 1, 129] {
                    let records = scalar.sample(shots, &mut a).unwrap();
                    assert_eq!(
                        native.sample_postselected_counts(shots, 7, &mut b).unwrap(),
                        count_records(&records, 7)
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
                let records = scalar.sample(65, &mut a).unwrap();
                let flat = native.sample_measurements_u8(65, &mut b).unwrap();
                assert_eq!(
                    flat.measurements,
                    records
                        .iter()
                        .flat_map(|s| s.measurements.iter().copied().map(u8::from))
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}
