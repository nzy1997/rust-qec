//! Complete original ConditionalNoiseTests categorical-channel method at
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f, with exact joint moments.
use num_rational::BigRational;
use rmagna::circuit::{Basis, BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
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
            "rmagna-original-guarded-channel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "I 0 1").unwrap();
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
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
fn original_source() -> String {
    let mut rates = ["0"; 15];
    rates[5] = "1/3"; // XY, not a pair of independent single-qubit faults.
    rates[10] = "1/6"; // YZ; identity retains the remaining half.
    format!("H 0\nM 0\nPAULI_CHANNEL_2({}) rec[-1] 1 2", rates.join(","))
}

#[test]
fn original_guarded_two_qubit_channel_order_and_mutual_exclusion() {
    let files = TargetFile::new();
    let physical = parse_circuit(&original_source(), &ParseOptions::default()).unwrap();
    let task = parse_task(
        r#"{"outputs":[1,2],"ideal_state":"target.magna","observables":[[0,1],[0,2],[0,3]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task
        .load_modules(&physical, &ParseOptions::default())
        .unwrap();
    let registry = ControllerRegistry::default();
    let mut conditional_evaluations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for proposal in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = proposal;
                let evaluator =
                    TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                let mut enumeration = TaskEnumerationOptions::default();
                enumeration.noise.max_weight = 1;
                enumeration.include_patterns = true;
                let full = evaluator.enumerate_physical(&enumeration).unwrap();
                // Keep the complete original assertion first: both external
                // behavioral controls must fail here, before shape checks.
                assert_eq!(
                    full.masses.error,
                    field(1, 4),
                    "original exact error assertion"
                );
                assert_eq!(full.masses.accepted, field(1, 1));
                assert_eq!(full.masses.target_success, Some(field(3, 4)));
                assert_eq!(full.masses.moments, [field(1, 2), field(2, 3), field(5, 6)]);
                assert_eq!(full.covered_probability.value(), &rational(1, 1));
                assert_eq!(full.omitted_probability.value(), &rational(0, 1));
                assert_eq!((full.patterns, full.histories), (3, 6));
                assert_eq!(full.target_normalization, Some(ExactScalar::one()));
                assert_eq!(full.full_error.lower, field(1, 4));
                assert_eq!(full.full_error.upper, field(1, 4));
                assert_eq!(
                    full.conditional_error_within_enumerated_sector,
                    Some(field(1, 4))
                );

                // Invert the two-bit Walsh transform to check the entire joint
                // distribution, rather than only three separate means.
                let arithmetic = FieldArithmetic::default();
                for (a, b, n, d) in [
                    (false, false, 3, 4),
                    (true, false, 1, 12),
                    (false, true, 0, 1),
                    (true, true, 1, 6),
                ] {
                    let mut p = full.masses.accepted.clone();
                    for (moment, negative) in full.masses.moments.iter().zip([a, b, a ^ b]) {
                        p = if negative {
                            arithmetic.subtract(&p, moment)
                        } else {
                            arithmetic.add(&p, moment)
                        }
                        .unwrap();
                    }
                    assert_eq!(arithmetic.scale(&p, &rational(1, 4)).unwrap(), field(n, d));
                }
                let patterns = full.pattern_responses.as_ref().unwrap();
                assert_eq!(patterns.len(), 3);
                for (alternative, probability, error) in [
                    (None, rational(1, 2), field(0, 1)),
                    (Some(5), rational(1, 3), field(1, 2)),
                    (Some(10), rational(1, 6), field(1, 2)),
                ] {
                    let faults = alternative
                        .map(|alternative| FaultSelection {
                            site: 1,
                            alternative,
                        })
                        .into_iter()
                        .collect::<Vec<_>>();
                    let pattern = patterns.iter().find(|row| row.choices == faults).unwrap();
                    assert_eq!(pattern.probability.value(), &probability);
                    assert_eq!(pattern.histories, 2);
                    assert_eq!(pattern.response.accepted, field(1, 1));
                    assert_eq!(pattern.response.error, error);
                    for guard in [false, true] {
                        let row = evaluator
                            .evaluate_prescribed_history(&faults, &[vec![guard]])
                            .unwrap();
                        conditional_evaluations += 1;
                        let failed = guard && alternative.is_some();
                        assert_eq!(row.accepted, field(1, 1));
                        assert_eq!(row.error, field(i64::from(failed), 1));
                        assert_eq!(row.target_success, Some(field(i64::from(!failed), 1)));
                        let moments = match (guard, alternative) {
                            (true, Some(5)) => [-1, -1, 1],
                            (true, Some(10)) => [-1, 1, -1],
                            _ => [1, 1, 1],
                        };
                        assert_eq!(row.moments, moments.map(|n| field(n, 1)));
                        assert_eq!(
                            row.history_mass,
                            ScalarArithmetic::default().omega(0, -2).unwrap()
                        );
                        assert_eq!(row.history_weight, field(1, 1));
                        if let Some(proposal_mass) = &row.history_proposal_mass {
                            assert_eq!(proposal_mass, &row.history_mass);
                        }
                        assert_eq!(
                            row.noise_probability,
                            if guard {
                                probability.clone()
                            } else {
                                rational(1, 1)
                            }
                        );
                        assert_eq!(row.noise_likelihood_ratio, rational(1, 1));
                        assert!(row.sampled_records.is_empty());
                        assert!(row.controller_calls.is_empty());
                    }
                }
                enumeration.noise.max_weight = 0;
                let zero = evaluator.enumerate_physical(&enumeration).unwrap();
                assert_eq!((zero.patterns, zero.histories), (1, 2));
                assert_eq!(zero.covered_probability.value(), &rational(1, 2));
                assert_eq!(zero.omitted_probability.value(), &rational(1, 2));
                assert_eq!(zero.masses.accepted, field(1, 2));
                assert_eq!(zero.masses.target_success, Some(field(1, 2)));
                assert_eq!(zero.masses.error, field(0, 1));
                assert_eq!(zero.masses.moments, [field(1, 2), field(1, 2), field(1, 2)]);
                assert_eq!(zero.pattern_responses.unwrap()[0].choices, []);
            }
        }
    }
    assert_eq!(conditional_evaluations, 48);
    let categories =
        original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 1); // The deterministic implicit readout is not physical noise.
    let channel = &categories[0];
    assert_eq!(channel.internal_site, 1);
    assert_eq!(channel.guard, Some(BitExpr::atom(BitAtom::Record(0))));
    assert_eq!(channel.identity_probability.value(), &rational(1, 2));
    assert_eq!(channel.channels.len(), 15);
    let labels = [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)];
    for (index, alternative) in channel.channels.iter().enumerate() {
        let code = index + 1;
        let factors = [(1, labels[code / 4]), (2, labels[code % 4])]
            .into_iter()
            .filter_map(|(qubit, basis)| basis.map(|basis| (qubit, basis)))
            .collect();
        assert_eq!(alternative.action, OriginalNoiseAction::Pauli { factors });
        assert_eq!(
            alternative.probability.value(),
            &match index {
                5 => rational(1, 3),
                10 => rational(1, 6),
                _ => rational(0, 1),
            }
        );
        assert_eq!(alternative.source, channel.source_operations[0]);
    }
}
