use rstim::qstd101::{Qstd101Document, Qstd101Operation};
use serde_json::json;

#[test]
fn serializes_minimal_gate_document() {
    let doc = Qstd101Document {
        standard: "QSTD101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qstd101Operation::Gate {
            gate: "H".to_string(),
            targets: vec![0],
            controls: Vec::new(),
            control_configs: None,
            params: Vec::new(),
            raw_targets: None,
            display: None,
            tags: Vec::new(),
        }],
        metadata: None,
        extensions: None,
    };
    let value = serde_json::to_value(&doc).unwrap();
    assert_eq!(value["standard"], json!("QSTD101-ZY"));
    assert_eq!(value["operations"][0]["type"], json!("gate"));
    assert_eq!(value["operations"][0]["gate"], json!("H"));
}
