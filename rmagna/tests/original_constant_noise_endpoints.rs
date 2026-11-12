// SPDX-License-Identifier: MPL-2.0
// Complete original ConditionalNoiseTests.test_declared_constant_guards_and_channel_endpoints
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py, exercised through the Rust public task API.

use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionMeter};
use rmagna::field::ExactField;
use rmagna::noise::RandomSource;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;

struct TargetFile(PathBuf);

impl TargetFile {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "rmagna-original-constant-noise-{}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("target.magna"), "I 0").unwrap();
        Self(directory)
    }
}

impl Drop for TargetFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("constant guards and endpoint probabilities require no entropy");
    }
}

fn physical_source(probability: u8, constant: bool) -> String {
    let mut source = String::from("M 0\nCONTROL[XOR] rec[-1] rec[-1] -> ctrl[0]\n");
    if constant {
        source.push_str("CONTROL[NOT] ctrl[0] -> ctrl[1]\n");
    }
    source.push_str(&format!(
        "X_ERROR({probability}) ctrl[{}] 1",
        u8::from(constant)
    ));
    source
}

#[test]
fn original_declared_constant_guards_and_channel_endpoints() {
    let files = TargetFile::new();
    let parse = ParseOptions::default();
    let task = parse_task(
        r#"{"outputs":[1],"ideal_state":"target.magna","observables":[[0,1]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let registry = ControllerRegistry::default();
    let one = BigRational::from_integer(1.into());
    let zero = BigRational::from_integer(0.into());
    let mut configurations = 0;
    for (probability, constant, expected_error) in [
        (0, false, false),
        (0, true, false),
        (1, false, false),
        (1, true, true),
    ] {
        let physical = parse_circuit(&physical_source(probability, constant), &parse).unwrap();
        let modules = task.load_modules(&physical, &parse).unwrap();
        let (error, success, z) = if expected_error {
            (
                ExactField::one(),
                ExactField::zero(),
                ExactField::one().negated(),
            )
        } else {
            (ExactField::zero(), ExactField::one(), ExactField::one())
        };
        let (raw_error, raw_success, raw_z) = if expected_error {
            (
                ExactScalar::one(),
                ExactScalar::zero(),
                ExactScalar::one().negated(),
            )
        } else {
            (ExactScalar::zero(), ExactScalar::one(), ExactScalar::one())
        };

        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for (strategy, route) in [
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
                    let mut limits = TaskEnumerationOptions::default();
                    limits.noise.max_weight = 1;
                    let full = evaluator.enumerate_physical(&limits).unwrap();

                    // The controls change only the circuit passed to the parser.
                    assert_eq!(full.masses.error, error, "original exact endpoint error");
                    assert_eq!(full.masses.accepted, ExactField::one());
                    assert_eq!(full.masses.target_success, Some(success.clone()));
                    assert_eq!(full.masses.moments.as_slice(), std::slice::from_ref(&z));
                    assert_eq!(full.covered_probability.value(), &one);
                    assert_eq!(full.omitted_probability.value(), &zero);
                    assert_eq!(full.target_normalization, Some(ExactScalar::one()));

                    // Public route and compiler diagnostics stand in for native
                    // private query parents. Sampling must leave them stable.
                    assert_eq!(physical.num_records(), 1);
                    assert_eq!(physical.external_calls(), 0);
                    assert!(evaluator.sampled_history());
                    assert_eq!(evaluator.execution_route(), route);
                    let compilation = evaluator.compilation().clone();
                    assert_eq!(compilation.strategy, compiler);
                    let meter = ExecutionMeter::default();
                    let draw = evaluator.sample_with_costs(&mut NoEntropy, &meter).unwrap();
                    assert_eq!(evaluator.compilation(), &compilation);
                    assert_eq!(meter.snapshot().decoder, ExecutionCosts::default().decoder);
                    assert_eq!(draw.history_mass, ExactScalar::one());
                    assert_eq!(draw.history_weight, ExactField::one());
                    assert_eq!(draw.noise_probability, one);
                    assert_eq!(draw.noise_likelihood_ratio, one);
                    assert_eq!(draw.raw_acceptance, ExactScalar::one());
                    assert_eq!(draw.raw_error, raw_error);
                    assert_eq!(draw.raw_target_success.as_ref(), Some(&raw_success));
                    assert_eq!(draw.raw_moments.as_slice(), std::slice::from_ref(&raw_z));
                    assert_eq!(draw.accepted, ExactField::one());
                    assert_eq!(draw.error, error);
                    assert_eq!(draw.target_success.as_ref(), Some(&success));
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(draw.moments.as_slice(), std::slice::from_ref(&z));
                    assert!(draw.sampled_records.is_empty());
                    assert!(draw.controller_calls.is_empty());
                    configurations += 1;
                }
            }
        }
    }
    assert_eq!(configurations, 32);
}
