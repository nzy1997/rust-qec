use rsinter::collect::{CollectOptions, collect};
use rsinter::decode::{CompiledDecoder, Decoder, VacuousDecoder};
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
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        Box::new(SlowCompiledDecoder { sleep: self.sleep })
    }
}

impl CompiledDecoder for SlowCompiledDecoder {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        thread::sleep(self.sleep);
        let obs_bytes = num_obs.div_ceil(8);
        vec![0u8; num_shots * obs_bytes]
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
