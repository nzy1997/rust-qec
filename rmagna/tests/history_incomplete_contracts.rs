//! Partial witnesses for the final original history-efficiency method in
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
//! The native floating-range diagnostic remains an explicit compatibility gap.

use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{
    FileLoadOptions, FileSamplerComparisonOptions, FileTask, FileTaskOptions,
    SamplerComparisonStatus,
};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::statistics::{EfficiencyStatus, HistoryEfficiency};
use rmagna::task::{RunStatus, TaskRunOptions};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new(source: &str, target: Option<&str>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let files = Self(std::env::temp_dir().join(format!(
            "rmagna-original-incomplete-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&files.0).unwrap();
        std::fs::write(files.0.join("physical.magna"), source).unwrap();
        if let Some(target) = target {
            std::fs::write(files.0.join("target.magna"), target).unwrap();
            std::fs::write(files.0.join("task.json"), json!({"sample_records":[0],"ideal_state":{"circuit":"target.magna","outputs":[0]}}).to_string()).unwrap();
        }
        files
    }
    fn load(&self, options: FileTaskOptions) -> FileTask {
        let task = self.0.join("task.json");
        FileTask::load(
            &self.0.join("physical.magna"),
            task.exists().then_some(task.as_path()),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
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
fn prepared_options() -> FileTaskOptions {
    let mut options = FileTaskOptions::default();
    options.evaluation.runtime.compile.probability_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.density_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.moment_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options
}

#[test]
fn original_one_query_pilots_fail_and_metric_free_identity_has_no_kappa() {
    let files = Files::new("H 0\nT 0\nMX 0\n", Some("H 0\n"));
    let file = files.load(prepared_options());
    let mut options = FileSamplerComparisonOptions::default();
    options.pilot.shots = 4;
    options.pilot.execution_limits.max_queries = 1;
    let result = file.compare_samplers(&options).unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Failed);
    assert!(result.decision().is_none());
    let value = result.to_json();
    assert_eq!(value["status"], "failed");
    assert!(value["gamma"].is_null());
    assert!(value["recommended_sampler"].is_null());
    assert_eq!(value["automatic_switching"], false);
    assert_eq!(value["production_started"], false);
    for (pilot, sampler, name) in [
        (
            result.affine_support(),
            HistorySampler::AffineSupport,
            "affine_support",
        ),
        (
            result.exact_marginal(),
            HistorySampler::ExactMarginal,
            "exact_marginal",
        ),
    ] {
        assert_eq!(pilot.history_sampler(), sampler);
        assert!(pilot.preparation().is_complete());
        let raw = pilot.raw_result().unwrap();
        assert_eq!(raw.status, RunStatus::Failed);
        assert_eq!(raw.requested, 4);
        assert!(raw.completed < 4);
        assert!(raw.execution_costs.query.attempted <= 1);
        assert_eq!(value["pilots"][name]["history_sampler"], name);
        assert_eq!(
            value["pilots"][name]["sampling_receipt"]["completed"],
            raw.completed
        );
    }
    drop(file);
    drop(files);
    assert_eq!(result.to_json(), value);

    let plain = Files::new("I 0\n", None);
    let prepared = plain
        .load(prepared_options())
        .prepare_sampling(&Default::default())
        .unwrap();
    let batch = prepared
        .run(&TaskRunOptions {
            shots: 2,
            ..Default::default()
        })
        .unwrap();
    let raw = batch.raw_result();
    assert_eq!(raw.status, RunStatus::Complete);
    assert_eq!(raw.completed, 2);
    let efficiency = raw.efficiency.as_ref().unwrap();
    assert_eq!(efficiency.status, EfficiencyStatus::NoErrorMetric);
    assert_eq!(efficiency.status.as_str(), "no_error_metric");
    assert_eq!(efficiency.kappa, None);
}

#[test]
fn original_tiny_errors_retain_exact_zero_and_positive_residuals_without_clamping() {
    let rational = |n: i64, d: i64| BigRational::new(n.into(), d.into());
    let two_to_negative_100000 = BigRational::new(1.into(), BigInt::from(1) << 100000_usize);
    let scientific_tuple = rational(1, 2) * two_to_negative_100000;
    let tiny = BigRational::new(1.into(), BigInt::from(1) << 100001_usize);
    assert_eq!(scientific_tuple, tiny);
    assert!(tiny > rational(0, 1));
    assert_eq!((tiny.numer().bits(), tiny.denom().bits()), (1, 100002));
    let arithmetic = FieldArithmetic::default();
    let field_tiny = arithmetic.from_rational(tiny.clone()).unwrap();
    assert_eq!(field_tiny.coefficients()[0], tiny);
    assert!(!field_tiny.is_zero());
    let summarize = |errors: &[BigRational]| {
        let mut state = HistoryEfficiency::new(arithmetic).unwrap();
        for error in errors {
            state
                .push(
                    &ExactField::one(),
                    Some(&arithmetic.from_rational(error.clone()).unwrap()),
                    &ExactField::one(),
                    &ExactField::one(),
                )
                .unwrap();
        }
        state.summarize().unwrap()
    };
    // Literal identical native errors: exact centered residuals are zero. The
    // native diagnostic_range_exceeded status is deliberately not emulated.
    let same_center = (&tiny + &tiny) / rational(2, 1);
    let same_delta = &tiny - same_center;
    assert_eq!(&same_delta * &same_delta, rational(0, 1));
    let same = summarize(&[tiny.clone(), tiny.clone()]);
    assert_eq!(same.completed, 2);
    assert_eq!(same.status, EfficiencyStatus::ZeroResidualVariance);
    assert_eq!(same.status.as_str(), "zero_residual_variance");
    assert!(
        same.kappa.is_none()
            && same.variance_coefficient.is_none()
            && same.exact_marginal_variance_coefficient.is_none()
    );

    // Independent centered rational sums for distinct zero/tiny draws give
    // a strictly positive coefficient tiny^2/4 = 2^-200004, with L/W=1.
    let center = &tiny / rational(2, 1);
    let left = -&center;
    let right = &tiny - &center;
    let variance = (&left * &left + &right * &right) / rational(2, 1);
    let literal = BigRational::new(1.into(), BigInt::from(1) << 200004_usize);
    assert_eq!(variance, literal);
    assert!(variance > rational(0, 1));
    let distinct = summarize(&[rational(0, 1), tiny]);
    assert_eq!(distinct.completed, 2);
    assert_eq!(distinct.status, EfficiencyStatus::Available);
    assert_eq!(distinct.kappa, Some(ExactField::one()));
    let exact_variance = arithmetic.from_rational(variance).unwrap();
    assert_eq!(distinct.variance_coefficient, Some(exact_variance.clone()));
    assert_eq!(
        distinct.exact_marginal_variance_coefficient,
        Some(exact_variance)
    );
}
