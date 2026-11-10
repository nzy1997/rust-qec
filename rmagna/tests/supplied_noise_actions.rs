// Pinned itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f supplied-noise
// boundary, independently exercised through the Rust public task API.
use num_rational::BigRational;
use rmagna::circuit::{Basis, Circuit, Operation, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::instrument::{FaultSelection, InsertionPreparation};
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, LoadedModules, SuppliedActionLimits, SuppliedNoiseActions,
    SuppliedPauliAction, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(target: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-supplied-actions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), target).unwrap();
        Self(path)
    }
    fn load(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
    ) -> (Circuit, LoadedModules) {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(task, &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
        (circuit, modules)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn noise_anchors(circuit: &Circuit) -> Vec<usize> {
    circuit
        .operations()
        .iter()
        .filter_map(|op| matches!(op.operation, Operation::Noise { .. }).then_some(op.id))
        .collect()
}
fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn modes() -> impl Iterator<Item = EvaluationOptions> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix]
            .into_iter()
            .flat_map(move |strategy| {
                [HistorySampler::ExactMarginal, HistorySampler::AffineSupport]
                    .into_iter()
                    .map(move |sampler| {
                        let mut options = EvaluationOptions::default();
                        options.runtime.compile.compiler_strategy = compiler;
                        options.runtime.strategy = strategy;
                        options.runtime.history_sampler = sampler;
                        options
                    })
            })
    })
}

#[test]
fn custom_z_at_a_guarded_x_anchor_preserves_exact_native_boundary() {
    let registry = ControllerRegistry::default();
    let zero = Fixture::new("I 0");
    let plus = Fixture::new("H 0");
    let (base, base_modules) = zero.load(
        "H 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nZ_ERROR(1/10) 1",
        r#"{"outputs":[1],"ideal_state":"target.magna"}"#,
        &registry,
    );
    let (discriminating, plus_modules) = plus.load(
        "H 0\nM 0\nH 1\nX_ERROR(1/3) rec[-1] 1\nZ_ERROR(1/10) 1",
        r#"{"outputs":[1],"ideal_state":"target.magna","sample_records":[0],"observables":[[1,0]]}"#,
        &registry,
    );
    for options in modes() {
        let half = options.scalars.omega(0, -2).unwrap();
        for likelihood in [ratio(9, 5), ratio(0, 1)] {
            let weight = options.fields.from_rational(likelihood.clone()).unwrap();
            for (circuit, modules, observe_guard) in [
                (&base, &base_modules, false),
                (&discriminating, &plus_modules, true),
            ] {
                let anchors = noise_anchors(circuit);
                let action = SuppliedPauliAction {
                    anchor: anchors[0],
                    wire: 1,
                    basis: Basis::Z,
                };
                let plan = TaskEvaluator::prepare_supplied_actions(
                    circuit,
                    modules,
                    &registry,
                    options,
                    SuppliedNoiseActions {
                        paulis: &[action],
                        report_flips: &[],
                        likelihood: &likelihood,
                    },
                    SuppliedActionLimits::default(),
                )
                .unwrap();
                let mut seen = [false; 2];
                for seed in 0..16 {
                    let draw = plan.sample(&mut SplitMix64::new(seed)).unwrap();
                    let active = if observe_guard {
                        assert_eq!(draw.sampled_records.len(), 1);
                        draw.sampled_records[0]
                    } else {
                        assert!(draw.sampled_records.is_empty());
                        draw.noise_probability == ratio(0, 1)
                    };
                    seen[usize::from(active)] = true;
                    let failed = active && observe_guard;
                    let raw_error = if failed {
                        half.clone()
                    } else {
                        ExactScalar::zero()
                    };
                    let raw_target = if failed {
                        ExactScalar::zero()
                    } else {
                        half.clone()
                    };
                    assert_eq!(draw.noise_likelihood_ratio, likelihood);
                    assert_eq!(draw.history_mass, half);
                    assert_eq!(draw.raw_acceptance, half);
                    assert_eq!(
                        draw.raw_error, raw_error,
                        "custom Z must follow the reported guard"
                    );
                    assert_eq!(draw.raw_target_success, Some(raw_target));
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(draw.accepted, weight);
                    assert_eq!(
                        draw.error,
                        if failed {
                            weight.clone()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(
                        draw.target_success,
                        Some(if failed {
                            ExactField::zero()
                        } else {
                            weight.clone()
                        })
                    );
                    assert_eq!(
                        draw.noise_probability,
                        if active { ratio(0, 1) } else { ratio(9, 10) }
                    );
                    if observe_guard {
                        assert_eq!(
                            draw.raw_moments,
                            [if failed { half.negated() } else { half.clone() }]
                        );
                        assert_eq!(
                            draw.moments,
                            [if failed {
                                weight.negated()
                            } else {
                                weight.clone()
                            }]
                        );
                    }
                }
                assert_eq!(seen, [true, true]);
            }
        }
    }
}

#[test]
fn representable_actions_use_the_same_categorical_law_and_stream() {
    let files = Fixture::new("H 0\nI 1");
    let registry = ControllerRegistry::default();
    let (physical, modules) = files.load(
        "H 0\nM(1/5) 0\nH 1\nE(1/4) rec[-1] X1 X2\nELSE_CORRELATED_ERROR(1/3) rec[-1] Z1 Z2",
        r#"{"outputs":[1,2],"ideal_state":"target.magna","sample_records":[0],"observables":[[1,0],[0,2]]}"#,
        &registry,
    );
    let anchors = noise_anchors(&physical);
    for options in modes() {
        let categorical = TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
        for (alternative, basis) in [(0, Basis::X), (1, Basis::Z)] {
            let faults = [
                FaultSelection {
                    site: 0,
                    alternative: 0,
                },
                FaultSelection {
                    site: 1,
                    alternative,
                },
            ];
            let actions = [
                SuppliedPauliAction {
                    anchor: anchors[alternative],
                    wire: 2,
                    basis,
                },
                SuppliedPauliAction {
                    anchor: anchors[alternative],
                    wire: 1,
                    basis,
                },
            ];
            for likelihood in [ratio(1, 1), ratio(9, 5)] {
                let prepared = TaskEvaluator::prepare_supplied_actions(
                    &physical,
                    &modules,
                    &registry,
                    options,
                    SuppliedNoiseActions {
                        paulis: &actions,
                        report_flips: &[0],
                        likelihood: &likelihood,
                    },
                    SuppliedActionLimits::default(),
                )
                .unwrap();
                let mut seen = [false; 2];
                let mut consumed_entropy = false;
                for seed in 0..16 {
                    let mut action_rng = SplitMix64::new(seed);
                    let mut category_rng = SplitMix64::new(seed);
                    let from_actions = prepared.sample(&mut action_rng).unwrap();
                    let from_category = categorical
                        .sample_supplied_noise(&faults, &likelihood, &mut category_rng)
                        .unwrap();
                    assert!(from_actions == from_category, "same exact evaluation");
                    let [active] = from_actions.sampled_records.as_slice() else {
                        panic!("expected one sampled report");
                    };
                    seen[usize::from(*active)] = true;
                    assert_eq!(
                        from_actions.noise_probability,
                        if *active { ratio(1, 20) } else { ratio(1, 5) }
                    );
                    assert_eq!(action_rng.state(), category_rng.state());
                    consumed_entropy |= action_rng.state() != seed;
                }
                assert_eq!(seen, [true, true]);
                assert!(consumed_entropy);
            }
        }
    }
}

#[test]
fn correlated_custom_actions_and_repetitions_respect_anchor_and_report_order() {
    let files = Fixture::new("H 0\nI 1");
    let registry = ControllerRegistry::default();
    let (physical, modules) = files.load(
        "M 0\nH 1\nE(1/4) rec[-1] X1 X2\nELSE_CORRELATED_ERROR(1/3) rec[-1] Z1 Z2",
        r#"{"outputs":[1,2],"ideal_state":"target.magna","sample_records":[0],"observables":[[1,0],[0,2]]}"#,
        &registry,
    );
    let anchors = noise_anchors(&physical);
    let z = SuppliedPauliAction {
        anchor: anchors[0],
        wire: 1,
        basis: Basis::Z,
    };
    let x = SuppliedPauliAction {
        anchor: anchors[1],
        wire: 2,
        basis: Basis::X,
    };
    let likelihood = ratio(9, 5);
    for mut options in modes() {
        options.runtime.compile.insertion_preparation =
            InsertionPreparation::Prepared(Default::default());
        let weight = options.fields.from_rational(likelihood.clone()).unwrap();
        for (paulis, reports, active, fails) in [
            (&[x, z][..], &[0][..], true, true),
            (&[x, z][..], &[0, 0][..], false, false),
            (&[z, z][..], &[0][..], true, false),
        ] {
            let prepared = TaskEvaluator::prepare_supplied_actions(
                &physical,
                &modules,
                &registry,
                options,
                SuppliedNoiseActions {
                    paulis,
                    report_flips: reports,
                    likelihood: &likelihood,
                },
                SuppliedActionLimits::default(),
            )
            .unwrap();
            let draw = prepared.sample(&mut SplitMix64::new(5)).unwrap();
            assert_eq!(draw.sampled_records, [active]);
            assert_eq!(
                draw.noise_probability,
                if active { ratio(0, 1) } else { ratio(1, 1) }
            );
            assert_eq!(draw.history_mass, ExactScalar::one());
            assert_eq!(draw.raw_acceptance, ExactScalar::one());
            assert_eq!(
                draw.raw_error,
                if fails {
                    ExactScalar::one()
                } else {
                    ExactScalar::zero()
                }
            );
            assert_eq!(
                draw.raw_target_success,
                Some(if fails {
                    ExactScalar::zero()
                } else {
                    ExactScalar::one()
                })
            );
            assert_eq!(draw.accepted, weight);
            let signed = if fails {
                weight.negated()
            } else {
                weight.clone()
            };
            assert_eq!(draw.moments, [signed.clone(), signed]);
        }
    }
}

#[test]
fn supplied_pauli_updates_a_later_causal_observation_and_controller() {
    let files = Fixture::new("I 0");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    let observed = Arc::clone(&calls);
    registry
        .register("echo", "1", move |values: &[bool]| {
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(values.to_vec())
        })
        .unwrap();
    let (physical, modules) = files.load(
        "M(1/5) 0\nH 1\nX_ERROR(1/4) rec[-1] 1\nMX 1\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 2",
        r#"{"outputs":[2],"ideal_state":"target.magna","sample_records":[0,1],"observables":[[0,1]]}"#,
        &registry,
    );
    let anchor = noise_anchors(&physical)[0];
    for options in modes() {
        for active in [false, true] {
            let likelihood = ratio(9, 5);
            let before = calls.load(Ordering::Relaxed);
            let prepared = TaskEvaluator::prepare_supplied_actions(
                &physical,
                &modules,
                &registry,
                options,
                SuppliedNoiseActions {
                    paulis: &[SuppliedPauliAction {
                        anchor,
                        wire: 1,
                        basis: Basis::Z,
                    }],
                    report_flips: if active { &[0] } else { &[] },
                    likelihood: &likelihood,
                },
                SuppliedActionLimits::default(),
            )
            .unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), before);
            let mut rng = SplitMix64::new(41);
            let draw = prepared.sample(&mut rng).unwrap();
            assert_eq!(draw.sampled_records, [active, active]);
            assert_eq!(draw.controller_calls[0].inputs, [active]);
            assert_eq!(calls.load(Ordering::Relaxed), before + 1);
            assert_eq!(
                draw.raw_error,
                if active {
                    ExactScalar::one()
                } else {
                    ExactScalar::zero()
                }
            );
            assert_eq!(
                draw.noise_probability,
                if active { ratio(0, 1) } else { ratio(4, 5) }
            );
        }
    }
}

#[test]
fn invalid_supplied_actions_and_raw_limits_are_rejected_before_any_callback() {
    let files = Fixture::new("I 0");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    let observed = Arc::clone(&calls);
    registry
        .register("echo", "1", move |values: &[bool]| {
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(values.to_vec())
        })
        .unwrap();
    let (physical, modules) = files.load(
        "M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nX_ERROR(1/4) ctrl[0] 1",
        r#"{"outputs":[1],"ideal_state":"target.magna"}"#,
        &registry,
    );
    let valid = SuppliedPauliAction {
        anchor: noise_anchors(&physical)[0],
        wire: 1,
        basis: Basis::Z,
    };
    let limits = SuppliedActionLimits::default();
    let zero = ratio(0, 1);
    for (paulis, reports, factor, budgets, expected) in [
        (
            &[SuppliedPauliAction { anchor: 0, ..valid }][..],
            &[][..],
            zero.clone(),
            limits,
            "noise anchor",
        ),
        (
            &[SuppliedPauliAction {
                anchor: usize::MAX,
                ..valid
            }][..],
            &[][..],
            zero.clone(),
            limits,
            "noise anchor",
        ),
        (
            &[SuppliedPauliAction { wire: 0, ..valid }][..],
            &[][..],
            zero.clone(),
            limits,
            "target wire",
        ),
        (
            &[valid][..],
            &[1, 1][..],
            zero.clone(),
            limits,
            "report record",
        ),
        (&[valid][..], &[][..], ratio(-1, 1), limits, "nonnegative"),
        (
            &[valid][..],
            &[][..],
            BigRational::new_raw(1.into(), 0.into()),
            limits,
            "positive denominator",
        ),
        (
            &[valid][..],
            &[][..],
            BigRational::new_raw(32.into(), 32.into()),
            limits,
            "coefficient bits",
        ),
        (
            &[valid, valid][..],
            &[][..],
            zero.clone(),
            SuppliedActionLimits {
                max_pauli_actions: 1,
                ..limits
            },
            "paulis require 2",
        ),
        (
            &[valid][..],
            &[0, 0][..],
            zero.clone(),
            SuppliedActionLimits {
                max_report_flips: 1,
                ..limits
            },
            "report_flips require 2",
        ),
    ] {
        let options = EvaluationOptions {
            fields: rmagna::field::FieldArithmetic::new(5),
            ..Default::default()
        };
        let error = TaskEvaluator::prepare_supplied_actions(
            &physical,
            &modules,
            &registry,
            options,
            SuppliedNoiseActions {
                paulis,
                report_flips: reports,
                likelihood: &factor,
            },
            budgets,
        )
        .err()
        .expect("admission must fail");
        assert!(error.message.contains(expected), "{error}");
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
    let prepared = TaskEvaluator::prepare_supplied_actions(
        &physical,
        &modules,
        &registry,
        EvaluationOptions::default(),
        SuppliedNoiseActions {
            paulis: &[valid],
            report_flips: &[],
            likelihood: &zero,
        },
        limits,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let draw = prepared.sample(&mut SplitMix64::new(19)).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(draw.accepted, ExactField::zero());
    assert_eq!(draw.raw_acceptance, ExactScalar::one());
}

#[test]
fn unguarded_custom_action_uses_the_live_wire_after_reset_without_entropy() {
    let files = Fixture::new("H 0");
    let registry = ControllerRegistry::default();
    let (physical, modules) = files.load(
        "X 1\nR 1\nH 1\nX_ERROR(1/3) 1",
        r#"{"outputs":[1],"ideal_state":"target.magna","observables":[[1,0]]}"#,
        &registry,
    );
    let action = SuppliedPauliAction {
        anchor: noise_anchors(&physical)[0],
        wire: 1,
        basis: Basis::Z,
    };
    let raw = BigRational::new_raw(18.into(), 10.into());
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        let evaluator = TaskEvaluator::prepare_supplied_actions(
            &physical,
            &modules,
            &registry,
            options,
            SuppliedNoiseActions {
                paulis: &[action],
                report_flips: &[],
                likelihood: &raw,
            },
            SuppliedActionLimits::default(),
        )
        .unwrap();
        let mut rng = SplitMix64::new(73);
        let draw = evaluator.sample(&mut rng).unwrap();
        let weight = options.fields.from_rational(ratio(9, 5)).unwrap();
        assert_eq!(rng.state(), 73);
        assert_eq!(draw.noise_likelihood_ratio, ratio(9, 5));
        assert_eq!(draw.noise_probability, ratio(0, 1));
        assert_eq!(draw.raw_error, ExactScalar::one());
        assert_eq!(draw.raw_target_success, Some(ExactScalar::zero()));
        assert_eq!(draw.raw_moments, [ExactScalar::one().negated()]);
        assert_eq!(draw.accepted, weight);
        assert_eq!(draw.error, weight);
        assert_eq!(draw.moments, [weight.negated()]);
    }
}

#[test]
fn unrelated_noise_keeps_the_normal_compiler_limit_order() {
    let files = Fixture::new("I 0");
    let registry = ControllerRegistry::default();
    let (physical, modules) = files.load(
        "H 0\nX_ERROR(1/3) 0\nREPEAT 64 {\n    DEPOLARIZE2(1/7) 0 1\n}",
        r#"{"outputs":[0]}"#,
        &registry,
    );
    let mut options = EvaluationOptions::default();
    options.runtime.compile.max_gates = 0;
    options.runtime.compile.max_noise_sites = 1;
    let ordinary = TaskEvaluator::new(&physical, &modules, &registry, options)
        .err()
        .expect("normal compiler must enforce the gate limit first");
    assert_eq!(ordinary.field, "circuit");
    assert!(ordinary.message.contains("max_gates"), "{ordinary}");
    let supplied = TaskEvaluator::prepare_supplied_actions(
        &physical,
        &modules,
        &registry,
        options,
        SuppliedNoiseActions {
            paulis: &[SuppliedPauliAction {
                anchor: noise_anchors(&physical)[0],
                wire: 0,
                basis: Basis::X,
            }],
            report_flips: &[],
            likelihood: &ratio(1, 1),
        },
        SuppliedActionLimits::default(),
    )
    .err()
    .expect("supplied evaluator must preserve the normal compiler limit order");
    assert_eq!(supplied, ordinary);
}
