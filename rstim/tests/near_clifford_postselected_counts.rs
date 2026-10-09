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
fn cultivation_cache_storage_keeps_records_counts_flat_outputs_and_literal_rng() {
    for text in [
        include_str!(
            "../../benchmarks/near_clifford/application_counts/fixtures/msc_d3_inject_cultivate_p1e-3.stim"
        ),
        include_str!(
            "../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        ),
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            for budget in [1 << 20, 16 << 20, 64 << 20] {
                let mut cached = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(2026100907);
                let mut b = a.clone();
                for shots in [1, 63, 64, 65, 129, 1024] {
                    assert_eq!(
                        cached.sample(shots, &mut a).unwrap(),
                        reference.sample(shots, &mut b).unwrap()
                    );
                    let rows = reference.sample(shots, &mut b).unwrap();
                    assert_eq!(
                        cached.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                        count_records(&rows, 0)
                    );
                    assert_eq!(
                        cached.sample_measurements_u8(17, &mut a).unwrap(),
                        reference.sample_measurements_u8(17, &mut b).unwrap()
                    );
                    // Re-enable counts after restoring packed cache entries in a raw call.
                    let rows = reference.sample(65, &mut b).unwrap();
                    assert_eq!(
                        cached.sample_postselected_counts(65, 0, &mut a).unwrap(),
                        count_records(&rows, 0)
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                    assert!(cached.coefficient_cache_reserved_bytes() <= budget);
                }
            }
        }
    }
}

#[test]
fn reconverging_coherent_rows_preserve_mixed_call_counts_records_and_rng() {
    let text = "REPEAT 5 {\nR 0 1 2\nH 0 1 2\nT 0 1 2\nCX 0 1\nDEPOLARIZE2(0.01) 1 2\nMY 0\nCX rec[-1] 2\nT_DAG 2\nMX 1\nMY 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\n}\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        for seed in [1739, 583] {
            let mut cached = plan.prepare_sampler().unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let initial = cached.coefficient_cache_reserved_bytes();
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            for shots in [1, 63, 64, 65, 129, 1024, 64] {
                let expected = scalar.sample(shots, &mut b).unwrap();
                assert_eq!(cached.sample(shots, &mut a).unwrap(), expected);
                let expected = scalar.sample(shots, &mut b).unwrap();
                assert_eq!(
                    cached.sample_postselected_counts(shots, 7, &mut a).unwrap(),
                    count_records(&expected, 7)
                );
                let expected = scalar.sample_measurements_u8(17, &mut b).unwrap();
                assert_eq!(cached.sample_measurements_u8(17, &mut a).unwrap(), expected);
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert!(cached.coefficient_cache_reserved_bytes() > initial);
        }
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

#[test]
fn sparse_maximum_observable_index_preserves_counts_and_rng() {
    let text = "R 0 1 2\nH 0 1\nT 0\nMX 0 1\nCX rec[-1] 2\nM 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(4294967295) rec[-3]\nOBSERVABLE_INCLUDE(2) rec[-1]\nOBSERVABLE_INCLUDE(4294967295) rec[-2]\nOBSERVABLE_INCLUDE(7)\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        for shots in [0, 1, 63, 64, 65, 1024] {
            let mut counts_sampler = plan.prepare_sampler().unwrap();
            let mut reference = plan.prepare_sampler().unwrap();
            let mut counts_rng = StdRng::seed_from_u64(20261008);
            let mut reference_rng = counts_rng.clone();
            for observable in [u32::MAX, 2, 7, u32::MAX] {
                let raw = reference.sample(shots, &mut reference_rng).unwrap();
                let counts = counts_sampler
                    .sample_postselected_counts(shots, observable, &mut counts_rng)
                    .unwrap();
                assert_eq!(counts, count_records(&raw, observable));
                assert_eq!(
                    counts_rng.clone().next_u64(),
                    reference_rng.clone().next_u64()
                );
            }
        }
    }
}

#[test]
fn packed_rejection_preserves_accepted_fallback_rows_and_following_raw_calls() {
    // The noncommuting MPP keeps MY before the detector. A small cache can admit
    // a rejected MY branch while accepted lanes need recorded scalar fallback.
    let text = "H 0 1\nT 0 1\nCX 0 1\nMY !0\nDETECTOR rec[-1]\nMPP(0.003) X0*X1\nREPEAT 70 {\nDEPOLARIZE1(0.001) 0 1\nH 2\nM(0.004) 2\nR 2\n}\nCX sweep[1] 0\nM 0 1\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\n";
    let wide = text.replace("REPEAT 70", "REPEAT 130").replace(
        "H 2\n",
        "DEPOLARIZE2(0.001) 0 1\nDEPOLARIZE2(0.37) 0 1\nX_ERROR(0) 0\nY_ERROR(1) 1\nH 2\n",
    );
    for text in [text, wide.as_str()] {
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
                let mut native = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(583);
                let mut b = a.clone();
                for shots in [32, 63, 64, 65, 127, 129, 1024, 64] {
                    let rows = reference
                        .sample_with_sweep(shots, &[false, true], &mut a)
                        .unwrap();
                    let expected = count_records(&rows, 7);
                    assert!(expected.accepted > 0 && expected.accepted < shots);
                    let actual = native
                        .sample_postselected_counts_with_sweep(shots, 7, &[false, true], &mut b)
                        .unwrap();
                    assert_eq!(actual, expected);
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
                let rows = reference
                    .sample_with_sweep(65, &[false, true], &mut a)
                    .unwrap();
                let flat = native
                    .sample_measurements_u8_with_sweep(65, &[false, true], &mut b)
                    .unwrap();
                assert_eq!(
                    flat.measurements,
                    rows.iter()
                        .flat_map(|row| row.measurements.iter().map(|bit| u8::from(*bit)))
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

#[test]
fn affine_counts_preserve_raw_parities_sparse_indices_sweeps_and_mixed_call_rng() {
    let texts = [
        "H 0 64\nS 0\nCX 0 64\nCZ 0 64\nDEPOLARIZE2(0.003) 0 64\nMPP(0.003) !X0*Y64\nCX rec[-1] 0\nMRY(0.37) !64\nCX sweep[1] 64\nMRX(1) !0\nMX(0) 0\nM 0 64\nDETECTOR rec[-1] rec[-3]\nDETECTOR rec[-2] rec[-2]\nOBSERVABLE_INCLUDE(4294967295) rec[-1]\nOBSERVABLE_INCLUDE(4294967295) rec[-2]\nOBSERVABLE_INCLUDE(7)\n",
        "REPEAT 70 {\nH 0\nM(0.003) 0\nDEPOLARIZE1(0.003) 0\nX_ERROR(0) 0\nY_ERROR(1) 0\nR 0\n}\nH 1\nM 0 1\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(4294967295) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1] rec[-1]\n",
        "X 0\nM !0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(4294967295) rec[-1]\nOBSERVABLE_INCLUDE(7)\n",
    ];
    for text in texts {
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        for budget in [0, 64 * 1024 * 1024] {
            for seed in [719, 1739, 2739] {
                let mut counts = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(seed);
                let mut b = a.clone();
                for (position, shots) in [0, 1, 31, 32, 63, 64, 65, 127, 129, 1024, 64]
                    .into_iter()
                    .enumerate()
                {
                    let sweep = [false, position % 2 == 0];
                    for observable in [u32::MAX, 7] {
                        let rows = reference.sample_with_sweep(shots, &sweep, &mut a).unwrap();
                        let actual = counts
                            .sample_postselected_counts_with_sweep(
                                shots, observable, &sweep, &mut b,
                            )
                            .unwrap();
                        assert_eq!(actual, count_records(&rows, observable));
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                    }
                    // A raw call before/after counts uses the same sampler and RNG history.
                    let rows = reference.sample_with_sweep(65, &sweep, &mut a).unwrap();
                    let flat = counts
                        .sample_measurements_u8_with_sweep(65, &sweep, &mut b)
                        .unwrap();
                    assert_eq!(
                        flat.measurements,
                        rows.iter()
                            .flat_map(|r| r.measurements.iter().copied().map(u8::from))
                            .collect::<Vec<_>>()
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }
}

#[test]
fn long_noise_spans_preserve_hits_readout_feedback_and_mixed_call_rng() {
    for probability in [0., 0.001, 0.37, 1.] {
        let text = format!(
            "H 0 1 2\nT 0 1 2\nREPEAT 40 {{\nX_ERROR({probability}) 0 1 2\nDEPOLARIZE2({probability}) 0 1\n}}\nMY(0.03) !0\nCX rec[-1] 2\nCX sweep[1] 1\nT_DAG 2\nREPEAT 35 {{\nZ_ERROR(0.003) 0 1 2\nDEPOLARIZE1(0.01) 2\n}}\nMPP(0.004) X1*Y2\nDETECTOR rec[-1]\nMRX 1\nM 0 1 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\nOBSERVABLE_INCLUDE(7) rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-2]\n"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            for seed in [739, 1739] {
                for budget in [0, 64 * 1024 * 1024] {
                    let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                    let mut native = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                    let mut a = StdRng::seed_from_u64(seed);
                    let mut b = a.clone();
                    for shots in [0, 1, 31, 32, 63, 64, 65, 129, 1] {
                        let rows = scalar
                            .sample_with_sweep(shots, &[false, true], &mut a)
                            .unwrap();
                        assert_eq!(
                            native
                                .sample_postselected_counts_with_sweep(
                                    shots,
                                    7,
                                    &[false, true],
                                    &mut b
                                )
                                .unwrap(),
                            count_records(&rows, 7),
                            "p={probability} shots={shots} seed={seed} {arithmetic:?} budget={budget}"
                        );
                        let rows = scalar
                            .sample_with_sweep(17, &[false, true], &mut a)
                            .unwrap();
                        assert_eq!(
                            native
                                .sample_with_sweep(17, &[false, true], &mut b)
                                .unwrap(),
                            rows
                        );
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn wide_coherent_rows_preserve_counts_records_and_following_rng() {
    let text = "REPEAT 3 {\nR 0 1 2 3 4 5\nH 0 1 2 3 4 5\nT 0 1 2 3 4 5\nDEPOLARIZE2(0.02) 0 1 2 3 4 5\nCX 0 1 1 2 2 3 3 4 4 5\nT_DAG 0 2 4\nCX 0 5 1 4 2 3\nT 1 3 5\nMY 0 1 2\nCX rec[-1] 5\nT_DAG 5\nMX 3 4 5\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(7) rec[-2] rec[-4]\n}\n";
    for policy in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
        for seed in [739, 1739] {
            let mut cached = plan.prepare_sampler().unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            for shots in [1, 63, 64, 65, 129, 1024, 1] {
                let expected = scalar.sample(shots, &mut b).unwrap();
                assert_eq!(cached.sample(shots, &mut a).unwrap(), expected);
                let expected = scalar.sample(shots, &mut b).unwrap();
                assert_eq!(
                    cached.sample_postselected_counts(shots, 7, &mut a).unwrap(),
                    count_records(&expected, 7)
                );
                assert_eq!(
                    cached.sample_measurements_u8(17, &mut a).unwrap(),
                    scalar.sample_measurements_u8(17, &mut b).unwrap()
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }
}
