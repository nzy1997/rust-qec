use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::{
    CheckpointOptions, ExactStatistics, HistoryEfficiency, StatisticsOptions,
};
use rmagna::task::{
    EvaluationOptions, RunFailure, RunPhase, RunTimings, StreamExportError, StreamRecordRetention,
    TaskOptions, TaskRunOptions, TaskRunStatistics, TaskStreamOptions, TaskStreamReport,
    TaskStreamStatus, TaskVariances, export_batch_metrics, export_stream_metrics, parse_task,
};
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

fn rational(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn literal(count: usize, requested: u64) -> TaskStreamReport {
    let mut raw = ExactStatistics::new(3, StatisticsOptions::default()).unwrap();
    let mut efficiency = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    let rows = [[1, 0, 1], [1, 1, -1], [0, 0, 0], [1, 0, 1]];
    for row in rows.iter().take(count) {
        let fields = row.map(|x| rational(x, 1));
        raw.push(&fields, &ExactField::one()).unwrap();
        efficiency
            .push(
                &fields[0],
                Some(&fields[1]),
                &ExactField::one(),
                &ExactField::one(),
            )
            .unwrap();
    }
    let statistics = raw.summarize().unwrap().map(|s| {
        let ratio = raw.paired_ratio(1, 0).unwrap();
        let variance = |i: usize| s.covariance_of_mean.as_ref().map(|m| m[i][i].clone());
        TaskRunStatistics {
            completed: s.count,
            accepted: s.means[0].clone(),
            error: Some(s.means[1].clone()),
            conditional_error: ratio.as_ref().map(|r| r.ratio.clone()),
            moments: vec![s.means[2].clone()],
            variances: TaskVariances {
                accepted: variance(0),
                error: variance(1),
                conditional_error: ratio.and_then(|r| r.variance),
                moments: vec![variance(2)],
            },
            mean_weight: s.mean_weight,
            effective_sample_size: s.effective_sample_size,
        }
    });
    TaskStreamReport {
        status: if count as u64 == requested {
            TaskStreamStatus::DrawsComplete
        } else {
            TaskStreamStatus::Running
        },
        requested,
        completed: count as u64,
        failed: 0,
        not_attempted: requested - count as u64,
        seed: 0,
        random_state: count as u64,
        statistics: Ok(statistics),
        efficiency: efficiency.summarize(),
        sampled_records: Some(
            rows.iter()
                .take(count)
                .map(|r| vec![r[0] != 0, r[1] != 0])
                .collect(),
        ),
        failure: None,
        execution_limits: Default::default(),
        execution_costs: Default::default(),
        timings: RunTimings {
            preparation_seconds: None,
            sampling_seconds: 0.0,
            total_seconds: 0.0,
        },
        completed_network_compilations: 0,
        completed_history_groups: 0,
    }
}
fn export(report: &TaskStreamReport) -> Value {
    export_stream_metrics(report, NumericOptions::default()).unwrap()
}
fn science(mut value: Value) -> Value {
    value.as_object_mut().unwrap().remove("timings");
    for key in ["compilation", "query", "decoder", "support"] {
        value["execution_costs"][key]
            .as_object_mut()
            .unwrap()
            .remove("seconds");
    }
    value
}

#[test]
fn finite_literal_means_paired_uncertainty_and_efficiency_are_provisional_until_complete() {
    let empty = literal(0, 4);
    let value = export(&empty);
    assert_eq!(value["status"], "running");
    assert_eq!(value["estimation_status"], "empty");
    assert!(value["estimates"].is_null());
    assert!(value["standard_errors"].is_null());
    assert_eq!(value["diagnostics"]["statistics"]["status"], "empty");
    let single = export(&literal(1, 4));
    assert_eq!(single["estimation_status"], "provisional");
    assert!(single["standard_errors"]["acceptance"].is_null());
    assert_eq!(single["estimates"]["acceptance"], 1.0);
    let mut report = literal(4, 9);
    let before = report.clone();
    let value = export(&report);
    assert_eq!(report, before);
    assert_eq!(value["status"], "running");
    assert_eq!(value["draw_status"], "running");
    assert_eq!(value["estimation_status"], "provisional");
    assert_eq!(
        value["estimates"],
        json!({"acceptance":0.75,"error":0.25,"conditional_error":1.0/3.0,"moments":[0.25]})
    );
    let s = report.statistics.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(s.variances.accepted, Some(rational(1, 16)));
    assert_eq!(s.variances.error, Some(rational(1, 16)));
    assert_eq!(s.variances.moments, vec![Some(rational(11, 48))]);
    assert_eq!(s.variances.conditional_error, Some(rational(8, 81)));
    assert_eq!(value["standard_errors"]["acceptance"], 0.25);
    assert_eq!(value["standard_errors"]["error"], 0.25);
    assert!(
        (value["standard_errors"]["conditional_error"]
            .as_f64()
            .unwrap()
            - 8.0_f64.sqrt() / 9.0)
            .abs()
            < 1e-15
    );
    assert!(
        (value["standard_errors"]["moments"][0].as_f64().unwrap() - (11.0_f64 / 48.0).sqrt()).abs()
            < 1e-15
    );
    assert_eq!(value["weights"]["ess"], 4.0);
    assert_eq!(value["weights"]["mean"], 1.0);
    assert_eq!(value["efficiency"]["kappa"], 1.0);
    assert!(
        (value["efficiency"]["variance_coefficient"]
            .as_f64()
            .unwrap()
            - 8.0 / 27.0)
            .abs()
            < 1e-15
    );
    report.status = TaskStreamStatus::DrawsComplete;
    report.requested = 4;
    report.not_attempted = 0;
    let complete = export(&report);
    assert_eq!(complete["status"], "complete");
    assert_eq!(complete["draw_status"], "draws_complete");
    assert_eq!(complete["estimation_status"], "final");
    assert_eq!(complete["estimates"], value["estimates"]);
    assert_eq!(complete["standard_errors"], value["standard_errors"]);
    report.status = TaskStreamStatus::Running;
    report.requested = u64::MAX;
    report.not_attempted = u64::MAX - 4;
    report.seed = u64::MAX;
    report.random_state = u64::MAX;
    let boundary = export(&report);
    assert_eq!(boundary["requested"].as_u64(), Some(u64::MAX));
    assert_eq!(boundary["seed"].as_u64(), Some(u64::MAX));
    assert_eq!(boundary["random_state"].as_u64(), Some(u64::MAX));
    assert_eq!(
        serde_json::from_str::<Value>(&boundary.to_string()).unwrap(),
        boundary
    );
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-stream-export-{}-{}",
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
type Trace = Arc<Mutex<Vec<Vec<bool>>>>;
fn make(
    source: &str,
    task: &str,
    evaluation: EvaluationOptions,
    sampling: NoiseSampling,
    behavior: u8,
) -> (FileTask, Trace) {
    let files = Files::new();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::clone(&trace);
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "export-v1", move |b: &[bool]| {
            calls.lock().unwrap().push(b.to_vec());
            match behavior {
                1 => Err("deliberate export callback failure".to_owned()),
                2 => panic!("deliberate export callback unwind"),
                _ => Ok(vec![b[0] && b[1]]),
            }
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let file = FileTask::from_loaded(
        circuit,
        modules,
        &registry,
        sampling,
        FileTaskOptions {
            evaluation,
            ..Default::default()
        },
        BTreeMap::new(),
    )
    .unwrap();
    (file, trace)
}
const SOURCE: &str = "H 0 1\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[0] det[1] -> ctrl[0]\nCH ctrl[0] 2";
const TASK: &str = r#"{"outputs":[2],"ideal_state":"ideal.magna","sample_records":[0,1],"observables":[[1,0],[1,1]]}"#;
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
fn restored_real_stream_exports_match_uninterrupted_science_and_complete_batch_metrics() {
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for strategy in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
                for importance in [false, true] {
                    for retain in [false, true] {
                        let mut evaluation = EvaluationOptions::default();
                        evaluation.runtime.history_sampler = sampler;
                        evaluation.runtime.strategy = strategy;
                        evaluation.runtime.compile.compiler_strategy = compiler;
                        let sampling = if importance {
                            NoiseSampling::Importance {
                                proposal_scale: "2".parse().unwrap(),
                            }
                        } else {
                            NoiseSampling::Physical
                        };
                        let (file, trace) = make(SOURCE, TASK, evaluation, sampling, 0);
                        let compiled = file.compile().unwrap();
                        let options = stream_options(retain);
                        let mut full = compiled.start_stream(options, "export-v1").unwrap();
                        let report = full.advance(7).unwrap();
                        let expected = export(&report);
                        let expected_trace = std::mem::take(&mut *trace.lock().unwrap());
                        assert_eq!(expected_trace.len(), 7);
                        let mut batch_options = options.run;
                        batch_options.max_retained_record_values = 14;
                        let batch = compiled.run(&batch_options).unwrap();
                        let batch =
                            export_batch_metrics(batch.raw_result(), NumericOptions::default())
                                .unwrap();
                        for key in [
                            "estimates",
                            "estimates_scientific",
                            "standard_errors",
                            "standard_errors_scientific",
                            "efficiency",
                            "weights",
                            "normalization",
                            "uncertainty",
                            "numerics",
                        ] {
                            assert_eq!(expected[key], batch[key], "{key}");
                        }
                        trace.lock().unwrap().clear();
                        for cut in [0, 1, 3, 7] {
                            let mut prefix = compiled.start_stream(options, "export-v1").unwrap();
                            if cut > 0 {
                                prefix.advance(cut).unwrap();
                            }
                            let before_calls = trace.lock().unwrap().len();
                            let observation = prefix.snapshot();
                            let exact = observation.clone();
                            let _ = export(&observation);
                            assert_eq!(observation, exact);
                            assert_eq!(trace.lock().unwrap().len(), before_calls);
                            let saved = prefix
                                .checkpoint_json(CheckpointOptions::default())
                                .unwrap();
                            let replacement = file.compile().unwrap();
                            let mut restored = replacement
                                .restore_stream_json(
                                    &saved,
                                    options,
                                    "export-v1",
                                    CheckpointOptions::default(),
                                )
                                .unwrap();
                            assert_eq!(
                                science(export(&restored.snapshot())),
                                science(export(&observation))
                            );
                            let final_report = restored.advance(99).unwrap();
                            let value = export(&final_report);
                            assert_eq!(science(value), science(expected.clone()));
                            assert_eq!(*trace.lock().unwrap(), expected_trace);
                            assert_eq!(final_report.random_state, report.random_state);
                            trace.lock().unwrap().clear();
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn genuine_zero_weights_and_diagnostic_failures_never_fabricate_complete_estimates() {
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
    let (file, _) = make(
        "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0,1,2,3]}"#,
        evaluation,
        NoiseSampling::Physical,
        0,
    );
    let compiled = file.compile().unwrap();
    let mut stream = compiled
        .start_stream(
            TaskStreamOptions {
                run: TaskRunOptions {
                    shots: 64,
                    seed: 23,
                    ..Default::default()
                },
                ..Default::default()
            },
            "export-v1",
        )
        .unwrap();
    let report = stream.advance(64).unwrap();
    let value = export(&report);
    let efficiency = report.efficiency.as_ref().unwrap();
    assert!(efficiency.zero_weights > 0);
    assert_eq!(
        value["efficiency"]["zero_weights"].as_u64(),
        Some(efficiency.zero_weights)
    );
    assert_eq!(value["status"], "complete");
    let (file, _) = make("I 0", "{}", Default::default(), NoiseSampling::Physical, 0);
    let compiled = file.compile().unwrap();
    let mut stream = compiled
        .start_stream(
            TaskStreamOptions {
                run: TaskRunOptions {
                    shots: 2,
                    statistics: StatisticsOptions {
                        arithmetic: FieldArithmetic::new(2),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            "small-v1",
        )
        .unwrap();
    let report = stream.advance(2).unwrap();
    assert_eq!(report.status, TaskStreamStatus::DrawsComplete);
    assert!(report.statistics.is_err() || report.efficiency.is_err());
    let before = report.clone();
    let value = export(&report);
    assert_eq!(report, before);
    assert_eq!(value["status"], "diagnostic_failed");
    assert_eq!(value["draw_status"], "draws_complete");
    assert_ne!(value["estimation_status"], "final");
    if report.statistics.is_err() {
        assert!(value["estimates"].is_null());
        assert_eq!(value["diagnostics"]["statistics"]["status"], "failed");
    }
    if report.efficiency.is_err() {
        assert!(value["efficiency"].is_null());
        assert_eq!(value["diagnostics"]["efficiency"]["status"], "failed");
    }
    // Separability fixture: valid statistical values remain provisional when
    // the efficiency computation is explicitly unavailable.
    let mut separated = literal(4, 4);
    separated.efficiency = Err(ExactStatistics::new(
        2,
        StatisticsOptions {
            max_dimensions: 1,
            ..Default::default()
        },
    )
    .err()
    .unwrap());
    let value = export(&separated);
    assert_eq!(value["status"], "diagnostic_failed");
    assert_eq!(value["estimation_status"], "provisional");
    assert_eq!(value["estimates"]["acceptance"], 0.75);
    assert_eq!(value["weights"]["ess"], 4.0);
}

#[test]
fn real_failed_and_unwound_prefixes_keep_the_actual_attempt_and_rng() {
    for behavior in [1, 2] {
        let (file, trace) = make(
            SOURCE,
            TASK,
            Default::default(),
            NoiseSampling::Physical,
            behavior,
        );
        let compiled = file.compile().unwrap();
        let mut stream = compiled
            .start_stream(stream_options(true), "failed-v1")
            .unwrap();
        let attempted =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stream.advance(1)));
        if behavior == 1 {
            assert!(attempted.unwrap().is_ok());
        } else {
            assert!(attempted.is_err());
        }
        let report = stream.snapshot();
        let before = report.clone();
        let value = export(&report);
        assert_eq!(report, before);
        assert_eq!(value["status"], "failed");
        assert_eq!(value["draw_status"], "failed");
        assert_eq!(value["completed"], 0);
        assert_eq!(value["failed"], 1);
        assert_eq!(value["not_attempted"], 6);
        assert_eq!(value["random_state"].as_u64(), Some(report.random_state));
        assert_eq!(
            value["failure"]["type"],
            if behavior == 1 { "TaskError" } else { "Unwind" }
        );
        assert!(value["estimates"].is_null());
        assert_eq!(trace.lock().unwrap().len(), 1);
    }
    let mut prefix = literal(4, 9);
    prefix.status = TaskStreamStatus::Failed;
    prefix.failed = 1;
    prefix.not_attempted = 4;
    prefix.failure = Some(RunFailure {
        phase: RunPhase::Sampling,
        shot: Some(4),
        kind: "TaskError",
        message: "bounded literal failure".to_owned(),
    });
    let value = export(&prefix);
    assert_eq!(value["estimation_status"], "provisional");
    assert_eq!(value["estimates"]["acceptance"], 0.75);
}

#[test]
fn scientific_underflow_overflow_and_numeric_budget_errors_preserve_the_typed_report() {
    let f = FieldArithmetic::default();
    let tiny = f
        .from_rational(BigRational::new(1.into(), BigInt::from(10).pow(400)))
        .unwrap();
    let huge = f
        .from_rational(BigRational::from_integer(BigInt::from(10).pow(400)))
        .unwrap();
    let mut report = literal(4, 4);
    let s = report.statistics.as_mut().unwrap().as_mut().unwrap();
    s.accepted = tiny.clone();
    s.error = Some(tiny.clone());
    s.conditional_error = Some(ExactField::one());
    s.variances.accepted = Some(f.multiply(&tiny, &tiny).unwrap());
    s.variances.error = s.variances.accepted.clone();
    s.variances.conditional_error = Some(ExactField::zero());
    s.moments = vec![tiny.negated(), huge.clone()];
    s.variances.moments = vec![
        Some(f.multiply(&tiny, &tiny).unwrap()),
        Some(f.multiply(&huge, &huge).unwrap()),
    ];
    let before = report.clone();
    let value = export(&report);
    assert_eq!(report, before);
    assert!(value["estimates"]["acceptance"].is_null());
    assert!(value["estimates"]["moments"][1].is_null());
    assert_eq!(
        value["estimates_scientific"]["acceptance"]["exponent2"],
        -1328
    );
    assert_eq!(
        value["estimates_scientific"]["moments"][1]["exponent2"],
        1329
    );
    assert!(
        value["numerics"]["underflow_fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "estimates.acceptance")
    );
    assert!(
        value["numerics"]["overflow_fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "estimates.moments[1]")
    );
    assert_eq!(value["standard_errors"]["conditional_error"], 0.0);
    assert!(matches!(
        export_stream_metrics(&report, NumericOptions { max_work_bits: 0 }),
        Err(StreamExportError::Numeric { .. })
    ));
    assert_eq!(report, before);
    report
        .statistics
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .variances
        .accepted = Some(rational(-1, 1));
    let before = report.clone();
    assert!(
        matches!(export_stream_metrics(&report,NumericOptions::default()),Err(StreamExportError::Numeric {field,..}) if field=="standard_errors.acceptance")
    );
    assert_eq!(report, before);
}

#[test]
fn malformed_public_reports_are_rejected_with_context_and_without_mutation() {
    type Change = fn(&mut TaskStreamReport);
    let changes: Vec<(&str, Change)> = vec![
        ("zero request", |r| r.requested = 0),
        ("overflow", |r| r.completed = u64::MAX),
        ("cursor", |r| r.not_attempted = 0),
        ("complete", |r| r.status = TaskStreamStatus::DrawsComplete),
        ("missing failure", |r| r.status = TaskStreamStatus::Failed),
        ("multiple failures", |r| {
            r.failed = 2;
            r.not_attempted -= 2;
        }),
        ("missing statistics", |r| r.statistics = Ok(None)),
        ("stat count", |r| {
            r.statistics.as_mut().unwrap().as_mut().unwrap().completed += 1
        }),
        ("moment shape", |r| {
            r.statistics
                .as_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .variances
                .moments
                .clear();
        }),
        ("missing ratio", |r| {
            r.statistics
                .as_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .conditional_error = None
        }),
        ("missing variance", |r| {
            r.statistics
                .as_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .variances
                .accepted = None
        }),
        ("efficiency count", |r| {
            r.efficiency.as_mut().unwrap().completed += 1
        }),
        ("zero count", |r| {
            r.efficiency.as_mut().unwrap().zero_weights = 5
        }),
        ("zero ess", |r| {
            r.efficiency.as_mut().unwrap().effective_sample_size = ExactField::zero()
        }),
        ("missing coefficient", |r| {
            r.efficiency.as_mut().unwrap().kappa = None
        }),
        ("missing rho", |r| r.efficiency.as_mut().unwrap().rho = None),
        ("weight mismatch", |r| {
            r.efficiency.as_mut().unwrap().mean_weight = Some(rational(2, 1))
        }),
        ("record count", |r| {
            r.sampled_records.as_mut().unwrap().pop();
        }),
        ("record shape", |r| {
            r.sampled_records.as_mut().unwrap()[1].push(true)
        }),
        ("nan timing", |r| r.timings.total_seconds = f64::NAN),
        ("negative timing", |r| r.timings.sampling_seconds = -1.0),
        ("small total", |r| r.timings.sampling_seconds = 1.0),
        ("preparation", |r| r.timings.preparation_seconds = Some(0.0)),
        ("cost balance", |r| r.execution_costs.query.succeeded = 1),
        ("cost time", |r| {
            r.execution_costs.decoder.seconds = f64::INFINITY
        }),
        ("quota", |r| {
            r.execution_costs.query.attempted = 1;
            r.execution_costs.query.succeeded = 1;
            r.execution_limits.max_queries = 0;
        }),
    ];
    for (name, change) in changes {
        let mut report = literal(4, 9);
        change(&mut report);
        let before = format!("{report:?}");
        let error = export_stream_metrics(&report, NumericOptions::default()).unwrap_err();
        assert!(
            matches!(error, StreamExportError::InvalidState(_)),
            "{name}: {error}"
        );
        assert!(
            error.to_string().contains("stream report"),
            "{name}: {error}"
        );
        assert_eq!(format!("{report:?}"), before);
    }
    for field in 0..4 {
        let mut report = literal(0, 9);
        match field {
            0 => report.completed_network_compilations = 1,
            1 => report.completed_history_groups = 1,
            2 => report.random_state = 1,
            _ => report.timings.sampling_seconds = 1.0,
        }
        assert!(export_stream_metrics(&report, NumericOptions::default()).is_err());
    }
    for field in 0..4 {
        let mut report = literal(4, 9);
        report.status = TaskStreamStatus::Failed;
        report.failed = 1;
        report.not_attempted = 4;
        report.failure = Some(RunFailure {
            phase: RunPhase::Sampling,
            shot: Some(4),
            kind: "TaskError",
            message: "failure".to_owned(),
        });
        let f = report.failure.as_mut().unwrap();
        match field {
            0 => f.phase = RunPhase::Preparation,
            1 => f.shot = None,
            2 => f.shot = Some(3),
            _ => f.kind = "ProgressError",
        }
        assert!(export_stream_metrics(&report, NumericOptions::default()).is_err());
    }
}

#[test]
fn undefined_error_metrics_and_running_diagnostics_remain_distinct_from_draw_failures() {
    let (file, _) = make(
        "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        Default::default(),
        NoiseSampling::Physical,
        0,
    );
    let compiled = file.compile().unwrap();
    let options = TaskStreamOptions {
        run: TaskRunOptions {
            shots: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let report = compiled
        .start_stream(options, "zero-acceptance-v1")
        .unwrap()
        .advance(2)
        .unwrap();
    let value = export(&report);
    assert_eq!(value["status"], "complete");
    assert_eq!(value["estimates"]["acceptance"], 0.0);
    assert_eq!(value["estimates"]["error"], 0.0);
    assert!(value["estimates"]["conditional_error"].is_null());
    assert!(value["standard_errors"]["conditional_error"].is_null());
    assert_eq!(value["efficiency"]["status"], "zero_acceptance");
    assert!(
        !value["numerics"]["underflow_fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "estimates.conditional_error")
    );

    let (file, _) = make("I 0", "{}", Default::default(), NoiseSampling::Physical, 0);
    let compiled = file.compile().unwrap();
    let report = compiled
        .start_stream(options, "no-error-v1")
        .unwrap()
        .advance(2)
        .unwrap();
    let value = export(&report);
    assert_eq!(value["status"], "complete");
    assert!(value["estimates"]["error"].is_null());
    assert!(value["estimates"]["conditional_error"].is_null());
    assert_eq!(value["efficiency"]["status"], "no_error_metric");

    let mut small = options;
    small.run.shots = 6;
    small.run.statistics.arithmetic = FieldArithmetic::new(2);
    let mut stream = compiled
        .start_stream(small, "running-diagnostic-v1")
        .unwrap();
    let prefix = stream.advance(2).unwrap();
    let value = export(&prefix);
    assert_eq!(value["draw_status"], "running");
    assert_eq!(value["status"], "diagnostic_failed");
    assert_eq!(value["completed"], 2);
    let failed = stream.advance(4).unwrap();
    let value = export(&failed);
    assert_eq!(value["status"], "failed");
    assert_eq!(value["draw_status"], "failed");
    assert_eq!(
        (failed.completed, failed.failed, failed.not_attempted),
        (3, 1, 2)
    );
    assert_ne!(value["estimation_status"], "final");
    assert!(
        value["diagnostics"]["statistics"]["status"] == "failed"
            || value["diagnostics"]["efficiency"]["status"] == "failed"
    );
}

#[test]
fn committed_resources_require_successful_measured_work_for_every_draw_status() {
    for status in [
        TaskStreamStatus::Running,
        TaskStreamStatus::DrawsComplete,
        TaskStreamStatus::Failed,
    ] {
        let mut report = literal(4, 9);
        report.status = status;
        match status {
            TaskStreamStatus::DrawsComplete => {
                report.requested = 4;
                report.not_attempted = 0;
            }
            TaskStreamStatus::Failed => {
                report.failed = 1;
                report.not_attempted = 4;
                report.failure = Some(RunFailure {
                    phase: RunPhase::Sampling,
                    shot: Some(4),
                    kind: "TaskError",
                    message: "resource accounting failure fixture".to_owned(),
                });
            }
            TaskStreamStatus::Running => {}
        }
        report.completed_network_compilations = 2;
        report.completed_history_groups = 5;
        report.execution_costs.compilation.attempted = 2;
        report.execution_costs.compilation.succeeded = 2;
        report.execution_costs.query.attempted = 2;
        report.execution_costs.query.succeeded = 2;
        report.execution_costs.support.attempted = 3;
        report.execution_costs.support.succeeded = 3;
        let before = report.clone();
        let value = export(&report);
        assert_eq!(report, before);
        assert_eq!(value["completed_resources"]["network_compilations"], 2);
        assert_eq!(value["completed_resources"]["history_groups"], 5);
        assert_eq!(value["estimates"]["conditional_error"], 1.0 / 3.0);
        assert_eq!(
            value["draw_status"],
            match status {
                TaskStreamStatus::Running => "running",
                TaskStreamStatus::DrawsComplete => "draws_complete",
                TaskStreamStatus::Failed => "failed",
            }
        );
        for (network, field) in [(true, "network compilations"), (false, "history groups")] {
            let mut invalid = report.clone();
            if network {
                invalid.completed_network_compilations = 3;
                invalid.execution_costs.compilation.attempted = 3;
                invalid.execution_costs.compilation.failed = 1;
            } else {
                invalid.completed_history_groups = 6;
                invalid.execution_costs.query.attempted = 3;
                invalid.execution_costs.query.failed = 1;
            }
            let before = invalid.clone();
            let error = export_stream_metrics(&invalid, NumericOptions::default()).unwrap_err();
            assert!(matches!(error, StreamExportError::InvalidState(_)));
            assert!(error.to_string().contains(field), "{error}");
            assert_eq!(invalid, before);
        }
        // Work from uncommitted attempts may legitimately exceed committed resources.
        report.completed_network_compilations = 1;
        report.completed_history_groups = 2;
        assert_eq!(export(&report)["estimates"], value["estimates"]);
    }
    // The combined success bound need not fit u64. Neither summation nor usize
    // conversion may wrap or truncate to an artificially smaller bound.
    let mut boundary = literal(4, 4);
    boundary.completed_network_compilations = usize::MAX;
    boundary.completed_history_groups = usize::MAX;
    boundary.execution_limits.max_queries = u64::MAX;
    boundary.execution_costs.compilation.attempted = u64::MAX;
    boundary.execution_costs.compilation.succeeded = u64::MAX;
    boundary.execution_costs.query.attempted = u64::MAX;
    boundary.execution_costs.query.succeeded = u64::MAX;
    boundary.execution_costs.support.attempted = 1;
    boundary.execution_costs.support.succeeded = 1;
    let before = boundary.clone();
    let value = export(&boundary);
    assert_eq!(value["status"], "complete");
    assert_eq!(
        value["completed_resources"]["history_groups"],
        json!(usize::MAX)
    );
    assert_eq!(boundary, before);
}

#[test]
fn operation_seconds_require_attempts_across_draw_and_diagnostic_states() {
    use rmagna::costs::{
        CostError, ExecutionMeter, ExecutionMeterCheckpoint, OperationCost, OperationKind,
    };
    fn cost(report: &mut TaskStreamReport, kind: OperationKind) -> &mut OperationCost {
        match kind {
            OperationKind::Compilation => &mut report.execution_costs.compilation,
            OperationKind::Query => &mut report.execution_costs.query,
            OperationKind::Decoder => &mut report.execution_costs.decoder,
            OperationKind::Support => &mut report.execution_costs.support,
        }
    }
    let operations = [
        (OperationKind::Compilation, "compilation"),
        (OperationKind::Query, "query"),
        (OperationKind::Decoder, "decoder"),
        (OperationKind::Support, "support"),
    ];
    let unavailable = ExactStatistics::new(
        2,
        StatisticsOptions {
            max_dimensions: 1,
            ..Default::default()
        },
    )
    .err()
    .unwrap();
    for status in [
        TaskStreamStatus::Running,
        TaskStreamStatus::DrawsComplete,
        TaskStreamStatus::Failed,
    ] {
        for statistics_valid in [false, true] {
            for efficiency_valid in [false, true] {
                let mut report = literal(4, 9);
                report.status = status;
                report.timings.sampling_seconds = 1.0;
                report.timings.total_seconds = 1.0;
                match status {
                    TaskStreamStatus::DrawsComplete => {
                        report.requested = 4;
                        report.not_attempted = 0;
                    }
                    TaskStreamStatus::Failed => {
                        report.failed = 1;
                        report.not_attempted = 4;
                        report.failure = Some(RunFailure {
                            phase: RunPhase::Sampling,
                            shot: Some(4),
                            kind: "TaskError",
                            message: "operation timing fixture failure".to_owned(),
                        });
                    }
                    TaskStreamStatus::Running => {}
                }
                if !statistics_valid {
                    report.statistics = Err(rmagna::task::TaskError {
                        field: "statistics".to_owned(),
                        message: "explicitly unavailable diagnostic fixture".to_owned(),
                    });
                }
                if !efficiency_valid {
                    report.efficiency = Err(unavailable.clone());
                }
                let baseline = export(&report);
                for (kind, name) in operations {
                    for seconds in [1.0, f64::from_bits(1)] {
                        let mut invalid = report.clone();
                        cost(&mut invalid, kind).seconds = seconds;
                        let before = invalid.clone();
                        assert!(matches!(
                            ExecutionMeter::from_checkpoint(ExecutionMeterCheckpoint {
                                limits: invalid.execution_limits,
                                costs: invalid.execution_costs,
                                elapsed: std::time::Duration::from_secs(1),
                            }),
                            Err(CostError::InvalidCheckpoint { operation, .. }) if operation == kind
                        ));
                        let error =
                            export_stream_metrics(&invalid, NumericOptions::default()).unwrap_err();
                        assert!(matches!(error, StreamExportError::InvalidState(_)));
                        assert!(
                            error
                                .to_string()
                                .contains(&format!("execution_costs.{name}.seconds")),
                            "{error}"
                        );
                        assert_eq!(invalid, before);
                        assert!(matches!(
                            export_stream_metrics(&invalid, NumericOptions { max_work_bits: 0 }),
                            Err(StreamExportError::InvalidState(_))
                        ));
                        assert_eq!(invalid, before);
                    }
                    let mut signed_zero = report.clone();
                    cost(&mut signed_zero, kind).seconds = -0.0;
                    assert!(export_stream_metrics(&signed_zero, NumericOptions::default()).is_ok());
                }
                // Inclusive operation times overlap. Their sum exceeds owned
                // sampling time, while each balanced operation has attempts.
                let mut attempted = report.clone();
                for (kind, _) in operations {
                    let c = cost(&mut attempted, kind);
                    c.attempted = 1;
                    c.succeeded = 1;
                    c.seconds = 1.0;
                }
                assert!(
                    ExecutionMeter::from_checkpoint(ExecutionMeterCheckpoint {
                        limits: attempted.execution_limits,
                        costs: attempted.execution_costs,
                        elapsed: std::time::Duration::from_secs(1),
                    })
                    .is_ok()
                );
                let before = attempted.clone();
                let value = export(&attempted);
                assert_eq!(attempted, before);
                for key in [
                    "draw_status",
                    "estimation_status",
                    "diagnostics",
                    "estimates",
                    "standard_errors",
                    "efficiency",
                    "weights",
                    "random_state",
                ] {
                    assert_eq!(value[key], baseline[key], "{key}");
                }
                for (kind, _) in operations {
                    cost(&mut attempted, kind).seconds = 0.0;
                }
                assert!(export_stream_metrics(&attempted, NumericOptions::default()).is_ok());
            }
        }
    }
    let mut empty = literal(0, 2);
    empty.status = TaskStreamStatus::Failed;
    empty.failed = 1;
    empty.not_attempted = 1;
    empty.failure = Some(RunFailure {
        phase: RunPhase::Sampling,
        shot: Some(0),
        kind: "TaskError",
        message: "empty timing fixture failure".to_owned(),
    });
    empty.timings.sampling_seconds = 1.0;
    empty.timings.total_seconds = 1.0;
    assert_eq!(export(&empty)["estimation_status"], "empty");
    for (kind, name) in operations {
        for seconds in [1.0, f64::from_bits(1)] {
            let mut invalid = empty.clone();
            cost(&mut invalid, kind).seconds = seconds;
            let before = invalid.clone();
            let error = export_stream_metrics(&invalid, NumericOptions::default()).unwrap_err();
            assert!(matches!(error, StreamExportError::InvalidState(_)));
            assert!(
                error
                    .to_string()
                    .contains(&format!("execution_costs.{name}.seconds"))
            );
            assert_eq!(invalid, before);
        }
        let c = cost(&mut empty, kind);
        c.attempted = 1;
        c.failed = 1;
        c.seconds = 1.0;
    }
    let value = export(&empty);
    assert_eq!(value["status"], "failed");
    assert_eq!(value["estimation_status"], "empty");
    assert!(value["estimates"].is_null());
}

#[test]
fn terminal_failure_messages_reject_empty_and_preserve_nonempty_bytes() {
    let unavailable = ExactStatistics::new(
        2,
        StatisticsOptions {
            max_dimensions: 1,
            ..Default::default()
        },
    )
    .err()
    .unwrap();
    for count in [0, 4] {
        for phase in [RunPhase::Sampling, RunPhase::Accumulation] {
            for kind in ["TaskError", "Unwind"] {
                for mask in 0..if count == 0 { 1 } else { 4 } {
                    let mut report = literal(count, 9);
                    report.status = TaskStreamStatus::Failed;
                    report.failed = 1;
                    report.not_attempted -= 1;
                    report.failure = Some(RunFailure {
                        phase,
                        shot: Some(count as u64),
                        kind,
                        message: "failure fixture".to_owned(),
                    });
                    if mask & 1 != 0 {
                        report.statistics = Err(rmagna::task::TaskError {
                            field: "statistics".to_owned(),
                            message: "unavailable fixture".to_owned(),
                        });
                    }
                    if mask & 2 != 0 {
                        report.efficiency = Err(unavailable.clone());
                    }
                    let baseline = export(&report);
                    report.failure.as_mut().unwrap().message.clear();
                    let before = report.clone();
                    for options in [
                        NumericOptions::default(),
                        NumericOptions { max_work_bits: 0 },
                    ] {
                        let error = export_stream_metrics(&report, options).unwrap_err();
                        assert!(matches!(error, StreamExportError::InvalidState(_)));
                        assert!(error.to_string().contains("failure.message"));
                        assert_eq!(report, before);
                    }
                    for message in ["failure", " ", "\t\n", "λ failure\n", "\0", "a\0b"] {
                        report.failure.as_mut().unwrap().message = message.to_owned();
                        let before = report.clone();
                        let result = export_stream_metrics(&report, NumericOptions::default());
                        assert!(result.is_ok());
                        let value = result.unwrap();
                        assert_eq!(report, before);
                        assert_eq!(value["failure"]["message"], message);
                        assert_eq!(value["failure"]["type"], kind);
                        assert_eq!(value["failure"]["shot"], count);
                        let roundtrip: Value =
                            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
                        assert_eq!(roundtrip["failure"]["message"], message);
                        for key in [
                            "draw_status",
                            "status",
                            "estimation_status",
                            "diagnostics",
                            "estimates",
                            "efficiency",
                            "weights",
                            "random_state",
                        ] {
                            assert_eq!(value[key], baseline[key]);
                        }
                    }
                }
            }
        }
    }
}
