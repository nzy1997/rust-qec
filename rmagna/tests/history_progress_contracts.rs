//! Complete finite progress/default/manual-sampler contract from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_history_efficiency.py, without importing its native builder.

use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{
    FileEnumerationOptions, FileLoadOptions, FilePreparationOptions, FileTask, FileTaskOptions,
};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{BatchProgress, ProgressObserver, RunStatus, TaskRunOptions, TaskRunResult};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new(sample_records: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let files = Self(std::env::temp_dir().join(format!(
            "rmagna-original-history-progress-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&files.0).unwrap();
        std::fs::write(files.0.join("physical.magna"), "H 0\nT 0\nMX 0\n").unwrap();
        std::fs::write(files.0.join("ideal.magna"), "H 0\n").unwrap();
        let task = json!({
            "outputs":[0],
            "sample_records":if sample_records {vec![0]} else {vec![]},
            "ideal_state":{"circuit":"ideal.magna","outputs":[0]}
        });
        std::fs::write(files.0.join("task.json"), task.to_string()).unwrap();
        files
    }
    fn load(&self, sampler: Option<HistorySampler>) -> FileTask {
        let mut options = FileTaskOptions::default();
        if let Some(sampler) = sampler {
            options.evaluation.runtime.history_sampler = sampler;
        }
        FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
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

fn description(value: &Value, sampler: HistorySampler) {
    let (name, expected) = match sampler {
        HistorySampler::AffineSupport => ("affine_support", Value::Null),
        HistorySampler::ExactMarginal => ("exact_marginal", json!("true conditional distribution")),
    };
    assert_eq!(value["history_sampler"], name);
    let sampling = value.get("sampling").unwrap().as_object().unwrap();
    // Missing and present-null are different contracts.
    assert_eq!(sampling.get("exact_marginal_records"), Some(&expected));
    assert_eq!(sampling.get("history_sampler"), Some(&json!(name)));
}

fn same_draws(a: &TaskRunResult, b: &TaskRunResult) {
    assert_eq!(
        (a.status, a.completed, a.failed, a.not_attempted, a.seed),
        (b.status, b.completed, b.failed, b.not_attempted, b.seed)
    );
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.partial_statistics, b.partial_statistics);
    assert_eq!(a.efficiency, b.efficiency);
    assert_eq!(a.sampled_records, b.sampled_records);
    assert_eq!(a.failures, b.failures);
    for (a, b) in [
        (a.execution_costs.compilation, b.execution_costs.compilation),
        (a.execution_costs.query, b.execution_costs.query),
        (a.execution_costs.decoder, b.execution_costs.decoder),
        (a.execution_costs.support, b.execution_costs.support),
    ] {
        assert_eq!(
            (a.attempted, a.succeeded, a.failed),
            (b.attempted, b.succeeded, b.failed)
        );
    }
}

fn sampler_weights(result: &TaskRunResult, sampler: HistorySampler) {
    let records = result.sampled_records.as_ref().unwrap();
    assert_eq!(records.len(), 21);
    assert!(records.iter().all(|row| row.len() == 1));
    assert!(records.iter().any(|row| row[0]));
    assert!(records.iter().any(|row| !row[0]));
    let efficiency = result.efficiency.as_ref().unwrap();
    assert_eq!(efficiency.completed, 21);
    assert_eq!(efficiency.zero_weights, 0);
    match sampler {
        HistorySampler::ExactMarginal => {
            assert_eq!(efficiency.kappa, Some(ExactField::one()));
            assert_eq!(efficiency.mean_weight, Some(ExactField::one()));
        }
        HistorySampler::AffineSupport => {
            // H;T;MX has Born masses (1 +/- 1/sqrt(2))/2 and uniform
            // affine proposals. Every draw must carry W=1 +/- 1/sqrt(2).
            // These exact first/second moments also detect an unreported
            // switch to unit-weight exact-marginal draws within the batch.
            let imbalance: i64 = records.iter().map(|row| if row[0] { -1 } else { 1 }).sum();
            let rational = |n: i64, d: i64| BigRational::new(n.into(), d.into());
            let f = FieldArithmetic::default();
            let sum = f
                .value([
                    rational(21, 1),
                    rational(imbalance, 2),
                    rational(0, 1),
                    rational(-imbalance, 2),
                ])
                .unwrap();
            let squares = f
                .value([
                    rational(63, 2),
                    rational(imbalance, 1),
                    rational(0, 1),
                    rational(-imbalance, 1),
                ])
                .unwrap();
            let n = f.from_rational(rational(21, 1)).unwrap();
            assert_eq!(efficiency.mean_weight, Some(f.divide(&sum, &n).unwrap()));
            assert_eq!(
                efficiency.effective_sample_size,
                f.divide(&f.multiply(&sum, &sum).unwrap(), &squares)
                    .unwrap()
            );
            assert_ne!(efficiency.mean_weight, Some(ExactField::one()));
        }
    }
}

#[test]
fn original_default_progress_and_manual_born_preserve_draws_and_record_semantics() {
    assert_eq!(
        FileTaskOptions::default()
            .evaluation
            .runtime
            .history_sampler,
        HistorySampler::AffineSupport
    );
    let files = Files::new(true);
    let run = TaskRunOptions {
        shots: 21,
        seed: 42,
        ..Default::default()
    };
    for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
        let file = files.load((sampler == HistorySampler::ExactMarginal).then_some(sampler));
        description(&file.inspect(&run).unwrap(), sampler);
        let compiled = file.compile().unwrap();
        let prepared = file
            .prepare_sampling(&FilePreparationOptions::default())
            .unwrap();
        description(&compiled.inspect(&run).unwrap(), sampler);
        description(&prepared.inspect(&run).unwrap(), sampler);
        for mode in 0..3 {
            let baseline = match mode {
                0 => file.run(&run),
                1 => compiled.run(&run),
                _ => prepared.run(&run),
            }
            .unwrap();
            let raw = baseline.raw_result();
            assert_eq!(
                (
                    raw.status,
                    raw.completed,
                    raw.failed,
                    raw.not_attempted,
                    raw.seed
                ),
                (RunStatus::Complete, 21, 0, 0, 42)
            );
            sampler_weights(raw, sampler);
            description(&baseline.to_json().unwrap(), sampler);
            let mut events = Vec::new();
            let mut callback = |event: &BatchProgress| {
                events.push(event.clone());
                Ok(())
            };
            let mut observer = ProgressObserver::new(NonZeroU64::new(8).unwrap(), &mut callback);
            let observed = match mode {
                0 => file.run_with_progress(&run, &mut observer),
                1 => compiled.run_with_progress(&run, &mut observer),
                _ => prepared.run_with_progress(&run, &mut observer),
            }
            .unwrap();
            same_draws(raw, observed.raw_result());
            let receipt = observer.receipt();
            assert_eq!(
                (
                    receipt.attempted,
                    receipt.succeeded,
                    receipt.failed,
                    receipt.last_completed
                ),
                (3, 3, 0, 21)
            );
            assert_eq!(
                events
                    .iter()
                    .map(|event| event.completed)
                    .collect::<Vec<_>>(),
                [8, 16, 21]
            );
            assert!(
                events
                    .iter()
                    .all(|event| event.requested == 21 && event.history_sampler == sampler)
            );
            assert_eq!(
                events.last().unwrap().efficiency.as_ref().unwrap(),
                observed.raw_result().efficiency.as_ref().unwrap()
            );
            assert_eq!(observed.raw_result().execution_costs.decoder.attempted, 0);
            let value = observed.to_json().unwrap();
            description(&value, sampler);
            assert_eq!(value["progress"]["succeeded"], 3);
            assert_eq!(value["progress"]["last_completed"], 21);
        }
    }
}

#[test]
fn exact_enumeration_keeps_integrated_histories_without_a_sampled_record_claim() {
    let files = Files::new(false);
    for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
        let file = files.load(Some(sampler));
        let compiled = file.compile().unwrap();
        for report in [
            file.enumerate(&FileEnumerationOptions::default()).unwrap(),
            compiled
                .enumerate(&FileEnumerationOptions::default())
                .unwrap(),
        ] {
            let value = report.to_json().unwrap();
            let sampling = value.get("sampling").unwrap().as_object().unwrap();
            assert_eq!(
                sampling.get("histories"),
                Some(&json!("all_positive_causal_histories"))
            );
            assert!(!sampling.contains_key("exact_marginal_records"));
            assert!(value.get("history_sampler").is_none());
        }
    }
}
