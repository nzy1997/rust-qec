// SPDX-License-Identifier: MPL-2.0
// Complete VirtualCliffordFeedbackTests::test_guarded_pauli_noise_uses_causal_assignments_only,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_virtual_clifford_feedback.py.
use num_rational::BigRational;
use rmagna::circuit::{Operation, ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::ExactField;
use rmagna::instrument::{FaultSelection, QueryPreparation};
use rmagna::noise::{NoiseSampling, RandomSource, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[test]
fn original_inactive_guard_discards_supplied_actions_and_reuses_preparation() {
    let mut configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
                let observed = calls.clone();
                let mut registry = ControllerRegistry::default();
                registry
                    .register("echo", "1", move |bits: &[bool]| {
                        observed.lock().unwrap().push(bits.to_vec());
                        Ok(bits.to_vec())
                    })
                    .unwrap();
                let parse = ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                };
                let physical = parse_circuit("H 0 1\nT 1\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nDEPOLARIZE1(1/3) ctrl[1] 1 1\nT 1\nH 1", &parse).unwrap();
                let modules = parse_task(
                    r#"{"outputs":[1],"observables":[[1,0],[0,1],[1,1]]}"#,
                    Path::new("."),
                    &TaskOptions::default(),
                )
                .unwrap()
                .load_modules(&physical, &parse)
                .unwrap();
                let mut options = EvaluationOptions::default();
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.compile.probability_preparation =
                    QueryPreparation::FixedFibers(Default::default());
                options.runtime.compile.moment_preparation =
                    QueryPreparation::FixedFibers(Default::default());
                // Count actual owning-task construction; Native preparation
                // counters describe a different boundary and query scope.
                let meter = ExecutionMeter::default();
                let evaluator = TaskEvaluator::new_with_costs(
                    &physical,
                    &modules,
                    &registry,
                    options,
                    &NoiseSampling::Physical,
                    &meter,
                )
                .unwrap();
                assert_eq!(meter.snapshot().compilation.attempted, 1);
                assert_eq!(meter.snapshot().compilation.succeeded, 1);
                assert_eq!(meter.snapshot().compilation.failed, 0);
                let noise_operations = physical
                    .operations()
                    .iter()
                    .filter(|op| matches!(op.operation, Operation::Noise { .. }))
                    .collect::<Vec<_>>();
                // M's report-flip category precedes the two independent repeated
                // depolarizing targets. Alternatives are X, Y, Z in this frontend.
                assert_eq!(noise_operations.len(), 2);
                let flip = FaultSelection {
                    site: evaluator
                        .potential_site(
                            physical
                                .operations()
                                .iter()
                                .find(|op| matches!(op.operation, Operation::Measure { .. }))
                                .unwrap()
                                .id,
                        )
                        .unwrap(),
                    alternative: 0,
                };
                let first = evaluator.potential_site(noise_operations[0].id).unwrap();
                let candidate = [
                    flip,
                    FaultSelection {
                        site: first,
                        alternative: 0,
                    },
                    FaultSelection {
                        site: first + 1,
                        alternative: 2,
                    },
                ];
                let mut outcomes = [false; 2];
                let mut warm = None;
                for seed in 0..8 {
                    let before = calls.lock().unwrap().len();
                    let mut left = SplitMix64::new(seed);
                    let actual = evaluator.sample_prescribed(&candidate, &mut left).unwrap();
                    let middle = calls.lock().unwrap().len();
                    let mut right = SplitMix64::new(seed);
                    let baseline = evaluator.sample_prescribed(&[flip], &mut right).unwrap();
                    assert!(
                        actual == baseline,
                        "inactive candidate changed complete task evaluation"
                    );
                    assert_eq!(left.next_u64(), right.next_u64());
                    let log = calls.lock().unwrap();
                    assert_eq!(&log[before..middle], &log[middle..]);
                    assert_eq!(middle - before, 1);
                    assert_eq!(actual.controller_calls.len(), 1);
                    assert_eq!(
                        actual.controller_calls[0].inputs,
                        actual.controller_calls[0].outputs
                    );
                    outcomes[usize::from(actual.controller_calls[0].inputs[0])] = true;
                    assert_eq!(actual.accepted, ExactField::one());
                    assert_eq!(
                        actual.moments,
                        [
                            ExactField::zero(),
                            ExactField::zero(),
                            rmagna::field::FieldArithmetic::default()
                                .from_rational("-1".parse::<BigRational>().unwrap())
                                .unwrap()
                        ]
                    );
                    assert_eq!(
                        actual.noise_probability,
                        "0".parse::<BigRational>().unwrap(),
                        "impossible report flip is a prescribed conditional diagnostic"
                    );
                    assert_eq!(
                        actual.noise_likelihood_ratio,
                        "1".parse::<BigRational>().unwrap()
                    );
                    assert_eq!(
                        actual.history_mass,
                        ScalarArithmetic::default().omega(0, -2).unwrap()
                    );
                    assert_eq!(actual.raw_moments[0], ExactScalar::zero());
                    assert_eq!(actual.raw_moments[1], ExactScalar::zero());
                    if causal == CausalStrategy::FixedTemplate {
                        assert_eq!(actual.network_compilations, 0);
                        let p = evaluator
                            .prepared_probability_statistics()
                            .unwrap()
                            .unwrap();
                        let m = evaluator.prepared_moment_statistics().unwrap().unwrap();
                        assert_eq!(
                            p.cached_queries,
                            if sampler == HistorySampler::ExactMarginal {
                                4
                            } else {
                                1
                            }
                        );
                        assert_eq!(m.cached_queries, 3);
                        if let Some(previous) = warm {
                            assert_eq!((p, m), previous);
                        } else {
                            warm = Some((p, m));
                        }
                    }
                }
                assert_eq!(outcomes, [true, true]);
                assert_eq!(meter.snapshot().compilation.succeeded, 1);
                configurations += 1;
            }
        }
    }
    assert_eq!(configurations, 8);
}
