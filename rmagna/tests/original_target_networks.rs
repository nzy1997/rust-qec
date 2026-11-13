// SPDX-License-Identifier: MPL-2.0
// Complete TargetNetworkTests(3) and WholeTaskRouteTests(2), itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_target_network.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, QueryPreparation, compile_affine, compile_unitary,
};
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, SplitMix64};
use rmagna::pauli::PauliWord;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskError, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::{CompilerStrategy, DensityLayout, PauliInsertion};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
fn field(s: &str) -> ExactField {
    FieldArithmetic::default()
        .from_rational(s.parse::<BigRational>().unwrap())
        .unwrap()
}
fn scalar(n: i64, power: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar([n.into(), 0.into(), 0.into(), 0.into()], power)
        .unwrap()
}
fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|q| value >> q & 1 != 0)).unwrap()
}
fn configs() -> impl Iterator<Item = EvaluationOptions> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [
            QueryPreparation::Cold,
            QueryPreparation::FixedFibers(Default::default()),
        ]
        .into_iter()
        .map(move |query| {
            let mut options = EvaluationOptions::default();
            options.runtime.compile.compiler_strategy = compiler;
            options.runtime.compile.probability_preparation = query;
            options.runtime.compile.density_preparation = query;
            options
        })
    })
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-target-networks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn evaluator(
        &self,
        physical: &str,
        target: &str,
        recovery: Option<&str>,
        mut task: Value,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
    ) -> Result<(TaskEvaluator, ExecutionMeter), TaskError> {
        std::fs::write(self.0.join("target.magna"), target).unwrap();
        task["ideal_state"] = json!("target.magna");
        if let Some(recovery) = recovery {
            std::fs::write(self.0.join("recovery.magna"), recovery).unwrap();
            task["recovery"] = json!("recovery.magna");
        }
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let source = parse_circuit(physical, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())?
            .load_modules(&source, &parse)?;
        let meter = ExecutionMeter::default();
        let evaluator = TaskEvaluator::new_with_costs(
            &source,
            &modules,
            registry,
            options,
            &NoiseSampling::Physical,
            &meter,
        )?;
        assert_eq!(meter.snapshot().compilation.succeeded, 2);
        assert_eq!(meter.snapshot().compilation.failed, 0);
        assert_eq!(meter.snapshot().query.succeeded, 1);
        Ok((evaluator, meter))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_linear_scalar_target_retains_separate_acceptance_and_recovery_response() {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let physical = "H 0\nT 0\nDEPOLARIZE1(1/3) 0\nH 1\nM(1/4) 1\nCX rec[-1] 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    for options in configs() {
        let (evaluator, meter) = files
            .evaluator(
                physical,
                "H 0\nT 0",
                Some("S 0"),
                json!({"outputs":[0]}),
                &ControllerRegistry::default(),
                options,
            )
            .unwrap();
        assert_eq!(evaluator.execution_route(), "linear_affine");
        let before = evaluator.compilation().clone();
        let mut warm = None;
        // Bloch vectors after S give success1/2,1,0,1/2 for I,X,Y,Z.
        // The independent reported record selects exactly half the mass.
        for category in 0..4 {
            for flip in [false, true] {
                let mut faults = vec![];
                if category != 0 {
                    faults.push(FaultSelection {
                        site: 0,
                        alternative: category - 1,
                    });
                }
                if flip {
                    faults.push(FaultSelection {
                        site: 1,
                        alternative: 0,
                    });
                }
                let draw = evaluator
                    .sample_prescribed(&faults, &mut SplitMix64::new(2))
                    .unwrap();
                let success = [
                    scalar(1, -2),
                    scalar(1, -1),
                    ExactScalar::zero(),
                    scalar(1, -2),
                ][category]
                    .clone();
                assert_eq!(draw.raw_acceptance, scalar(1, -1));
                assert_eq!(draw.raw_target_success, Some(success.clone()));
                assert_eq!(
                    draw.raw_error,
                    a.add(&scalar(1, -1), &success.negated()).unwrap()
                );
                assert_eq!(draw.accepted, field("1/2"));
                assert_eq!(
                    draw.target_success,
                    Some(FieldArithmetic::default().from_scalar(&success).unwrap())
                );
                assert_eq!(draw.history_mass, ExactScalar::one());
                assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                assert_eq!(draw.network_compilations, 0);
                let probability = if category == 0 { "2/3" } else { "1/9" }
                    .parse::<BigRational>()
                    .unwrap()
                    * if flip { "1/4" } else { "3/4" }
                        .parse::<BigRational>()
                        .unwrap();
                assert_eq!(draw.noise_probability, probability);
                let statistics = (
                    evaluator.prepared_probability_statistics().unwrap(),
                    evaluator.prepared_density_statistics().unwrap(),
                );
                if let Some(previous) = warm {
                    assert_eq!(statistics, previous);
                } else {
                    warm = Some(statistics);
                }
            }
        }
        let source = parse_circuit(physical, &ParseOptions::default()).unwrap();
        let instrument = compile_affine(&source, &options.runtime.compile, &a).unwrap();
        let scheduler = NoiseSampler::physical(
            NoiseModel::physical(&instrument, &NoiseOptions::default()).unwrap(),
            NoiseOptions::default(),
        );
        for seed in 7..27 {
            let mut expected_rng = SplitMix64::new(seed);
            let noise = scheduler.draw(&mut expected_rng).unwrap();
            let expected = evaluator
                .sample_prescribed(&noise.faults, &mut SplitMix64::new(2))
                .unwrap();
            let mut actual_rng = SplitMix64::new(seed);
            let actual = evaluator
                .sample_with_costs(&mut actual_rng, &meter)
                .unwrap();
            assert!(actual == expected);
            assert_eq!(actual_rng, expected_rng);
        }
        let mut cutoff = TaskEnumerationOptions::default();
        cutoff.noise.max_weight = 0;
        let partial = evaluator.enumerate_physical(&cutoff).unwrap();
        assert_eq!(
            partial.covered_probability.value(),
            &"1/2".parse::<BigRational>().unwrap()
        );
        assert_eq!(
            partial.omitted_probability.value(),
            &"1/2".parse::<BigRational>().unwrap()
        );
        assert_eq!(partial.patterns, 1);
        assert_eq!(partial.masses.accepted, field("1/4"));
        assert_eq!(partial.masses.target_success, Some(field("1/8")));
        assert_eq!(partial.masses.error, field("1/8"));
        cutoff.noise.max_weight = 2;
        let complete = evaluator.enumerate_physical(&cutoff).unwrap();
        assert_eq!(
            complete.covered_probability.value(),
            &"1".parse::<BigRational>().unwrap()
        );
        assert_eq!(
            complete.omitted_probability.value(),
            &"0".parse::<BigRational>().unwrap()
        );
        assert_eq!(complete.patterns, 8);
        assert_eq!(complete.masses.accepted, field("1/2"));
        assert_eq!(complete.masses.target_success, Some(field("1/4")));
        assert_eq!(complete.masses.error, field("1/4"));
        assert_eq!(evaluator.compilation(), &before);
        assert_eq!(meter.snapshot().compilation.succeeded, 2);
        assert_eq!(meter.snapshot().query.succeeded, 41);
        assert_eq!(meter.snapshot().query.failed, 0);
        assert_eq!(
            (
                evaluator.prepared_probability_statistics().unwrap(),
                evaluator.prepared_density_statistics().unwrap()
            ),
            warm.unwrap()
        );
    }
}
#[test]
fn original_unit_target_reuses_one_overlap_family_for_all_physical_paulis() {
    let a = ScalarArithmetic::default();
    let empty = BitVector::zeros(0).unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source = parse_circuit("H 0\nT 0", &ParseOptions::default()).unwrap();
        let options = CompileOptions {
            compiler_strategy: compiler,
            ..Default::default()
        };
        let protocol = compile_unitary(&source, &options, &a).unwrap();
        let target = compile_unitary(&source, &options, &a).unwrap();
        let layout = DensityLayout {
            outputs: &[0],
            event_rows: &[],
        };
        // The original X_ERROR anchor is after H/T. Arbitrary supplied Pauli
        // assignments use the public reusable unitary insertion/overlap family.
        for fixed in [false, true] {
            let plan = if fixed {
                protocol
                    .prepare_density_overlap_with_fibers(
                        layout,
                        &target,
                        layout,
                        1,
                        Default::default(),
                        &a,
                    )
                    .unwrap()
            } else {
                protocol
                    .prepare_density_overlap(layout, &target, layout, &a)
                    .unwrap()
            };
            let resource = plan
                .fixed_phase_resource()
                .map(|r| (r as *const _ as usize, r.quotient_rank(), r.num_terms()));
            if let Some((_, rank, terms)) = resource {
                assert_eq!(rank, 0);
                assert!(terms <= 1);
            }
            for (pauli, expected) in [
                ('I', ExactScalar::one()),
                ('X', scalar(1, -1)),
                ('Y', scalar(1, -1)),
                ('Z', ExactScalar::zero()),
            ] {
                let faults = if pauli == 'I' {
                    vec![]
                } else {
                    vec![PauliInsertion {
                        before_gate: 2,
                        word: PauliWord::new(
                            bits(1, usize::from(pauli == 'X' || pauli == 'Y')),
                            bits(1, usize::from(pauli == 'Z' || pauli == 'Y')),
                            if pauli == 'Y' { 2 } else { 0 },
                        )
                        .unwrap(),
                    }]
                };
                let view = protocol.with_insertions(&faults, &a).unwrap();
                for _ in 0..4 {
                    let value = plan
                        .evaluate(&view, &target, &empty, &empty, 1, &a)
                        .unwrap();
                    assert_eq!(value.value, expected);
                    assert_eq!(value.quotient_rank, 0);
                    assert!(value.terms <= 1);
                    assert_eq!(
                        plan.fixed_phase_resource().map(|r| (
                            r as *const _ as usize,
                            r.quotient_rank(),
                            r.num_terms()
                        )),
                        resource
                    );
                }
            }
        }
    }
}
#[test]
fn original_many_outputs_trace_before_one_term_contraction() {
    let files = Files::new();
    for mut options in configs() {
        options.runtime.sampling.max_terms = 1;
        for width in [1, 4, 16, 64] {
            let wires = (0..width)
                .map(|q| q.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let target = format!("H {wires}\nT {wires}");
            let physical = format!(
                "{target}\nZ_ERROR(0) 0\nH {width}\nM {width}\nDETECTOR rec[-1]\nPOSTSELECT det[0]"
            );
            let (evaluator, meter) = files
                .evaluator(
                    &physical,
                    &target,
                    None,
                    json!({"outputs":(0..width).collect::<Vec<_>>()}),
                    &ControllerRegistry::default(),
                    options,
                )
                .unwrap();
            let mut warm = None;
            for fault in [false, true] {
                let selections = if fault {
                    vec![FaultSelection {
                        site: 0,
                        alternative: 0,
                    }]
                } else {
                    vec![]
                };
                for seed in 0..4 {
                    let draw = evaluator
                        .sample_prescribed(&selections, &mut SplitMix64::new(seed))
                        .unwrap();
                    assert_eq!(draw.raw_acceptance, scalar(1, -1));
                    assert_eq!(
                        draw.raw_target_success,
                        Some(if fault {
                            ExactScalar::zero()
                        } else {
                            scalar(1, -1)
                        })
                    );
                    assert_eq!(
                        draw.raw_error,
                        if fault {
                            scalar(1, -1)
                        } else {
                            ExactScalar::zero()
                        }
                    );
                    assert_eq!(draw.accepted, field("1/2"));
                    assert_eq!(
                        draw.error,
                        if fault {
                            field("1/2")
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(draw.history_mass, ExactScalar::one());
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(draw.network_compilations, 0);
                    assert_eq!(draw.acceptance_resources.quotient_rank, 0);
                    assert!(draw.acceptance_resources.terms <= 1);
                    let resource = draw.target_resources.unwrap();
                    assert_eq!(resource.quotient_rank, 0);
                    assert!(resource.terms <= 1);
                    let stats = (
                        evaluator.prepared_probability_statistics().unwrap(),
                        evaluator.prepared_density_statistics().unwrap(),
                    );
                    if let Some(previous) = warm {
                        assert_eq!(stats, previous);
                    } else {
                        warm = Some(stats);
                    }
                }
            }
            assert_eq!(meter.snapshot().compilation.succeeded, 2);
        }
    }
}
#[test]
fn original_terminal_and_decoder_selects_composed_causal_route_without_preparation_calls() {
    let files = Files::new();
    for base in configs() {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
                let observed = calls.clone();
                let mut registry = ControllerRegistry::default();
                registry
                    .register("and", "1", move |bits: &[bool]| {
                        assert_eq!(bits.len(), 2);
                        observed.lock().unwrap().push(bits.to_vec());
                        Ok(vec![bits[0] & bits[1]])
                    })
                    .unwrap();
                let mut options = base;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                let physical = parse_circuit("I 0", &ParseOptions::default()).unwrap();
                assert!(!physical.needs_history_sampling());
                let (evaluator,meter)=files.evaluator("I 0","I 0",Some("H 1 2\nM 1 2\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCX ctrl[0] 0"),json!({}),&registry,options).unwrap();
                assert!(evaluator.sampled_history());
                assert_eq!(
                    evaluator.execution_route(),
                    if causal == CausalStrategy::FixedTemplate {
                        "causal_fixed_template"
                    } else {
                        "causal_cold_prefix"
                    }
                );
                assert!(calls.lock().unwrap().is_empty());
                for seed in 3..7 {
                    let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
                    assert_eq!(draw.controller_calls.len(), 1);
                    let input = &draw.controller_calls[0].inputs;
                    assert_eq!(
                        draw.error,
                        if input[0] & input[1] {
                            ExactField::one()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(draw.accepted, ExactField::one());
                    if causal == CausalStrategy::FixedTemplate {
                        assert_eq!(draw.network_compilations, 0);
                    }
                }
                assert_eq!(calls.lock().unwrap().len(), 4);
                calls.lock().unwrap().clear();
                let mut enumeration = TaskEnumerationOptions::default();
                enumeration.noise.max_weight = 0;
                let result = evaluator.enumerate_physical(&enumeration).unwrap();
                assert_eq!(result.patterns, 1);
                assert_eq!(result.histories, 4);
                assert_eq!(result.masses.accepted, ExactField::one());
                assert_eq!(result.masses.error, field("1/4"));
                assert_eq!(result.masses.target_success, Some(field("3/4")));
                let mut inputs = calls.lock().unwrap().clone();
                inputs.sort();
                inputs.dedup();
                assert_eq!(
                    inputs,
                    [
                        vec![false, false],
                        vec![false, true],
                        vec![true, false],
                        vec![true, true]
                    ]
                );
                assert_eq!(meter.snapshot().compilation.succeeded, 2);
            }
        }
    }
}
#[test]
fn original_target_unitary_acts_only_on_selected_outputs_in_declared_order() {
    let files = Files::new();
    for options in configs() {
        let (evaluator, _) = files
            .evaluator(
                "H 4\nT 4\nCX 4 2",
                "H 0\nT 0\nCX 0 1",
                None,
                json!({"outputs":[4,2]}),
                &ControllerRegistry::default(),
                options,
            )
            .unwrap();
        let draw = evaluator.sample(&mut SplitMix64::new(0)).unwrap();
        assert_eq!(draw.error, ExactField::zero());
        assert_eq!(draw.target_success, Some(ExactField::one()));
        for (outputs, error) in [(vec![4, 2], "0"), (vec![2, 4], "1")] {
            let (evaluator, _) = files
                .evaluator(
                    "I 4\nX 2",
                    "I 0\nX 1",
                    None,
                    json!({"outputs":outputs}),
                    &ControllerRegistry::default(),
                    options,
                )
                .unwrap();
            assert_eq!(
                evaluator.sample(&mut SplitMix64::new(0)).unwrap().error,
                field(error)
            );
        }
        for invalid in ["H 2", "M 0", "R 0", "X_ERROR(1/10) 0"] {
            let error = match files.evaluator(
                "I 0 1",
                invalid,
                None,
                json!({}),
                &ControllerRegistry::default(),
                options,
            ) {
                Err(e) => e,
                Ok(_) => panic!("invalid original target module accepted: {invalid}"),
            };
            assert!(error.field.starts_with("ideal_state"), "{error}");
        }
    }
}
