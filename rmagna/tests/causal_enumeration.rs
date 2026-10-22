use rmagna::circuit::{BitAtom, BitExpr, Operation, ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_unitary};
use rmagna::runtime::{
    CausalProgram, CausalStrategy, ControllerRegistry, HistoryEnumerationOptions, RuntimeError,
    RuntimeOptions,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn parse(text: &str, registry: &ControllerRegistry) -> rmagna::circuit::Circuit {
    parse_circuit(
        text,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap()
}
fn records(count: usize) -> Vec<BitExpr> {
    (0..count)
        .map(|i| BitExpr::atom(BitAtom::Record(i)))
        .collect()
}
fn strategies() -> [CausalStrategy; 2] {
    [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix]
}
fn options(strategy: CausalStrategy) -> RuntimeOptions {
    RuntimeOptions {
        strategy,
        ..Default::default()
    }
}

#[test]
fn majority_traversal_keeps_rejected_histories_and_exact_joint_masses() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", |bits: &[bool]| {
            Ok(vec![bits.iter().filter(|&&b| b).count() >= 2])
        })
        .unwrap();
    let source = parse(
        "H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nPOSTSELECT det[0]",
        &registry,
    );
    let target = compile_unitary(&parse("I 0", &registry), &CompileOptions::default(), &a).unwrap();
    for strategy in strategies() {
        let program = CausalProgram::new(&source, &registry, options(strategy), &a).unwrap();
        let result = program
            .enumerate_prescribed_histories(
                &records(3),
                &[],
                HistoryEnumerationOptions::default(),
                &a,
            )
            .unwrap();
        assert_eq!(result.histories.len(), 8);
        let mut accepted = ExactScalar::zero();
        let mut error = ExactScalar::zero();
        let mut mass = ExactScalar::zero();
        let mut seen = std::collections::BTreeSet::new();
        for row in &result.histories {
            let bits = row.requested_values();
            assert!(seen.insert(bits.to_vec()));
            let majority = bits.iter().filter(|&&b| b).count() >= 2;
            assert_eq!(row.controller_calls()[0].inputs, bits);
            assert_eq!(row.controller_calls()[0].outputs, [majority]);
            assert_eq!(row.history_mass(), &a.omega(0, -6).unwrap());
            assert_eq!(row.entropy_words(), (0, 0));
            assert_eq!(row.noise_likelihood_ratio().to_string(), "1");
            assert!(row.history_proposal_mass().is_none());
            let m = row
                .instrument()
                .masses(&[3], &target, row.faults(), 256, &a)
                .unwrap();
            let expected = if !bits[0] {
                row.history_mass().clone()
            } else {
                ExactScalar::zero()
            };
            assert_eq!(m.accepted.value, expected);
            assert_eq!(
                m.error,
                if majority {
                    expected
                } else {
                    ExactScalar::zero()
                }
            );
            accepted = a.add(&accepted, &m.accepted.value).unwrap();
            error = a.add(&error, &m.error).unwrap();
            mass = a.add(&mass, row.history_mass()).unwrap();
        }
        assert_eq!(mass, ExactScalar::one());
        assert_eq!(accepted, a.omega(0, -2).unwrap());
        assert_eq!(error, a.omega(0, -6).unwrap());
        if strategy == CausalStrategy::FixedTemplate {
            assert_eq!(result.network_compilations, 0);
        } else {
            assert!(
                result.network_compilations
                    > result
                        .histories
                        .iter()
                        .map(|h| h.network_compilations())
                        .sum()
            );
        }
        let limits = HistoryEnumerationOptions {
            max_branches: 8,
            max_record_bits: 3,
            max_marginal_queries: result.marginal_queries,
            max_replay_attempts: result.replay_attempts,
        };
        let exact = program
            .enumerate_prescribed_histories(&records(3), &[], limits, &a)
            .unwrap();
        assert_eq!(exact.marginal_queries, result.marginal_queries);
        assert_eq!(exact.replay_attempts, result.replay_attempts);
        assert_eq!(exact.network_compilations, result.network_compilations);
        for (limited, name) in [
            (
                HistoryEnumerationOptions {
                    max_branches: 7,
                    ..limits
                },
                "max_branches",
            ),
            (
                HistoryEnumerationOptions {
                    max_record_bits: 2,
                    ..limits
                },
                "max_record_bits",
            ),
            (
                HistoryEnumerationOptions {
                    max_marginal_queries: limits.max_marginal_queries - 1,
                    ..limits
                },
                "max_marginal_queries",
            ),
            (
                HistoryEnumerationOptions {
                    max_replay_attempts: limits.max_replay_attempts - 1,
                    ..limits
                },
                "max_replay_attempts",
            ),
        ] {
            assert!(
                matches!(program.enumerate_prescribed_histories(&records(3), &[], limited, &a), Err(RuntimeError::EnumerationLimit { option, .. }) if option == name)
            );
        }
    }
}

#[test]
fn biased_report_flips_and_guarded_actions_use_only_causal_reported_inputs() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |b: &[bool]| Ok(vec![b[0]]))
        .unwrap();
    registry
        .register("parity", "1", |b: &[bool]| Ok(vec![b[0] ^ b[1]]))
        .unwrap();
    let source = parse(
        "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nX_ERROR(1/3) ctrl[0] 2\nM 1\nDETECTOR rec[-1]\nCONTROL[parity] det[1] ctrl[0] -> ctrl[1]\nCX ctrl[1] 2\nM 2\nDETECTOR rec[-1]\nPOSTSELECT det[1]",
        &registry,
    );
    // Literal Born probabilities for HTH|0>: (2 +/- sqrt(2))/4.
    let p0 = a
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
        .unwrap();
    let p1 = a.subtract(&ExactScalar::one(), &p0).unwrap();
    for strategy in strategies() {
        let program = CausalProgram::new(&source, &registry, options(strategy), &a).unwrap();
        let sites = source
            .operations()
            .iter()
            .filter_map(|op| program.potential_site(op.id))
            .collect::<Vec<_>>();
        assert_eq!(sites.len(), 4);
        let guard = source
            .operations()
            .iter()
            .find(|op| matches!(op.operation, Operation::Noise { .. }))
            .unwrap();
        let guard_site = program.potential_site(guard.id).unwrap();
        for flip in [false, true] {
            for fault in [false, true] {
                let mut faults = Vec::new();
                if flip {
                    faults.push(FaultSelection {
                        site: sites[0],
                        alternative: 0,
                    });
                }
                if fault {
                    faults.push(FaultSelection {
                        site: guard_site,
                        alternative: 0,
                    });
                }
                let result = program
                    .enumerate_prescribed_histories(
                        &records(3),
                        &faults,
                        HistoryEnumerationOptions::default(),
                        &a,
                    )
                    .unwrap();
                assert_eq!(result.histories.len(), 3);
                let mut seen = std::collections::BTreeSet::new();
                let mut total = ExactScalar::zero();
                for row in result.histories {
                    let bits = row.requested_values();
                    assert!(seen.insert(bits.to_vec()));
                    assert!(bits[0] || !bits[1]);
                    assert_eq!(bits[2], bits[0] ^ bits[1] ^ (bits[0] && fault));
                    assert_eq!(row.controller_calls()[0].inputs, [bits[0]]);
                    assert_eq!(row.controller_calls()[1].inputs, [bits[1], bits[0]]);
                    assert_eq!(row.controller_calls()[1].outputs, [bits[0] ^ bits[1]]);
                    let first = if bits[0] ^ flip { &p1 } else { &p0 };
                    let expected = if bits[0] {
                        a.multiply(first, &a.omega(0, -2).unwrap()).unwrap()
                    } else {
                        first.clone()
                    };
                    assert_eq!(row.history_mass(), &expected);
                    let report_p: num_rational::BigRational =
                        if flip { "1/7" } else { "6/7" }.parse().unwrap();
                    let guard_p: num_rational::BigRational = if !bits[0] {
                        "1"
                    } else if fault {
                        "1/3"
                    } else {
                        "2/3"
                    }
                    .parse()
                    .unwrap();
                    assert_eq!(row.noise_probability(), &(report_p * guard_p));
                    assert_eq!(row.entropy_words(), (0, 0));
                    total = a.add(&total, row.history_mass()).unwrap();
                }
                assert_eq!(total, ExactScalar::one());
            }
        }
    }
}

#[test]
fn deterministic_empty_and_interference_zero_groups_have_one_positive_history() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    registry
        .register("always", "1", |_: &[bool]| Ok(vec![true]))
        .unwrap();
    for strategy in strategies() {
        for (text, count, expected_calls) in [
            ("I 0", 0, 0),
            ("CONTROL[always] -> ctrl[0]\nCX ctrl[0] 0", 0, 1),
            (
                "H 0\nT 0\nT_DAG 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nPOSTSELECT !det[0]",
                1,
                1,
            ),
        ] {
            let program =
                CausalProgram::new(&parse(text, &registry), &registry, options(strategy), &a)
                    .unwrap();
            let result = program
                .enumerate_prescribed_histories(
                    &records(count),
                    &[],
                    HistoryEnumerationOptions {
                        max_branches: 1,
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
            assert_eq!(result.histories.len(), 1);
            let row = &result.histories[0];
            assert_eq!(row.history_mass(), &ExactScalar::one());
            assert_eq!(row.controller_calls().len(), expected_calls);
            assert_eq!(row.requested_values(), vec![false; count]);
            assert_eq!(row.entropy_words(), (0, 0));
            if count == 1 {
                assert!(
                    row.instrument()
                        .acceptance(row.faults(), 256, &a)
                        .unwrap()
                        .value
                        .is_zero()
                );
            }
        }
    }
}

#[test]
fn callback_errors_shapes_and_observed_nondeterminism_fail_explicitly() {
    let a = ScalarArithmetic::default();
    for strategy in strategies() {
        for shape in [false, true] {
            let mut registry = ControllerRegistry::default();
            registry
                .register("bad", "1", move |_: &[bool]| {
                    if shape {
                        Ok(vec![])
                    } else {
                        Err("decoder failure".to_owned())
                    }
                })
                .unwrap();
            let program = CausalProgram::new(
                &parse("CONTROL[bad] -> ctrl[0]\nI 0", &registry),
                &registry,
                options(strategy),
                &a,
            )
            .unwrap();
            let error = program
                .enumerate_prescribed_histories(&[], &[], HistoryEnumerationOptions::default(), &a)
                .err()
                .unwrap();
            if shape {
                assert!(matches!(
                    error,
                    RuntimeError::OutputShape {
                        expected: 1,
                        actual: 0,
                        ..
                    }
                ));
            } else {
                assert!(
                    matches!(error, RuntimeError::Controller { message, .. } if message == "decoder failure")
                );
            }
        }
        let mut registry = ControllerRegistry::default();
        let state = Arc::new(AtomicBool::new(false));
        registry
            .register("unstable", "1", move |_: &[bool]| {
                Ok(vec![state.fetch_xor(true, Ordering::SeqCst)])
            })
            .unwrap();
        let source = parse(
            "CONTROL[unstable] -> ctrl[0]\nH 0\nM 0\nCX ctrl[0] 1",
            &registry,
        );
        let program = CausalProgram::new(&source, &registry, options(strategy), &a).unwrap();
        assert!(
            matches!(program.enumerate_prescribed_histories(&records(1), &[], HistoryEnumerationOptions::default(), &a), Err(RuntimeError::Controller { message, .. }) if message.contains("deterministic and stateless"))
        );
    }
}

#[test]
fn zero_forced_and_inactive_guard_choices_remain_conditional_diagnostics() {
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    for strategy in strategies() {
        for rate in ["0", "1"] {
            let source = parse(
                &format!("H 0\nM 0\nX_ERROR({rate}) rec[-1] 1\nM 1"),
                &registry,
            );
            let program = CausalProgram::new(&source, &registry, options(strategy), &a).unwrap();
            let noise_op = source
                .operations()
                .iter()
                .find(|op| matches!(op.operation, Operation::Noise { .. }))
                .unwrap()
                .id;
            let site = program.potential_site(noise_op).unwrap();
            for selected in [false, true] {
                let faults = if selected {
                    vec![FaultSelection {
                        site,
                        alternative: 0,
                    }]
                } else {
                    vec![]
                };
                let result = program
                    .enumerate_prescribed_histories(
                        &records(2),
                        &faults,
                        HistoryEnumerationOptions::default(),
                        &a,
                    )
                    .unwrap();
                assert_eq!(result.histories.len(), 2);
                for row in result.histories {
                    let active = row.requested_values()[0];
                    assert_eq!(row.requested_values()[1], active && selected);
                    assert_eq!(row.history_mass(), &a.omega(0, -2).unwrap());
                    assert_eq!(
                        row.noise_probability().to_string(),
                        if active && selected != (rate == "1") {
                            "0"
                        } else {
                            "1"
                        }
                    );
                    assert_eq!(row.faults().len(), usize::from(active && selected));
                    if let Some(fault) = row.faults().first() {
                        assert_eq!(fault.alternative, 0);
                        assert_eq!(
                            row.instrument().noise_sites()[fault.site].source_operations(),
                            [noise_op]
                        );
                    }
                    assert_eq!(row.entropy_words(), (0, 0));
                }
            }
            // A constant-false guard has no action even for a forced potential choice.
            let source = parse(
                &format!("CONTROL[XOR] -> ctrl[0]\nX_ERROR({rate}) ctrl[0] 0\nM 0"),
                &registry,
            );
            let program = CausalProgram::new(&source, &registry, options(strategy), &a).unwrap();
            let op = source
                .operations()
                .iter()
                .find(|op| matches!(op.operation, Operation::Noise { .. }))
                .unwrap()
                .id;
            let faults = [FaultSelection {
                site: program.potential_site(op).unwrap(),
                alternative: 0,
            }];
            let result = program
                .enumerate_prescribed_histories(
                    &records(1),
                    &faults,
                    HistoryEnumerationOptions::default(),
                    &a,
                )
                .unwrap();
            assert_eq!(result.histories.len(), 1);
            let row = &result.histories[0];
            assert_eq!(row.requested_values(), [false]);
            assert_eq!(row.history_mass(), &ExactScalar::one());
            assert_eq!(row.noise_probability().to_string(), "1");
            assert!(row.faults().is_empty());
        }
    }
}

#[test]
fn enumeration_ignores_sampling_proposals_and_validates_inputs_and_zero_budgets() {
    use rmagna::noise::NoiseSampling;
    use rmagna::runtime::HistorySampler;
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let source = parse(
        "H 0\nT 0\nH 0\nX_ERROR(1/10) 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] det[0] !det[0] -> ctrl[0] ctrl[1] ctrl[2]",
        &registry,
    );
    let p0 = a
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
        .unwrap();
    for strategy in strategies() {
        let program = CausalProgram::new_with_sampling(
            &source,
            &registry,
            RuntimeOptions {
                history_sampler: HistorySampler::AffineSupport,
                ..options(strategy)
            },
            &a,
            &NoiseSampling::Importance {
                proposal_scale: "2".parse().unwrap(),
            },
        )
        .unwrap();
        let result = program
            .enumerate_prescribed_histories(
                &records(1),
                &[],
                HistoryEnumerationOptions::default(),
                &a,
            )
            .unwrap();
        assert_eq!(result.histories.len(), 2);
        for row in &result.histories {
            let bit = row.requested_values()[0];
            assert_eq!(row.controller_calls()[0].inputs, [bit, bit, !bit]);
            assert_eq!(
                row.history_mass(),
                &if bit {
                    a.subtract(&ExactScalar::one(), &p0).unwrap()
                } else {
                    p0.clone()
                }
            );
            assert_eq!(row.noise_likelihood_ratio().to_string(), "1");
            assert_eq!(row.noise_probability().to_string(), "9/10");
            assert!(row.history_proposal_mass().is_none());
            assert_eq!(row.entropy_words(), (0, 0));
        }
        for (limited, expected) in [
            (
                HistoryEnumerationOptions {
                    max_branches: 0,
                    ..Default::default()
                },
                "max_branches",
            ),
            (
                HistoryEnumerationOptions {
                    max_marginal_queries: 0,
                    ..Default::default()
                },
                "max_marginal_queries",
            ),
            (
                HistoryEnumerationOptions {
                    max_replay_attempts: 0,
                    ..Default::default()
                },
                "max_replay_attempts",
            ),
        ] {
            assert!(
                matches!(program.enumerate_prescribed_histories(&records(1), &[], limited, &a), Err(RuntimeError::EnumerationLimit { option, .. }) if option == expected)
            );
        }
        assert!(
            program
                .enumerate_prescribed_histories(
                    &records(2),
                    &[],
                    HistoryEnumerationOptions::default(),
                    &a
                )
                .is_err()
        );
        assert!(matches!(
            program.enumerate_prescribed_histories(
                &records(1),
                &[FaultSelection {
                    site: usize::MAX,
                    alternative: 0
                }],
                HistoryEnumerationOptions::default(),
                &a
            ),
            Err(RuntimeError::Noise(_))
        ));
    }
}
