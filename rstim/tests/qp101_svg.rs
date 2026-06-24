use rstim::parser::parse_lines;
use rstim::qp101::{
    export_qp101, Qp101Annotation, Qp101Display, Qp101Document, Qp101Operation, Qp101PauliBasis,
    Qp101TargetRef,
};
use rstim::qp101_svg::render_svg;

#[test]
fn svg_renderer_draws_wires_gates_and_ticks() {
    let instrs =
        parse_lines("QUBIT_COORDS(0, 0) 0\nQUBIT_COORDS(1, 0) 1\nH 0\nCX 0 1\nTICK\nM 0 1\n")
            .expect("test circuit should parse");
    let mut doc = export_qp101(&instrs).expect("test circuit should export to QP101");
    doc.operations.push(Qp101Operation::Gate {
        gate: "CUSTOM".to_string(),
        targets: vec![1],
        controls: Vec::new(),
        control_configs: None,
        params: Vec::new(),
        raw_targets: Some(vec![Qp101TargetRef::Qubit {
            index: 1,
            inverted: None,
        }]),
        display: Some(Qp101Display {
            label: Some("A&B<test>".to_string()),
        }),
        tags: Vec::new(),
        annotations: Vec::new(),
    });

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    assert!(svg.starts_with("<svg"), "SVG should start with <svg: {svg}");
    for attr in ["width=\"512\"", "height=\"112\"", "viewBox=\"0 0 512 112\""] {
        assert!(svg.contains(attr), "SVG missing root attr {attr}: {svg}");
    }
    for marker in ["q0", "q1", "H", "CX", "tick"] {
        assert!(
            svg.contains(marker),
            "SVG missing semantic marker {marker}: {svg}"
        );
    }
    for element in ["<line", "<rect", "<circle"] {
        assert!(
            svg.contains(element),
            "SVG missing visible element {element}: {svg}"
        );
    }
    assert!(
        svg.contains("A&amp;B&lt;test&gt;"),
        "display label should be XML-escaped: {svg}"
    );
    assert!(
        !svg.contains("A&B<test>"),
        "display label must not appear as raw XML-sensitive text: {svg}"
    );
}

#[test]
fn svg_renderer_rejects_zero_qubits() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 0,
        operations: Vec::new(),
        metadata: None,
        extensions: None,
    };

    let err = render_svg(&doc).expect_err("zero-qubit document should fail layout");
    assert!(
        err.contains("num_qubits") || err.contains("qubits"),
        "error should name num_qubits or qubits, got {err}"
    );
}

#[test]
fn svg_renderer_draws_cz_and_swap_specializations() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 3,
        operations: vec![
            Qp101Operation::Gate {
                gate: "CZ".to_string(),
                targets: vec![1],
                controls: vec![0],
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Gate {
                gate: "SWAP".to_string(),
                targets: vec![1, 2],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    assert!(
        svg.contains("class=\"CZ\""),
        "CZ should render specialized wiring: {svg}"
    );
    assert!(
        svg.contains("class=\"target CZ\""),
        "CZ should render a labeled target box: {svg}"
    );
    assert!(
        svg.contains("class=\"SWAP\""),
        "SWAP should render specialized wiring: {svg}"
    );
    assert!(
        svg.contains(">SWAP</text>"),
        "SWAP should retain its note label: {svg}"
    );
}

#[test]
fn svg_renderer_falls_back_when_paired_gate_operands_are_unmatched() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 3,
        operations: vec![Qp101Operation::Gate {
            gate: "CX".to_string(),
            targets: vec![1, 2],
            controls: vec![0],
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

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    assert!(
        svg.contains("class=\"gate-box\""),
        "unmatched paired operands should render a generic fallback box: {svg}"
    );
    assert!(
        svg.contains("height=\"124\""),
        "fallback box should span all validated lanes instead of dropping one: {svg}"
    );
    assert!(
        !svg.contains("class=\"target CX\""),
        "unmatched paired operands must not partially render a specialized CX shape: {svg}"
    );
}

#[test]
fn svg_renderer_renders_unsupported_multi_qubit_gate_as_fallback_box() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qp101Operation::Gate {
            gate: "ISWAP".to_string(),
            targets: vec![0, 1],
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

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    assert_eq!(
        svg.matches("class=\"gate-box\"").count(),
        1,
        "unsupported multi-qubit gate should render as one fallback box: {svg}"
    );
    assert!(
        svg.contains("height=\"76\""),
        "fallback box should span both qubit lanes: {svg}"
    );
    assert!(
        svg.contains(">ISWAP</text>"),
        "fallback box should keep the gate label: {svg}"
    );
}

#[test]
fn svg_renderer_renders_qp101_fallback_operations_and_annotations() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![
            Qp101Operation::QubitCoords {
                coords: vec![0.0],
                targets: vec![0],
                annotations: Vec::new(),
            },
            Qp101Operation::ShiftCoords {
                delta: vec![1.0],
                annotations: Vec::new(),
            },
            Qp101Operation::Noise {
                gate: "PAULI_CHANNEL_1".to_string(),
                params: vec![0.1, 0.0, 0.0],
                raw_targets: vec![
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 1,
                        inverted: None,
                    },
                    Qp101TargetRef::Rec { offset: -1 },
                    Qp101TargetRef::Combiner,
                    Qp101TargetRef::Sweep { index: 0 },
                ],
                annotations: vec![annotation("noise", Some("q\""), Some("p'"))],
            },
            Qp101Operation::Repeat {
                count: 2,
                body: vec![Qp101Operation::Gate {
                    gate: "X".to_string(),
                    targets: vec![0],
                    controls: Vec::new(),
                    control_configs: None,
                    params: Vec::new(),
                    raw_targets: None,
                    display: None,
                    tags: Vec::new(),
                    annotations: Vec::new(),
                }],
                annotations: vec![annotation("loop", Some("round"), Some("body"))],
            },
            Qp101Operation::Gate {
                gate: "M".to_string(),
                targets: vec![0],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: vec![0.0],
                sources: vec![Qp101TargetRef::Rec { offset: -1 }],
                annotations: vec![annotation("det", None, Some("seen"))],
            },
            Qp101Operation::ObservableInclude {
                index: 7,
                sources: vec![Qp101TargetRef::Rec { offset: -1 }],
                annotations: vec![annotation("obs", Some("logical"), None)],
            },
            Qp101Operation::Annotation {
                kind: "NOTE".to_string(),
                text: "A&B<test>".to_string(),
                annotations: vec![annotation("meta", Some("kind"), Some("text"))],
            },
            Qp101Operation::Gate {
                gate: "SWAP".to_string(),
                targets: vec![0],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Gate {
                gate: "EMPTY".to_string(),
                targets: Vec::new(),
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: Some(vec![
                    Qp101TargetRef::Rec { offset: -2 },
                    Qp101TargetRef::Combiner,
                    Qp101TargetRef::Sweep { index: 1 },
                ]),
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };

    let svg = render_svg(&doc).expect("fallback QP101 operations should render");

    for marker in [
        "PAULI_CHANNEL_1",
        "repeat x2",
        "DETECTOR",
        "D0 = m1",
        "OBS_INCLUDE(7)",
        "L7 *= m1",
        "loop: round: body",
        "NOTE: A&amp;B&lt;test&gt;",
        "SWAP",
        "EMPTY",
    ] {
        assert!(
            svg.contains(marker),
            "SVG missing fallback marker {marker}: {svg}"
        );
    }
    assert!(
        svg.contains("noise: q&quot;: p&apos;"),
        "annotations should render and escape quote/apostrophe characters: {svg}"
    );
    assert!(
        svg.find("class=\"wire\"")
            .expect("wire layer should be present")
            < svg
                .find("loop: round: body")
                .expect("repeat annotation should be present"),
        "repeat annotations should render in the foreground buffer after wires: {svg}"
    );
}

#[test]
fn svg_renderer_rejects_out_of_range_qubit_targets() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![Qp101Operation::Gate {
            gate: "H".to_string(),
            targets: vec![3],
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

    let err = render_svg(&doc).expect_err("out-of-range qubit target should fail layout");
    assert!(
        err.contains("qubit 3") && err.contains("num_qubits"),
        "error should name the invalid target and qubit count, got {err}"
    );
}

#[test]
fn svg_renderer_resolves_detector_observable_sources() {
    let detector_doc = export_qp101(
        &parse_lines("M 0\nDETECTOR rec[-1]\n").expect("detector source fixture should parse"),
    )
    .expect("detector source fixture should export");
    let detector_svg = render_svg(&detector_doc).expect("detector source fixture should render");

    for marker in [">m1</text>", ">DETECTOR</text>", ">D0 = m1</text>"] {
        assert!(
            detector_svg.contains(marker),
            "detector SVG should contain {marker}: {detector_svg}"
        );
    }
    assert!(
        !detector_svg.contains(">D0 = rec[-1]</text>"),
        "detector source should resolve to the existing measurement anchor: {detector_svg}"
    );

    let observable_doc = export_qp101(
        &parse_lines("M 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n")
            .expect("observable source fixture should parse"),
    )
    .expect("observable source fixture should export");
    let observable_svg =
        render_svg(&observable_doc).expect("observable source fixture should render");

    for marker in [">OBS_INCLUDE(2)</text>", ">L2 *= m1</text>"] {
        assert!(
            observable_svg.contains(marker),
            "observable SVG should contain {marker}: {observable_svg}"
        );
    }

    let malformed_doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![
            Qp101Operation::Gate {
                gate: "M".to_string(),
                targets: vec![0],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -99 }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };
    let malformed_svg = render_svg(&malformed_doc).expect("malformed source should render");

    assert!(
        malformed_svg.contains(">D0 = rec[-99]</text>"),
        "unavailable rec source should remain visible as raw text: {malformed_svg}"
    );
    assert!(
        !malformed_svg.contains(">D0 = m1</text>"),
        "unavailable rec source must not invent the nearest anchor: {malformed_svg}"
    );

    let hand_built_doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![
                    Qp101TargetRef::Sweep { index: 0 },
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 1,
                        inverted: Some(true),
                    },
                ],
                annotations: Vec::new(),
            },
            Qp101Operation::ObservableInclude {
                index: 3,
                sources: vec![Qp101TargetRef::Qubit {
                    index: 1,
                    inverted: Some(true),
                }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };
    let hand_built_svg = render_svg(&hand_built_doc).expect("hand-built sources should render");

    for marker in [
        ">D0 = sweep[0]*!X1</text>",
        ">OBS_INCLUDE(3)</text>",
        ">L3 *= !q1</text>",
    ] {
        assert!(
            hand_built_svg.contains(marker),
            "hand-built source SVG should contain {marker}: {hand_built_svg}"
        );
    }
}

#[test]
fn svg_renderer_covers_source_history_edge_cases() {
    let empty_source_doc = qp101_doc(
        1,
        vec![
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::ObservableInclude {
                index: 5,
                sources: Vec::new(),
                annotations: Vec::new(),
            },
        ],
    );
    let empty_source_svg =
        render_svg(&empty_source_doc).expect("empty source labels should render");

    for marker in [">D0 = -</text>", ">L5 *= -</text>"] {
        assert!(
            empty_source_svg.contains(marker),
            "empty source SVG should contain {marker}: {empty_source_svg}"
        );
    }

    let hand_built_doc = qp101_doc(
        3,
        vec![
            Qp101Operation::Gate {
                gate: "MPP".to_string(),
                targets: Vec::new(),
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: Some(Vec::new()),
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Gate {
                gate: "MPP".to_string(),
                targets: vec![0, 1],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Gate {
                gate: "MXX".to_string(),
                targets: vec![0, 1, 2],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Noise {
                gate: "HERALDED_PAULI_CHANNEL_1".to_string(),
                params: vec![0.0, 0.0, 0.0, 0.0],
                raw_targets: vec![
                    Qp101TargetRef::Sweep { index: 0 },
                    Qp101TargetRef::Combiner,
                    Qp101TargetRef::Qubit {
                        index: 1,
                        inverted: None,
                    },
                ],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -5 }],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -4 }],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -3 }],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -2 }],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -1 }],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Pauli {
                    basis: Qp101PauliBasis::Y,
                    qubit: 0,
                    inverted: None,
                }],
                annotations: Vec::new(),
            },
        ],
    );
    let svg = render_svg(&hand_built_doc).expect("hand-built history fixture should render");

    for marker in [
        ">m1</text>",
        ">m2</text>",
        ">m3</text>",
        ">m4</text>",
        ">m5</text>",
        ">D0 = m1</text>",
        ">D1 = m2</text>",
        ">D2 = m3</text>",
        ">D3 = m4</text>",
        ">D4 = m5</text>",
        ">D5 = Y0</text>",
    ] {
        assert!(
            svg.contains(marker),
            "hand-built history SVG should contain {marker}: {svg}"
        );
    }
    assert!(
        !svg.contains(">m6</text>"),
        "combiner-only raw targets should not create measurement anchors: {svg}"
    );
}

#[test]
fn svg_renderer_preserves_explicit_combiner_sources() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qp101Operation::Detector {
            coords: Vec::new(),
            sources: vec![
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
            ],
            annotations: Vec::new(),
        }],
        metadata: None,
        extensions: None,
    };

    let svg = render_svg(&doc).expect("combiner source fixture should render");

    assert!(
        svg.contains(">D0 = X0*Z1</text>"),
        "explicit combiner should not introduce extra separators: {svg}"
    );
    assert!(
        !svg.contains(">D0 = X0***Z1</text>"),
        "explicit combiner must not be surrounded by duplicate separators: {svg}"
    );
}

#[test]
fn svg_renderer_prefers_resolved_source_lanes_for_host_lane() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![
            Qp101Operation::Gate {
                gate: "M".to_string(),
                targets: vec![1],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Gate {
                gate: "M".to_string(),
                targets: vec![0],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![
                    Qp101TargetRef::Rec { offset: -2 },
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 0,
                        inverted: None,
                    },
                ],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::Z,
                        qubit: 1,
                        inverted: None,
                    },
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 0,
                        inverted: None,
                    },
                ],
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Sweep { index: 0 }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };

    let svg = render_svg(&doc).expect("host lane precedence fixture should render");

    let m1_y = text_y(&svg, "m1").expect("m1 anchor should have a y coordinate");
    let m2_y = text_y(&svg, "m2").expect("m2 anchor should have a y coordinate");

    let resolved_source_y =
        text_y(&svg, "D0 = m1*X0").expect("resolved source label should be present");
    assert_eq!(
        resolved_source_y, m1_y,
        "resolved measurement sources should host on the measurement lane: {svg}"
    );

    let explicit_source_y =
        text_y(&svg, "D1 = Z1*X0").expect("explicit source label should be present");
    assert_eq!(
        explicit_source_y, m2_y,
        "explicit-only source labels should host on their minimum qubit lane: {svg}"
    );

    let sweep_source_y =
        text_y(&svg, "D2 = sweep[0]").expect("sweep-only source label should be present");
    assert_eq!(
        sweep_source_y, m2_y,
        "source labels with no qubit lane should fall back to q0: {svg}"
    );
}

#[test]
fn svg_renderer_resolves_mpp_and_pair_measurement_sources() {
    let instrs = parse_lines(
        "MPP X0*Z1 Y2\nMXX 3 4\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .expect("mpp and pair-measurement source fixture should parse");
    let doc = export_qp101(&instrs).expect("mpp and pair-measurement source fixture should export");

    let svg = render_svg(&doc).expect("mpp and pair-measurement source fixture should render");

    for marker in [
        ">MPP</text>",
        ">MXX</text>",
        ">m1</text>",
        ">m2</text>",
        ">m3</text>",
        ">D0 = m1</text>",
        ">D1 = m2</text>",
        ">D2 = m3</text>",
    ] {
        assert!(
            svg.contains(marker),
            "SVG should contain measurement-source marker {marker}: {svg}"
        );
    }
    assert!(
        !svg.contains(">D0 = rec[-3]</text>")
            && !svg.contains(">D1 = rec[-2]</text>")
            && !svg.contains(">D2 = rec[-1]</text>"),
        "MPP and MXX sources should resolve to existing anchors instead of raw rec text: {svg}"
    );
}

#[test]
fn svg_renderer_resolves_mpad_and_heralded_measurement_sources() {
    let mpad_doc = export_qp101(
        &parse_lines("MPAD 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n")
            .expect("mpad source fixture should parse"),
    )
    .expect("mpad source fixture should export");
    let mpad_svg = render_svg(&mpad_doc).expect("mpad source fixture should render");

    for marker in [
        ">MPAD</text>",
        ">m1</text>",
        ">m2</text>",
        ">D0 = m1</text>",
        ">D1 = m2</text>",
    ] {
        assert!(
            mpad_svg.contains(marker),
            "MPAD SVG should contain {marker}: {mpad_svg}"
        );
    }

    let heralded_doc = export_qp101(
        &parse_lines("HERALDED_ERASE(1) 0\nDETECTOR rec[-1]\n")
            .expect("heralded source fixture should parse"),
    )
    .expect("heralded source fixture should export");
    let heralded_svg = render_svg(&heralded_doc).expect("heralded source fixture should render");

    for marker in [">HERALDED_ERASE</text>", ">m1</text>", ">D0 = m1</text>"] {
        assert!(
            heralded_svg.contains(marker),
            "heralded SVG should contain {marker}: {heralded_svg}"
        );
    }
}

#[test]
fn svg_renderer_labels_measurements_with_global_anchors() {
    let instrs =
        parse_lines("M 0\nMRL 1\nMX 0\n").expect("measurement anchor fixture should parse");
    let mut doc = export_qp101(&instrs).expect("measurement anchor fixture should export");
    match &mut doc.operations[0] {
        Qp101Operation::Gate { annotations, .. } => {
            annotations.push(annotation("measure", Some("first"), Some("annotated")));
        }
        op => panic!("expected first operation to be a measurement gate, got {op:?}"),
    }
    let original_doc = doc.clone();

    let svg = render_svg(&doc).expect("measurement anchor fixture should render");

    for marker in [">M</text>", ">MRL</text>", ">MX</text>"] {
        assert!(
            svg.contains(marker),
            "SVG should keep original measurement gate label {marker}: {svg}"
        );
    }
    for anchor in [">m1</text>", ">m2-m3</text>", ">m4</text>"] {
        assert!(
            svg.contains(anchor),
            "SVG should contain measurement anchor {anchor}: {svg}"
        );
    }
    assert!(
        svg.find(">m1</text>").expect("m1 should be present")
            < svg.find(">m2-m3</text>").expect("m2-m3 should be present"),
        "m1 should appear before the MRL span: {svg}"
    );
    assert!(
        svg.find(">m2-m3</text>").expect("m2-m3 should be present")
            < svg.find(">m4</text>").expect("m4 should be present"),
        "MRL should reserve m2 and m3 before MX receives m4: {svg}"
    );
    let anchor_y = text_y(&svg, "m1").expect("m1 anchor should have a y coordinate");
    let annotation_y = text_y(&svg, "measure: first: annotated")
        .expect("measurement annotation should have a y coordinate");
    assert_eq!(
        annotation_y,
        anchor_y + 12,
        "measurement annotations should render one text line below their anchor: {svg}"
    );
    assert_eq!(
        doc, original_doc,
        "SVG rendering must not mutate the QP101 document"
    );

    let reset_only =
        export_qp101(&parse_lines("R 0\nRX 1\n").expect("reset-only fixture should parse"))
            .expect("reset-only fixture should export");
    let reset_svg = render_svg(&reset_only).expect("reset-only fixture should render");

    assert!(
        !reset_svg.contains(">m1</text>"),
        "reset-only gates must not receive measurement anchors: {reset_svg}"
    );
}

#[test]
fn svg_renderer_keeps_last_lane_measurement_annotations_in_viewbox() {
    let instrs = parse_lines("M 0\n").expect("measurement annotation fixture should parse");
    let mut doc = export_qp101(&instrs).expect("measurement annotation fixture should export");
    match &mut doc.operations[0] {
        Qp101Operation::Gate { annotations, .. } => {
            annotations.push(annotation("measure", Some("single"), Some("annotated")));
        }
        op => panic!("expected first operation to be a measurement gate, got {op:?}"),
    }

    let svg = render_svg(&doc).expect("measurement annotation fixture should render");

    let height = root_attr_i32(&svg, "height").expect("root height should be present");
    let viewbox_height = root_viewbox_height(&svg).expect("viewBox height should be present");
    let annotation_y = text_y(&svg, "measure: single: annotated")
        .expect("measurement annotation should have a y coordinate");
    assert!(
        annotation_y + 4 <= height,
        "measurement annotation baseline should fit within the SVG height with padding: {svg}"
    );
    assert_eq!(
        viewbox_height, height,
        "viewBox height should track the rendered SVG height: {svg}"
    );
}

#[test]
fn svg_renderer_draws_repeat_groups_and_iteration_boundaries() {
    let instrs = parse_lines("REPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n")
        .expect("repeat group fixture should parse");
    let doc = export_qp101(&instrs).expect("repeat group fixture should export");

    let svg = render_svg(&doc).expect("repeat group fixture should render");

    for marker in [
        "class=\"repeat-group\"",
        ">repeat x2</text>",
        "class=\"repeat-iteration-boundary\"",
        ">iter 2</text>",
        ">m1</text>",
        ">m2</text>",
        ">D0 = m1</text>",
        ">D1 = m2</text>",
    ] {
        assert!(
            svg.contains(marker),
            "repeat SVG should contain {marker}: {svg}"
        );
    }
    assert_eq!(
        svg.matches(">m1</text>").count(),
        1,
        "first repeat iteration should contain exactly one m1 anchor: {svg}"
    );
    assert_eq!(
        svg.matches(">m2</text>").count(),
        1,
        "second repeat iteration should continue to m2 instead of resetting to m1: {svg}"
    );
    assert!(
        !svg.contains(">D1 = m1</text>"),
        "second detector source must not resolve to the first iteration anchor: {svg}"
    );
    assert!(
        svg.find(">m1</text>").expect("m1 should be present")
            < svg.find(">m2</text>").expect("m2 should be present"),
        "measurement anchors should appear in expanded repeat order: {svg}"
    );
    assert!(
        svg.find("class=\"gate-box\"")
            .expect("body gate boxes should be present")
            < svg
                .find(">repeat x2</text>")
                .expect("repeat label should be present"),
        "repeat labels should paint after body gates so they remain visible: {svg}"
    );
    assert!(
        svg.find("class=\"gate-box\"")
            .expect("body gate boxes should be present")
            < svg
                .find(">iter 2</text>")
                .expect("iter label should be present"),
        "iteration labels should paint after body gates so they remain visible: {svg}"
    );

    let first_gate_top =
        first_element_attr_i32(&svg, "<rect class=\"gate-box\"", "y").expect("gate y");
    let repeat_label_y = text_y(&svg, "repeat x2").expect("repeat label y");
    let iter_label_y = text_y(&svg, "iter 2").expect("iteration label y");
    assert!(
        repeat_label_y < first_gate_top,
        "repeat label baseline should sit above the first body gate band: {svg}"
    );
    assert!(
        iter_label_y > 0 && iter_label_y < first_gate_top,
        "iteration label baseline should stay inside the viewBox and above the first body gate band: {svg}"
    );
}

#[test]
fn svg_renderer_draws_nested_repeat_groups_and_preserves_measurement_order() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 3 {\n    M 0\n    DETECTOR rec[-1]\n  }\n}\n")
        .expect("nested repeat fixture should parse");
    let doc = export_qp101(&instrs).expect("nested repeat fixture should export");

    let svg = render_svg(&doc).expect("nested repeat fixture should render");

    assert_eq!(
        svg.matches("class=\"repeat-group\"").count(),
        3,
        "nested repeats should render one outer and two expanded inner repeat groups: {svg}"
    );
    assert_eq!(
        svg.matches(">repeat x2</text>").count(),
        1,
        "outer repeat label should render once: {svg}"
    );
    assert_eq!(
        svg.matches(">repeat x3</text>").count(),
        2,
        "inner repeat label should render once per expanded outer iteration: {svg}"
    );
    let outer_labels = text_positions(&svg, "repeat x2");
    let inner_labels = text_positions(&svg, "repeat x3");
    assert_eq!(
        outer_labels.len(),
        1,
        "outer repeat should have one positioned label: {svg}"
    );
    assert_eq!(
        inner_labels.len(),
        2,
        "inner repeat should have two positioned labels: {svg}"
    );
    assert!(
        !inner_labels.contains(&outer_labels[0]),
        "no inner repeat label should share the outer repeat label coordinate: {svg}"
    );
    assert_eq!(
        svg.matches(">iter 2</text>").count(),
        3,
        "nested repeats should include one outer iter 2 marker and two inner iter 2 markers: {svg}"
    );
    assert_eq!(
        svg.matches(">iter 3</text>").count(),
        2,
        "each expanded inner repeat should include an iter 3 marker: {svg}"
    );

    for index in 1..=6 {
        let measurement = format!(">m{index}</text>");
        let detector = format!(">D{} = m{index}</text>", index - 1);
        assert_eq!(
            svg.matches(&measurement).count(),
            1,
            "nested repeat should contain exactly one {measurement}: {svg}"
        );
        assert_eq!(
            svg.matches(&detector).count(),
            1,
            "nested repeat should contain exactly one {detector}: {svg}"
        );
    }
}

#[test]
fn svg_renderer_keeps_deep_nested_repeat_labels_inside_group_bounds() {
    let instrs =
        parse_lines("REPEAT 2 {\n  REPEAT 2 {\n    REPEAT 2 {\n      M 0\n    }\n  }\n}\n")
            .expect("deep nested repeat fixture should parse");
    let doc = export_qp101(&instrs).expect("deep nested repeat fixture should export");

    let svg = render_svg(&doc).expect("deep nested repeat fixture should render");
    let label_bounds = repeat_group_label_bounds(&svg);

    assert!(
        !label_bounds.is_empty(),
        "deep nested repeat fixture should render repeat group labels: {svg}"
    );
    for (rect_left, rect_right, label_x) in label_bounds {
        assert!(
            label_x >= rect_left && label_x <= rect_right,
            "repeat label x={label_x} should stay inside its own group bounds {rect_left}..={rect_right}: {svg}"
        );
    }
}

#[test]
fn svg_renderer_offsets_repeat_annotations_from_first_body_measurement_anchor() {
    let instrs = parse_lines("REPEAT 2 {\n  M 0\n}\n")
        .expect("annotated repeat measurement fixture should parse");
    let mut doc =
        export_qp101(&instrs).expect("annotated repeat measurement fixture should export");
    match &mut doc.operations[0] {
        Qp101Operation::Repeat { annotations, .. } => {
            annotations.push(annotation("loop", Some("round"), Some("body")));
        }
        op => panic!("expected first operation to be a repeat block, got {op:?}"),
    }

    let svg = render_svg(&doc).expect("annotated repeat measurement fixture should render");

    let repeat_annotation =
        text_xy(&svg, "loop: round: body").expect("repeat annotation should be positioned");
    let first_anchor = text_xy(&svg, "m1").expect("first measurement anchor should be positioned");
    assert_ne!(
        repeat_annotation, first_anchor,
        "repeat annotation should not overlap the first body measurement anchor: {svg}"
    );
}

#[test]
fn svg_renderer_assigns_measurement_anchors_in_expanded_repeat_order() {
    let instrs = parse_lines("M 0\nREPEAT 2 {\n  M 0\n}\nM 0\n")
        .expect("repeat measurement anchor fixture should parse");
    let doc = export_qp101(&instrs).expect("repeat measurement anchor fixture should export");

    let svg = render_svg(&doc).expect("repeat measurement anchor fixture should render");

    for anchor in ["m1", "m2", "m3", "m4"] {
        let marker = format!(">{anchor}</text>");
        assert_eq!(
            svg.matches(&marker).count(),
            1,
            "SVG should contain one {anchor} measurement anchor: {svg}"
        );
    }
    assert!(
        svg.find(">m1</text>").expect("m1 should be present")
            < svg.find(">m2</text>").expect("m2 should be present"),
        "repeat body should continue measurement anchors after top-level m1: {svg}"
    );
    assert!(
        svg.find(">m2</text>").expect("m2 should be present")
            < svg.find(">m3</text>").expect("m3 should be present"),
        "second repeat iteration should continue after first repeat-body m2: {svg}"
    );
    assert!(
        svg.find(">m3</text>").expect("m3 should be present")
            < svg.find(">m4</text>").expect("m4 should be present"),
        "post-repeat measurement should continue after the expanded repeat-body m3: {svg}"
    );
}

fn root_attr_i32(svg: &str, attr: &str) -> Option<i32> {
    let root_end = svg.find('>')?;
    let attrs = &svg[..root_end];
    let needle = format!("{attr}=\"");
    let value_start = attrs.find(&needle)? + needle.len();
    let value = &attrs[value_start..];
    let value_end = value.find('"')?;
    value[..value_end].parse().ok()
}

fn root_viewbox_height(svg: &str) -> Option<i32> {
    let root_end = svg.find('>')?;
    let attrs = &svg[..root_end];
    let value_start = attrs.find("viewBox=\"")? + "viewBox=\"".len();
    let value = &attrs[value_start..];
    let value_end = value.find('"')?;
    value[..value_end].split_whitespace().nth(3)?.parse().ok()
}

fn qp101_doc(num_qubits: usize, operations: Vec<Qp101Operation>) -> Qp101Document {
    Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits,
        operations,
        metadata: None,
        extensions: None,
    }
}

fn text_y(svg: &str, content: &str) -> Option<i32> {
    text_xy(svg, content).map(|(_, y)| y)
}

fn text_xy(svg: &str, content: &str) -> Option<(i32, i32)> {
    text_positions(svg, content).into_iter().next()
}

fn text_positions(svg: &str, content: &str) -> Vec<(i32, i32)> {
    let needle = format!(">{content}</text>");
    let mut positions = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative_end) = svg[search_start..].find(&needle) {
        let text_end = search_start + relative_end;
        if let Some(text_start) = svg[..text_end].rfind("<text") {
            let attrs = &svg[text_start..text_end];
            if let (Some(x_start), Some(y_start)) = (attrs.find(" x=\""), attrs.find(" y=\"")) {
                let x = &attrs[x_start + " x=\"".len()..];
                let y = &attrs[y_start + " y=\"".len()..];
                if let (Some(x_end), Some(y_end)) = (x.find('"'), y.find('"')) {
                    if let (Ok(x), Ok(y)) = (x[..x_end].parse(), y[..y_end].parse()) {
                        positions.push((x, y));
                    }
                }
            }
        }
        search_start = text_end + needle.len();
    }
    positions
}

fn repeat_group_label_bounds(svg: &str) -> Vec<(i32, i32, i32)> {
    let mut bounds = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative_rect_start) = svg[search_start..].find("<rect class=\"repeat-group\"") {
        let rect_start = search_start + relative_rect_start;
        let Some(rect_end) = svg[rect_start..].find("/>") else {
            break;
        };
        let rect_attrs = &svg[rect_start..rect_start + rect_end];
        let Some(label_start_relative) =
            svg[rect_start + rect_end..].find("<text class=\"repeat-group-label\"")
        else {
            break;
        };
        let label_start = rect_start + rect_end + label_start_relative;
        let Some(label_end) = svg[label_start..].find("</text>") else {
            break;
        };
        let label_attrs = &svg[label_start..label_start + label_end];

        if let (Some(rect_x), Some(rect_width), Some(label_x)) = (
            svg_attr_i32(rect_attrs, "x"),
            svg_attr_i32(rect_attrs, "width"),
            svg_attr_i32(label_attrs, "x"),
        ) {
            bounds.push((rect_x, rect_x + rect_width, label_x));
        }
        search_start = label_start + label_end;
    }
    bounds
}

fn svg_attr_i32(attrs: &str, name: &str) -> Option<i32> {
    let needle = format!("{name}=\"");
    let value_start = attrs.find(&needle)? + needle.len();
    let value = &attrs[value_start..];
    let value_end = value.find('"')?;
    value[..value_end].parse().ok()
}

fn first_element_attr_i32(svg: &str, start: &str, name: &str) -> Option<i32> {
    let element_start = svg.find(start)?;
    let element_end = svg[element_start..].find('>')?;
    svg_attr_i32(&svg[element_start..element_start + element_end], name)
}

fn annotation(kind: &str, label: Option<&str>, text: Option<&str>) -> Qp101Annotation {
    Qp101Annotation {
        kind: kind.to_string(),
        target_slots: Vec::new(),
        label: label.map(str::to_string),
        text: text.map(str::to_string),
        style: None,
        tags: Vec::new(),
        context: None,
    }
}
