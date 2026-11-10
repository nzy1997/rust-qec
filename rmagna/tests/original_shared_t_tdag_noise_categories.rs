// SPDX-License-Identifier: MPL-2.0
// Complete original ConditionalNoiseTests.test_shared_t_tdag_noise_has_no_guarded_route_or_virtual_noise
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py, exercised through Rust public APIs.

use num_rational::BigRational;
use rmagna::circuit::{Basis, Operation, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::{
    OriginalNoiseAction, OriginalNoiseOptions, SplitMix64, original_noise_categories,
};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct TargetFile(PathBuf);

impl TargetFile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-shared-t-tdag-{}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "H 0\nT 0").unwrap();
        Self(path)
    }
}

impl Drop for TargetFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rational(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(numerator.into(), denominator.into())
}

fn field(numerator: i64, denominator: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(numerator, denominator))
        .unwrap()
}

fn sqrt_two_over(denominator: i64) -> ExactField {
    // sqrt(2) = omega - omega^3 in the public field basis.
    FieldArithmetic::default()
        .value([
            rational(0, 1),
            rational(1, denominator),
            rational(0, 1),
            rational(-1, denominator),
        ])
        .unwrap()
}

fn physical_source(external_echo: bool) -> String {
    let (middle, control) = if external_echo {
        (
            "DETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\n",
            "ctrl[0]",
        )
    } else {
        ("", "rec[-1]")
    };
    format!("H 0 1\nM 0\n{middle}T 1\nCS_DAG {control} 1\nZ_ERROR(1/4) 1")
}

#[test]
fn original_shared_t_tdag_noise_has_no_guarded_route_or_virtual_noise() {
    let files = TargetFile::new();
    let calls = Arc::new(Mutex::new(Vec::<bool>::new()));
    let mut registry = ControllerRegistry::default();
    let echo_calls = Arc::clone(&calls);
    registry
        .register("echo", "1", move |bits: &[bool]| {
            assert_eq!(bits.len(), 1);
            echo_calls.lock().unwrap().push(bits[0]);
            Ok(bits.to_vec())
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..ParseOptions::default()
    };
    let task = parse_task(
        r#"{"outputs":[1],"ideal_state":"target.magna","observables":[[1,0],[1,1],[0,1]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let quarter = rational(1, 4);
    let three_quarters = rational(3, 4);
    let half = ScalarArithmetic::default().omega(0, -2).unwrap();
    let zero = ExactField::zero();
    let one = ExactField::one();
    let axis = sqrt_two_over(2);
    let full_moments = [sqrt_two_over(4), zero.clone(), zero.clone()];
    let mut configurations = 0;
    let mut draws = 0;

    for external_echo in [false, true] {
        let physical = parse_circuit(&physical_source(external_echo), &parse).unwrap();
        let categories =
            original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
        assert_eq!(categories.len(), 1, "original source noise category count");
        let category = &categories[0];
        let noise = physical.operations().last().unwrap();
        let measure = physical
            .operations()
            .iter()
            .find(|operation| matches!(&operation.operation, Operation::Measure { .. }))
            .unwrap();
        assert!(!physical.readout_noise_declared(measure.id));
        assert_eq!(category.internal_site, 1);
        assert!(category.guard.is_none());
        assert_eq!(category.identity_probability.value(), &three_quarters);
        assert_eq!(category.channels.len(), 1);
        let channel = &category.channels[0];
        assert_eq!(
            category.source_operations.as_slice(),
            std::slice::from_ref(&channel.source)
        );
        assert_eq!(channel.source.operation, noise.id);
        assert_eq!(channel.source.location, noise.location);
        assert_eq!(channel.probability.value(), &quarter);
        assert_eq!(
            channel.action,
            OriginalNoiseAction::Pauli {
                factors: vec![(1, Basis::Z)]
            }
        );
        assert_eq!(physical.external_calls(), usize::from(external_echo));
        let modules = task.load_modules(&physical, &parse).unwrap();

        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for (strategy, causal_route) in [
                (CausalStrategy::FixedTemplate, "causal_fixed_template"),
                (CausalStrategy::ColdPrefix, "causal_cold_prefix"),
            ] {
                for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                    let mut options = EvaluationOptions::default();
                    options.runtime.compile.compiler_strategy = compiler;
                    options.runtime.strategy = strategy;
                    options.runtime.history_sampler = sampler;
                    let evaluator =
                        TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();

                    // Direct record feedback stays in the linear instrument.
                    // Only the external controller needs a causal route.
                    assert_eq!(evaluator.sampled_history(), external_echo);
                    assert_eq!(
                        evaluator.execution_route(),
                        if external_echo {
                            causal_route
                        } else {
                            "linear_affine"
                        }
                    );
                    assert_eq!(evaluator.potential_site(measure.id), Some(0));
                    assert_eq!(evaluator.potential_site(noise.id), Some(1));
                    let compilation = evaluator.compilation().clone();
                    assert_eq!(compilation.strategy, compiler);

                    let mut limits = TaskEnumerationOptions::default();
                    limits.noise.max_weight = 1;
                    let full = evaluator.enumerate_physical(&limits).unwrap();
                    // The two measured values have equal probability. For a
                    // zero measured bit the errors for I and Z are 0 and 1. With
                    // the bit set the relative phase is -i and both errors
                    // are 1/2. The noise law weights I by 3/4 and Z by 1/4.
                    assert_eq!(full.masses.error, field(3, 8), "original full exact error");
                    assert_eq!(full.covered_probability.value(), &rational(1, 1));
                    assert_eq!(full.omitted_probability.value(), &rational(0, 1));
                    assert_eq!(full.masses.accepted, one);
                    assert_eq!(full.masses.target_success, Some(field(5, 8)));
                    assert_eq!(full.masses.moments, full_moments);
                    assert_eq!(full.target_normalization, Some(ExactScalar::one()));

                    for seed in 0..16 {
                        let before = calls.lock().unwrap().len();
                        let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
                        let trace = calls.lock().unwrap();
                        assert_eq!(trace.len() - before, usize::from(external_echo));
                        assert_eq!(draw.controller_calls.len(), usize::from(external_echo));
                        let bit = trace.get(before).copied();
                        if let Some(bit) = bit {
                            let call = &draw.controller_calls[0];
                            assert_eq!(call.call, 0);
                            assert_eq!(call.name, "echo");
                            assert_eq!(call.inputs, [bit]);
                            assert_eq!(call.outputs, [bit]);
                        }
                        let fault = draw.noise_probability == quarter;
                        if !fault {
                            assert_eq!(draw.noise_probability, three_quarters);
                        }
                        let error = match (bit, fault) {
                            (None, false) => field(1, 4),
                            (None, true) => field(3, 4),
                            (Some(false), false) => zero.clone(),
                            (Some(false), true) => one.clone(),
                            (Some(true), _) => field(1, 2),
                        };
                        let x = if fault { axis.negated() } else { axis.clone() };
                        let y = match bit {
                            None => zero.clone(),
                            Some(bit) if bit ^ fault => axis.negated(),
                            Some(_) => axis.clone(),
                        };
                        assert_eq!(draw.accepted, one);
                        assert_eq!(draw.error, error);
                        assert_eq!(
                            draw.target_success,
                            Some(FieldArithmetic::default().subtract(&one, &error).unwrap())
                        );
                        assert_eq!(draw.moments, [x, y, zero.clone()]);
                        assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                        assert_eq!(draw.history_weight, one);
                        assert_eq!(
                            draw.history_mass,
                            if external_echo {
                                half.clone()
                            } else {
                                ExactScalar::one()
                            }
                        );
                        assert_eq!(draw.noise_likelihood_ratio, rational(1, 1));
                        assert!(draw.sampled_records.is_empty());
                        assert_eq!(evaluator.compilation(), &compilation);
                        if strategy == CausalStrategy::FixedTemplate {
                            assert_eq!(draw.network_compilations, 0);
                        }
                        draws += 1;
                    }
                    configurations += 1;
                }
            }
        }
    }
    assert_eq!(configurations, 16);
    assert_eq!(draws, 256);
}
