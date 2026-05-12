use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::qp101::{
    export_qp101, export_qp101_with_sample_trace, Qp101Annotation, Qp101AnnotationStyle,
    Qp101Display, Qp101Document, Qp101Operation, Qp101PauliBasis, Qp101TargetRef,
};
use rstim::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};
use serde_json::json;

#[test]
fn serializes_minimal_gate_document_full_contract() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qp101Operation::Gate {
            gate: "H".to_string(),
            targets: vec![0],
            controls: Vec::new(),
            control_configs: None,
            params: Vec::new(),
            raw_targets: None,
            display: None,
            tags: Vec::new(),
            annotations: Vec::new(),
        }],
        metadata: None,
        extensions: None,
    };
    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value,
        json!({
            "standard": "QP101-ZY",
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
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![
            Qp101Operation::Tick {
                annotations: Vec::new(),
            },
            Qp101Operation::Repeat {
                count: 3,
                body: vec![Qp101Operation::Tick {
                    annotations: Vec::new(),
                }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };

    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value,
        json!({
            "standard": "QP101-ZY",
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
    let rec = Qp101TargetRef::Rec { offset: -2 };
    let pauli = Qp101TargetRef::Pauli {
        basis: Qp101PauliBasis::X,
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
fn serializes_display_annotation_and_remaining_target_variants() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 3,
        operations: vec![
            Qp101Operation::Gate {
                gate: "CX".to_string(),
                targets: vec![1],
                controls: vec![0],
                control_configs: Some(vec![true]),
                params: vec![0.125],
                raw_targets: Some(vec![
                    Qp101TargetRef::Qubit {
                        index: 0,
                        inverted: Some(true),
                    },
                    Qp101TargetRef::Sweep { index: 5 },
                ]),
                display: Some(Qp101Display {
                    label: Some("feedback".to_string()),
                }),
                tags: vec!["tagged".to_string()],
                annotations: Vec::new(),
            },
            Qp101Operation::Annotation {
                kind: "note".to_string(),
                text: "hello".to_string(),
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };

    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value,
        json!({
            "standard": "QP101-ZY",
            "version": "1.0",
            "num_qubits": 3,
            "operations": [
                {
                    "type": "gate",
                    "gate": "CX",
                    "targets": [1],
                    "controls": [0],
                    "control_configs": [true],
                    "params": [0.125],
                    "raw_targets": [
                        {
                            "kind": "qubit",
                            "index": 0,
                            "inverted": true
                        },
                        {
                            "kind": "sweep",
                            "index": 5
                        }
                    ],
                    "display": {
                        "label": "feedback"
                    },
                    "tags": ["tagged"]
                },
                {
                    "type": "annotation",
                    "kind": "note",
                    "text": "hello"
                }
            ]
        })
    );
}

#[test]
fn serializes_operation_annotations_when_present() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![Qp101Operation::Noise {
            gate: "X_ERROR".to_string(),
            params: vec![0.001],
            raw_targets: vec![Qp101TargetRef::Qubit {
                index: 0,
                inverted: None,
            }],
            annotations: vec![Qp101Annotation {
                kind: "marker".to_string(),
                target_slots: vec![0],
                label: Some("X".to_string()),
                text: Some("repeat[2]".to_string()),
                style: Some(Qp101AnnotationStyle {
                    preset: Some("danger".to_string()),
                    color: Some("red".to_string()),
                    highlight: Some(true),
                }),
                tags: vec!["dem-origin".to_string()],
                context: Some(json!({
                    "dem_error_index": 17,
                    "source_branch": "X"
                })),
            }],
        }],
        metadata: None,
        extensions: None,
    };

    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(
        value["operations"][0]["annotations"][0],
        json!({
            "kind": "marker",
            "target_slots": [0],
            "label": "X",
            "text": "repeat[2]",
            "style": {
                "preset": "danger",
                "color": "red",
                "highlight": true
            },
            "tags": ["dem-origin"],
            "context": {
                "dem_error_index": 17,
                "source_branch": "X"
            }
        })
    );
}

#[test]
fn export_preserves_repeat_and_tick() {
    let instrs = parse_lines("H 0\nTICK\nREPEAT 2 {\n  M 0\n}\n").unwrap();
    let doc = export_qp101(&instrs).unwrap();
    assert!(matches!(doc.operations[1], Qp101Operation::Tick { .. }));
    match &doc.operations[2] {
        Qp101Operation::Repeat { count, body, .. } => {
            assert_eq!(*count, 2);
            assert_eq!(body.len(), 1);
            match &body[0] {
                Qp101Operation::Gate {
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
    let doc = export_qp101(&instrs).unwrap();
    match &doc.operations[0] {
        Qp101Operation::QubitCoords {
            coords, targets, ..
        } => {
            assert_eq!(coords, &vec![1.0, 2.0]);
            assert_eq!(targets, &vec![0]);
        }
        other => panic!("unexpected coords op: {other:?}"),
    }
    match &doc.operations[2] {
        Qp101Operation::Detector {
            coords, sources, ..
        } => {
            assert_eq!(coords, &vec![5.0, 6.0]);
            assert_eq!(sources, &vec![Qp101TargetRef::Rec { offset: -1 }]);
        }
        other => panic!("unexpected detector op: {other:?}"),
    }
}

#[test]
fn export_uses_raw_targets_for_feedback() {
    let instrs = parse_lines("M 0\nCX rec[-1] 1\n").unwrap();
    let doc = export_qp101(&instrs).unwrap();
    match &doc.operations[1] {
        Qp101Operation::Gate {
            raw_targets: Some(raw_targets),
            ..
        } => {
            assert_eq!(
                raw_targets,
                &vec![
                    Qp101TargetRef::Rec { offset: -1 },
                    Qp101TargetRef::Qubit {
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
    let doc = export_qp101(&instrs).unwrap();
    match &doc.operations[0] {
        Qp101Operation::Gate {
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
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 0,
                        inverted: None,
                    },
                    Qp101TargetRef::Combiner,
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::Z,
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
    let err = export_qp101(&instrs).unwrap_err();
    assert!(err.contains("QUBIT_COORDS"));
}

#[test]
fn export_rejects_detector_with_non_rec_source() {
    let parsed = parse_lines("DETECTOR 0\n").unwrap();
    assert!(export_qp101(&parsed).is_err());

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
    assert!(export_qp101(&manual).is_err());
}

#[test]
fn export_rejects_invalid_observable_include_shape() {
    let extra_args = parse_lines("OBSERVABLE_INCLUDE(0,1) rec[-1]\n").unwrap();
    assert!(export_qp101(&extra_args).is_err());

    let non_rec_sources = parse_lines("OBSERVABLE_INCLUDE(0) 0\n").unwrap();
    assert!(export_qp101(&non_rec_sources).is_err());
}

#[test]
fn export_rejects_invalid_observable_include_values() {
    for (args, needle) in [
        (vec![-1.0], "non-negative integer"),
        (vec![0.5], "non-negative integer"),
        (vec![u32::MAX as f64 + 1.0], "exceeds u32 range"),
    ] {
        let instrs = vec![StimInstr::Op {
            name: "OBSERVABLE_INCLUDE".to_string(),
            tag: None,
            args,
            targets: vec![StimTarget::Rec(-1)],
        }];
        let err = export_qp101(&instrs).unwrap_err();
        assert!(err.contains(needle), "unexpected error: {err}");
    }
}

#[test]
fn export_includes_framework_metadata() {
    let instrs = parse_lines("H 0\n").unwrap();
    let doc = export_qp101(&instrs).unwrap();
    let value = serde_json::to_value(doc).unwrap();
    assert_eq!(value["metadata"], json!({ "framework": "rstim" }));
    assert_eq!(value["standard"], "QP101-ZY");
}

#[test]
fn export_classifies_loss_as_noise() {
    let instrs = parse_lines("LOSS(0.25) 0\n").unwrap();
    let doc = export_qp101(&instrs).unwrap();
    let value = serde_json::to_value(doc).unwrap();

    assert_eq!(value["operations"][0]["type"], "noise");
    assert_eq!(value["operations"][0]["gate"], "LOSS");
}

#[test]
fn export_qp101_with_sample_trace_adds_noise_measurement_and_detector_annotations() {
    let circuit = parse_lines("REPEAT 2 {\n  LOSS(1) 0\n  M 0\n  DETECTOR rec[-1]\n}\nMRL 0\n")
        .unwrap();
    let trace = SampleTrace {
        noise_events: vec![
            NoiseEvent {
                op_path: vec![0, 0],
                repeat_iterations: vec![1],
                instr_name: "LOSS".to_string(),
                target_slots: vec![0],
                target_qubits: vec![0],
                occurred: true,
                branch_label: Some("L".to_string()),
            },
            NoiseEvent {
                op_path: vec![1],
                repeat_iterations: vec![],
                instr_name: "LOSS".to_string(),
                target_slots: vec![0],
                target_qubits: vec![0],
                occurred: false,
                branch_label: None,
            },
        ],
        measurement_events: vec![
            MeasurementEvent {
                op_path: vec![0, 1],
                repeat_iterations: vec![1],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "M".to_string(),
                measurement_index: 2,
                bit: true,
                loss_cause: true,
                component: MeasurementComponent::Value,
            },
            MeasurementEvent {
                op_path: vec![1],
                repeat_iterations: vec![],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "MRL".to_string(),
                measurement_index: 3,
                bit: true,
                loss_cause: false,
                component: MeasurementComponent::LossFlag,
            },
            MeasurementEvent {
                op_path: vec![1],
                repeat_iterations: vec![],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "MRL".to_string(),
                measurement_index: 4,
                bit: true,
                loss_cause: true,
                component: MeasurementComponent::Value,
            },
        ],
        detector_events: vec![
            DetectorEvent {
                op_path: vec![0, 2],
                repeat_iterations: vec![0],
                detector_index: 0,
                flipped: false,
            },
            DetectorEvent {
                op_path: vec![0, 2],
                repeat_iterations: vec![1],
                detector_index: 1,
                flipped: true,
            },
        ],
    };

    let doc = export_qp101_with_sample_trace(&circuit, &trace).unwrap();
    let value = serde_json::to_value(doc).unwrap();

    let loss_annotations = value["operations"][0]["body"][0]["annotations"]
        .as_array()
        .unwrap();
    assert_eq!(loss_annotations.len(), 1);
    assert_eq!(loss_annotations[0]["label"], "L");
    assert_eq!(loss_annotations[0]["target_slots"], json!([0]));
    assert_eq!(loss_annotations[0]["style"]["preset"], "danger");
    assert_eq!(loss_annotations[0]["text"], "repeat[1]");
    assert_eq!(
        loss_annotations[0]["context"]["target_qubits"],
        json!([0])
    );

    let repeat_measure_annotations = value["operations"][0]["body"][1]["annotations"]
        .as_array()
        .unwrap();
    assert_eq!(repeat_measure_annotations.len(), 1);
    assert_eq!(repeat_measure_annotations[0]["label"], "1[L]");
    assert_eq!(repeat_measure_annotations[0]["target_slots"], json!([0]));
    assert_eq!(repeat_measure_annotations[0]["text"], "repeat[1]");
    assert_eq!(
        repeat_measure_annotations[0]["context"]["measurement_index"],
        json!(2)
    );
    assert_eq!(
        repeat_measure_annotations[0]["context"]["component"],
        json!("value")
    );

    let detector_annotations = value["operations"][0]["body"][2]["annotations"]
        .as_array()
        .unwrap();
    assert_eq!(detector_annotations.len(), 1);
    assert_eq!(detector_annotations[0]["label"], "D1");
    assert_eq!(
        detector_annotations[0]["tags"],
        json!(["dem-symptom", "query-result"])
    );
    assert_eq!(detector_annotations[0]["text"], "repeat[1]");
    assert_eq!(detector_annotations[0]["context"]["detector_index"], json!(1));

    let loss_visible_annotations = value["operations"][1]["annotations"].as_array().unwrap();
    assert_eq!(loss_visible_annotations.len(), 1);
    assert_eq!(loss_visible_annotations[0]["label"], "L=1 | M=1[L]");
    assert_eq!(loss_visible_annotations[0]["target_slots"], json!([0]));
    assert_eq!(loss_visible_annotations[0]["context"]["loss_visible"], json!(true));
    assert_eq!(
        loss_visible_annotations[0]["context"]["components"],
        json!({
            "loss_flag": {
                "measurement_index": 3,
                "bit": true,
            },
            "value": {
                "measurement_index": 4,
                "bit": true,
                "loss_cause": true,
            }
        })
    );
}

#[test]
fn export_qp101_with_sample_trace_rejects_duplicate_loss_visible_components() {
    let circuit = parse_lines("MRL 0\n").unwrap();
    let trace = SampleTrace {
        noise_events: vec![],
        measurement_events: vec![
            MeasurementEvent {
                op_path: vec![0],
                repeat_iterations: vec![],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "MRL".to_string(),
                measurement_index: 1,
                bit: true,
                loss_cause: false,
                component: MeasurementComponent::LossFlag,
            },
            MeasurementEvent {
                op_path: vec![0],
                repeat_iterations: vec![],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "MRL".to_string(),
                measurement_index: 2,
                bit: false,
                loss_cause: false,
                component: MeasurementComponent::LossFlag,
            },
            MeasurementEvent {
                op_path: vec![0],
                repeat_iterations: vec![],
                target_slot: 0,
                target_qubit: 0,
                instr_name: "MRL".to_string(),
                measurement_index: 3,
                bit: true,
                loss_cause: true,
                component: MeasurementComponent::Value,
            },
        ],
        detector_events: vec![],
    };

    let err = export_qp101_with_sample_trace(&circuit, &trace).unwrap_err();
    assert!(err.contains("duplicate"));
    assert!(err.contains("loss_flag"));
}

#[test]
fn export_preserves_observable_noise_tags_and_special_targets() {
    let instrs = vec![
        StimInstr::Op {
            name: "OBSERVABLE_INCLUDE".to_string(),
            tag: None,
            args: vec![2.0],
            targets: vec![StimTarget::Rec(-1)],
        },
        StimInstr::Op {
            name: "DEPOLARIZE1".to_string(),
            tag: None,
            args: vec![0.125],
            targets: vec![StimTarget::Qubit(0)],
        },
        StimInstr::Op {
            name: "CX".to_string(),
            tag: Some("feedback".to_string()),
            args: vec![],
            targets: vec![StimTarget::QubitInv(0), StimTarget::Sweep(4)],
        },
    ];
    let doc = export_qp101(&instrs).unwrap();

    match &doc.operations[0] {
        Qp101Operation::ObservableInclude {
            index, sources, ..
        } => {
            assert_eq!(*index, 2);
            assert_eq!(sources, &vec![Qp101TargetRef::Rec { offset: -1 }]);
        }
        other => panic!("unexpected observable op: {other:?}"),
    }

    match &doc.operations[1] {
        Qp101Operation::Noise {
            gate,
            params,
            raw_targets,
            ..
        } => {
            assert_eq!(gate, "DEPOLARIZE1");
            assert_eq!(params, &vec![0.125]);
            assert_eq!(
                raw_targets,
                &vec![Qp101TargetRef::Qubit {
                    index: 0,
                    inverted: None,
                }]
            );
        }
        other => panic!("unexpected noise op: {other:?}"),
    }

    match &doc.operations[2] {
        Qp101Operation::Gate {
            gate,
            targets,
            raw_targets,
            tags,
            ..
        } => {
            assert_eq!(gate, "CX");
            assert_eq!(targets, &vec![0]);
            assert_eq!(
                raw_targets,
                &Some(vec![
                    Qp101TargetRef::Qubit {
                        index: 0,
                        inverted: Some(true),
                    },
                    Qp101TargetRef::Sweep { index: 4 },
                ])
            );
            assert_eq!(tags, &vec!["feedback".to_string()]);
        }
        other => panic!("unexpected gate op: {other:?}"),
    }
}

#[test]
fn export_rejects_tick_with_targets() {
    let instrs = parse_lines("TICK 0\n").unwrap();
    let err = export_qp101(&instrs).unwrap_err();
    assert!(err.contains("TICK"));
}

#[test]
fn export_rejects_tick_with_args() {
    let instrs = parse_lines("TICK(1)\n").unwrap();
    let err = export_qp101(&instrs).unwrap_err();
    assert!(err.contains("TICK"));
}

#[test]
fn export_rejects_shift_coords_with_targets() {
    let instrs = parse_lines("SHIFT_COORDS(1) 0\n").unwrap();
    let err = export_qp101(&instrs).unwrap_err();
    assert!(err.contains("SHIFT_COORDS"));
}
