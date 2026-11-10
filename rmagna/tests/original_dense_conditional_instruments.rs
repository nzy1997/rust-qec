// SPDX-License-Identifier: MPL-2.0
// Original ConditionalNoiseTests.test_exact_dense_instruments_and_sampling
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py, exercised through Rust public APIs.

use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionMeter};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::cmp::Ordering;
use std::path::PathBuf;

const DRAWS_PER_CASE: u32 = 512;

struct TargetFile(PathBuf);

impl TargetFile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-dense-conditional-{}",
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

const AND_INPUTS: &str = "DETECTOR rec[-2]\nDETECTOR rec[-1]\n";
const AFFINE: &str = "CONTROL[XOR] rec[-2] rec[-1] -> ctrl[0]\n";
const EXTERNAL: &str = "CONTROL[and] det[-2] det[-1] -> ctrl[0]\n";
const MIXED: &str =
    "CONTROL[and] det[-2] det[-1] -> ctrl[1]\nCONTROL[XOR] ctrl[1] rec[-2] -> ctrl[0]\n";

fn physical_source(control: &str, feedback: &str, detector: usize) -> String {
    format!(
        "H 0 1 2\nT 0 2\nH 0\nM(1/4) 0 1\n{control}{feedback} ctrl[0] 2\n\
         CORRELATED_ERROR(1/4) ctrl[0] Y2 X1\n\
         ELSE_CORRELATED_ERROR(1/3) ctrl[0] Z2\n\
         Z_ERROR(0) ctrl[0] 2\nZ_ERROR(1) ctrl[0] 2\n\
         M(1/5) 1\nX_ERROR(1/3) rec[-1] 2\n\
         DETECTOR rec[-3] rec[-1]\nPOSTSELECT det[{detector}]"
    )
}

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

type RealCoefficients = [(i64, i64); 2];

fn real(coefficients: RealCoefficients) -> ExactField {
    // The independently enumerated pair is in the basis (1, sqrt(2)).
    let [a, b] = coefficients.map(|(n, d)| rational(n, d));
    FieldArithmetic::default()
        .value([a, b.clone(), rational(0, 1), -b])
        .unwrap()
}

fn assert_original_mean(label: &str, quantity: &str, sum: &ExactField, expected: &ExactField) {
    let arithmetic = FieldArithmetic::default();
    let mean = arithmetic
        .scale(sum, &rational(1, i64::from(DRAWS_PER_CASE)))
        .unwrap();
    let delta = arithmetic.subtract(&mean, expected).unwrap();
    let tolerance = real([(13, 200), (0, 1)]);
    for signed in [&delta, &delta.negated()] {
        assert_ne!(
            arithmetic
                .subtract(signed, &tolerance)
                .unwrap()
                .real_sign()
                .unwrap(),
            Ordering::Greater,
            "{label} {quantity} mean {mean:?} versus {expected:?}"
        );
    }
}

fn check_sampling(
    sampling: &TaskEvaluator,
    label: &str,
    accepted: &ExactField,
    error: &ExactField,
    external: bool,
) {
    let meter = ExecutionMeter::default();
    let prepared = sampling.prepare_sampling_queries(Default::default(), &meter);
    assert_eq!(prepared.result, Ok(()), "{label}");
    assert!(prepared.all_families_retained(), "{label}");
    assert_eq!(prepared.counts.failed, 0, "{label}");
    assert_eq!(prepared.by_kind["acceptance"].planned, 1, "{label}");
    assert_eq!(prepared.by_kind["target_density"].planned, 1, "{label}");
    assert_eq!(prepared.by_kind["reported_support"].planned, 1, "{label}");
    assert!(
        prepared.by_kind["reported_probability"].planned > 0,
        "{label}"
    );
    assert_eq!(meter.snapshot(), ExecutionCosts::default(), "{label}");
    let compilation = sampling.compilation().clone();
    let probability = sampling.prepared_probability_statistics().unwrap();
    let density = sampling.prepared_density_statistics().unwrap();
    let support = sampling.prepared_support_statistics().unwrap();
    assert!(probability.as_ref().unwrap().cached_queries > 0, "{label}");
    assert!(density.as_ref().unwrap().cached_queries > 0, "{label}");
    assert!(support.as_ref().unwrap().labels > 0, "{label}");

    let arithmetic = FieldArithmetic::default();
    let mut rng = SplitMix64::new(291);
    let mut accepted_sum = ExactField::zero();
    let mut error_sum = ExactField::zero();
    for _ in 0..DRAWS_PER_CASE {
        let draw = sampling.sample_with_costs(&mut rng, &meter).unwrap();
        assert_eq!(draw.network_compilations, 0, "{label}");
        assert_eq!(sampling.compilation(), &compilation, "{label}");
        assert_eq!(
            sampling.prepared_probability_statistics().unwrap(),
            probability,
            "{label}"
        );
        assert_eq!(
            sampling.prepared_density_statistics().unwrap(),
            density,
            "{label}"
        );
        assert_eq!(
            sampling.prepared_support_statistics().unwrap(),
            support,
            "{label}"
        );
        assert!(draw.target_resources.is_some(), "{label}");
        assert_eq!(
            draw.target_normalization,
            Some(ExactScalar::one()),
            "{label}"
        );
        // Affine support proposes histories uniformly on its current support.
        // The draw must carry the exact correction for the biased physical law.
        let proposal = arithmetic
            .from_scalar(draw.history_proposal_mass.as_ref().unwrap())
            .unwrap();
        assert_eq!(proposal.real_sign().unwrap(), Ordering::Greater, "{label}");
        assert_eq!(
            arithmetic
                .multiply(&proposal, &draw.history_weight)
                .unwrap(),
            arithmetic.from_scalar(&draw.history_mass).unwrap(),
            "{label}"
        );
        assert_eq!(draw.noise_likelihood_ratio, rational(1, 1), "{label}");
        assert!(draw.sampled_records.is_empty(), "{label}");
        assert_eq!(
            draw.controller_calls.len(),
            usize::from(external),
            "{label}"
        );
        assert_eq!(
            arithmetic
                .add(&draw.error, draw.target_success.as_ref().unwrap())
                .unwrap(),
            draw.accepted,
            "{label}"
        );
        accepted_sum = arithmetic.add(&accepted_sum, &draw.accepted).unwrap();
        error_sum = arithmetic.add(&error_sum, &draw.error).unwrap();
    }
    let costs = meter.snapshot();
    assert_eq!(costs.compilation.attempted, 0, "{label}");
    assert_eq!(costs.query.failed, 0, "{label}");
    assert!(
        costs.query.succeeded >= 2 * u64::from(DRAWS_PER_CASE),
        "{label}"
    );
    assert_original_mean(label, "acceptance", &accepted_sum, accepted);
    assert_original_mean(label, "error", &error_sum, error);
}

#[test]
fn original_dense_conditional_instruments_and_sampling() {
    let files = TargetFile::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", |bits: &[bool]| {
            let [a, b] = bits else {
                panic!("original AND requires two reported inputs");
            };
            Ok(vec![*a && *b])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..ParseOptions::default()
    };
    let task = parse_task(
        r#"{"outputs":[2],"ideal_state":"target.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let external = format!("{AND_INPUTS}{EXTERNAL}");
    let mixed = format!("{AND_INPUTS}{MIXED}");
    // Exact branch weights were independently summed and cross-checked against
    // the original matrix fixture and the original native enumeration.
    let cases = [
        (
            "affine",
            AFFINE,
            0,
            [(43, 80), (0, 1)],
            [[(33, 320), (-3, 320)], [(157, 960), (-1, 240)]],
        ),
        (
            "external",
            external.as_str(),
            2,
            [(77, 160), (3, 640)],
            [[(11, 128), (-11, 512)], [(167, 1920), (-167, 7680)]],
        ),
        (
            "mixed",
            mixed.as_str(),
            2,
            [(83, 160), (-3, 640)],
            [[(9, 128), (-9, 512)], [(173, 1920), (-173, 7680)]],
        ),
    ];
    let mut enumerations = 0;
    let mut draws = 0;
    let arithmetic = FieldArithmetic::default();
    for (mode, control, detector, acceptance_coefficients, errors) in cases {
        let accepted = real(acceptance_coefficients);
        for (feedback, error_coefficients) in ["CX", "CS"].into_iter().zip(errors) {
            let error = real(error_coefficients);
            let success = arithmetic.subtract(&accepted, &error).unwrap();
            let physical =
                parse_circuit(&physical_source(control, feedback, detector), &parse).unwrap();
            let modules = task.load_modules(&physical, &parse).unwrap();
            for compiler in [
                CompilerStrategy::ParityCopies,
                CompilerStrategy::SharedCommutingLayer,
            ] {
                let label = format!("{mode} {feedback} {compiler:?}");
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.history_sampler = HistorySampler::AffineSupport;
                let mut limits = TaskEnumerationOptions::default();
                limits.noise.max_weight = 7;
                for (strategy, route) in [
                    (CausalStrategy::FixedTemplate, "causal_fixed_template"),
                    (CausalStrategy::ColdPrefix, "causal_cold_prefix"),
                ] {
                    options.runtime.strategy = strategy;
                    let evaluation =
                        TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                    assert_eq!(evaluation.execution_route(), route, "{label}");
                    let full = evaluation.enumerate_physical(&limits).unwrap();
                    assert_eq!(
                        full.masses.accepted, accepted,
                        "original dense exact acceptance {label}"
                    );
                    assert_eq!(
                        full.masses.error, error,
                        "original dense exact error {label}"
                    );
                    assert_eq!(
                        full.masses.target_success.as_ref(),
                        Some(&success),
                        "{label}"
                    );
                    assert_eq!(full.covered_probability.value(), &rational(1, 1), "{label}");
                    assert_eq!(full.omitted_probability.value(), &rational(0, 1), "{label}");
                    assert_eq!(
                        full.target_normalization,
                        Some(ExactScalar::one()),
                        "{label}"
                    );
                    enumerations += 1;
                }
                options.runtime.strategy = CausalStrategy::FixedTemplate;
                options.runtime.compile.probability_preparation =
                    QueryPreparation::Native(Default::default());
                options.runtime.compile.density_preparation =
                    QueryPreparation::Native(Default::default());
                let sampling = TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                check_sampling(&sampling, &label, &accepted, &error, detector != 0);
                draws += DRAWS_PER_CASE;
            }
        }
    }
    assert_eq!(enumerations, 24);
    assert_eq!(draws, 6144);
}
