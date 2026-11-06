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

// Complete original method test_importance_weights_rejected_draws_and_aggregate_ratio
// from the same pinned MPL-2.0 module above. The deterministic 25-draw multiset
// realizes the proposal law exactly; it makes no Monte Carlo uncertainty claim.
const WEIGHTED_NONLINEAR: &str = "X_ERROR(1/10) 0\nX_ERROR(1/5) 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nPOSTSELECT ctrl[0]";

struct WeightedNoiseWords {
    words: [u64; 2],
    used: usize,
}
impl rmagna::noise::RandomSource for WeightedNoiseWords {
    fn next_u64(&mut self) -> u64 {
        let word = *self
            .words
            .get(self.used)
            .expect("unexpected extra noise or history entropy");
        self.used += 1;
        word
    }
}
fn weighted_nonlinear_evaluator(
    files: &Files,
    (compiler, runtime): (CompilerStrategy, CausalStrategy),
) -> TaskEvaluator {
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", |bits: &[bool]| {
            assert_eq!(bits.len(), 2);
            Ok(vec![bits[0] & bits[1]])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(WEIGHTED_NONLINEAR, &parse).unwrap();
    let task = parse_task(
        r#"{"outputs":[0],"ideal_state":"zero.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task.load_modules(&physical, &parse).unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.strategy = runtime;
    options.runtime.compile.compiler_strategy = compiler;
    let evaluator = TaskEvaluator::new_with_sampling(
        &physical,
        &modules,
        &registry,
        options,
        &rmagna::noise::NoiseSampling::Importance {
            proposal_scale: rational("2"),
        },
    )
    .unwrap();
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

#[test]
fn original_importance_weights_rejected_draws_and_aggregate_ratio() {
    use rmagna::statistics::{ExactStatistics, HistoryEfficiency, StatisticsOptions};

    let files = Files::new();
    let arithmetic = FieldArithmetic::default();
    // Index = a + 2*b. These are independent physical/proposal truth tables.
    let multiplicities = [12, 3, 8, 2];
    let physical = ["18/25", "2/25", "9/50", "1/50"];
    let proposal = ["12/25", "3/25", "8/25", "2/25"];
    let likelihoods = ["3/2", "2/3", "9/16", "1/4"];
    for strategy in strategies() {
        let evaluator = weighted_nonlinear_evaluator(&files, strategy);
        let mut statistics = ExactStatistics::new(3, StatisticsOptions::default()).unwrap();
        let mut efficiency = HistoryEfficiency::new(arithmetic).unwrap();
        let mut rejected = 0;
        let mut individual_ratios = ExactField::zero();
        for mask in 0..4 {
            let a = mask & 1 != 0;
            let b = mask & 2 != 0;
            let accepted = !(a & b);
            let error = accepted & a;
            let target = accepted & !a;
            let scalar_bit = |bit| {
                if bit {
                    ExactScalar::one()
                } else {
                    ExactScalar::zero()
                }
            };
            let weight = field(likelihoods[mask]);
            assert_eq!(
                arithmetic
                    .divide(&field(physical[mask]), &field(proposal[mask]))
                    .unwrap(),
                weight
            );
            assert_eq!(
                arithmetic
                    .scale(&field(proposal[mask]), &rational("25"))
                    .unwrap(),
                field(&multiplicities[mask].to_string())
            );
            for _ in 0..multiplicities[mask] {
                // Identity uses zero; fault words lie in the exact categorical
                // buckets for q=1/5 and q=2/5. No native RNG ABI is assumed.
                let mut rng = WeightedNoiseWords {
                    words: [if a { 4 } else { 0 }, if b { 3 } else { 0 }],
                    used: 0,
                };
                let draw = evaluator.sample(&mut rng).unwrap();
                assert_eq!(rng.used, 2, "two actual noise words per draw");
                assert_eq!(draw.noise_probability, rational(physical[mask]));
                assert_eq!(draw.noise_likelihood_ratio, rational(likelihoods[mask]));
                assert_eq!(draw.history_mass, ExactScalar::one());
                assert_eq!(draw.history_proposal_mass, None);
                assert_eq!(draw.history_weight, ExactField::one());
                assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                assert!(draw.sampled_records.is_empty());
                assert_eq!(draw.controller_calls.len(), 1);
                let call = &draw.controller_calls[0];
                assert_eq!(call.name, "and");
                assert_eq!(call.call, 0);
                assert_eq!(call.inputs, vec![a, b]);
                assert_eq!(call.outputs, vec![a & b]);
                assert_eq!(draw.raw_acceptance, scalar_bit(accepted));
                assert_eq!(draw.raw_error, scalar_bit(error));
                assert_eq!(draw.raw_target_success, Some(scalar_bit(target)));
                assert!(draw.raw_moments.is_empty());
                assert!(draw.moments.is_empty());
                let weighted_bit = |bit| {
                    if bit {
                        weight.clone()
                    } else {
                        ExactField::zero()
                    }
                };
                assert_eq!(draw.accepted, weighted_bit(accepted));
                assert_eq!(draw.error, weighted_bit(error));
                assert_eq!(draw.target_success, Some(weighted_bit(target)));
                rejected += usize::from(draw.accepted.is_zero());
                if !draw.accepted.is_zero() {
                    individual_ratios = arithmetic
                        .add(
                            &individual_ratios,
                            &arithmetic.divide(&draw.error, &draw.accepted).unwrap(),
                        )
                        .unwrap();
                }
                let contributions = vec![
                    draw.accepted.clone(),
                    draw.error.clone(),
                    draw.target_success.clone().unwrap(),
                ];
                // Contributions already contain the likelihood. The separate
                // weight is a diagnostic and must not multiply them again.
                statistics.push(&contributions, &weight).unwrap();
                efficiency
                    .push(&draw.accepted, Some(&draw.error), &weight, &weight)
                    .unwrap();
            }
        }
        let summary = statistics.summarize().unwrap().unwrap();
        assert_eq!(summary.count, 25, "all draws include both rejected samples");
        assert_eq!(rejected, 2);
        assert_eq!(
            summary.means,
            vec![field("49/50"), field("2/25"), field("9/10")],
            "exact means apply the likelihood once"
        );
        assert_eq!(summary.mean_weight, field("1"));
        assert_eq!(summary.effective_sample_size, field("2400/119"));
        let ratio = statistics.paired_ratio(1, 0).unwrap().unwrap().ratio;
        assert_eq!(ratio, field("4/49"), "ratio of aggregate weighted masses");
        let naive_mean = arithmetic
            .scale(&individual_ratios, &rational("1/25"))
            .unwrap();
        assert_eq!(naive_mean, field("3/25"));
        assert_ne!(ratio, naive_mean);
        let diagnostic = efficiency.summarize().unwrap();
        assert_eq!(diagnostic.completed, 25);
        assert_eq!(diagnostic.mean_weight, Some(field("1")));
        assert_eq!(diagnostic.effective_sample_size, field("2400/119"));
    }
}

// Complete original method test_full_history_final_readout_and_explicit_state
// from the same pinned MPL-2.0 module above. Each callback reads the reported
// detector history and the old decision before overwriting that same register.
const FULL_HISTORY: &str = "CONTROL[XOR](1) -> ctrl[0]\nX 0\nMR 0\nDETECTOR rec[-1]\nX 0\nMR 0\nDETECTOR rec[-1]\nX 0\nM(1) 0\nDETECTOR rec[-1]\nCONTROL[decode] det[0] det[1] det[-1] ctrl[0] -> ctrl[0]\nPOSTSELECT ctrl[0]";

type FullHistoryCalls = std::sync::Arc<std::sync::Mutex<Vec<Vec<bool>>>>;

fn full_history_evaluator(
    (compiler, runtime): (CompilerStrategy, CausalStrategy),
    calls: FullHistoryCalls,
) -> TaskEvaluator {
    let mut registry = ControllerRegistry::default();
    registry
        .register("decode", "1", move |bits: &[bool]| {
            assert_eq!(bits.len(), 4);
            calls.lock().unwrap().push(bits.to_vec());
            Ok(vec![(bits[0] & bits[1]) ^ bits[2] ^ bits[3]])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(FULL_HISTORY, &parse).unwrap();
    let task = parse_task(
        r#"{"outputs":[]}"#,
        &std::env::temp_dir(),
        &TaskOptions::default(),
    )
    .unwrap();
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

#[test]
fn original_full_history_final_readout_and_explicit_state() {
    for strategy in strategies() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let evaluator = full_history_evaluator(strategy, std::sync::Arc::clone(&calls));
        assert!(
            calls.lock().unwrap().is_empty(),
            "preparation must not invoke the opaque callback"
        );
        for (index, seed) in (0..3).enumerate() {
            let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
            // Keep the known-answer acceptance assertion before transcript
            // checks: the private first-MR-to-M control must fail here.
            assert_eq!(draw.accepted, field("1"), "full-history exact acceptance");
            assert_eq!(draw.error, field("0"));
            assert_eq!(draw.raw_acceptance, ExactScalar::one());
            assert_eq!(draw.raw_error, ExactScalar::zero());
            assert_eq!(draw.history_mass, ExactScalar::one());
            assert_eq!(draw.history_proposal_mass, None);
            assert_eq!(draw.history_weight, ExactField::one());
            assert_eq!(draw.noise_probability, rational("1"));
            assert_eq!(draw.noise_likelihood_ratio, rational("1"));
            assert_eq!(draw.target_success, None);
            assert_eq!(draw.raw_target_success, None);
            assert_eq!(draw.target_normalization, None);
            assert!(draw.sampled_records.is_empty());
            assert!(draw.moments.is_empty());
            assert!(draw.raw_moments.is_empty());
            assert_eq!(draw.controller_calls.len(), 1);
            let call = &draw.controller_calls[0];
            assert_eq!(call.name, "decode");
            assert_eq!(call.inputs, vec![true, true, false, true]);
            assert_eq!(call.outputs, vec![false]);
            assert_eq!(
                calls.lock().unwrap().len(),
                index + 1,
                "one actual callback invocation per draw"
            );
        }
        assert_eq!(
            *calls.lock().unwrap(),
            vec![vec![true, true, false, true]; 3],
            "reported history and prior value are read before overwrite"
        );
    }
}

// Complete original method test_report_flips_select_reported_bits_and_preserve_true_output
// from the same pinned MPL-2.0 module above. Readout noise changes a report,
// while the measured output retains its true quantum state.
const REPORT_FLIP: &str = "X 0 1\nM(1) 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nPOSTSELECT ctrl[0]";
const REPORT_FLIP_TASK: &str =
    r#"{"outputs":[0],"sample_records":[0,1],"ideal_state":"zero.magna"}"#;

fn report_flip_categories(text: &str, (compiler, runtime): (CompilerStrategy, CausalStrategy)) {
    use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
    use rmagna::runtime::{CausalProgram, RuntimeOptions};
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", |bits: &[bool]| Ok(vec![bits[0] & bits[1]]))
        .unwrap();
    let physical = parse_circuit(
        text,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let categories =
        original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 1, "one original declared report category");
    let category = &categories[0];
    assert_eq!(category.identity_probability.value(), &rational("0"));
    assert_eq!(category.channels.len(), 1);
    assert_eq!(category.channels[0].probability.value(), &rational("1"));
    assert_eq!(
        category.channels[0].action,
        OriginalNoiseAction::ReportFlip { record: 0 }
    );
    assert!(category.guard.is_none());
    let options = RuntimeOptions {
        strategy: runtime,
        compile: rmagna::instrument::CompileOptions {
            compiler_strategy: compiler,
            ..Default::default()
        },
        ..Default::default()
    };
    let arithmetic = ScalarArithmetic::default();
    let program = CausalProgram::new(&physical, &registry, options, &arithmetic).unwrap();
    let resolved = program
        .resolve(&[], &mut SplitMix64::new(1), &arithmetic)
        .unwrap();
    let sites = resolved.instrument().noise_sites();
    assert_eq!(
        sites.len(),
        2,
        "implicit ideal measurement remains an internal site"
    );
    assert_eq!(category.internal_site, 0);
    assert_eq!(
        sites[category.internal_site].identity_probability(),
        &category.identity_probability
    );
    assert_eq!(sites[category.internal_site].alternatives().len(), 1);
    assert_eq!(
        sites[category.internal_site].alternatives()[0].probability(),
        &category.channels[0].probability
    );
    assert_eq!(sites[1].identity_probability().value(), &rational("1"));
}

#[test]
fn original_report_flips_select_reported_bits_and_preserve_true_output() {
    let files = Files::new();
    let mixed = REPORT_FLIP.replace(
        "POSTSELECT ctrl[0]",
        "CONTROL[XOR] ctrl[0] rec[-2] -> ctrl[1]\nPOSTSELECT ctrl[1]",
    );
    for strategy in strategies() {
        for text in [REPORT_FLIP, mixed.as_str()] {
            let evaluator = evaluator(&files, text, REPORT_FLIP_TASK, strategy);
            let draw = evaluator.sample(&mut SplitMix64::new(1)).unwrap();
            assert_eq!(draw.sampled_records, vec![false, true]);
            assert_eq!(draw.accepted, field("1"));
            // The physical-error control preserves reports and acceptance;
            // this known-answer assertion must detect its changed true output.
            assert_eq!(draw.error, field("1"), "report-flip exact joint error");
            assert_eq!(draw.raw_acceptance, ExactScalar::one());
            assert_eq!(draw.raw_error, ExactScalar::one());
            assert_eq!(draw.target_success, Some(field("0")));
            assert_eq!(draw.raw_target_success, Some(ExactScalar::zero()));
            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
            assert_eq!(draw.history_mass, ExactScalar::one());
            assert_eq!(draw.history_proposal_mass, None);
            assert_eq!(draw.history_weight, ExactField::one());
            assert_eq!(draw.noise_probability, rational("1"));
            assert_eq!(draw.noise_likelihood_ratio, rational("1"));
            assert!(draw.moments.is_empty());
            assert!(draw.raw_moments.is_empty());
            assert_eq!(draw.controller_calls.len(), 1);
            let call = &draw.controller_calls[0];
            assert_eq!(call.name, "and");
            assert_eq!(call.inputs, vec![false, true]);
            assert_eq!(call.outputs, vec![false]);
            // Original declaration metadata and resolved compiler sites expose
            // different counts; neither count is normalized to the other.
            report_flip_categories(text, strategy);
        }
    }
}

// Complete original method test_versions_overwrites_repeat_and_explicit_prior_decisions
// from the same pinned MPL-2.0 module above. Selections retain the version read
// at their source position, including across repeated blocks and later writes.
const VERSIONED_REPEAT: &str = "CONTROL[XOR](0) -> ctrl[0]\nREPEAT 2 {\nR 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nPOSTSELECT ctrl[0]\n}\nCONTROL[XOR](1) -> ctrl[0]";
const VERSIONED_PRIOR: &str = "H 0 1\nM 0 1\nDETECTOR rec[-2]\nCONTROL[echo] det[-1] -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[and] ctrl[0] det[-1] -> ctrl[0]\nCONTROL[NOT] ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]\nCONTROL[XOR](0) -> ctrl[0]";
const VERSIONED_TASK: &str = r#"{"outputs":[]}"#;

fn assert_original_versioned_selections() {
    use rmagna::circuit::{BitExpr, Operation, RegisterVersion};
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let circuit = parse_circuit(
        VERSIONED_REPEAT,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let analysis = circuit.control_analysis();
    let selections = circuit
        .operations()
        .iter()
        .filter_map(|operation| match &operation.operation {
            Operation::Postselect { expression } => Some(expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 2);
    assert_eq!(
        Some(selections[0]),
        analysis.expression(RegisterVersion::new(0, 1))
    );
    assert_eq!(
        Some(selections[1]),
        analysis.expression(RegisterVersion::new(0, 2))
    );
    assert_ne!(selections[0], selections[1]);
    assert_eq!(analysis.expressions().len(), 4);
    assert_eq!(
        analysis.expression(RegisterVersion::new(0, 0)),
        Some(&BitExpr::default())
    );
    assert_eq!(
        analysis.expression(RegisterVersion::new(0, 3)),
        Some(&BitExpr::default().inverted())
    );
}

#[test]
fn original_versions_overwrites_repeat_and_explicit_prior_decisions() {
    // These original parsed-version checks are independent of evaluation input,
    // so the missing-selection control cannot bypass or alter them.
    assert_original_versioned_selections();
    let files = Files::new();
    for strategy in strategies() {
        for text in [VERSIONED_REPEAT, VERSIONED_PRIOR] {
            let evaluator = evaluator(&files, text, VERSIONED_TASK, strategy);
            let result = exact(&evaluator);
            assert_totals(&result, "1/4", "0", None, &[], None);
        }
    }
}
