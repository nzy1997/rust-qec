use rmagna::costs::ExecutionLimits;
use rmagna::file_api::{
    FileLoadOptions, FilePreparationOptions, FileTask, FileTaskOptions, export_progress,
};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{
    BatchProgress, ProgressObserver, RunPhase, RunStatus, TaskError, TaskRunOptions, TaskRunResult,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new(target: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-file-progress-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let f = Self(root);
        f.put("circuit.magna","H 0\nT 0\nH 0\nH 1 2\nM(1/7) 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nX_ERROR(1/5) ctrl[0] 3\nM 3\nOBSERVABLE_INCLUDE(0) rec[-1]");
        f.put("task.json", if target {r#"{"outputs":[3],"sample_records":[0,3],"ideal_state":{"circuit":"ideal.magna","outputs":[0]}}"#} else {r#"{"error_observables":[0],"sample_records":[0,3]}"#});
        f.put("ideal.magna", "H 0");
        f
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn load(&self, sampler: HistorySampler, calls: Arc<AtomicUsize>) -> FileTask {
        self.load_with_sampling(sampler, calls, NoiseSampling::Physical)
    }
    fn load_with_sampling(
        &self,
        sampler: HistorySampler,
        calls: Arc<AtomicUsize>,
        sampling: NoiseSampling,
    ) -> FileTask {
        let mut registry = ControllerRegistry::default();
        registry
            .register("majority", "1", move |bits: &[bool]| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(vec![bits.iter().filter(|v| **v).count() >= 2])
            })
            .unwrap();
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.history_sampler = sampler;
        options.evaluation.runtime.compile.probability_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.evaluation.runtime.compile.density_preparation =
            QueryPreparation::FixedFibers(Default::default());
        FileTask::load(
            &self.0.join("circuit.magna"),
            Some(&self.0.join("task.json")),
            &registry,
            sampling,
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
fn same(a: &TaskRunResult, b: &TaskRunResult) {
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
fn fingerprint(a: &Value, b: &Value, every: u64) {
    assert_eq!(b["options"]["run"]["progress_every"], every);
    for key in [
        "circuit",
        "task",
        "ideal_state",
        "recovery",
        "controllers",
        "sampling",
    ] {
        assert_eq!(a["fingerprints"][key], b["fingerprints"][key]);
    }
    assert_ne!(a["fingerprints"]["options"], b["fingerprints"]["options"]);
}
#[test]
fn fresh_reused_and_prepared_progress_preserve_streams_costs_and_owned_metadata() {
    for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
        for sampling in [
            NoiseSampling::Physical,
            NoiseSampling::Importance {
                proposal_scale: num_rational::BigRational::new(3.into(), 2.into()),
            },
        ] {
            let f = Files::new(true);
            let calls = Arc::new(AtomicUsize::new(0));
            let file = f.load_with_sampling(sampler, calls.clone(), sampling);
            let compiled = file.compile().unwrap();
            let prepared = file
                .prepare_sampling(&FilePreparationOptions::default())
                .unwrap();
            let run = TaskRunOptions {
                shots: 9,
                seed: 731,
                ..Default::default()
            };
            for mode in 0..3 {
                let baseline = match mode {
                    0 => file.run(&run),
                    1 => compiled.run(&run),
                    _ => prepared.run(&run),
                }
                .unwrap();
                for interval in [1, 4, 9, 10, u64::MAX] {
                    let before = calls.load(Ordering::Relaxed);
                    let mut events = Vec::new();
                    let mut callback = |event: &BatchProgress| {
                        events.push(event.clone());
                        Ok(())
                    };
                    let mut observer =
                        ProgressObserver::new(NonZeroU64::new(interval).unwrap(), &mut callback);
                    let report = match mode {
                        0 => file.run_with_progress(&run, &mut observer),
                        1 => compiled.run_with_progress(&run, &mut observer),
                        _ => prepared.run_with_progress(&run, &mut observer),
                    }
                    .unwrap();
                    same(report.raw_result(), baseline.raw_result());
                    assert_eq!(calls.load(Ordering::Relaxed) - before, 9);
                    let receipt = observer.receipt();
                    let exported = report.to_json().unwrap();
                    assert_eq!(receipt.failed, 0);
                    assert_eq!(receipt.last_completed, 9);
                    assert_eq!(exported["progress"]["attempted"], receipt.attempted);
                    assert_eq!(
                        exported["timings"]["progress_seconds"],
                        json!(receipt.seconds)
                    );
                    fingerprint(baseline.metadata(), report.metadata(), interval);
                    if mode == 2 {
                        assert!(exported["options"]["sampling_preparation"].is_object());
                        assert_eq!(
                            exported["compilation"]["query_preparation_included_in_this_call"],
                            false
                        );
                    }
                    let expected = (1..=9)
                        .filter(|k| k % interval == 0 || *k == 9)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        events.iter().map(|e| e.completed).collect::<Vec<_>>(),
                        expected
                    );
                    assert_eq!(
                        events.last().unwrap().efficiency.as_ref().unwrap(),
                        report.raw_result().efficiency.as_ref().unwrap()
                    );
                    drop(report);
                    assert_eq!(exported["completed"], 9);
                }
            }
        }
    }
}
#[test]
fn failed_progress_keeps_prefix_and_reset_precedes_validation_and_preparation() {
    let f = Files::new(true);
    let file = f.load(HistorySampler::ExactMarginal, Arc::new(AtomicUsize::new(0)));
    let mut callback = |_: &BatchProgress| {
        Err(TaskError {
            field: "observer".into(),
            message: "closed".into(),
        })
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
    let run = TaskRunOptions {
        shots: 7,
        ..Default::default()
    };
    let report = file.run_with_progress(&run, &mut observer).unwrap();
    let raw = report.raw_result();
    assert_eq!(
        (raw.status, raw.completed, raw.failed, raw.not_attempted),
        (RunStatus::Failed, 2, 0, 5)
    );
    assert_eq!(raw.failures[0].phase, RunPhase::Progress);
    assert_eq!(raw.failures[0].shot, None);
    assert_eq!(raw.sampled_records.as_ref().unwrap().len(), 2);
    assert_eq!(report.to_json().unwrap()["progress"]["failed"], 1);
    assert!(
        file.run_with_progress(&TaskRunOptions { shots: 0, ..run }, &mut observer)
            .is_err()
    );
    assert_eq!(observer.receipt().attempted, 0);
    let failed = file
        .run_with_progress(
            &TaskRunOptions {
                execution_limits: ExecutionLimits {
                    max_queries: 0,
                    ..Default::default()
                },
                ..run
            },
            &mut observer,
        )
        .unwrap();
    assert_eq!(failed.raw_result().completed, 0);
    assert_eq!(failed.raw_result().failures[0].phase, RunPhase::Preparation);
    assert_eq!(failed.to_json().unwrap()["progress"]["attempted"], 0);
    let prepared = file
        .prepare_sampling(&FilePreparationOptions::default())
        .unwrap();
    let reused = prepared
        .run_with_progress(
            &TaskRunOptions {
                shots: 2,
                execution_limits: ExecutionLimits {
                    max_queries: 0,
                    ..Default::default()
                },
                ..run
            },
            &mut observer,
        )
        .unwrap();
    assert_eq!(reused.raw_result().failures[0].phase, RunPhase::Sampling);
    assert_eq!(observer.receipt().attempted, 0);
}
#[test]
fn numeric_prefix_export_errors_remain_diagnostics_and_do_not_abort_sampling() {
    let f = Files::new(false);
    let file = f.load(HistorySampler::AffineSupport, Arc::new(AtomicUsize::new(0)));
    let mut diagnostics = Vec::new();
    let mut callback = |event: &BatchProgress| {
        diagnostics.push(export_progress(event, NumericOptions { max_work_bits: 0 }));
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
    let report = file
        .run_with_progress(
            &TaskRunOptions {
                shots: 7,
                ..Default::default()
            },
            &mut observer,
        )
        .unwrap();
    assert_eq!(report.raw_result().status, RunStatus::Complete);
    assert_eq!(observer.receipt().succeeded, 4);
    assert_eq!(observer.receipt().failed, 0);
    assert!(
        diagnostics
            .iter()
            .all(|v| v["efficiency"].is_null()
                && v["diagnostic_failure"]["phase"] == "numeric_export")
    );
    assert_eq!(report.to_json().unwrap()["completed"], 7);
}

#[test]
fn owned_receipts_survive_sources_and_observers_and_keep_range_diagnostics() {
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use rmagna::field::FieldArithmetic;
    use rmagna::statistics::EfficiencyStatus;
    let files = Files::new(false);
    let file = files.load(HistorySampler::ExactMarginal, Arc::new(AtomicUsize::new(0)));
    let mut events = Vec::new();
    let (report, receipt) = {
        let mut callback = |event: &BatchProgress| {
            events.push(event.clone());
            Ok(())
        };
        let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
        let report = file
            .run_with_progress(
                &TaskRunOptions {
                    shots: 5,
                    ..Default::default()
                },
                &mut observer,
            )
            .unwrap();
        (report, observer.receipt())
    };
    drop(file);
    drop(files);
    let owned = report.to_json().unwrap();
    assert_eq!(owned["progress"]["last_completed"], 5);
    assert_eq!(owned["progress"]["attempted"], receipt.attempted);
    let mut event = events.pop().unwrap();
    let efficiency = event.efficiency.as_mut().unwrap();
    efficiency.status = EfficiencyStatus::Available;
    efficiency.kappa = Some(
        FieldArithmetic::default()
            .from_rational(BigRational::from_integer(BigInt::from(10).pow(400)))
            .unwrap(),
    );
    let diagnostic = export_progress(&event, NumericOptions::default());
    assert_eq!(
        diagnostic["efficiency"]["status"],
        "diagnostic_range_exceeded"
    );
    assert!(diagnostic["efficiency"]["kappa"].is_null());
    assert!(diagnostic["efficiency"]["kappa_scientific"].is_object());
    assert!(diagnostic["diagnostic_failure"].is_null());
    assert_eq!(report.to_json().unwrap(), owned);
}
#[test]
fn file_deadlines_include_the_final_observer_in_all_session_modes() {
    use std::time::Duration;
    let files = Files::new(false);
    let file = files.load(HistorySampler::ExactMarginal, Arc::new(AtomicUsize::new(0)));
    let compiled = file.compile().unwrap();
    let prepared = file
        .prepare_sampling(&FilePreparationOptions::default())
        .unwrap();
    for mode in 0..3 {
        let run = TaskRunOptions {
            shots: 1,
            execution_limits: ExecutionLimits {
                max_duration: Some(Duration::from_millis(500)),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut callback = |_: &BatchProgress| {
            std::thread::sleep(Duration::from_millis(550));
            Ok(())
        };
        let mut observer = ProgressObserver::new(NonZeroU64::new(1).unwrap(), &mut callback);
        let report = match mode {
            0 => file.run_with_progress(&run, &mut observer),
            1 => compiled.run_with_progress(&run, &mut observer),
            _ => prepared.run_with_progress(&run, &mut observer),
        }
        .unwrap();
        let raw = report.raw_result();
        assert_eq!(
            (raw.status, raw.completed, raw.failed, raw.not_attempted),
            (RunStatus::Failed, 1, 0, 0)
        );
        assert_eq!(raw.failures[0].phase, RunPhase::Progress);
        assert!(raw.failures[0].message.contains("max_seconds"));
        let receipt = observer.receipt();
        assert_eq!((receipt.succeeded, receipt.failed), (1, 0));
        assert!(receipt.seconds >= 0.55);
        assert!(raw.timings.total_seconds >= receipt.seconds + raw.timings.sampling_seconds);
        assert_eq!(report.to_json().unwrap()["progress"]["succeeded"], 1);
    }
}
