use rstim::qstd101::{
    Qstd101Document, Qstd101Operation, Qstd101PauliBasis, Qstd101TargetRef,
};
use serde_json::json;

#[test]
fn serializes_minimal_gate_document_full_contract() {
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
    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value,
        json!({
            "standard": "QSTD101-ZY",
            "version": "1.0",
            "num_qubits": 2,
            "operations": [
                {
                    "type": "gate",
                    "gate": "H",
                    "targets": [0]
                }
            ]
        })
    );
}

#[test]
fn serializes_tick_and_repeat_operations_full_contract() {
    let doc = Qstd101Document {
        standard: "QSTD101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![
            Qstd101Operation::Tick,
            Qstd101Operation::Repeat {
                count: 3,
                body: vec![Qstd101Operation::Tick],
            },
        ],
        metadata: None,
        extensions: None,
    };

    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value,
        json!({
            "standard": "QSTD101-ZY",
            "version": "1.0",
            "num_qubits": 1,
            "operations": [
                { "type": "tick" },
                {
                    "type": "repeat",
                    "count": 3,
                    "body": [{ "type": "tick" }]
                }
            ]
        })
    );
}

#[test]
fn serializes_target_refs_rec_and_pauli() {
    let rec = Qstd101TargetRef::Rec { offset: -2 };
    let pauli = Qstd101TargetRef::Pauli {
        basis: Qstd101PauliBasis::X,
        qubit: 7,
        inverted: None,
    };

    assert_eq!(
        serde_json::to_value(rec).unwrap(),
        json!({
            "kind": "rec",
            "offset": -2
        })
    );
    assert_eq!(
        serde_json::to_value(pauli).unwrap(),
        json!({
            "kind": "pauli",
            "basis": "X",
            "qubit": 7
        })
    );
}
