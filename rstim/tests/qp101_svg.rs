use rstim::parser::parse_lines;
use rstim::qp101::{export_qp101, Qp101Display, Qp101Document, Qp101Operation, Qp101TargetRef};
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
    for attr in [
        "width=\"512\"",
        "height=\"112\"",
        "viewBox=\"0 0 512 112\"",
    ] {
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

    assert!(svg.contains("class=\"CZ\""), "CZ should render specialized wiring: {svg}");
    assert!(
        svg.contains("class=\"target CZ\""),
        "CZ should render a labeled target box: {svg}"
    );
    assert!(svg.contains("class=\"SWAP\""), "SWAP should render specialized wiring: {svg}");
    assert!(svg.contains(">SWAP</text>"), "SWAP should retain its note label: {svg}");
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
