use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{CompiledDecoder, Decoder, VacuousDecoder};
use rsinter::failure::FailureKind;
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

struct SlowDecoder {
    sleep: Duration,
}

struct SlowCompiledDecoder {
    sleep: Duration,
}

impl Decoder for SlowDecoder {
    fn compile_for_dem(
        &self,
        _dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(SlowCompiledDecoder { sleep: self.sleep }))
    }
}

impl CompiledDecoder for SlowCompiledDecoder {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        thread::sleep(self.sleep);
        let obs_bytes = num_obs.div_ceil(8);
        Ok(vec![0u8; num_shots * obs_bytes])
    }
}

fn make_task() -> Task {
    let circuit =
        parse_lines("X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]")
            .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    Task {
        circuit,
        decoder: "vacuous".into(),
        dem,
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions {
            max_shots: Some(1000),
            max_errors: None,
            max_wall_seconds: None,
        },
    }
}

fn make_clean_task() -> Task {
    let circuit = parse_lines("M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    Task {
        circuit,
        decoder: "vacuous".into(),
        dem,
        metadata: serde_json::json!({"d": 3, "clean": true}),
        collection_options: CollectionOptions {
            max_shots: Some(16),
            max_errors: None,
            max_wall_seconds: None,
        },
    }
}

fn make_decoders() -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    decoders
}

fn make_slow_decoders(sleep: Duration) -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(SlowDecoder { sleep }));
    decoders
}

struct FailingDecoder {
    message: &'static str,
}

struct DecodeErrorDecoder;

struct DecodeErrorCompiledDecoder;

struct WrongPredictionLengthDecoder;

struct WrongPredictionLengthCompiledDecoder;

impl Decoder for FailingDecoder {
    fn compile_for_dem(
        &self,
        _dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        Err(self.message.to_string())
    }
}

impl Decoder for DecodeErrorDecoder {
    fn compile_for_dem(
        &self,
        _dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(DecodeErrorCompiledDecoder))
    }
}

impl CompiledDecoder for DecodeErrorCompiledDecoder {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        _num_shots: usize,
        _num_dets: usize,
        _num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        Err("decode failed".into())
    }
}

impl Decoder for WrongPredictionLengthDecoder {
    fn compile_for_dem(
        &self,
        _dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(WrongPredictionLengthCompiledDecoder))
    }
}

impl CompiledDecoder for WrongPredictionLengthCompiledDecoder {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        _num_shots: usize,
        _num_dets: usize,
        _num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}

fn make_failing_decoders(
    message: &'static str,
) -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(FailingDecoder { message }));
    decoders
}

fn make_decode_error_decoders() -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(DecodeErrorDecoder));
    decoders
}

fn make_wrong_prediction_length_decoders() -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(WrongPredictionLengthDecoder));
    decoders
}

fn collect_branch_options() -> CollectOptions {
    CollectOptions {
        num_workers: 1,
        max_shots: Some(1),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    }
}

#[test]
fn collect_single_task_vacuous() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };
    let results = collect(vec![make_task()], make_decoders(), &options).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].shots >= 1000);
    // With X_ERROR(0.1) and vacuous decoder, ~10% error rate
    assert!(results[0].errors > 0);
}

#[test]
fn collect_respects_max_errors() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: None,
        max_errors: Some(10),
        max_wall_seconds: None,
        max_batch_size: Some(64),
        start_batch_size: 16,
        save_resume_filepath: None,
        print_progress: false,
    };
    let results = collect(vec![make_task()], make_decoders(), &options).unwrap();
    assert!(results[0].errors >= 10);
}

#[test]
fn collect_csv_resume() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(500),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: Some(path.clone()),
        print_progress: false,
    };
    let r1 = collect(vec![make_task()], make_decoders(), &options).unwrap();
    assert!(r1[0].shots >= 500);

    // Resume — should load existing and continue
    let options2 = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: Some(path),
        print_progress: false,
    };
    let r2 = collect(vec![make_task()], make_decoders(), &options2).unwrap();
    assert!(r2[0].shots >= 1000);
    assert!(r2[0].shots >= r1[0].shots);
}

#[test]
fn collect_respects_wall_clock() {
    let mut task = make_task();
    task.collection_options = CollectionOptions {
        max_shots: None,
        max_errors: None,
        max_wall_seconds: None,
    };

    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.09),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![task],
        make_slow_decoders(Duration::from_millis(35)),
        &options,
    )
    .unwrap();

    let stats = &results[0];
    assert!(stats.seconds >= 0.09, "seconds={}", stats.seconds);
    assert!(stats.seconds < 0.5, "seconds={}", stats.seconds);
    assert!(stats.shots > 0, "shots={}", stats.shots);
    assert!(stats.shots < 20, "shots={}", stats.shots);
}

#[test]
fn collect_rejects_non_positive_wall_clock() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.0),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let err = collect(vec![make_task()], make_decoders(), &options).unwrap_err();

    assert!(err.contains("max_wall_seconds must be positive"), "{err}");
}

#[test]
fn collect_rejects_task_specific_non_positive_wall_clock() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };
    let mut task = make_task();
    task.collection_options.max_wall_seconds = Some(-0.5);

    let err = collect(vec![task], make_decoders(), &options).unwrap_err();

    assert!(err.contains("max_wall_seconds must be positive"), "{err}");
}

#[test]
fn collect_reports_ok_failure_kind_for_clean_runs() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(16),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(16),
        start_batch_size: 16,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(vec![make_clean_task()], make_decoders(), &options).unwrap();

    assert_eq!(results[0].failure_kind, FailureKind::Ok);
}

#[test]
fn collect_reports_logical_failure_kind_for_logical_errors() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(vec![make_task()], make_decoders(), &options).unwrap();

    assert!(results[0].errors > 0);
    assert_eq!(results[0].failure_kind, FailureKind::LogicalFailure);
}

#[test]
fn collect_reports_timeout_failure_kind_for_wall_clock_stop() {
    let mut task = make_clean_task();
    task.collection_options.max_shots = None;

    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.09),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![task],
        make_slow_decoders(Duration::from_millis(35)),
        &options,
    )
    .unwrap();

    assert_eq!(results[0].failure_kind, FailureKind::Timeout);
}

#[test]
fn collect_records_decoder_failure_as_task_stats() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![make_clean_task()],
        make_failing_decoders("HiGHS backend error: compile failed"),
        &options,
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SolverFailure);
}

#[test]
fn collect_missing_decoder_remains_caller_error() {
    let mut task = make_clean_task();
    task.decoder = "missing".into();

    let err = collect(vec![task], make_decoders(), &collect_branch_options()).unwrap_err();

    assert_eq!(err, "decoder not found: missing");
}

#[test]
fn collect_records_decode_error_as_solver_failure_task_stats() {
    let results = collect(
        vec![make_clean_task()],
        make_decode_error_decoders(),
        &collect_branch_options(),
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SolverFailure);
}

#[test]
fn collect_records_wrong_prediction_length_as_solver_failure_task_stats() {
    let results = collect(
        vec![make_clean_task()],
        make_wrong_prediction_length_decoders(),
        &collect_branch_options(),
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SolverFailure);
}

#[test]
fn collect_records_detector_buffer_mismatch_as_sampler_error_task_stats() {
    let mut task = make_clean_task();
    task.collection_options.max_shots = Some(1);
    task.dem.set_min_counts(9, 1);

    let results = collect(vec![task], make_decoders(), &collect_branch_options()).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SamplerError);
}

#[test]
fn collect_records_sampler_failure_as_sampler_error_task_stats() {
    let mut task = make_clean_task();
    task.collection_options.max_shots = Some(1);
    task.circuit = parse_lines("ML 0\n").unwrap();

    let results = collect(vec![task], make_decoders(), &collect_branch_options()).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SamplerError);
}

#[test]
fn collect_records_observable_buffer_mismatch_as_sampler_error_task_stats() {
    let mut task = make_clean_task();
    task.collection_options.max_shots = Some(1);
    task.dem.set_min_counts(1, 9);

    let results = collect(vec![task], make_decoders(), &collect_branch_options()).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::SamplerError);
}

#[test]
fn collect_zero_start_batch_size_finishes_without_collecting() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: None,
        start_batch_size: 0,
        save_resume_filepath: None,
        print_progress: false,
    };

    let mut task = make_clean_task();
    task.collection_options.max_shots = None;
    let results = collect(vec![task], make_decoders(), &options).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 0);
    assert_eq!(results[0].failure_kind, FailureKind::Ok);
}

#[test]
fn collect_grows_batch_size_without_an_explicit_cap() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(2),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: None,
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let mut task = make_clean_task();
    task.collection_options.max_shots = None;
    let results = collect(vec![task], make_decoders(), &options).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 2);
    assert_eq!(results[0].failure_kind, FailureKind::Ok);
}

#[test]
fn collect_prefers_task_specific_shot_limit_over_global_limit() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: None,
        max_batch_size: Some(8),
        start_batch_size: 8,
        save_resume_filepath: None,
        print_progress: false,
    };

    let mut task = make_clean_task();
    task.collection_options.max_shots = Some(3);
    let results = collect(vec![task], make_decoders(), &options).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].shots, 3);
    assert_eq!(results[0].errors, 0);
    assert_eq!(results[0].failure_kind, FailureKind::Ok);
}
