use rstim::parser::parse_lines;
use rstim::qp101::{
    Qp101Annotation, Qp101Display, Qp101Document, Qp101Operation, Qp101PauliBasis, Qp101TargetRef,
    export_qp101,
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
        "detector",
        "L7",
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
fn svg_renderer_labels_measurements_with_global_anchors() {
    let instrs =
        parse_lines("M 0\nMRL 1\nMX 0\n").expect("measurement anchor fixture should parse");
    let doc = export_qp101(&instrs).expect("measurement anchor fixture should export");
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
