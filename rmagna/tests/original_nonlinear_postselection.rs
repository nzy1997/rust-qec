// SPDX-License-Identifier: MPL-2.0
// Complete finite contracts from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_nonlinear_postselection.py, NonlinearPostselectionTests:
// test_noncommuting_readout_truth_table_and_target,
// test_mixed_affine_selector_integrates_unobserved_record,
// test_joint_duplicate_conflicting_detector_and_task_conditions.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumeration, TaskEnumerationOptions, TaskEvaluation, TaskEvaluator,
    TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const NONCOMMUTING: &str = "H 0\nM 0\nH 0\nM 0\nCX rec[-2] 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nPOSTSELECT ctrl[0]";
const MIXED: &str = "H 0 1 2\nM 0 1 2\nCX rec[-1] 3\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nPOSTSELECT ctrl[1]";
const MIXED_TASK: &str = r#"{"outputs":[3],"ideal_state":"zero.magna"}"#;
const JOINT_PREFIX: &str = "H 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[echo] det[-2] det[-1] -> ctrl[0] ctrl[1]\n";

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-nonlinear-{}-{}",
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
fn rational(text: &str) -> BigRational {
    text.parse().unwrap()
}
fn field(text: &str) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(text))
        .unwrap()
}
fn raw_field(value: &ExactScalar) -> ExactField {
    FieldArithmetic::default().from_scalar(value).unwrap()
}
fn strategies() -> impl Iterator<Item = (CompilerStrategy, CausalStrategy)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate]
            .into_iter()
            .map(move |runtime| (compiler, runtime))
    })
}
fn evaluator(
    files: &Files,
    text: &str,
    task: &str,
    (compiler, runtime): (CompilerStrategy, CausalStrategy),
) -> TaskEvaluator {
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", |bits: &[bool]| {
            assert_eq!(
                bits.len(),
                2,
                "only the two declared AND inputs are observed"
            );
            Ok(vec![bits[0] & bits[1]])
        })
        .unwrap();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(text, &parse).unwrap();
    let task = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
    let modules = task.load_modules(&physical, &parse).unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.strategy = runtime;
    options.runtime.compile.compiler_strategy = compiler;
    let evaluator = TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
    assert!(evaluator.sampled_history());
    assert_eq!(
        evaluator.execution_route(),
        match runtime {
            CausalStrategy::ColdPrefix => "causal_cold_prefix",
            CausalStrategy::FixedTemplate => "causal_fixed_template",
        }
    );
    evaluator
}
fn exact(evaluator: &TaskEvaluator) -> TaskEnumeration {
    let mut options = TaskEnumerationOptions::default();
    options.noise.max_weight = 0;
    let result = evaluator.enumerate_physical(&options).unwrap();
    assert_eq!(result.covered_probability.value(), &rational("1"));
    assert_eq!(result.omitted_probability.value(), &rational("0"));
    assert_eq!(result.patterns, 1);
    assert_eq!(result.histories, 4, "two independent observed quantum bits");
    result
}
fn assert_totals(
    result: &TaskEnumeration,
    acceptance: &str,
    error: &str,
    target: Option<&str>,
    moments: &[&str],
    conditional: Option<&str>,
) {
    assert_eq!(
        result.masses.accepted,
        field(acceptance),
        "exact acceptance truth table"
    );
    assert_eq!(
        result.masses.error,
        field(error),
        "exact unconditional error"
    );
    assert_eq!(result.masses.target_success, target.map(field));
    assert_eq!(
        result.masses.moments,
        moments.iter().map(|x| field(x)).collect::<Vec<_>>()
    );
    assert_eq!(result.full_acceptance.lower, field(acceptance));
    assert_eq!(result.full_acceptance.upper, field(acceptance));
    assert_eq!(result.full_error.lower, field(error));
    assert_eq!(result.full_error.upper, field(error));
    assert_eq!(
        result.conditional_error_within_enumerated_sector,
        conditional.map(field)
    );
    match &result.conditional_error {
        Some(interval) => {
            assert_ne!(acceptance, "0");
            let expected = field(conditional.unwrap_or("0"));
            assert_eq!(interval.lower, expected);
            assert_eq!(interval.upper, expected);
        }
        None => assert_eq!(
            acceptance, "0",
            "conditional interval is undefined at zero acceptance"
        ),
    }
    assert_eq!(
        result.target_normalization,
        target.map(|_| ExactScalar::one())
    );
}
fn and_history(draw: &TaskEvaluation) -> usize {
    assert_eq!(
        draw.history_mass,
        ScalarArithmetic::default().omega(0, -4).unwrap(),
        "integrated history mass keeps only the two external observations"
    );
    assert_eq!(draw.history_proposal_mass, None);
    assert_eq!(draw.history_weight, ExactField::one());
    assert_eq!(draw.noise_probability, rational("1"));
    assert_eq!(draw.noise_likelihood_ratio, rational("1"));
    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
    assert!(
        draw.sampled_records.is_empty(),
        "no final record is requested"
    );
    assert_eq!(draw.controller_calls.len(), 1);
    let call = &draw.controller_calls[0];
    assert_eq!(call.name, "and");
    assert_eq!(call.inputs.len(), 2);
    let index = 2 * usize::from(call.inputs[0]) + usize::from(call.inputs[1]);
    assert_eq!(call.outputs, vec![index == 3]);
    index
}
fn assert_noncommuting_draw(draw: &TaskEvaluation) {
    let index = and_history(draw);
    // Literal independent truth table: Z then X on one wire are independent.
    assert_eq!(draw.accepted, field(["1", "1", "1", "0"][index]));
    assert_eq!(draw.error, field(["0", "0", "1", "0"][index]));
    assert_eq!(
        draw.target_success,
        Some(field(["1", "1", "0", "0"][index]))
    );
    assert_eq!(draw.moments, vec![field(["1", "1", "-1", "0"][index])]);
    assert_eq!(
        raw_field(&draw.raw_acceptance),
        field(["1/4", "1/4", "1/4", "0"][index])
    );
    assert_eq!(
        raw_field(&draw.raw_error),
        field(["0", "0", "1/4", "0"][index])
    );
    assert_eq!(
        raw_field(draw.raw_target_success.as_ref().unwrap()),
        field(["1/4", "1/4", "0", "0"][index])
    );
    assert_eq!(draw.raw_moments.len(), 1);
    assert_eq!(
        raw_field(&draw.raw_moments[0]),
        field(["1/4", "1/4", "-1/4", "0"][index])
    );
}
fn assert_mixed_draw(draw: &TaskEvaluation) {
    let index = and_history(draw);
    // c is integrated: half of each captured a,b history satisfies c = a AND b.
    assert_eq!(
        draw.accepted,
        field("1/2"),
        "unobserved third record is integrated"
    );
    assert_eq!(draw.error, field(["0", "0", "0", "1/2"][index]));
    assert_eq!(
        draw.target_success,
        Some(field(["1/2", "1/2", "1/2", "0"][index]))
    );
    assert_eq!(raw_field(&draw.raw_acceptance), field("1/8"));
    assert_eq!(
        raw_field(&draw.raw_error),
        field(["0", "0", "0", "1/8"][index])
    );
    assert_eq!(
        raw_field(draw.raw_target_success.as_ref().unwrap()),
        field(["1/8", "1/8", "1/8", "0"][index])
    );
    assert!(draw.moments.is_empty());
    assert!(draw.raw_moments.is_empty());
    // Native sampled_parities counts independent observations. Rust's
    // history_queries counts probability queries and is not that same metric.
}

#[test]
fn original_noncommuting_readout_truth_table_and_target() {
    let files = Files::new();
    for strategy in strategies() {
        let evaluator = evaluator(
            &files,
            NONCOMMUTING,
            r#"{"outputs":[1],"ideal_state":"zero.magna","observables":[[0,1]]}"#,
            strategy,
        );
        assert_totals(
            &exact(&evaluator),
            "3/4",
            "1/4",
            Some("1/2"),
            &["1/4"],
            Some("1/3"),
        );
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            let draw = evaluator
                .evaluate_prescribed_history(&[], &[vec![a, b]])
                .unwrap();
            assert_eq!(and_history(&draw), 2 * usize::from(a) + usize::from(b));
            assert_noncommuting_draw(&draw);
        }
        for seed in 0..8 {
            assert_noncommuting_draw(&evaluator.sample(&mut SplitMix64::new(seed)).unwrap());
        }
    }
}

#[test]
fn original_mixed_affine_selector_integrates_unobserved_record() {
    let files = Files::new();
    for strategy in strategies() {
        let evaluator = evaluator(&files, MIXED, MIXED_TASK, strategy);
        for seed in 0..8 {
            assert_mixed_draw(&evaluator.sample(&mut SplitMix64::new(seed)).unwrap());
        }
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            let draw = evaluator
                .evaluate_prescribed_history(&[], &[vec![a, b]])
                .unwrap();
            assert_eq!(and_history(&draw), 2 * usize::from(a) + usize::from(b));
            assert_mixed_draw(&draw);
        }
        assert_totals(
            &exact(&evaluator),
            "1/2",
            "1/8",
            Some("3/8"),
            &[],
            Some("1/4"),
        );
    }
}

#[test]
fn original_joint_duplicate_conflicting_detector_and_task_conditions() {
    let files = Files::new();
    // Preserve all seven complete original suffixes in their source order.
    let cases = [
        ("POSTSELECT ctrl[0] ctrl[1]", "1/4"),
        (
            "CONTROL[NOT] ctrl[0] -> ctrl[2]\nCONTROL[NOT] ctrl[1] -> ctrl[3]\nPOSTSELECT ctrl[2] ctrl[3]",
            "1/4",
        ),
        ("POSTSELECT ctrl[0] ctrl[0]", "1/2"),
        (
            "POSTSELECT ctrl[0]\nCONTROL[NOT] ctrl[0] -> ctrl[2]\nPOSTSELECT ctrl[2]",
            "0",
        ),
        ("POSTSELECT ctrl[1] det[0]", "1/4"),
        ("POSTSELECT ctrl[0]\nPOSTSELECT !det[0]", "0"),
        (
            "POSTSELECT ctrl[0]\nDETECTOR rec[-2] rec[-1]\nPOSTSELECT !det[3]",
            "1/4",
        ),
    ];
    for strategy in strategies() {
        for (suffix, acceptance) in cases {
            let evaluator = evaluator(&files, &format!("{JOINT_PREFIX}{suffix}"), "{}", strategy);
            assert_totals(&exact(&evaluator), acceptance, "0", None, &[], None);
        }
    }
}

// Complete original method test_dense_noncommuting_magic_instrument_with_report_noise
// from the same pinned module above. The exact two-dimensional branch oracle
// sums P_y(c) T H P_z(b) T S P_x(a) T H|0> over all 16 (a,b,c,flip)
// choices, selecting (a AND c) XOR (b XOR flip). The middle quantum bit b
// remains integrated; the opaque controller observes only a and c.
const DENSE_MAGIC: &str = "H 0\nT 0\nMX 0\nS 0\nT 0\nM(1/3) 0\nH 0\nT 0\nMY 0\nDETECTOR rec[-3]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-2] -> ctrl[1]\nCONTROL[NOT] ctrl[1] -> ctrl[2]\nPOSTSELECT ctrl[2]";

fn dense_field(rational_part: &str, sqrt2_part: &str) -> ExactField {
    let arithmetic = FieldArithmetic::default();
    let sqrt2 = raw_field(&ScalarArithmetic::default().omega(0, 1).unwrap());
    arithmetic
        .add(
            &field(rational_part),
            &arithmetic.scale(&sqrt2, &rational(sqrt2_part)).unwrap(),
        )
        .unwrap()
}

#[test]
fn original_dense_noncommuting_magic_instrument_with_report_noise() {
    let files = Files::new();
    std::fs::write(files.0.join("y_plus.magna"), "H 0\nS 0").unwrap();
    // Exact branch sums, independent of either compiler/runtime implementation.
    let acceptance = dense_field("13/24", "-1/24");
    let error = field("7/24");
    let target = dense_field("1/4", "-1/24");
    let conditional = dense_field("91/167", "7/167");
    let arithmetic = FieldArithmetic::default();
    assert_eq!(arithmetic.add(&error, &target).unwrap(), acceptance);
    assert_eq!(
        arithmetic.multiply(&conditional, &acceptance).unwrap(),
        error
    );
    for strategy in strategies() {
        let evaluator = evaluator(
            &files,
            DENSE_MAGIC,
            r#"{"outputs":[0],"ideal_state":"y_plus.magna"}"#,
            strategy,
        );
        let mut options = TaskEnumerationOptions {
            include_patterns: true,
            ..Default::default()
        };
        options.noise.max_weight = 1;
        let result = evaluator.enumerate_physical(&options).unwrap();
        assert_eq!(
            result.masses.accepted, acceptance,
            "exact dense report-noise acceptance"
        );
        assert_eq!(result.masses.error, error);
        assert_eq!(result.masses.target_success, Some(target.clone()));
        assert!(result.masses.moments.is_empty());
        assert_eq!(result.target_normalization, Some(ExactScalar::one()));
        assert_eq!(result.covered_probability.value(), &rational("1"));
        assert_eq!(result.omitted_probability.value(), &rational("0"));
        assert_eq!(result.patterns, 2);
        assert_eq!(result.histories, 8);
        assert_eq!(result.by_weight.len(), 2);
        for (weight, probability) in [(0, "2/3"), (1, "1/3")] {
            let sector = &result.by_weight[weight];
            assert_eq!(sector.weight, weight);
            assert_eq!(sector.probability.value(), &rational(probability));
            assert_eq!(sector.patterns, 1);
            assert_eq!(sector.histories, 4);
        }
        let patterns = result.pattern_responses.as_ref().unwrap();
        assert_eq!(patterns.len(), 2);
        assert_eq!(
            patterns.iter().map(|p| p.histories).collect::<Vec<_>>(),
            vec![4, 4]
        );
        assert_eq!(result.full_acceptance.lower, acceptance);
        assert_eq!(result.full_acceptance.upper, acceptance);
        assert_eq!(result.full_error.lower, error);
        assert_eq!(result.full_error.upper, error);
        assert_eq!(
            result.conditional_error_within_enumerated_sector,
            Some(conditional.clone())
        );
        let interval = result.conditional_error.as_ref().unwrap();
        assert_eq!(interval.lower, conditional);
        assert_eq!(interval.upper, conditional);
    }
}
