// SPDX-License-Identifier: MPL-2.0
// Complete original ConditionalNoiseTests.test_noncommuting_guard_chain_and_target_route
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py; independent Rust public-API witness.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionMeter};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;

struct TargetFile(PathBuf);
impl TargetFile {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-noncommuting-chain-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "I 0").unwrap();
        Self(path)
    }
}
impl Drop for TargetFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn original_source() -> &'static str {
    "H 0\nT 0\nH 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nM 1\nX_ERROR(1/2) rec[-1] 2\nM 2"
}

#[test]
fn original_noncommuting_guard_chain_and_target_route() {
    let files = TargetFile::new();
    let parse = ParseOptions::default();
    let physical = parse_circuit(original_source(), &parse).unwrap();
    let task = parse_task(
        r#"{"outputs":[2],"ideal_state":"target.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task.load_modules(&physical, &parse).unwrap();
    let registry = ControllerRegistry::default();
    let arithmetic = FieldArithmetic::default();
    // omega - omega^3 = sqrt(2): (2 - sqrt(2))/24.
    let coefficients = [
        rational(1, 12),
        rational(-1, 24),
        rational(0, 1),
        rational(1, 24),
    ];
    let expected_error = arithmetic.value(coefficients.clone()).unwrap();
    let first_one = arithmetic
        .value([
            rational(1, 2),
            rational(-1, 4),
            rational(0, 1),
            rational(1, 4),
        ])
        .unwrap();
    let scalar = ScalarArithmetic::default();
    let histories = [
        scalar
            .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
            .unwrap(),
        scalar
            .scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
            .unwrap(),
    ];
    let mut sampling_calls = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                let enumeration =
                    TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                let mut limits = TaskEnumerationOptions::default();
                limits.noise.max_weight = 2;
                limits.include_patterns = true;
                let full = enumeration.enumerate_physical(&limits).unwrap();
                // Both external input controls must fail this original oracle.
                assert_eq!(
                    full.masses.error, expected_error,
                    "original exact physical error"
                );
                assert_eq!(full.masses.error.coefficients(), &coefficients);
                assert_eq!(full.masses.accepted, ExactField::one());
                let target = full.masses.target_success.as_ref().unwrap();
                assert_eq!(
                    arithmetic.add(&full.masses.error, target).unwrap(),
                    ExactField::one()
                );
                assert_eq!(full.covered_probability.value(), &rational(1, 1));
                assert_eq!(full.omitted_probability.value(), &rational(0, 1));
                assert_eq!(full.target_normalization, Some(ExactScalar::one()));
                assert_eq!((full.patterns, full.histories), (4, 8));
                assert!(full.masses.moments.is_empty());
                let patterns = full.pattern_responses.unwrap();
                for pattern in &patterns {
                    let both = pattern.choices.len() == 2;
                    assert_eq!(pattern.histories, 2);
                    assert_eq!(pattern.response.accepted, ExactField::one());
                    assert_eq!(
                        pattern.response.error,
                        if both {
                            first_one.clone()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(
                        arithmetic
                            .add(
                                &pattern.response.error,
                                pattern.response.target_success.as_ref().unwrap()
                            )
                            .unwrap(),
                        ExactField::one()
                    );
                }
                if strategy == CausalStrategy::ColdPrefix {
                    continue;
                }
                // A new evaluator isolates reusable sampling families from enumeration.
                // Native's three private parents are represented by explicit Rust
                // history/support, acceptance and target families, not equal counters.
                options.runtime.compile.probability_preparation =
                    QueryPreparation::Native(Default::default());
                options.runtime.compile.density_preparation =
                    QueryPreparation::Native(Default::default());
                let sampling = TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                let meter = ExecutionMeter::default();
                let prepared = sampling.prepare_sampling_queries(Default::default(), &meter);
                assert_eq!(prepared.result, Ok(()));
                assert!(prepared.all_families_retained());
                assert_eq!(prepared.counts.cold, 0);
                assert_eq!(prepared.counts.failed, 0);
                assert_eq!(prepared.by_kind["acceptance"].planned, 1);
                assert_eq!(prepared.by_kind["target_density"].planned, 1);
                assert!(prepared.by_kind["reported_probability"].planned > 0);
                if sampler == HistorySampler::AffineSupport {
                    assert_eq!(prepared.by_kind["reported_support"].planned, 1);
                }
                assert_eq!(
                    meter.snapshot(),
                    ExecutionCosts::default(),
                    "preparation is not execution"
                );
                let compilation = sampling.compilation().clone();
                let probability = sampling.prepared_probability_statistics().unwrap();
                let density = sampling.prepared_density_statistics().unwrap();
                let support = sampling.prepared_support_statistics().unwrap();
                assert!(probability.unwrap().cached_queries > 0);
                assert!(density.unwrap().cached_queries > 0);
                let mut random = SplitMix64::new(13);
                for _ in 0..16 {
                    let draw = sampling.sample(&mut random).unwrap();
                    sampling_calls += 1;
                    assert_eq!(
                        draw.network_compilations, 0,
                        "sampling reused prepared networks"
                    );
                    assert_eq!(sampling.compilation(), &compilation);
                    assert_eq!(
                        sampling.prepared_probability_statistics().unwrap(),
                        probability
                    );
                    assert_eq!(sampling.prepared_density_statistics().unwrap(), density);
                    assert_eq!(sampling.prepared_support_statistics().unwrap(), support);
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(
                        arithmetic
                            .add(&draw.error, draw.target_success.as_ref().unwrap())
                            .unwrap(),
                        draw.accepted
                    );
                    assert_eq!(draw.accepted, draw.history_weight);
                    assert_eq!(draw.noise_likelihood_ratio, rational(1, 1));
                    assert!(histories.contains(&draw.history_mass));
                    assert_eq!(draw.raw_acceptance, draw.history_mass);
                    assert!(
                        draw.raw_error == ExactScalar::zero()
                            || draw.raw_error == draw.history_mass
                    );
                    assert_eq!(
                        scalar
                            .add(&draw.raw_error, draw.raw_target_success.as_ref().unwrap())
                            .unwrap(),
                        draw.raw_acceptance
                    );
                    assert!(draw.sampled_records.is_empty());
                    assert!(draw.controller_calls.is_empty());
                }
            }
        }
    }
    assert_eq!(sampling_calls, 64);
}
