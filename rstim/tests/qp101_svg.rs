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
