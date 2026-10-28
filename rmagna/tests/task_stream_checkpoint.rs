use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::field::FieldArithmetic;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::{CheckpointOptions, StatisticsOptions};
use rmagna::task::{
    ContentFingerprints, EvaluationOptions, StreamRecordRetention, TaskEvaluator, TaskOptions,
    TaskRunOptions, TaskStreamIdentity, TaskStreamOptions, TaskStreamStatus, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn rejected(e: &TaskEvaluator, text: &str, options: TaskStreamOptions) {
    assert!(
        e.restore_stream_json(
            text,
            options,
            &declared_identity(),
            CheckpointOptions::default()
        )
        .is_err(),
        "accepted malformed checkpoint: {text}"
    );
}
#[test]
fn strict_schema_and_inconsistent_state_are_rejected_before_controller_calls() {
    let (e, trace) = fixture(
        HistorySampler::AffineSupport,
        CompilerStrategy::ParityCopies,
        CausalStrategy::FixedTemplate,
        true,
    );
    let options = stream_options(true);
    let mut stream = e.start_stream(options).unwrap();
    stream.advance(3).unwrap();
    let text = checkpoint(&stream);
    let base: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(trace.lock().unwrap().len(), 3);
    for field in base.as_object().unwrap().keys() {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(field);
        rejected(&e, &v.to_string(), options);
    }
    for object in ["identity", "meter"] {
        for field in base[object].as_object().unwrap().keys() {
            let mut v = base.clone();
            v[object].as_object_mut().unwrap().remove(field);
            rejected(&e, &v.to_string(), options);
        }
        for replacement in [json!([]), json!({"$serde_json::private::Number":"1"})] {
            let mut v = base.clone();
            v[object] = replacement;
            rejected(&e, &v.to_string(), options);
        }
        let mut v = base.clone();
        v[object]["unknown"] = json!(true);
        rejected(&e, &v.to_string(), options);
    }
    let duplicate = format!("{{\"schema_version\":1,{}", &text[1..]);
    rejected(&e, &duplicate, options);
    for (field, value) in [
        ("unknown", json!(0)),
        ("schema_version", json!(2)),
        (
            "schema_version",
            json!({"$serde_json::private::Number":"1"}),
        ),
        ("attestation", json!("verified")),
        ("kind", json!("exact_statistics")),
        ("failed", json!("2")),
        ("committed", json!("7")),
        ("not_attempted", json!("0")),
        ("status", json!("draws_complete")),
        ("original_seed", json!("732")),
        ("has_error", json!(false)),
        ("record_policy", json!("discard")),
        ("records", json!(null)),
        ("records", json!([[true]])),
        ("records", json!([[1, 0], [1, 0], [1, 0]])),
        (
            "completed_network_compilations",
            json!(u64::MAX.to_string()),
        ),
        ("completed_history_groups", json!(u64::MAX.to_string())),
    ] {
        let mut v = base.clone();
        v[field] = value;
        rejected(&e, &v.to_string(), options);
    }
    for count in [
        "",
        "00",
        "01",
        "-0",
        "-1",
        "+1",
        " 1",
        "1.0",
        "1e0",
        "18446744073709551616",
    ] {
        let mut v = base.clone();
        v["random_state"] = json!(count);
        rejected(&e, &v.to_string(), options);
    }
    for value in [
        json!(1),
        json!({"$serde_json::private::Number":"1"}),
        json!(null),
    ] {
        let mut v = base.clone();
        v["random_state"] = value;
        rejected(&e, &v.to_string(), options);
    }
    for bits in [
        "7ff0000000000000",
        "fff0000000000000",
        "7ff8000000000000",
        "bff0000000000000",
        "0",
        "3FF0000000000000",
    ] {
        let mut v = base.clone();
        v["sampling_seconds"] = json!(bits);
        rejected(&e, &v.to_string(), options);
    }
    for cost in ["query", "compilation", "decoder", "support"] {
        for field in ["attempted", "succeeded", "failed", "seconds"] {
            let mut v = base.clone();
            v["meter"][cost].as_object_mut().unwrap().remove(field);
            rejected(&e, &v.to_string(), options);
        }
        let mut v = base.clone();
        v["meter"][cost] = json!([]);
        rejected(&e, &v.to_string(), options);
    }
    for value in [
        json!([]),
        json!({"seconds":"0"}),
        json!({"seconds":"0","nanoseconds":"1000000000"}),
        json!({"seconds":"0","nanoseconds":"0","extra":0}),
    ] {
        let mut v = base.clone();
        v["meter"]["elapsed"] = value;
        rejected(&e, &v.to_string(), options);
    }
    for value in [
        json!([]),
        json!({"phase":"progress","shot":"3","kind":"ProgressError","message":"invalid phase"}),
        json!({"phase":"sampling","shot":"3","kind":"TaskError","message":"inconsistent cursor"}),
        json!({"phase":"sampling","shot":"2","kind":"Unwind","message":"wrong shot"}),
    ] {
        let mut v = base.clone();
        v["failure"] = value;
        rejected(&e, &v.to_string(), options);
    }
    for field in ["statistics", "efficiency"] {
        for invalid in ["[]", "{\"$serde_json::private::Number\":\"1\"}"] {
            let mut v = base.clone();
            v[field] = json!(invalid);
            rejected(&e, &v.to_string(), options);
        }
    }
    let raw = base["statistics"].as_str().unwrap();
    let mut v = base.clone();
    v["statistics"] = json!(format!("{{\"schema_version\":1,{}", &raw[1..]));
    rejected(&e, &v.to_string(), options);
    let mut efficiency: Value = serde_json::from_str(base["efficiency"].as_str().unwrap()).unwrap();
    efficiency["statistics"]["sums"][0][0] = json!(["999", "1"]);
    let mut v = base.clone();
    v["efficiency"] = json!(efficiency.to_string());
    rejected(&e, &v.to_string(), options);
    assert_eq!(trace.lock().unwrap().len(), 3);
}

#[test]
fn caller_identity_and_actual_settings_are_independent_restore_gates() {
    let (e, trace) = fixture(
        HistorySampler::ExactMarginal,
        CompilerStrategy::ParityCopies,
        CausalStrategy::FixedTemplate,
        false,
    );
    let options = stream_options(true);
    let stream = e.start_stream(options).unwrap();
    let saved = checkpoint(&stream);
    for field in 0..8 {
        let mut id = declared_identity();
        match field {
            0 => id.content.circuit = "a".repeat(64),
            1 => id.content.task = "a".repeat(64),
            2 => id.content.ideal_state = "a".repeat(64),
            3 => id.content.recovery = "a".repeat(64),
            4 => id.content.controllers = "a".repeat(64),
            5 => id.content.options = "a".repeat(64),
            6 => id.content.sampling = "a".repeat(64),
            _ => id.implementation.push_str("-changed"),
        };
        assert!(
            e.restore_stream_json(&saved, options, &id, CheckpointOptions::default())
                .is_err()
        );
    }
    let mut changes = Vec::new();
    let mut o = options;
    o.run.seed += 1;
    changes.push(o);
    let mut o = options;
    o.run.shots += 1;
    o.run.max_retained_record_values = 16;
    changes.push(o);
    let mut o = options;
    o.run.max_retained_record_values -= 1;
    changes.push(o);
    let mut o = options;
    o.run.execution_limits.max_queries = 1;
    changes.push(o);
    let mut o = options;
    o.run.execution_limits.max_duration = Some(Duration::from_nanos(7));
    changes.push(o);
    let mut o = options;
    o.run.statistics.max_dimensions += 1;
    changes.push(o);
    let mut o = options;
    o.run.statistics.arithmetic = FieldArithmetic::new(1024);
    changes.push(o);
    let mut o = options;
    o.records = StreamRecordRetention::Discard;
    changes.push(o);
    for changed in changes {
        let error = e
            .restore_stream_json(
                &saved,
                changed,
                &declared_identity(),
                CheckpointOptions::default(),
            )
            .err()
            .unwrap();
        assert!(error.field.contains("actual_settings"));
    }
    for (sampler, compiler, strategy, importance) in [
        (
            HistorySampler::AffineSupport,
            CompilerStrategy::ParityCopies,
            CausalStrategy::FixedTemplate,
            false,
        ),
        (
            HistorySampler::ExactMarginal,
            CompilerStrategy::SharedCommutingLayer,
            CausalStrategy::FixedTemplate,
            false,
        ),
        (
            HistorySampler::ExactMarginal,
            CompilerStrategy::ParityCopies,
            CausalStrategy::ColdPrefix,
            false,
        ),
        (
            HistorySampler::ExactMarginal,
            CompilerStrategy::ParityCopies,
            CausalStrategy::FixedTemplate,
            true,
        ),
    ] {
        let (other, calls) = fixture(sampler, compiler, strategy, importance);
        rejected(&other, &saved, options);
        assert!(calls.lock().unwrap().is_empty());
    }
    assert!(trace.lock().unwrap().is_empty());
    let restored = e
        .restore_stream_json(
            &saved,
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    let mut changed = declared_identity();
    changed.implementation.push('x');
    assert!(
        restored
            .checkpoint_json(&changed, CheckpointOptions::default())
            .is_err()
    );
    let basic = make(
        "I 0",
        "{}",
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let basic_options = stream_options(false);
    let saved = checkpoint(&basic.start_stream(basic_options).unwrap());
    for choice in 0..7 {
        let mut cfg = EvaluationOptions::default();
        match choice {
            0 => cfg.runtime.max_history_queries += 1,
            1 => cfg.runtime.max_history_inputs += 1,
            2 => cfg.runtime.max_prefix_compiles += 1,
            3 => cfg.runtime.noise.max_entropy_words += 1,
            4 => cfg.runtime.noise.max_denominator_bits += 1,
            5 => cfg.runtime.sampling.max_entropy_words += 1,
            _ => cfg.runtime.sampling.max_terms += 1,
        };
        let other = make(
            "I 0",
            "{}",
            &ControllerRegistry::default(),
            cfg,
            &NoiseSampling::Physical,
        );
        rejected(&other, &saved, basic_options);
    }
}

#[test]
fn byte_count_arithmetic_and_retention_endpoints_keep_exact_values() {
    let (e, trace) = fixture(
        HistorySampler::ExactMarginal,
        CompilerStrategy::ParityCopies,
        CausalStrategy::FixedTemplate,
        false,
    );
    let options = stream_options(true);
    let stream = e.start_stream(options).unwrap();
    let saved = checkpoint(&stream);
    assert!(
        e.restore_stream_json(
            &saved,
            options,
            &declared_identity(),
            CheckpointOptions {
                max_json_bytes: saved.len()
            }
        )
        .is_ok()
    );
    for limit in [0, saved.len() - 1] {
        assert!(
            e.restore_stream_json(
                &saved,
                options,
                &declared_identity(),
                CheckpointOptions {
                    max_json_bytes: limit
                }
            )
            .is_err()
        );
    }
    assert!(
        stream
            .checkpoint_json(
                &declared_identity(),
                CheckpointOptions { max_json_bytes: 0 }
            )
            .is_err()
    );
    let cap = saved.len() + 128;
    assert!(
        stream
            .checkpoint_json(
                &declared_identity(),
                CheckpointOptions {
                    max_json_bytes: cap
                }
            )
            .unwrap()
            .len()
            <= cap
    );
    assert!(trace.lock().unwrap().is_empty());
    let basic = make(
        "I 0",
        "{}",
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let wide = TaskStreamOptions {
        run: TaskRunOptions {
            shots: u64::MAX,
            seed: u64::MAX,
            max_retained_record_values: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let initial = basic.start_stream(wide).unwrap();
    let text = checkpoint(&initial);
    let wire: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(wire["requested"], u64::MAX.to_string());
    assert_eq!(wire["random_state"], u64::MAX.to_string());
    let mut restored = basic
        .restore_stream_json(
            &text,
            wide,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    assert_eq!(restored.advance(1).unwrap().completed, 1);
    let mut bad = wire;
    bad["random_state"] = json!("0");
    rejected(&basic, &bad.to_string(), wide);
    let narrow = TaskStreamOptions {
        run: TaskRunOptions {
            shots: 2,
            statistics: StatisticsOptions {
                max_dimensions: 2,
                arithmetic: FieldArithmetic::new(1),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut stream = basic.start_stream(narrow).unwrap();
    stream.advance(1).unwrap();
    let text = checkpoint(&stream);
    assert!(
        basic
            .restore_stream_json(
                &text,
                narrow,
                &declared_identity(),
                CheckpointOptions::default()
            )
            .is_ok()
    );
    let mut wire: Value = serde_json::from_str(&text).unwrap();
    let mut raw: Value = serde_json::from_str(wire["statistics"].as_str().unwrap()).unwrap();
    raw["sums"][0][0] = json!(["2", "1"]);
    wire["statistics"] = json!(raw.to_string());
    rejected(&basic, &wire.to_string(), narrow);
}

#[test]
fn failed_controller_and_unwound_prefixes_restore_as_terminal_without_callbacks() {
    for unwind in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let log = Arc::clone(&calls);
        let mut registry = ControllerRegistry::default();
        registry
            .register("fail", "1", move |b: &[bool]| {
                if log.fetch_add(1, Ordering::SeqCst) == 1 {
                    if unwind {
                        panic!("checkpoint deliberate unwind");
                    }
                    return Err("checkpoint deliberate failure".to_owned());
                }
                Ok(b.to_vec())
            })
            .unwrap();
        let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[fail] det[0] -> ctrl[0]\nCX ctrl[0] 1";
        let task = r#"{"sample_records":[0]}"#;
        let e = make(
            source,
            task,
            &registry,
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
        );
        let options = TaskStreamOptions {
            run: TaskRunOptions {
                shots: 4,
                seed: 53,
                ..Default::default()
            },
            records: StreamRecordRetention::Retain,
        };
        let mut stream = e.start_stream(options).unwrap();
        stream.advance(1).unwrap();
        if unwind {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stream.advance(1)))
                    .is_err()
            );
        } else {
            assert_eq!(stream.advance(1).unwrap().status, TaskStreamStatus::Failed);
        }
        let saved = checkpoint(&stream);
        let other = make(
            source,
            task,
            &registry,
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
        );
        let mut restored = other
            .restore_stream_json(
                &saved,
                options,
                &declared_identity(),
                CheckpointOptions::default(),
            )
            .unwrap();
        let before = plain(&checkpoint(&restored));
        let report = restored.snapshot();
        assert_eq!(
            (report.completed, report.failed, report.not_attempted),
            (1, 1, 2)
        );
        assert_eq!(
            report.failure.unwrap().kind,
            if unwind { "Unwind" } else { "TaskError" }
        );
        assert_eq!(report.execution_costs.decoder.failed, 1);
        assert_eq!(report.sampled_records.unwrap().len(), 1);
        assert!(restored.advance(1).is_err());
        assert_eq!(plain(&checkpoint(&restored)), before);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-stream-checkpoint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("ideal.magna"), "I 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn declared_identity() -> TaskStreamIdentity {
    // Fixed caller declarations deliberately exercise automatic option binding.
    TaskStreamIdentity {
        content: ContentFingerprints {
            circuit: "1".repeat(64),
            task: "2".repeat(64),
            ideal_state: "3".repeat(64),
            recovery: "4".repeat(64),
            controllers: "5".repeat(64),
            options: "6".repeat(64),
            sampling: "7".repeat(64),
        },
        implementation: "checkpoint-fixture-v1".to_owned(),
    }
}
fn make(
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
    sampling: &NoiseSampling,
) -> TaskEvaluator {
    let files = Files::new();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let source = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
    TaskEvaluator::new_with_sampling(&source, &modules, registry, evaluation, sampling).unwrap()
}
type Trace = Arc<Mutex<Vec<Vec<bool>>>>;
fn fixture(
    sampler: HistorySampler,
    compiler: CompilerStrategy,
    strategy: CausalStrategy,
    importance: bool,
) -> (TaskEvaluator, Trace) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&trace);
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "checkpoint-v1", move |bits: &[bool]| {
            log.lock().unwrap().push(bits.to_vec());
            Ok(vec![bits[0] && bits[1]])
        })
        .unwrap();
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.history_sampler = sampler;
    evaluation.runtime.strategy = strategy;
    evaluation.runtime.compile.compiler_strategy = compiler;
    let sampling = if importance {
        NoiseSampling::Importance {
            proposal_scale: BigRational::from_integer(2.into()),
        }
    } else {
        NoiseSampling::Physical
    };
    (
        make(
            "H 0 1\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[0] det[1] -> ctrl[0]\nCH ctrl[0] 2",
            r#"{"outputs":[2],"ideal_state":"ideal.magna","sample_records":[0,1],"observables":[[1,0],[1,1]]}"#,
            &registry,
            evaluation,
            &sampling,
        ),
        trace,
    )
}
fn stream_options(retain: bool) -> TaskStreamOptions {
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

#[test]
fn native_transport_settings_gate_restore_before_any_controller_callback() {
    use rmagna::{
        instrument::QueryPreparation,
        phase::{NativePhaseOptions, NativeTransportMode},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("observe", "transport-v1", move |bits: &[bool]| {
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(bits.to_vec())
        })
        .unwrap();
    let source =
        "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[observe] det[0] -> ctrl[0]\nCH ctrl[0] 1";
    let task = r#"{"sample_records":[0]}"#;
    let mut native = NativePhaseOptions::default();
    native.transport.mode = NativeTransportMode::Prepared;
    let mut base = EvaluationOptions::default();
    base.runtime.compile.probability_preparation = QueryPreparation::Native(native);
    base.runtime.compile.density_preparation = QueryPreparation::Native(native);
    base.runtime.compile.moment_preparation = QueryPreparation::Native(native);
    let evaluator = make(source, task, &registry, base, &NoiseSampling::Physical);
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "preparation must not invoke a controller"
    );
    let options = stream_options(false);
    let initial = evaluator.start_stream(options).unwrap();
    let saved = checkpoint(&initial);
    for family in 0..3 {
        for setting in 0..4 {
            let mut changed = native;
            match setting {
                0 => changed.transport.mode = NativeTransportMode::Off,
                1 => changed.transport.limits.max_terms += 1,
                2 => changed.transport.limits.max_geometry_words += 1,
                _ => changed.transport.limits.max_construction_work += 1,
            }
            let mut config = base;
            match family {
                0 => {
                    config.runtime.compile.probability_preparation =
                        QueryPreparation::Native(changed)
                }
                1 => config.runtime.compile.density_preparation = QueryPreparation::Native(changed),
                _ => config.runtime.compile.moment_preparation = QueryPreparation::Native(changed),
            }
            let other = make(source, task, &registry, config, &NoiseSampling::Physical);
            let error = other
                .restore_stream_json(
                    &saved,
                    options,
                    &declared_identity(),
                    CheckpointOptions::default(),
                )
                .err()
                .unwrap();
            assert!(error.field.contains("actual_settings"), "{error:?}");
            assert_eq!(
                calls.load(Ordering::Relaxed),
                0,
                "incompatible restore must precede callbacks"
            );
        }
    }
    let mut restored = evaluator
        .restore_stream_json(
            &saved,
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    restored.advance(1).unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "callback fixture must be live"
    );
}
fn plain(value: &str) -> Value {
    let mut v: Value = serde_json::from_str(value).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("sampling_seconds");
    o.remove("total_seconds");
    let m = o["meter"].as_object_mut().unwrap();
    m.remove("elapsed");
    for name in ["compilation", "query", "decoder", "support"] {
        m[name].as_object_mut().unwrap().remove("seconds");
    }
    v
}
fn checkpoint(stream: &rmagna::task::TaskStream<'_>) -> String {
    stream
        .checkpoint_json(&declared_identity(), CheckpointOptions::default())
        .unwrap()
}

#[test]
fn every_cut_restores_exact_totals_rng_records_callbacks_and_cumulative_work() {
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                for importance in [false, true] {
                    for retain in [false, true] {
                        let (e, trace) = fixture(sampler, compiler, strategy, importance);
                        let options = stream_options(retain);
                        let mut full = e.start_stream(options).unwrap();
                        full.advance(7).unwrap();
                        let expected = plain(&checkpoint(&full));
                        let expected_trace = std::mem::take(&mut *trace.lock().unwrap());
                        assert_eq!(expected_trace.len(), 7);
                        for cut in [0, 1, 3, 7] {
                            let mut prefix = e.start_stream(options).unwrap();
                            if cut > 0 {
                                prefix.advance(cut).unwrap();
                            }
                            let saved = checkpoint(&prefix);
                            assert_eq!(trace.lock().unwrap().len(), cut as usize);
                            let (other, other_trace) =
                                fixture(sampler, compiler, strategy, importance);
                            let mut restored = other
                                .restore_stream_json(
                                    &saved,
                                    options,
                                    &declared_identity(),
                                    CheckpointOptions::default(),
                                )
                                .unwrap();
                            assert!(other_trace.lock().unwrap().is_empty());
                            assert_eq!(
                                restored.declared_checkpoint_identity(),
                                Some(&declared_identity())
                            );
                            assert_eq!(plain(&checkpoint(&restored)), plain(&saved));
                            restored.advance(99).unwrap();
                            assert_eq!(restored.snapshot().status, TaskStreamStatus::DrawsComplete);
                            assert_eq!(plain(&checkpoint(&restored)), expected);
                            let joined = trace
                                .lock()
                                .unwrap()
                                .iter()
                                .chain(other_trace.lock().unwrap().iter())
                                .cloned()
                                .collect::<Vec<_>>();
                            assert_eq!(joined, expected_trace);
                            trace.lock().unwrap().clear();
                        }
                    }
                }
            }
        }
    }
}

fn variant(case: usize) -> (TaskEvaluator, Trace, TaskStreamOptions) {
    if case >= 16 {
        let mut evaluation = EvaluationOptions::default();
        evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
        evaluation.runtime.compile.compiler_strategy = if case == 16 {
            CompilerStrategy::ParityCopies
        } else {
            CompilerStrategy::SharedCommutingLayer
        };
        let e = make(
            "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0],"sample_records":[0,1,2,3]}"#,
            &ControllerRegistry::default(),
            evaluation,
            &NoiseSampling::Physical,
        );
        return (
            e,
            Arc::new(Mutex::new(Vec::new())),
            TaskStreamOptions {
                run: TaskRunOptions {
                    shots: 64,
                    seed: 23,
                    max_retained_record_values: if case == 16 { 256 } else { 0 },
                    ..Default::default()
                },
                records: if case == 16 {
                    StreamRecordRetention::Retain
                } else {
                    StreamRecordRetention::Discard
                },
            },
        );
    }
    let (e, t) = fixture(
        if case & 1 == 0 {
            HistorySampler::ExactMarginal
        } else {
            HistorySampler::AffineSupport
        },
        if case & 2 == 0 {
            CompilerStrategy::ParityCopies
        } else {
            CompilerStrategy::SharedCommutingLayer
        },
        if case & 2 == 0 {
            CausalStrategy::FixedTemplate
        } else {
            CausalStrategy::ColdPrefix
        },
        case & 4 != 0,
    );
    (e, t, stream_options(case & 8 != 0))
}
#[test]
#[ignore = "subprocess fixture invoked explicitly by the cross-process recovery test"]
fn checkpoint_subprocess_helper() {
    let dir = PathBuf::from(std::env::var_os("RMAGNA_CHECKPOINT_DIRECTORY").unwrap());
    let mode = std::env::var("RMAGNA_CHECKPOINT_MODE").unwrap();
    let case: usize = std::env::var("RMAGNA_CHECKPOINT_CASE")
        .unwrap()
        .parse()
        .unwrap();
    let (e, trace, options) = variant(case);
    let mut stream = if mode == "resume" {
        e.restore_stream_json(
            &std::fs::read_to_string(dir.join("saved.json")).unwrap(),
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap()
    } else {
        e.start_stream(options).unwrap()
    };
    assert!(trace.lock().unwrap().is_empty());
    stream
        .advance(if mode == "produce" {
            3
        } else {
            options.run.shots
        })
        .unwrap();
    let data = checkpoint(&stream);
    if mode == "produce" {
        std::fs::write(dir.join("saved.json"), &data).unwrap();
    }
    std::fs::write(
        dir.join(format!("{mode}.json")),
        json!({"checkpoint":plain(&data),"trace":*trace.lock().unwrap()}).to_string(),
    )
    .unwrap();
}
fn process(dir: &Path, case: usize, mode: &str) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "checkpoint_subprocess_helper",
            "--nocapture",
        ])
        .env("RMAGNA_CHECKPOINT_DIRECTORY", dir)
        .env("RMAGNA_CHECKPOINT_MODE", mode)
        .env("RMAGNA_CHECKPOINT_CASE", case.to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn a_saved_prefix_continues_in_a_fresh_process_for_both_samplers_and_compilers() {
    for case in 0..18 {
        let files = Files::new();
        for mode in ["baseline", "produce", "resume"] {
            process(&files.0, case, mode);
        }
        let read = |mode| {
            serde_json::from_str::<Value>(
                &std::fs::read_to_string(files.0.join(format!("{mode}.json"))).unwrap(),
            )
            .unwrap()
        };
        let base = read("baseline");
        let first = read("produce");
        let last = read("resume");
        assert_eq!(last["checkpoint"], base["checkpoint"]);
        let joined = first["trace"]
            .as_array()
            .unwrap()
            .iter()
            .chain(last["trace"].as_array().unwrap())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(Value::Array(joined), base["trace"]);
        assert_eq!(
            first["trace"].as_array().unwrap().len(),
            if case < 16 { 3 } else { 0 }
        );
        assert_eq!(
            last["trace"].as_array().unwrap().len(),
            if case < 16 { 4 } else { 0 }
        );
        if case >= 16 {
            let efficiency: Value =
                serde_json::from_str(last["checkpoint"]["efficiency"].as_str().unwrap()).unwrap();
            assert!(
                efficiency["zero_weights"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    > 0
            );
            assert_eq!(last["checkpoint"]["committed"], "64");
        }
    }
}

#[test]
fn cumulative_quotas_and_terminal_failures_survive_restoration() {
    let registry = ControllerRegistry::default();
    let source = "H 0\nT 0\nH 0\nX_ERROR(1/7) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let task = r#"{"error_observables":[0],"sample_records":[0]}"#;
    let e = make(
        source,
        task,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let q = e
        .run_batch(&TaskRunOptions::default())
        .unwrap()
        .execution_costs
        .query
        .attempted;
    assert!(q > 0);
    let options = TaskStreamOptions {
        run: TaskRunOptions {
            shots: 5,
            seed: 31,
            execution_limits: ExecutionLimits {
                max_queries: 2 * q - 1,
                ..Default::default()
            },
            ..Default::default()
        },
        records: StreamRecordRetention::Retain,
    };
    let mut original = e.start_stream(options).unwrap();
    original.advance(1).unwrap();
    let saved = checkpoint(&original);
    let other = make(
        source,
        task,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let mut restored = other
        .restore_stream_json(
            &saved,
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    let a = original.advance(4).unwrap();
    let b = restored.advance(4).unwrap();
    assert_eq!((b.completed, b.failed, b.not_attempted), (1, 1, 3));
    assert_eq!(b.execution_costs.query.attempted, 2 * q - 1);
    assert_eq!(b.random_state, a.random_state);
    assert_eq!(plain(&checkpoint(&restored)), plain(&checkpoint(&original)));
    let failed = checkpoint(&restored);
    let mut terminal = e
        .restore_stream_json(
            &failed,
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    let before = plain(&checkpoint(&terminal));
    assert!(terminal.advance(1).is_err());
    assert_eq!(plain(&checkpoint(&terminal)), before);
    let expired = TaskStreamOptions {
        run: TaskRunOptions {
            shots: 3,
            seed: 71,
            execution_limits: ExecutionLimits {
                max_duration: Some(Duration::ZERO),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let initial = e.start_stream(expired).unwrap();
    let saved = checkpoint(&initial);
    let mut resumed = e
        .restore_stream_json(
            &saved,
            expired,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    let result = resumed.advance(1).unwrap();
    assert_eq!(
        (
            result.completed,
            result.failed,
            result.not_attempted,
            result.random_state
        ),
        (0, 1, 2, 71)
    );
    assert_eq!(result.execution_costs.query.attempted, 0);
}

#[test]
fn raw_small_budget_statistics_restore_even_when_derived_diagnostics_fail() {
    let e = make(
        "I 0",
        "{}",
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let options = TaskStreamOptions {
        run: TaskRunOptions {
            shots: 6,
            seed: 42,
            statistics: StatisticsOptions {
                arithmetic: FieldArithmetic::new(2),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut stream = e.start_stream(options).unwrap();
    let prefix = stream.advance(2).unwrap();
    assert!(prefix.statistics.is_err() || prefix.efficiency.is_err());
    let saved = checkpoint(&stream);
    let mut restored = e
        .restore_stream_json(
            &saved,
            options,
            &declared_identity(),
            CheckpointOptions::default(),
        )
        .unwrap();
    let final_state = restored.advance(4).unwrap();
    assert_eq!(
        (
            final_state.completed,
            final_state.failed,
            final_state.not_attempted
        ),
        (3, 1, 2)
    );
    assert!(restored.advance(1).is_err());
    let failed = checkpoint(&restored);
    assert!(
        e.restore_stream_json(
            &failed,
            options,
            &declared_identity(),
            CheckpointOptions::default()
        )
        .unwrap()
        .advance(1)
        .is_err()
    );
}
