//! End-to-end: generate circuit -> collect -> fit_binomial
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::VacuousDecoder;
use rsinter::stats::fit_binomial;
use rsinter::task::{CollectionOptions, Task};
use rstim::codegen::repetition_code_memory;
use rstim::error_analyzer::ErrorAnalyzer;
use std::collections::HashMap;

#[test]
fn end_to_end_rep_code() {
    let circuit = repetition_code_memory(3, 1, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let task = Task {
        circuit,
        decoder: "vacuous".into(),
        dem,
        metadata: serde_json::json!({"d": 3, "p": 0.01}),
        collection_options: CollectionOptions {
            max_shots: Some(10_000),
            max_errors: None,
        },
    };
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> =
        HashMap::new();
    decoders.insert("vacuous".into(), Box::new(VacuousDecoder));
    let options = CollectOptions {
        num_workers: 2,
        max_shots: Some(10_000),
        max_errors: None,
        max_batch_size: Some(1024),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };
    let results = collect(vec![task], decoders, &options).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].shots >= 10_000);

    let fit = fit_binomial(results[0].shots, results[0].errors, 1000.0);
    assert!(fit.low.unwrap() < fit.best.unwrap());
    assert!(fit.best.unwrap() < fit.high.unwrap());
}
