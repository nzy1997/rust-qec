use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::qstd101::{
    export_qstd101, Qstd101Document, Qstd101Operation, Qstd101PauliBasis, Qstd101TargetRef,
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

#[test]
fn export_preserves_repeat_and_tick() {
    let instrs = parse_lines("H 0\nTICK\nREPEAT 2 {\n  M 0\n}\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    assert!(matches!(doc.operations[1], Qstd101Operation::Tick));
    match &doc.operations[2] {
        Qstd101Operation::Repeat { count, body } => {
            assert_eq!(*count, 2);
            assert_eq!(body.len(), 1);
            match &body[0] {
                Qstd101Operation::Gate {
                    gate,
                    targets,
                    raw_targets,
                    ..
                } => {
                    assert_eq!(gate, "M");
                    assert_eq!(targets, &vec![0]);
                    assert_eq!(raw_targets, &None);
                }
                other => panic!("unexpected repeat body op: {other:?}"),
            }
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn export_preserves_detector_and_coords() {
    let instrs = parse_lines("QUBIT_COORDS(1,2) 0\nM 0\nDETECTOR(5,6) rec[-1]\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    match &doc.operations[0] {
        Qstd101Operation::QubitCoords { coords, targets } => {
            assert_eq!(coords, &vec![1.0, 2.0]);
            assert_eq!(targets, &vec![0]);
        }
        other => panic!("unexpected coords op: {other:?}"),
    }
    match &doc.operations[2] {
        Qstd101Operation::Detector { coords, sources } => {
            assert_eq!(coords, &vec![5.0, 6.0]);
            assert_eq!(sources, &vec![Qstd101TargetRef::Rec { offset: -1 }]);
        }
        other => panic!("unexpected detector op: {other:?}"),
    }
}

#[test]
fn export_uses_raw_targets_for_feedback() {
    let instrs = parse_lines("M 0\nCX rec[-1] 1\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    match &doc.operations[1] {
        Qstd101Operation::Gate {
            raw_targets: Some(raw_targets),
            ..
        } => {
            assert_eq!(
                raw_targets,
                &vec![
                    Qstd101TargetRef::Rec { offset: -1 },
                    Qstd101TargetRef::Qubit {
                        index: 1,
                        inverted: None,
                    },
                ]
            );
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn export_gate_keeps_pauli_qubit_lanes_and_raw_targets() {
    let instrs = parse_lines("MPP X0*Z1\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    match &doc.operations[0] {
        Qstd101Operation::Gate {
            gate,
            targets,
            raw_targets,
            ..
        } => {
            assert_eq!(gate, "MPP");
            assert_eq!(targets, &vec![0, 1]);
            assert_eq!(
                raw_targets,
                &Some(vec![
                    Qstd101TargetRef::Pauli {
                        basis: Qstd101PauliBasis::X,
                        qubit: 0,
                        inverted: None,
                    },
                    Qstd101TargetRef::Combiner,
                    Qstd101TargetRef::Pauli {
                        basis: Qstd101PauliBasis::Z,
                        qubit: 1,
                        inverted: None,
                    },
                ])
            );
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn export_rejects_qubit_coords_with_inverted_target() {
    let instrs = parse_lines("QUBIT_COORDS(1,2) !0\n").unwrap();
    let err = export_qstd101(&instrs).unwrap_err();
    assert!(err.contains("QUBIT_COORDS"));
}

#[test]
fn export_rejects_detector_with_non_rec_source() {
    let parsed = parse_lines("DETECTOR 0\n").unwrap();
    assert!(export_qstd101(&parsed).is_err());

    let manual = vec![StimInstr::Op {
        name: "DETECTOR".to_string(),
        tag: None,
        args: vec![1.0, 2.0],
        targets: vec![StimTarget::Pauli {
            qubit: 3,
            basis: PauliBasis::X,
            inverted: false,
        }],
    }];
    assert!(export_qstd101(&manual).is_err());
}

#[test]
fn export_rejects_invalid_observable_include_shape() {
    let extra_args = parse_lines("OBSERVABLE_INCLUDE(0,1) rec[-1]\n").unwrap();
    assert!(export_qstd101(&extra_args).is_err());

    let non_rec_sources = parse_lines("OBSERVABLE_INCLUDE(0) 0\n").unwrap();
    assert!(export_qstd101(&non_rec_sources).is_err());
}

#[test]
fn export_includes_framework_metadata() {
    let instrs = parse_lines("H 0\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(value["metadata"], json!({ "framework": "rstim" }));
}

#[test]
fn export_rejects_tick_with_targets() {
    let instrs = parse_lines("TICK 0\n").unwrap();
    let err = export_qstd101(&instrs).unwrap_err();
    assert!(err.contains("TICK"));
}

#[test]
fn export_rejects_tick_with_args() {
    let instrs = parse_lines("TICK(1)\n").unwrap();
    let err = export_qstd101(&instrs).unwrap_err();
    assert!(err.contains("TICK"));
}

#[test]
fn export_rejects_shift_coords_with_targets() {
    let instrs = parse_lines("SHIFT_COORDS(1) 0\n").unwrap();
    let err = export_qstd101(&instrs).unwrap_err();
    assert!(err.contains("SHIFT_COORDS"));
}
