// SPDX-License-Identifier: MPL-2.0
// Complete NativeFileRuntimeTests contracts from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_file_native.py.
// Identity-first categories and typed exact statistics translate the native ABI;
// independently seeded Rust streams do not claim native random-stream identity.
use num_bigint::BigInt;
use num_rational::BigRational as R;
use num_traits::{One, ToPrimitive, Zero};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
use rmagna::numeric::{Binary64, NumericOptions, approximate_real, approximate_sqrt};
use rmagna::probability::Probability;
use rmagna::runtime::ControllerRegistry;
use rmagna::statistics::{ExactStatistics, StatisticsOptions};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions, export_batch_metrics,
    parse_task,
};
use serde_json::json;
use std::path::Path;

fn r(n: i64, d: i64) -> R {
    R::new(n.into(), d.into())
}
fn f(value: R) -> ExactField {
    FieldArithmetic::default().from_rational(value).unwrap()
}
fn rational(value: &ExactField) -> R {
    assert!(value.coefficients()[1..].iter().all(Zero::is_zero));
    value.coefficients()[0].clone()
}
fn finite(value: Binary64) -> f64 {
    match value {
        Binary64::Finite(x) => x,
        other => panic!("expected finite value: {other:?}"),
    }
}
fn near(actual: f64, expected: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= 1e-11 * (1. + expected.abs()),
        "{actual} != {expected}"
    );
}
fn sampler(alternatives: &[Vec<Probability>]) -> NoiseSampler {
    let categories = alternatives
        .iter()
        .map(|row| {
            let residual = Probability::checked_sum(row.clone()).unwrap().complement();
            std::iter::once(residual)
                .chain(row.iter().cloned())
                .collect()
        })
        .collect();
    NoiseSampler::physical(
        NoiseModel::new(categories, &NoiseOptions::default()).unwrap(),
        NoiseOptions::default(),
    )
}
fn selected(sampler: &NoiseSampler, sites: usize, seed: u64) -> Vec<i64> {
    let draw = sampler.draw(&mut SplitMix64::new(seed)).unwrap();
    let mut choices = vec![-1; sites];
    for fault in draw.faults {
        choices[fault.site] = fault.alternative as i64;
    }
    choices
}

#[test]
fn original_categories_zero_one_residual_and_large_rational_inputs() {
    let denominator = BigInt::from(10).pow(500);
    let large = Probability::from_ratio(denominator.clone(), denominator).unwrap();
    let cases = vec![
        vec![],
        vec![Probability::zero()],
        vec![Probability::one()],
        vec![Probability::zero(), large],
    ];
    assert_eq!(selected(&sampler(&cases), 4, 10), [-1, -1, 0, 1]);
    let law = sampler(&[vec![
        Probability::from_rational(r(1, 10)).unwrap(),
        Probability::from_rational(r(1, 5)).unwrap(),
    ]]);
    let mut counts = [0; 3];
    for seed in 0..1024 {
        let choice = selected(&law, 1, seed);
        counts[(choice[0] + 1) as usize] += 1;
    }
    assert_eq!(counts.iter().sum::<usize>(), 1024);
    for (count, expected) in counts.into_iter().zip([0.7, 0.1, 0.2]) {
        assert!((count as f64 / 1024. - expected).abs() < 0.045);
    }
}

#[test]
fn original_category_rejects_invalid_fraction() {
    for (n, d) in [(1, 0), (-1, 2), (1, -2)] {
        assert!(
            Probability::from_ratio(n.into(), d.into()).is_err(),
            "invalid original fraction {n}/{d}"
        );
    }
    assert!(
        Probability::checked_sum([
            Probability::from_rational(r(3, 4)).unwrap(),
            Probability::from_rational(r(1, 2)).unwrap()
        ])
        .is_err()
    );
}

// Independent literal Fraction witnesses are derived from the original decimal
// rows, rather than from ExactStatistics or its streaming sum-product algorithm.
#[test]
fn original_native_summary_matches_paired_delta_method() {
    let rows = [
        [r(1, 5), r(1, 10), r(3, 10), r(-1, 10)],
        [r(4, 5), r(1, 5), r(-1, 5), r(1, 2)],
        [r(1, 2), r(0, 1), r(1, 10), r(1, 10)],
        [r(1, 1), r(2, 5), r(7, 10), r(-3, 10)],
    ];
    let mut accumulator = ExactStatistics::new(4, StatisticsOptions::default()).unwrap();
    for row in &rows {
        accumulator
            .push(
                &row.iter().cloned().map(f).collect::<Vec<_>>(),
                &ExactField::one(),
            )
            .unwrap();
    }
    let summary = accumulator.summarize().unwrap().unwrap();
    assert_eq!(summary.count, 4);
    assert_eq!(summary.mean_weight, ExactField::one());
    assert_eq!(summary.effective_sample_size, f(r(4, 1)));
    let means = [r(5, 8), r(7, 40), r(9, 40), r(1, 20)];
    let variances = [r(49, 1600), r(7, 960), r(57, 1600), r(7, 240)];
    // The whole centered row list also anchors every cross-covariance.
    let cov = summary.covariance_of_mean.as_ref().unwrap();
    for i in 0..4 {
        assert_eq!(rational(&summary.means[i]), means[i]);
        assert_eq!(rational(&cov[i][i]), variances[i]);
        near(
            finite(
                approximate_real(&summary.means[i], NumericOptions::default())
                    .unwrap()
                    .binary64,
            ),
            means[i].to_f64().unwrap(),
        );
        near(
            finite(
                approximate_sqrt(&cov[i][i], NumericOptions::default())
                    .unwrap()
                    .binary64,
            ),
            variances[i].to_f64().unwrap().sqrt(),
        );
        for j in 0..4 {
            let centered = rows
                .iter()
                .map(|row| (&row[i] - &means[i]) * (&row[j] - &means[j]))
                .sum::<R>()
                / r(12, 1);
            assert_eq!(rational(&cov[i][j]), centered);
        }
    }
    let paired = accumulator.paired_ratio(1, 0).unwrap().unwrap();
    assert_eq!(paired.ratio, f(r(7, 25)));
    assert_eq!(paired.variance, Some(f(r(9128, 1171875))));
    near(
        finite(
            approximate_real(&paired.ratio, NumericOptions::default())
                .unwrap()
                .binary64,
        ),
        7. / 25.,
    );
    near(
        finite(
            approximate_sqrt(paired.variance.as_ref().unwrap(), NumericOptions::default())
                .unwrap()
                .binary64,
        ),
        (9128.0_f64 / 1171875.).sqrt(),
    );
}

fn rejected(shots: u64, has_error: bool) -> serde_json::Value {
    let parse = ParseOptions::default();
    let source = parse_circuit(
        "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        &parse,
    )
    .unwrap();
    let task = if has_error {
        r#"{"error_observables":[0]}"#
    } else {
        "{}"
    };
    let modules = parse_task(task, Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
    let evaluator = TaskEvaluator::new(
        &source,
        &modules,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    )
    .unwrap();
    let result = evaluator
        .run_batch(&TaskRunOptions {
            shots,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Complete);
    assert_eq!(result.completed, shots);
    assert_eq!(result.failed, 0);
    assert_eq!(result.not_attempted, 0);
    export_batch_metrics(&result, NumericOptions::default()).unwrap()
}
#[test]
fn original_summary_missing_metrics_single_draw_and_zero_acceptance() {
    // Empty accumulator, rather than an invalid invented zero-shot batch.
    let empty = ExactStatistics::new(2, StatisticsOptions::default()).unwrap();
    assert_eq!(empty.summarize().unwrap(), None);
    assert_eq!(empty.paired_ratio(1, 0).unwrap(), None);
    let singleton = rejected(1, false);
    assert_eq!(singleton["estimates"]["acceptance"], 0.0);
    assert!(singleton["estimates"]["conditional_error"].is_null());
    assert!(singleton["estimates"]["error"].is_null());
    assert!(singleton["standard_errors"]["acceptance"].is_null());
    let zero = rejected(2, true);
    assert_eq!(zero["estimates"]["acceptance"], 0.0);
    assert_eq!(zero["estimates"]["error"], 0.0);
    assert!(zero["estimates"]["conditional_error"].is_null());
    assert!(zero["standard_errors"]["conditional_error"].is_null());
    assert_eq!(zero["standard_errors"]["acceptance"], 0.0);
    assert_eq!(zero["standard_errors"]["error"], 0.0);
    assert_eq!(zero["estimates"]["moments"], json!([]));
}

// Complete ImportanceSamplingTests contracts from the same pinned commit,
// tests/core/test_importance_sampling.py. Synthetic rows test statistics only.
use rmagna::costs::{ExecutionCosts, ExecutionLimits};
use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
use rmagna::noise::{NoiseSampling, RandomSource};
use rmagna::scalar::ScalarArithmetic;
use rmagna::statistics::HistoryEfficiency;
use rmagna::task::{RunTimings, TaskRunResult, TaskRunStatistics, TaskVariances};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

// Build an explicit synthetic report from public typed accumulators. No circuit
// sampling work or timing is attributed to the four literal summary rows.
fn summarize_rows(rows: &[Vec<R>], weights: &[R]) -> TaskRunResult {
    assert_eq!(rows.len(), weights.len());
    let mut accumulator =
        ExactStatistics::new(rows[0].len(), StatisticsOptions::default()).unwrap();
    let mut efficiency = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    for (row, weight) in rows.iter().zip(weights) {
        let values = row.iter().cloned().map(f).collect::<Vec<_>>();
        let weight = f(weight.clone());
        accumulator.push(&values, &weight).unwrap();
        efficiency
            .push(&values[0], Some(&values[1]), &weight, &weight)
            .unwrap();
    }
    let summary = accumulator.summarize().unwrap().unwrap();
    let cov = summary.covariance_of_mean.as_ref().unwrap();
    let paired = accumulator.paired_ratio(1, 0).unwrap().unwrap();
    let count = rows.len() as u64;
    let statistics = TaskRunStatistics {
        completed: count,
        accepted: summary.means[0].clone(),
        error: Some(summary.means[1].clone()),
        conditional_error: Some(paired.ratio),
        moments: summary.means[2..].to_vec(),
        variances: TaskVariances {
            accepted: Some(cov[0][0].clone()),
            error: Some(cov[1][1].clone()),
            conditional_error: paired.variance,
            moments: (2..summary.means.len())
                .map(|i| Some(cov[i][i].clone()))
                .collect(),
        },
        mean_weight: summary.mean_weight,
        effective_sample_size: summary.effective_sample_size,
    };
    TaskRunResult {
        execution_limits: ExecutionLimits::default(),
        execution_costs: ExecutionCosts::default(),
        status: RunStatus::Complete,
        requested: count,
        completed: count,
        failed: 0,
        not_attempted: 0,
        seed: 0,
        statistics: Some(statistics),
        partial_statistics: None,
        efficiency: Some(efficiency.summarize().unwrap()),
        sampled_records: None,
        failures: vec![],
        timings: RunTimings {
            preparation_seconds: None,
            sampling_seconds: 0.0,
            total_seconds: 0.0,
        },
        completed_network_compilations: 0,
        completed_history_groups: 0,
    }
}
#[test]
fn original_weighted_summary_uses_joint_means_and_paired_ratio_errors() {
    let weights = [r(2, 1), r(1, 2), r(1, 3), r(1, 1)];
    let acceptance = [r(1, 1), r(1, 2), r(0, 1), r(1, 1)];
    let errors = [r(1, 2), r(0, 1), r(0, 1), r(1, 4)];
    let rows = weights
        .iter()
        .zip(&acceptance)
        .zip(&errors)
        .map(|((w, a), e)| vec![w * a, w * e, w * (a - r(2, 1) * e)])
        .collect::<Vec<_>>();
    let result = summarize_rows(&rows, &weights);
    let summary = result.statistics.as_ref().unwrap();
    assert_eq!(summary.accepted, f(r(13, 16)));
    assert_eq!(summary.error, Some(f(r(5, 16))));
    assert_eq!(summary.moments, [f(r(3, 16))]);
    assert_eq!(summary.conditional_error, Some(f(r(5, 13))));
    assert_eq!(summary.variances.conditional_error, Some(f(r(872, 85683))));
    assert_eq!(summary.mean_weight, f(r(23, 24)));
    assert_eq!(summary.effective_sample_size, f(r(529, 193)));
    let exported = export_batch_metrics(&result, NumericOptions::default()).unwrap();
    for (key, expected) in [
        ("acceptance", 13. / 16.),
        ("error", 5. / 16.),
        ("conditional_error", 5. / 13.),
    ] {
        near(exported["estimates"][key].as_f64().unwrap(), expected);
    }
    near(
        exported["estimates"]["moments"][0].as_f64().unwrap(),
        3. / 16.,
    );
    near(
        exported["standard_errors"]["conditional_error"]
            .as_f64()
            .unwrap(),
        (872.0_f64 / 85683.).sqrt(),
    );
    near(exported["weights"]["ess"].as_f64().unwrap(), 529. / 193.);
    near(exported["weights"]["mean"].as_f64().unwrap(), 23. / 24.);
}
struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("invalid likelihood consumed entropy")
    }
}
#[test]
fn original_huge_and_tiny_weights_remain_range_safe() {
    for sign in [1, -1] {
        let huge = BigInt::one() << 2000usize;
        let weight = if sign > 0 {
            R::from_integer(huge)
        } else {
            R::new(BigInt::one(), huge)
        };
        let approximate = approximate_real(&f(weight.clone()), NumericOptions::default()).unwrap();
        assert_eq!(approximate.scientific.mantissa / 2., 0.5);
        assert_eq!(approximate.scientific.exponent + 1, sign * 2000 + 1);
        let result = summarize_rows(
            &[
                vec![weight.clone(), weight.clone()],
                vec![weight.clone(), weight.clone()],
            ],
            &[weight.clone(), weight],
        );
        let exported = export_batch_metrics(&result, NumericOptions::default()).unwrap();
        assert!(exported["estimates"]["acceptance"].is_null());
        assert_eq!(exported["estimates"]["conditional_error"], 1.0);
        assert_eq!(
            exported["estimates_scientific"]["acceptance"],
            json!({"mantissa":0.5,"exponent2":sign*2000+1})
        );
        let field = if sign > 0 {
            "overflow_fields"
        } else {
            "underflow_fields"
        };
        assert!(
            exported["numerics"][field]
                .as_array()
                .unwrap()
                .contains(&json!("estimates.acceptance"))
        );
        assert_eq!(exported["weights"]["ess"], 2.0);
    }
    let source = parse_circuit("H 0\nM 0", &ParseOptions::default()).unwrap();
    let modules = parse_task(
        r#"{"sample_records":[0]}"#,
        Path::new("."),
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&source, &ParseOptions::default())
    .unwrap();
    let evaluator = TaskEvaluator::new(
        &source,
        &modules,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    )
    .unwrap();
    let error = evaluator
        .sample_supplied_noise(&[], &r(-1, 1), &mut NoEntropy)
        .err()
        .expect("negative supplied likelihood was accepted");
    assert_eq!(error.field, "noise");
    assert!(error.message.contains("nonnegative"));
    // The valid route actually needs entropy; the negative guard is not vacuous.
    struct Counted {
        source: SplitMix64,
        words: usize,
    }
    impl RandomSource for Counted {
        fn next_u64(&mut self) -> u64 {
            self.words += 1;
            self.source.next_u64()
        }
    }
    let mut valid = Counted {
        source: SplitMix64::new(7),
        words: 0,
    };
    let accepted = evaluator
        .sample_supplied_noise(&[], &R::one(), &mut valid)
        .unwrap();
    assert_eq!(accepted.sampled_records.len(), 1);
    assert!(valid.words > 0);
    assert_ne!(valid.source.state(), 7);
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-native-importance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("zero.magna"), "I 0").unwrap();
        Self(p)
    }
    fn file_task(&self, source: &str, sampling: NoiseSampling) -> FileTask {
        std::fs::write(self.0.join("c.magna"), source).unwrap();
        std::fs::write(
            self.0.join("task.json"),
            r#"{"outputs":[1],"ideal_state":"zero.magna"}"#,
        )
        .unwrap();
        FileTask::load(
            &self.0.join("c.magna"),
            Some(&self.0.join("task.json")),
            &ControllerRegistry::default(),
            sampling,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_complex_terminal_recovery_keeps_true_born_sampling() {
    let files = Files::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            captured.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    std::fs::write(
        files.0.join("recovery.magna"),
        "H 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0",
    )
    .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit("X_ERROR(1/10) 0", &parse).unwrap();
    // The original infers the physical output. Rust names output zero explicitly.
    let modules = parse_task(
        r#"{"outputs":[0],"ideal_state":"zero.magna","recovery":"recovery.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&physical, &parse)
    .unwrap();
    let evaluator = TaskEvaluator::new_with_sampling(
        &physical,
        &modules,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Importance {
            proposal_scale: r(5, 1),
        },
    )
    .unwrap();
    let mut branches = BTreeSet::new();
    for seed in 0..12 {
        let before = seen.lock().unwrap().len();
        let draw = evaluator
            .sample_supplied_noise(
                &[FaultSelection {
                    site: 0,
                    alternative: 0,
                }],
                &r(1, 5),
                &mut SplitMix64::new(seed),
            )
            .unwrap();
        let history = seen.lock().unwrap();
        assert_eq!(history.len(), before + 1);
        assert_eq!(history[before].len(), 1);
        let bit = history[before][0];
        branches.insert(bit);
        assert_eq!(draw.accepted, f(r(1, 5)));
        assert_eq!(
            draw.error,
            if bit { ExactField::zero() } else { f(r(1, 5)) }
        );
        assert_eq!(draw.controller_calls.len(), 1);
        let call = &draw.controller_calls[0];
        assert_eq!(call.name, "echo");
        assert_eq!(call.inputs, [bit]);
        assert_eq!(call.outputs, [bit]);
        assert_eq!(draw.noise_likelihood_ratio, r(1, 5));
        assert_eq!(draw.history_weight, ExactField::one());
        let weight = FieldArithmetic::default()
            .multiply(
                &f(draw.noise_likelihood_ratio.clone()),
                &draw.history_weight,
            )
            .unwrap();
        let scaled = approximate_real(&weight, NumericOptions::default())
            .unwrap()
            .scientific;
        assert_eq!((scaled.mantissa / 2., scaled.exponent + 1), (0.8, -2));
    }
    assert_eq!(seen.lock().unwrap().len(), 12);
    assert_eq!(branches, BTreeSet::from([false, true]));
}
fn model(rows: &[&[&str]]) -> NoiseModel {
    NoiseModel::new(
        rows.iter()
            .map(|row| row.iter().map(|p| p.parse().unwrap()).collect())
            .collect(),
        &NoiseOptions::default(),
    )
    .unwrap()
}
#[test]
fn original_native_draws_return_exact_likelihood_products() {
    // P/Q is exactly the original factor ABI, with identity-first normalized laws.
    let proposal = model(&[&["1/2", "1/2"], &["1/2", "1/4", "1/4"]]);
    let physical = model(&[&["9/10", "1/10"], &["4/5", "1/10", "1/10"]]);
    let weighted =
        NoiseSampler::importance(physical, proposal.clone(), NoiseOptions::default()).unwrap();
    let plain = NoiseSampler::physical(proposal.clone(), NoiseOptions::default());
    for seed in 0..64 {
        let mut left = SplitMix64::new(seed);
        let mut right = SplitMix64::new(seed);
        let draw = weighted.draw(&mut left).unwrap();
        let unweighted = plain.draw(&mut right).unwrap();
        assert_eq!(draw.faults, unweighted.faults);
        assert_eq!(draw.entropy_words, unweighted.entropy_words);
        assert_eq!(left, right);
        let mut choices = [false; 2];
        for fault in &draw.faults {
            choices[fault.site] = true;
        }
        let factor0 = if choices[0] { r(1, 5) } else { r(9, 5) };
        let factor1 = if choices[1] { r(2, 5) } else { r(8, 5) };
        assert_eq!(draw.likelihood_ratio, factor0 * factor1);
    }
    let empty = model(&[]);
    let empty_sampler =
        NoiseSampler::importance(empty.clone(), empty.clone(), NoiseOptions::default()).unwrap();
    let mut rng = SplitMix64::new(1);
    let before = rng;
    let draw = empty_sampler.draw(&mut rng).unwrap();
    assert!(draw.faults.is_empty());
    assert_eq!(draw.likelihood_ratio, R::one());
    assert_eq!(draw.entropy_words, 0);
    assert_eq!(rng, before);
    assert!(matches!(
        NoiseSampler::importance(empty, proposal, NoiseOptions::default()),
        Err(rmagna::noise::NoiseError::ProposalShape)
    ));
}
#[test]
fn original_reproducibility_and_physical_mode_unchanged() {
    let files = Files::new();
    let source = "X_ERROR(1/10) 0\nY_ERROR(1/5) 0\nM(1/20) 0\nCX rec[-1] 1";
    let run = TaskRunOptions {
        shots: 32,
        seed: 4,
        ..Default::default()
    };
    let policies = [
        NoiseSampling::default(),
        NoiseSampling::Physical,
        NoiseSampling::Importance {
            proposal_scale: r(1, 1),
        },
        NoiseSampling::Importance {
            proposal_scale: r(2, 1),
        },
        NoiseSampling::Importance {
            proposal_scale: r(2, 1),
        },
    ];
    let reports = policies
        .iter()
        .map(|policy| files.file_task(source, policy.clone()).run(&run).unwrap())
        .collect::<Vec<_>>();
    for report in &reports {
        assert_eq!(report.raw_result().status, RunStatus::Complete);
        assert_eq!(report.raw_result().completed, 32);
    }
    for index in [1, 2] {
        assert_eq!(
            reports[0].raw_result().statistics,
            reports[index].raw_result().statistics
        );
        assert_eq!(
            reports[0].to_json().unwrap()["estimates"],
            reports[index].to_json().unwrap()["estimates"]
        );
    }
    assert_eq!(
        reports[2]
            .raw_result()
            .statistics
            .as_ref()
            .unwrap()
            .effective_sample_size,
        f(r(32, 1))
    );
    let unit_metadata = reports[2].metadata();
    assert_eq!(unit_metadata["proposal_summary"]["scaled_categories"], 0);
    assert_eq!(
        reports[3].raw_result().statistics,
        reports[4].raw_result().statistics
    );
    assert_eq!(
        reports[3].metadata()["fingerprints"]["sampling"],
        reports[4].metadata()["fingerprints"]["sampling"]
    );
    assert_ne!(
        reports[0].to_json().unwrap()["fingerprints"]["sampling"],
        reports[3].to_json().unwrap()["fingerprints"]["sampling"]
    );
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let instrument = compile_affine(
        &circuit,
        &CompileOptions::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    let physical = NoiseModel::physical(&instrument, &NoiseOptions::default()).unwrap();
    let plain = NoiseSampler::configured(
        physical.clone(),
        &policies[0],
        &BTreeSet::new(),
        NoiseOptions::default(),
    )
    .unwrap();
    let unit = NoiseSampler::configured(
        physical,
        &policies[2],
        &BTreeSet::new(),
        NoiseOptions::default(),
    )
    .unwrap();
    assert_eq!(unit.scaled_proposal_summary().unwrap().scaled_categories, 0);
    let mut left = SplitMix64::new(51);
    let mut right = SplitMix64::new(51);
    let a = (0..64)
        .map(|_| plain.draw(&mut left).unwrap())
        .collect::<Vec<_>>();
    let b = (0..64)
        .map(|_| unit.draw(&mut right).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(a.len(), 64);
    assert_eq!(a, b);
    assert_eq!(left, right);
}
