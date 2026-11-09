// SPDX-License-Identifier: MPL-2.0
// Complete original ConditionalNoiseTests.test_parity_guard_does_not_require_all_raw_records
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py; independent Rust public-API witness.
use num_rational::BigRational;
use rmagna::circuit::{Basis, BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::compile_unitary;
use rmagna::noise::{
    OriginalNoiseAction, OriginalNoiseOptions, RandomSource, SplitMix64, original_noise_categories,
};
use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;

struct TargetFile(PathBuf);
impl TargetFile {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-parity-only-{}-{}",
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
struct CountedRandom {
    stream: SplitMix64,
    words: usize,
}
impl CountedRandom {
    fn new(seed: u64) -> Self {
        Self {
            stream: SplitMix64::new(seed),
            words: 0,
        }
    }
}
impl RandomSource for CountedRandom {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        self.stream.next_u64()
    }
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, 1))
        .unwrap()
}
fn original_source() -> &'static str {
    "H 0 1\nM 0 1\nCONTROL[XOR] rec[-2] rec[-1] -> ctrl[0]\nX_ERROR(1) ctrl[0] 2\nZ_ERROR(1) ctrl[0] 2"
}

#[test]
fn original_parity_guard_does_not_require_all_raw_records() {
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
    let scalar = ScalarArithmetic::default();
    let half = scalar.omega(0, -2).unwrap();
    let mut parity = BitExpr::atom(BitAtom::Record(0));
    parity.xor_assign(&BitExpr::atom(BitAtom::Record(1)));
    let mut paired_draws = 0;
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
                let program =
                    CausalProgram::new(&physical, &registry, options.runtime, &scalar).unwrap();
                let target = compile_unitary(
                    &parse_circuit("I 0", &parse).unwrap(),
                    &options.runtime.compile,
                    &scalar,
                )
                .unwrap();
                let evaluator =
                    TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                let compilation = evaluator.compilation().clone();
                // These streams pair Rust runtime and task draws only. Native's
                // Random(1) trajectory is not equated with SplitMix64.
                let mut low_random = CountedRandom::new(13);
                let mut task_random = CountedRandom::new(13);
                let mut seen = [false; 2];
                for _ in 0..16 {
                    let low_before = low_random.words;
                    let task_before = task_random.words;
                    let low = program
                        .sample(&[2], &target, &mut low_random, &scalar)
                        .unwrap();
                    let draw = evaluator.sample(&mut task_random).unwrap();
                    paired_draws += 1;
                    assert_eq!(low.history_mass, half);
                    let bit = low
                        .conditions
                        .first()
                        .expect("one actual guard observation")
                        .1;
                    let failed = if bit {
                        half.clone()
                    } else {
                        ExactScalar::zero()
                    };
                    assert_eq!(
                        low.masses.error, failed,
                        "original conditional physical error"
                    );
                    assert_eq!(low.masses.accepted.value, half);
                    assert_eq!(
                        low.masses.target_success.value,
                        if bit {
                            ExactScalar::zero()
                        } else {
                            half.clone()
                        }
                    );
                    // Two physical channels observe the same canonical XOR.
                    // A single raw bit has the same mean error, but must fail here.
                    assert_eq!(
                        low.conditions,
                        [(parity.clone(), bit), (parity.clone(), bit)],
                        "both channels share the canonical parity, without raw histories"
                    );
                    assert_eq!((low.noise_entropy_words, low.history_entropy_words), (0, 1));
                    assert_eq!(low_random.words - low_before, 1, "actual runtime entropy");
                    assert_eq!(task_random.words - task_before, 1, "actual task entropy");
                    assert_eq!(low_random.stream.state(), task_random.stream.state());
                    assert_eq!(
                        low.history_proposal_mass,
                        (sampler == HistorySampler::AffineSupport).then(|| half.clone())
                    );
                    assert_eq!(low.noise_probability, rational(1, 1));
                    assert_eq!(low.noise_likelihood_ratio, rational(1, 1));
                    assert!(low.controller_calls.is_empty());
                    assert_eq!(draw.raw_acceptance, low.masses.accepted.value);
                    assert_eq!(draw.raw_error, low.masses.error);
                    assert_eq!(
                        draw.raw_target_success,
                        Some(low.masses.target_success.value)
                    );
                    assert_eq!(draw.history_mass, half);
                    assert_eq!(draw.history_proposal_mass, low.history_proposal_mass);
                    assert_eq!(draw.history_weight, ExactField::one());
                    assert_eq!(draw.accepted, ExactField::one());
                    assert_eq!(draw.error, field(i64::from(bit)));
                    assert_eq!(draw.target_success, Some(field(i64::from(!bit))));
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(draw.noise_probability, rational(1, 1));
                    assert_eq!(draw.noise_likelihood_ratio, rational(1, 1));
                    assert!(draw.sampled_records.is_empty());
                    assert!(draw.controller_calls.is_empty());
                    assert!(draw.moments.is_empty() && draw.raw_moments.is_empty());
                    if strategy == CausalStrategy::FixedTemplate {
                        assert_eq!(draw.network_compilations, 0);
                        assert_eq!(evaluator.compilation(), &compilation);
                    }
                    seen[usize::from(bit)] = true;
                }
                assert_eq!(seen, [true, true], "both coarse histories were sampled");
            }
        }
    }
    assert_eq!(paired_draws, 128);
    assert_eq!(physical.num_records(), 2);
    assert_eq!(physical.external_calls(), 0);
    let categories =
        original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 2);
    for (category, site, basis) in [(&categories[0], 2, Basis::X), (&categories[1], 3, Basis::Z)] {
        // The two measurements occupy implicit internal sites 0 and 1.
        assert_eq!(category.internal_site, site);
        assert_eq!(category.guard, Some(parity.clone()));
        assert_eq!(category.identity_probability.value(), &rational(0, 1));
        assert_eq!(category.channels.len(), 1);
        assert_eq!(category.channels[0].probability.value(), &rational(1, 1));
        assert_eq!(
            category.channels[0].action,
            OriginalNoiseAction::Pauli {
                factors: vec![(2, basis)]
            }
        );
    }
    // Native's private parent-list length is verified in the original baseline.
    // Rust's public proof is shared conditions, one entropy word and exact masses.
}
