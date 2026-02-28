use rsinter::task::{Task, CollectionOptions};
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;

#[test]
fn strong_id_deterministic() {
    let circuit = parse_lines("X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let t1 = Task {
        circuit: circuit.clone(),
        decoder: "vacuous".into(),
        dem: dem.clone(),
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions::default(),
    };
    let t2 = Task {
        circuit,
        decoder: "vacuous".into(),
        dem,
        metadata: serde_json::json!({"d": 3}),
        collection_options: CollectionOptions::default(),
    };
    assert_eq!(t1.strong_id(), t2.strong_id());
    assert_eq!(t1.strong_id().len(), 64); // SHA256 hex
}

#[test]
fn strong_id_changes_with_decoder() {
    let circuit = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let t1 = Task {
        circuit: circuit.clone(),
        decoder: "a".into(),
        dem: dem.clone(),
        metadata: serde_json::Value::Null,
        collection_options: CollectionOptions::default(),
    };
    let t2 = Task {
        circuit,
        decoder: "b".into(),
        dem,
        metadata: serde_json::Value::Null,
        collection_options: CollectionOptions::default(),
    };
    assert_ne!(t1.strong_id(), t2.strong_id());
}
