// SPDX-License-Identifier: MPL-2.0
// Complete FileApiTests report and measurement methods from
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py, translated to the public Rust APIs.

use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileLoadOptions, FileRunReport, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, RandomSource};
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::statistics::{ExactStatistics, HistoryEfficiency, StatisticsOptions};
use rmagna::task::{
    EvaluationOptions, RunPhase, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions,
    TaskRunStatistics, TaskVariances, export_batch_metrics, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const SELECTED: &str = "H 0 1\nM 0 1\nDETECTOR rec[-2] rec[-1]\nPOSTSELECT det[0]";
const REJECTED: &str = "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
const UNSELECTED: &str = "H 0 1\nM 0 1";
const FLIPPED: &str = "M(1) 0\nCX rec[-1] 1\nDETECTOR rec[-1]\nPOSTSELECT !det[0]";
const MOMENT_TASK: &str = r#"{"observables":[[0,1],[0,2]]}"#;

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-file-reports-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    fn run(
        &self,
        source: &str,
        task: Option<&str>,
        evaluation: EvaluationOptions,
        run: TaskRunOptions,
    ) -> FileRunReport {
        let circuit_path = self.write("physical.magna", source);
        let task_path = task.map(|text| self.write("task.json", text));
        let file = FileTask::load(
            &circuit_path,
            task_path.as_deref(),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions {
                evaluation,
                ..Default::default()
            },
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        let report = file.run(&run).unwrap();
        let json = report.to_json().unwrap();
        assert_eq!(json["route"], "linear_affine");
        assert_eq!(
            json["compilation"]["compiler_diagnostics"]["strategy"],
            format!("{:?}", evaluation.runtime.compile.compiler_strategy),
        );
        let sampler = match evaluation.runtime.history_sampler {
            HistorySampler::ExactMarginal => "exact_marginal",
            HistorySampler::AffineSupport => "affine_support",
        };
        assert_eq!(json["history_sampler"], sampler);
        assert_eq!(std::fs::read_to_string(circuit_path).unwrap(), source);
        if let Some(text) = task {
            assert_eq!(std::fs::read_to_string(task_path.unwrap()).unwrap(), text);
        }
        report
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn configurations() -> Vec<EvaluationOptions> {
    let mut options = Vec::new();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut evaluation = EvaluationOptions::default();
                evaluation.runtime.compile.compiler_strategy = compiler;
                evaluation.runtime.strategy = causal;
                evaluation.runtime.history_sampler = sampler;
                options.push(evaluation);
            }
        }
    }
    assert_eq!(options.len(), 8);
    options
}

fn fraction(numerator: i64, denominator: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(numerator.into(), denominator.into()))
        .unwrap()
}

fn run_options(shots: u64) -> TaskRunOptions {
    TaskRunOptions {
        shots,
        ..Default::default()
    }
}

fn assert_complete(report: &FileRunReport, shots: u64) -> Value {
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Complete);
    assert_eq!(
        (raw.requested, raw.completed, raw.failed, raw.not_attempted),
        (shots, shots, 0, 0)
    );
    assert!(raw.failures.is_empty());
    assert!(raw.partial_statistics.is_none());
    assert_eq!(raw.statistics.as_ref().unwrap().completed, shots);
    let json = report.to_json().unwrap();
    assert_eq!(json["status"], "complete");
    assert_eq!(json["completed"], shots);
    json
}

#[test]
fn original_joint_error_ratio_zero_acceptance_and_failed_budgets() {
    let files = Files::new();
    // The native empty target infers its width; explicit Rust targets own it.
    let zero_one = files.write("zero-one.magna", "I 0");
    let zero_two = files.write("zero-two.magna", "I 0 1");
    let selected_task = r#"{"outputs":[0,1],"ideal_state":"zero-two.magna"}"#;
    let rejected_task = r#"{"outputs":[0],"ideal_state":"zero-one.magna"}"#;
    for evaluation in configurations() {
        let selected = files.run(SELECTED, Some(selected_task), evaluation, run_options(3));
        let json = assert_complete(&selected, 3);
        let statistics = selected.raw_result().statistics.as_ref().unwrap();
        assert_eq!(statistics.accepted, fraction(1, 2));
        assert_eq!(statistics.error, Some(fraction(1, 4)));
        assert_eq!(statistics.conditional_error, Some(fraction(1, 2)));
        assert_eq!(statistics.variances.accepted, Some(ExactField::zero()));
        assert_eq!(statistics.variances.error, Some(ExactField::zero()));
        assert_eq!(
            statistics.variances.conditional_error,
            Some(ExactField::zero())
        );
        assert_eq!(statistics.mean_weight, ExactField::one());
        assert_eq!(statistics.effective_sample_size, fraction(3, 1));
        assert!(statistics.moments.is_empty());
        assert_eq!(
            json["estimates"],
            json!({"acceptance":0.5,"error":0.25,"conditional_error":0.5,"moments":[]})
        );
        assert_eq!(
            json["standard_errors"],
            json!({"acceptance":0.0,"error":0.0,"conditional_error":0.0,"moments":[]})
        );

        let rejected = files.run(REJECTED, Some(rejected_task), evaluation, run_options(2));
        let json = assert_complete(&rejected, 2);
        let statistics = rejected.raw_result().statistics.as_ref().unwrap();
        assert_eq!(statistics.accepted, ExactField::zero());
        assert_eq!(statistics.error, Some(ExactField::zero()));
        assert_eq!(statistics.conditional_error, None);
        assert_eq!(statistics.variances.accepted, Some(ExactField::zero()));
        assert_eq!(statistics.variances.error, Some(ExactField::zero()));
        assert_eq!(statistics.variances.conditional_error, None);
        assert_eq!(
            json["estimates"],
            json!({"acceptance":0.0,"error":0.0,"conditional_error":null,"moments":[]})
        );
        assert!(json["standard_errors"]["conditional_error"].is_null());

        let failed = files.run(
            UNSELECTED,
            None,
            evaluation,
            TaskRunOptions {
                execution_limits: ExecutionLimits {
                    max_queries: 2,
                    ..Default::default()
                },
                ..run_options(3)
            },
        );
        let raw = failed.raw_result();
        assert_eq!(raw.status, RunStatus::Failed);
        assert_eq!(
            (raw.requested, raw.completed, raw.failed, raw.not_attempted),
            (3, 2, 1, 0)
        );
        assert!(raw.statistics.is_none());
        let partial = raw.partial_statistics.as_ref().unwrap();
        assert_eq!(partial.completed, 2);
        assert_eq!(partial.accepted, ExactField::one());
        assert_eq!(partial.error, None);
        assert_eq!(partial.conditional_error, None);
        assert_eq!(partial.variances.accepted, Some(ExactField::zero()));
        assert_eq!(partial.mean_weight, ExactField::one());
        assert_eq!(partial.effective_sample_size, fraction(2, 1));
        assert_eq!(raw.execution_costs.query.attempted, 2);
        assert_eq!(raw.execution_costs.query.succeeded, 2);
        assert_eq!(raw.execution_costs.query.failed, 0);
        assert_eq!(raw.failures.len(), 1);
        assert_eq!(raw.failures[0].phase, RunPhase::Sampling);
        assert_eq!(raw.failures[0].shot, Some(2));
        assert_eq!(raw.failures[0].kind, "TaskError");
        assert!(raw.failures[0].message.contains("max_queries = 2"));
        let json = failed.to_json().unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["completed"], 2);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["not_attempted"], 0);
        assert!(json["estimates"].is_null());
        assert_eq!(
            json["partial_statistics"]["estimates"],
            json!({"acceptance":1.0,"error":null,"conditional_error":null,"moments":[]})
        );
        assert_eq!(json["resources"]["queries"], 2);
        assert_eq!(json["failures"][0]["phase"], "sampling");
        assert_eq!(json["failures"][0]["shot"], 2);
        assert_eq!(json["failures"][0]["type"], "TaskError");
        assert!(
            json["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("max_queries = 2")
        );
    }
    assert_eq!(std::fs::read_to_string(zero_one).unwrap(), "I 0");
    assert_eq!(std::fs::read_to_string(zero_two).unwrap(), "I 0 1");
}

#[test]
fn original_tiny_exact_masses_keep_scientific_values_and_conditional_ratio() {
    let arithmetic = FieldArithmetic::default();
    let accepted = arithmetic
        .from_rational(BigRational::new(9.into(), BigInt::from(1) << 1160))
        .unwrap();
    let error = arithmetic
        .from_rational(BigRational::new(3.into(), BigInt::from(1) << 1160))
        .unwrap();
    let mut statistics = ExactStatistics::new(2, StatisticsOptions::default()).unwrap();
    let mut efficiency = HistoryEfficiency::new(arithmetic).unwrap();
    for _ in 0..2 {
        statistics
            .push(&[accepted.clone(), error.clone()], &ExactField::one())
            .unwrap();
        efficiency
            .push(
                &accepted,
                Some(&error),
                &ExactField::one(),
                &ExactField::one(),
            )
            .unwrap();
    }
    let summary = statistics.summarize().unwrap().unwrap();
    let ratio = statistics.paired_ratio(1, 0).unwrap().unwrap();
    assert_eq!(summary.count, 2);
    assert_eq!(summary.means, [accepted.clone(), error.clone()]);
    assert_eq!(ratio.ratio, fraction(1, 3));
    assert_eq!(ratio.variance, Some(ExactField::zero()));
    let covariance = summary.covariance_of_mean.unwrap();
    assert_eq!(covariance, vec![vec![ExactField::zero(); 2]; 2]);
    let typed = TaskRunStatistics {
        completed: summary.count,
        accepted: summary.means[0].clone(),
        error: Some(summary.means[1].clone()),
        conditional_error: Some(ratio.ratio),
        moments: vec![],
        variances: TaskVariances {
            accepted: Some(covariance[0][0].clone()),
            error: Some(covariance[1][1].clone()),
            conditional_error: ratio.variance,
            moments: vec![],
        },
        mean_weight: summary.mean_weight,
        effective_sample_size: summary.effective_sample_size,
    };
    // Synthetic summary witness. A genuine empty run supplies the validated
    // batch envelope; its physical probability is not the tiny synthetic mass.
    let files = Files::new();
    let mut result = files
        .run("", None, EvaluationOptions::default(), run_options(2))
        .into_raw_result();
    result.statistics = Some(typed.clone());
    result.efficiency = Some(efficiency.summarize().unwrap());
    let json = export_batch_metrics(&result, NumericOptions::default()).unwrap();
    assert!(json["estimates"]["acceptance"].is_null());
    assert!(json["estimates"]["error"].is_null());
    assert_eq!(json["estimates"]["conditional_error"], 1.0 / 3.0);
    assert_eq!(
        json["estimates_scientific"]["acceptance"],
        json!({"mantissa":0.5625,"exponent2":-1156})
    );
    assert_eq!(
        json["estimates_scientific"]["error"],
        json!({"mantissa":0.75,"exponent2":-1158})
    );
    assert_eq!(
        json["standard_errors"],
        json!({"acceptance":0.0,"error":0.0,"conditional_error":0.0,"moments":[]})
    );
    let scientific_zero = json!({"mantissa":0.0,"exponent2":0});
    assert_eq!(
        json["standard_errors_scientific"],
        json!({
            "acceptance":scientific_zero,"error":scientific_zero,
            "conditional_error":scientific_zero,"moments":[]
        })
    );
    let underflow = json["numerics"]["underflow_fields"].as_array().unwrap();
    assert!(underflow.contains(&json!("estimates.acceptance")));
    assert!(underflow.contains(&json!("estimates.error")));
    assert!(
        json["numerics"]["overflow_fields"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let serialized = serde_json::to_string(&json).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&serialized).unwrap(), json);
    assert_eq!(result.statistics, Some(typed));
    assert_eq!(result.statistics.as_ref().unwrap().accepted, accepted);
    assert_eq!(result.statistics.as_ref().unwrap().error, Some(error));
}

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("the original deterministic measurement must not consume entropy")
    }
}

fn measurement_case(source: &str, task: &str, moments: &[i64]) {
    let files = Files::new();
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let operations = circuit.operations().to_vec();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    let registry = ControllerRegistry::default();
    let expected: Vec<_> = moments.iter().map(|&n| fraction(n, 1)).collect();
    for evaluation in configurations() {
        let report = files.run(source, Some(task), evaluation, run_options(1));
        let json = assert_complete(&report, 1);
        let statistics = report.raw_result().statistics.as_ref().unwrap();
        assert_eq!(statistics.accepted, ExactField::one());
        assert_eq!(statistics.error, None);
        assert_eq!(statistics.conditional_error, None);
        assert_eq!(statistics.moments, expected);
        assert!(json["estimates"]["error"].is_null());
        assert!(json["estimates"]["conditional_error"].is_null());
        assert_eq!(json["estimates"]["acceptance"], 1.0);
        let exported_moments: Vec<_> = moments.iter().map(|&value| value as f64).collect();
        assert_eq!(json["estimates"]["moments"], json!(exported_moments));
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, evaluation).unwrap();
        assert_eq!(evaluator.execution_route(), "linear_affine");
        assert_eq!(
            evaluator.compilation().strategy,
            evaluation.runtime.compile.compiler_strategy
        );
        let draw = evaluator.sample(&mut NoEntropy).unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.raw_acceptance, ExactScalar::one());
        assert_eq!(draw.error, ExactField::zero());
        assert_eq!(draw.raw_error, ExactScalar::zero());
        assert_eq!(draw.target_success, None);
        assert_eq!(draw.raw_target_success, None);
        assert_eq!(draw.target_normalization, None);
        assert_eq!(draw.moments, expected);
        let raw_moments: Vec<_> = moments
            .iter()
            .map(|&n| {
                if n < 0 {
                    ExactScalar::one().negated()
                } else {
                    ExactScalar::one()
                }
            })
            .collect();
        assert_eq!(draw.raw_moments, raw_moments);
        assert_eq!(draw.history_mass, ExactScalar::one());
        assert_eq!(draw.history_proposal_mass, None);
        assert_eq!(draw.history_weight, ExactField::one());
        assert_eq!(draw.noise_probability, BigRational::from_integer(1.into()));
        assert_eq!(
            draw.noise_likelihood_ratio,
            BigRational::from_integer(1.into())
        );
        assert!(draw.controller_calls.is_empty());
        assert!(draw.sampled_records.is_empty());
        assert_eq!(circuit.operations(), operations);
        assert_eq!(circuit.to_text(), source);
    }
}

#[test]
fn original_report_flip_changes_feedback_and_task_but_not_true_projection() {
    measurement_case(FLIPPED, MOMENT_TASK, &[1, -1]);
}

#[test]
fn original_pauli_products_resets_and_measure_reset_reports() {
    let one = "\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let examples = [
        format!("H 0 1\nMXX 0 1{one}"),
        format!("RY 0 1\nMYY 0 1{one}"),
        format!("H 0\nCX 0 1\nMZZ 0 1{one}"),
        "H 0\nCX 0 1\nMPP X0*X1 Z0*Z1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nPOSTSELECT det[1]".into(),
        format!("X 0\nR 0\nM 0{one}"),
        "RY 0\nMRY(1) 0\nMY 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\nPOSTSELECT det[1]".into(),
        "RX 0\nMRX(1) 0\nMX 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\nPOSTSELECT det[1]".into(),
    ];
    assert_eq!(examples.len(), 7);
    for source in examples {
        measurement_case(&source, "{}", &[]);
    }
}
