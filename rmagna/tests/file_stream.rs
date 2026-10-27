use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions, FileTaskStream};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::CheckpointOptions;
use rmagna::task::{StreamRecordRetention, TaskRunOptions, TaskStreamOptions, TaskStreamStatus};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

const IMPLEMENTATION: &str = "file-stream-integration-v1";
const SOURCE: &str = "H 0 1\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[0] det[1] -> ctrl[0]\nCH ctrl[0] 2";
const TASK: &str = r#"{"outputs":[2],"ideal_state":"ideal.magna","recovery":"recovery.magna","sample_records":[0,1],"observables":[[1,0],[1,1]]}"#;
type Trace = Arc<Mutex<Vec<Vec<bool>>>>;
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-file-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let files = Self(path);
        files.put("physical.magna", SOURCE);
        files.put("task.json", TASK);
        files.put("ideal.magna", "I 0");
        files.put("recovery.magna", "I 2");
        files
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn load(
        &self,
        options: FileTaskOptions,
        sampling: NoiseSampling,
        version: &str,
        declaration: &str,
        behavior: u8,
    ) -> (FileTask, Trace) {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&trace);
        let mut registry = ControllerRegistry::default();
        registry
            .register("and", version, move |bits: &[bool]| {
                log.lock().unwrap().push(bits.to_vec());
                match behavior {
                    1 => Err("intentional file-stream callback failure".to_owned()),
                    2 => panic!("intentional file-stream callback unwind"),
                    _ => Ok(vec![bits[0] && bits[1]]),
                }
            })
            .unwrap();
        let task = FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
            &registry,
            sampling,
            options,
            &FileLoadOptions::default(),
            BTreeMap::from([("and".to_owned(), json!({"declared_code":declaration}))]),
        )
        .unwrap();
        (task, trace)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(retain: bool) -> TaskStreamOptions {
    TaskStreamOptions {
        run: TaskRunOptions {
            shots: 7,
            seed: 731,
            max_retained_record_values: if retain { 14 } else { 0 },
            ..Default::default()
        },
        records: if retain {
            StreamRecordRetention::Retain
        } else {
            StreamRecordRetention::Discard
        },
    }
}
fn saved(stream: &FileTaskStream<'_>) -> String {
    stream
        .checkpoint_json(CheckpointOptions::default())
        .unwrap()
}
fn scientific(text: &str) -> Value {
    let mut value: Value = serde_json::from_str(text).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("sampling_seconds");
    object.remove("total_seconds");
    let meter = object["meter"].as_object_mut().unwrap();
    meter.remove("elapsed");
    for name in ["compilation", "query", "decoder", "support"] {
        meter[name].as_object_mut().unwrap().remove("seconds");
    }
    value
}

#[test]
fn immutable_snapshots_resume_all_routes_proposals_and_record_policies_after_file_removal() {
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for strategy in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
                for importance in [false, true] {
                    for retain in [false, true] {
                        let files = Files::new();
                        let mut configuration = FileTaskOptions::default();
                        configuration.evaluation.runtime.history_sampler = sampler;
                        configuration.evaluation.runtime.strategy = strategy;
                        configuration.evaluation.runtime.compile.compiler_strategy = compiler;
                        let sampling = if importance {
                            NoiseSampling::Importance {
                                proposal_scale: "2".parse().unwrap(),
                            }
                        } else {
                            NoiseSampling::Physical
                        };
                        let (task, trace) = files.load(configuration, sampling, "v1", "one", 0);
                        let compiled = task.compile().unwrap();
                        let options = options(retain);
                        let mut full = compiled.start_stream(options, IMPLEMENTATION).unwrap();
                        full.advance(99).unwrap();
                        let expected = scientific(&saved(&full));
                        let expected_trace = std::mem::take(&mut *trace.lock().unwrap());
                        assert_eq!(expected_trace.len(), 7);
                        assert_eq!(full.snapshot().completed, 7);
                        assert_eq!(full.snapshot().sampled_records.is_some(), retain);
                        files.put("physical.magna", "invalid replacement");
                        files.put("task.json", "invalid replacement");
                        files.put("ideal.magna", "invalid replacement");
                        files.put("recovery.magna", "invalid replacement");
                        std::fs::remove_dir_all(&files.0).unwrap();
                        for cut in [0, 1, 3, 7] {
                            let mut prefix =
                                compiled.start_stream(options, IMPLEMENTATION).unwrap();
                            if cut != 0 {
                                prefix.advance(cut).unwrap();
                            }
                            let checkpoint = saved(&prefix);
                            let before = trace.lock().unwrap().len();
                            let replacement = task.clone().compile().unwrap();
                            let mut restored = replacement
                                .restore_stream_json(
                                    &checkpoint,
                                    options,
                                    IMPLEMENTATION,
                                    CheckpointOptions::default(),
                                )
                                .unwrap();
                            assert_eq!(trace.lock().unwrap().len(), before);
                            assert_eq!(restored.identity(), prefix.identity());
                            assert_eq!(scientific(&saved(&restored)), scientific(&checkpoint));
                            restored.advance(99).unwrap();
                            assert_eq!(restored.status(), TaskStreamStatus::DrawsComplete);
                            assert_eq!(restored.random_state(), full.random_state());
                            assert_eq!(scientific(&saved(&restored)), expected);
                            assert_eq!(*trace.lock().unwrap(), expected_trace);
                            trace.lock().unwrap().clear();
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn changed_loaded_content_and_declarations_reject_before_callbacks() {
    let files = Files::new();
    let (task, trace) = files.load(Default::default(), NoiseSampling::Physical, "v1", "one", 0);
    let compiled = task.compile().unwrap();
    let options = options(true);
    let mut stream = compiled.start_stream(options, IMPLEMENTATION).unwrap();
    stream.advance(1).unwrap();
    let checkpoint = saved(&stream);
    assert_eq!(trace.lock().unwrap().len(), 1);
    let original = stream.identity().clone();
    for (name, changed, unchanged, category) in [
        (
            "physical.magna",
            SOURCE.replace("T 0", "S 0"),
            SOURCE,
            "circuit",
        ),
        ("task.json", TASK.replace("[1,1]", "[0,1]"), TASK, "task"),
        ("ideal.magna", "X 0".to_owned(), "I 0", "ideal_state"),
        ("recovery.magna", "Z 2".to_owned(), "I 2", "recovery"),
    ] {
        files.put(name, &changed);
        let (changed, calls) =
            files.load(Default::default(), NoiseSampling::Physical, "v1", "one", 0);
        let identity = changed.stream_identity(options, IMPLEMENTATION).unwrap();
        assert_ne!(
            identity.content.to_json()[category],
            original.content.to_json()[category]
        );
        assert!(
            changed
                .compile()
                .unwrap()
                .restore_stream_json(
                    &checkpoint,
                    options,
                    IMPLEMENTATION,
                    CheckpointOptions::default()
                )
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
        files.put(name, unchanged);
    }
    for (version, declaration) in [("v2", "one"), ("v1", "two")] {
        let (changed, calls) = files.load(
            Default::default(),
            NoiseSampling::Physical,
            version,
            declaration,
            0,
        );
        assert_ne!(
            changed
                .stream_identity(options, IMPLEMENTATION)
                .unwrap()
                .content
                .controllers,
            original.content.controllers
        );
        assert!(
            changed
                .compile()
                .unwrap()
                .restore_stream_json(
                    &checkpoint,
                    options,
                    IMPLEMENTATION,
                    CheckpointOptions::default()
                )
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
    }
    for change in 0..4 {
        let mut configuration = FileTaskOptions::default();
        let mut sampling = NoiseSampling::Physical;
        match change {
            0 => configuration.numeric.max_work_bits += 1,
            1 => configuration.evaluation.runtime.history_sampler = HistorySampler::AffineSupport,
            2 => {
                configuration.evaluation.runtime.compile.compiler_strategy =
                    CompilerStrategy::SharedCommutingLayer
            }
            _ => {
                sampling = NoiseSampling::Importance {
                    proposal_scale: "2".parse().unwrap(),
                }
            }
        }
        let (changed, calls) = files.load(configuration, sampling, "v1", "one", 0);
        assert!(
            changed
                .compile()
                .unwrap()
                .restore_stream_json(
                    &checkpoint,
                    options,
                    IMPLEMENTATION,
                    CheckpointOptions::default()
                )
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
    }
    assert!(
        compiled
            .restore_stream_json(
                &checkpoint,
                options,
                "different",
                CheckpointOptions::default()
            )
            .is_err()
    );
    for change in 0..5 {
        let mut changed = options;
        match change {
            0 => changed.run.seed += 1,
            1 => {
                changed.run.shots += 1;
                changed.run.max_retained_record_values += 2;
            }
            2 => changed.run.execution_limits.max_queries -= 1,
            3 => changed.records = StreamRecordRetention::Discard,
            _ => changed.run.statistics.max_dimensions += 1,
        }
        assert!(
            compiled
                .restore_stream_json(
                    &checkpoint,
                    changed,
                    IMPLEMENTATION,
                    CheckpointOptions::default()
                )
                .is_err()
        );
    }
    assert_eq!(trace.lock().unwrap().len(), 1);
}

#[test]
fn discarded_u64_max_streams_do_not_change_batch_retention_or_fingerprints() {
    let files = Files::new();
    let (task, trace) = files.load(Default::default(), NoiseSampling::Physical, "v1", "one", 0);
    let normal = options(true);
    let batch_before = task.fingerprints(&normal.run).unwrap();
    let retained = task.stream_identity(normal, IMPLEMENTATION).unwrap();
    let mut discard = normal;
    discard.records = StreamRecordRetention::Discard;
    let discarded = task.stream_identity(discard, IMPLEMENTATION).unwrap();
    assert_ne!(retained.content.options, discarded.content.options);
    assert_ne!(retained.content.options, batch_before.options);
    for category in [
        "circuit",
        "task",
        "ideal_state",
        "recovery",
        "controllers",
        "sampling",
    ] {
        assert_eq!(
            retained.content.to_json()[category],
            batch_before.to_json()[category]
        );
        assert_eq!(
            discarded.content.to_json()[category],
            batch_before.to_json()[category]
        );
    }
    let options = TaskStreamOptions {
        run: TaskRunOptions {
            shots: u64::MAX,
            seed: u64::MAX,
            max_retained_record_values: 0,
            ..Default::default()
        },
        records: StreamRecordRetention::Discard,
    };
    assert!(task.fingerprints(&options.run).is_err());
    let compiled = task.compile().unwrap();
    assert!(compiled.run(&options.run).is_err());
    let mut stream = compiled.start_stream(options, IMPLEMENTATION).unwrap();
    let report = stream.advance(1).unwrap();
    assert_eq!(report.completed, 1);
    assert_eq!(report.not_attempted, u64::MAX - 1);
    assert!(report.sampled_records.is_none());
    let checkpoint = saved(&stream);
    let restored = compiled
        .restore_stream_json(
            &checkpoint,
            options,
            IMPLEMENTATION,
            CheckpointOptions::default(),
        )
        .unwrap();
    assert_eq!(scientific(&saved(&restored)), scientific(&checkpoint));
    assert_eq!(trace.lock().unwrap().len(), 1);
    let retaining = TaskStreamOptions {
        records: StreamRecordRetention::Retain,
        ..options
    };
    assert!(compiled.start_stream(retaining, IMPLEMENTATION).is_err());
    assert!(
        compiled
            .restore_stream_json(
                &checkpoint,
                retaining,
                IMPLEMENTATION,
                CheckpointOptions::default()
            )
            .is_err()
    );
    assert_eq!(trace.lock().unwrap().len(), 1);
    assert_eq!(task.fingerprints(&normal.run).unwrap(), batch_before);
}

#[test]
fn invalid_labels_checkpoints_and_identity_budgets_do_no_sampling_work() {
    let files = Files::new();
    let (task, trace) = files.load(Default::default(), NoiseSampling::Physical, "v1", "one", 0);
    let compiled = task.compile().unwrap();
    let options = options(true);
    let checkpoint = saved(&compiled.start_stream(options, IMPLEMENTATION).unwrap());
    for label in ["", " \t\n"] {
        assert!(compiled.start_stream(options, label).is_err());
        assert!(
            compiled
                .restore_stream_json(&checkpoint, options, label, CheckpointOptions::default())
                .is_err()
        );
    }
    let large = "x".repeat(FileTaskOptions::default().fingerprints.max_json_bytes + 1);
    assert!(compiled.start_stream(options, &large).is_err());
    for text in ["", "[]", "{}", &checkpoint[..checkpoint.len() - 1]] {
        assert!(
            compiled
                .restore_stream_json(text, options, IMPLEMENTATION, CheckpointOptions::default())
                .is_err()
        );
    }
    let tiny = CheckpointOptions { max_json_bytes: 1 };
    assert!(
        compiled
            .restore_stream_json(&checkpoint, options, IMPLEMENTATION, tiny)
            .is_err()
    );
    assert!(
        compiled
            .start_stream(options, IMPLEMENTATION)
            .unwrap()
            .checkpoint_json(tiny)
            .is_err()
    );
    // The smallest budget admitting immutable content lacks the larger run and
    // retention configuration. Find that endpoint without any execution.
    let registry = ControllerRegistry::default();
    files.put("physical.magna", "H 0\nM 0");
    files.put("task.json", r#"{"sample_records":[0]}"#);
    let load = |limit| {
        let mut configuration = FileTaskOptions::default();
        configuration.fingerprints.max_json_bytes = limit;
        FileTask::load(
            &files.0.join("physical.magna"),
            Some(&files.0.join("task.json")),
            &registry,
            NoiseSampling::Physical,
            configuration,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
    };
    let (mut low, mut high) = (0, 16384);
    assert!(load(high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        if load(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let bounded = load(low).unwrap();
    assert!(matches!(
        bounded.stream_identity(options, IMPLEMENTATION),
        Err(rmagna::file_api::FileApiError::Fingerprint(_))
    ));
    assert!(trace.lock().unwrap().is_empty());
}

#[test]
fn ordinary_failure_and_caught_unwind_restore_as_terminal() {
    for behavior in [1, 2] {
        let files = Files::new();
        let (task, trace) = files.load(
            Default::default(),
            NoiseSampling::Physical,
            "v1",
            "one",
            behavior,
        );
        let compiled = task.compile().unwrap();
        let options = options(true);
        let mut stream = compiled.start_stream(options, IMPLEMENTATION).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stream.advance(1)));
        if behavior == 1 {
            assert!(result.unwrap().is_ok());
        } else {
            assert!(result.is_err());
        }
        let report = stream.snapshot();
        assert_eq!(report.status, TaskStreamStatus::Failed);
        assert_eq!(
            (report.completed, report.failed, report.not_attempted),
            (0, 1, 6)
        );
        assert_eq!(
            report.failure.as_ref().unwrap().kind,
            if behavior == 1 { "TaskError" } else { "Unwind" }
        );
        let checkpoint = saved(&stream);
        let replacement = task.compile().unwrap();
        let mut restored = replacement
            .restore_stream_json(
                &checkpoint,
                options,
                IMPLEMENTATION,
                CheckpointOptions::default(),
            )
            .unwrap();
        assert_eq!(scientific(&saved(&restored)), scientific(&checkpoint));
        assert!(restored.advance(1).is_err());
        assert_eq!(trace.lock().unwrap().len(), 1);
    }
}

#[test]
fn restored_sampling_quota_is_cumulative_and_excludes_repreparation() {
    let files = Files::new();
    let (task, trace) = files.load(Default::default(), NoiseSampling::Physical, "v1", "one", 0);
    let compiled = task.compile().unwrap();
    let mut probe = compiled
        .start_stream(options(true), IMPLEMENTATION)
        .unwrap();
    let first = probe.advance(1).unwrap();
    let queries = first.execution_costs.query.attempted;
    assert!(queries > 0);
    trace.lock().unwrap().clear();
    let mut options = options(true);
    options.run.execution_limits.max_queries = queries;
    let mut prefix = compiled.start_stream(options, IMPLEMENTATION).unwrap();
    assert_eq!(prefix.advance(1).unwrap().completed, 1);
    let checkpoint = saved(&prefix);
    let replacement = task.compile().unwrap();
    let mut restored = replacement
        .restore_stream_json(
            &checkpoint,
            options,
            IMPLEMENTATION,
            CheckpointOptions::default(),
        )
        .unwrap();
    assert_eq!(scientific(&saved(&restored)), scientific(&checkpoint));
    let report = restored.advance(1).unwrap();
    assert_eq!(report.status, TaskStreamStatus::Failed);
    assert_eq!(
        (report.completed, report.failed, report.not_attempted),
        (1, 1, 5)
    );
    assert_eq!(report.execution_costs.query.attempted, queries);
    assert!(report.failure.unwrap().message.contains("query"));
    assert_eq!(trace.lock().unwrap().len(), 1);
    let terminal = saved(&restored);
    let mut restored = replacement
        .restore_stream_json(
            &terminal,
            options,
            IMPLEMENTATION,
            CheckpointOptions::default(),
        )
        .unwrap();
    assert!(restored.advance(1).is_err());
    assert_eq!(scientific(&saved(&restored)), scientific(&terminal));
}
