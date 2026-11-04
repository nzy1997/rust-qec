// SPDX-License-Identifier: MPL-2.0
// Complete finite contract from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_importance_sampling.py:
// ImportanceSamplingTests.test_all_fault_configurations_recover_exact_physical_acceptance_and_error.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::{NoiseSampling, RandomSource};
use rmagna::runtime::ControllerRegistry;
use rmagna::scalar::ExactScalar;
use rmagna::task::{EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const CIRCUIT: &str = "X_ERROR(1/10) 0\nM 0\nX_ERROR(1/5) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
const PHYSICAL: [&str; 4] = ["18/25", "9/50", "2/25", "1/50"];
// Each admitted fault word lies in the exact integer categorical bucket.
// Identity uses word zero; all four configurations consume two actual words.
#[derive(Clone, Copy)]
struct ProposalCase {
    scale: &'static str,
    fault_words: [u64; 2],
    probabilities: [&'static str; 4],
    likelihoods: [&'static str; 4],
}
const PROPOSALS: [ProposalCase; 3] = [
    ProposalCase {
        scale: "1/2",
        fault_words: [19, 9],
        probabilities: ["171/200", "19/200", "9/200", "1/200"],
        likelihoods: ["16/19", "36/19", "16/9", "4"],
    },
    ProposalCase {
        scale: "1",
        fault_words: [9, 4],
        probabilities: PHYSICAL,
        likelihoods: ["1", "1", "1", "1"],
    },
    ProposalCase {
        scale: "2",
        fault_words: [4, 3],
        probabilities: ["12/25", "8/25", "3/25", "2/25"],
        likelihoods: ["3/2", "9/16", "2/3", "1/4"],
    },
];

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-importance-outcomes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("zero.magna"), "I 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Words {
    words: [u64; 2],
    used: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = *self
            .words
            .get(self.used)
            .expect("unexpected extra categorical entropy");
        self.used += 1;
        word
    }
}

fn rational(text: &str) -> BigRational {
    text.parse().unwrap()
}
fn field(text: &str) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(text))
        .unwrap()
}
fn scalar_bit(value: bool) -> ExactScalar {
    if value {
        ExactScalar::one()
    } else {
        ExactScalar::zero()
    }
}

fn evaluator(files: &Files, scale: &str, strategy: CompilerStrategy) -> TaskEvaluator {
    let registry = ControllerRegistry::default();
    let parse = ParseOptions::default();
    let physical = parse_circuit(CIRCUIT, &parse).unwrap();
    let task = parse_task(
        r#"{"outputs":[1],"ideal_state":"zero.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task.load_modules(&physical, &parse).unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.compile.compiler_strategy = strategy;
    TaskEvaluator::new_with_sampling(
        &physical,
        &modules,
        &registry,
        options,
        &NoiseSampling::Importance {
            proposal_scale: rational(scale),
        },
    )
    .unwrap()
}

fn assert_known_draw(
    draw: &TaskEvaluation,
    bits: (bool, bool),
    physical: &str,
    likelihood: &str,
    context: &str,
) {
    let accepted = !bits.0;
    let error = accepted && bits.1;
    let success = accepted && !bits.1;
    assert_eq!(
        draw.noise_probability,
        rational(physical),
        "physical probability: {context}"
    );
    assert_eq!(
        draw.noise_likelihood_ratio,
        rational(likelihood),
        "likelihood: {context}"
    );
    assert_eq!(
        draw.raw_acceptance,
        scalar_bit(accepted),
        "raw postselection acceptance: {context}"
    );
    assert_eq!(draw.raw_error, scalar_bit(error), "raw error: {context}");
    assert_eq!(
        draw.raw_target_success,
        Some(scalar_bit(success)),
        "raw target success: {context}"
    );
    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
    assert_eq!(
        draw.accepted,
        if accepted {
            field(likelihood)
        } else {
            ExactField::zero()
        },
        "per-draw weighted acceptance: {context}"
    );
    assert_eq!(
        draw.error,
        if error {
            field(likelihood)
        } else {
            ExactField::zero()
        },
        "per-draw weighted error: {context}"
    );
    assert_eq!(
        draw.target_success,
        Some(if success {
            field(likelihood)
        } else {
            ExactField::zero()
        }),
        "per-draw weighted target success: {context}"
    );
    assert_eq!(draw.history_mass, ExactScalar::one());
    assert_eq!(draw.history_proposal_mass, None);
    assert_eq!(draw.history_weight, ExactField::one());
    assert_eq!(draw.network_compilations, 0);
    assert_eq!(draw.history_queries, 0);
    assert!(draw.sampled_records.is_empty());
    assert!(draw.controller_calls.is_empty());
    assert!(draw.moments.is_empty());
    assert!(draw.raw_moments.is_empty());
}

#[test]
fn original_all_fault_configurations_recover_exact_physical_acceptance_and_error() {
    let files = Files::new();
    let arithmetic = FieldArithmetic::default();
    let mut configurations = 0;
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for ProposalCase {
            scale,
            fault_words,
            probabilities,
            likelihoods,
        } in PROPOSALS
        {
            let evaluator = evaluator(&files, scale, strategy);
            assert_eq!(evaluator.execution_route(), "linear_affine");
            assert!(!evaluator.sampled_history());
            let mut normalization = rational("0");
            let mut proposal_total = rational("0");
            let mut physical_total = rational("0");
            let mut accepted = ExactField::zero();
            let mut error = ExactField::zero();
            let mut target_success = ExactField::zero();
            for (index, bits) in [(false, false), (false, true), (true, false), (true, true)]
                .into_iter()
                .enumerate()
            {
                let context = format!("{strategy:?}, scale={scale}, faults={bits:?}");
                let mut words = Words {
                    words: [
                        if bits.0 { fault_words[0] } else { 0 },
                        if bits.1 { fault_words[1] } else { 0 },
                    ],
                    used: 0,
                };
                let draw = evaluator.sample(&mut words).unwrap();
                // Check actual producer output before any aggregate can mask it.
                assert_known_draw(&draw, bits, PHYSICAL[index], likelihoods[index], &context);
                assert_eq!(words.used, 2, "actual entropy consumption: {context}");
                let q = rational(probabilities[index]);
                normalization += &q * &draw.noise_likelihood_ratio;
                proposal_total += &q;
                physical_total += &draw.noise_probability;
                accepted = arithmetic
                    .add(&accepted, &arithmetic.scale(&draw.accepted, &q).unwrap())
                    .unwrap();
                error = arithmetic
                    .add(&error, &arithmetic.scale(&draw.error, &q).unwrap())
                    .unwrap();
                target_success = arithmetic
                    .add(
                        &target_success,
                        &arithmetic
                            .scale(draw.target_success.as_ref().unwrap(), &q)
                            .unwrap(),
                    )
                    .unwrap();
                configurations += 1;
            }
            assert_eq!(proposal_total, rational("1"));
            assert_eq!(physical_total, rational("1"));
            assert_eq!(normalization, rational("1"));
            assert_eq!(accepted, field("9/10"));
            assert_eq!(error, field("9/50"));
            assert_eq!(target_success, field("18/25"));
            assert_eq!(arithmetic.divide(&error, &accepted).unwrap(), field("1/5"));
        }
    }
    assert_eq!(configurations, 24);
}
