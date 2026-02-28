use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::VacuousDecoder;
use rsinter::task::{Task, CollectionOptions};
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use std::collections::HashMap;

fn make_task() -> Task {
    let circuit = parse_lines(
        "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    )
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
        },
    }
}

fn make_decoders() -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    decoders
}

#[test]
fn collect_single_task_vacuous() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(1000),
        max_errors: None,
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
        max_batch_size: Some(256),
        start_batch_size: 64,
        save_resume_filepath: Some(path),
        print_progress: false,
    };
    let r2 = collect(vec![make_task()], make_decoders(), &options2).unwrap();
    assert!(r2[0].shots >= 1000);
    assert!(r2[0].shots >= r1[0].shots);
}
