use rstim::compiled::{compile_circuit, CompiledBlock};
use rstim::parser::parse_lines;

#[test]
fn compile_circuit_preserves_counts_and_repeat_regions() {
    let instrs = parse_lines(
        "R 0\nREPEAT 3 {\n  M 0\n  DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();

    let compiled = compile_circuit(&instrs).unwrap();
    let expected_repeat_body = match &instrs[1] {
        rstim::ir::StimInstr::Repeat { body, .. } => body.clone(),
        other => panic!("expected repeat instruction, got {other:?}"),
    };

    assert_eq!(compiled.source, instrs);
    assert_eq!(compiled.num_qubits, 1);
    assert_eq!(compiled.num_measurements, 3);
    assert_eq!(compiled.num_detectors, 3);
    assert_eq!(compiled.num_observables, 1);
    assert_eq!(compiled.blocks.len(), 3);

    match &compiled.blocks[1] {
        CompiledBlock::Repeat(region) => {
            assert_eq!(region.count, 3);
            assert_eq!(region.body_source, expected_repeat_body);
            assert_eq!(region.measurement_span, 1);
            assert_eq!(region.detector_span, 1);
            assert_eq!(region.body.len(), 1);
        }
        other => panic!("expected repeat block, got {other:?}"),
    }
}

#[test]
fn compile_circuit_sets_feature_flags_from_source() {
    let instrs =
        parse_lines("REPEAT 2 {\n  M 0\n  REPEAT 3 {\n    LOSS(1) 0\n    CX rec[-1] 1\n  }\n}\n")
            .unwrap();

    let compiled = compile_circuit(&instrs).unwrap();

    assert!(compiled.flags.has_loss);
    assert!(compiled.flags.has_feedback);
    assert!(compiled.flags.has_nested_repeat);
    assert_eq!(compiled.num_qubits, 2);
    assert_eq!(compiled.num_measurements, 2);
    assert_eq!(compiled.num_detectors, 0);
    assert_eq!(compiled.num_observables, 0);
}

#[test]
fn compile_circuit_distinguishes_non_nested_repeat_circuits() {
    let compiled = compile_circuit(&parse_lines("REPEAT 4 {\n  M 0\n}\n").unwrap()).unwrap();

    assert!(!compiled.flags.has_loss);
    assert!(!compiled.flags.has_feedback);
    assert!(!compiled.flags.has_nested_repeat);
    assert_eq!(compiled.blocks.len(), 1);

    match &compiled.blocks[0] {
        CompiledBlock::Repeat(region) => {
            assert_eq!(region.count, 4);
            assert_eq!(region.measurement_span, 1);
            assert_eq!(region.detector_span, 0);
        }
        other => panic!("expected repeat block, got {other:?}"),
    }
}
