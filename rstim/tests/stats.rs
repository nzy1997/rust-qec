use rstim::parser::parse_lines;
use rstim::stats;

#[test]
fn num_qubits_simple() {
    let instrs = parse_lines("H 0\nCX 0 3\nM 3").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 4);
}

#[test]
fn num_qubits_empty() {
    let instrs = parse_lines("").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 0);
}

#[test]
fn num_measurements_simple() {
    let instrs = parse_lines("M 0 1 2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}

#[test]
fn num_measurements_with_repeat() {
    let instrs = parse_lines("REPEAT 10 {\n  M 0 1\n}").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 20);
}

#[test]
fn num_measurements_mpp() {
    let instrs = parse_lines("MPP X0*Y1 Z2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 2);
}

#[test]
fn num_measurements_loss_visible_single_qubit_family() {
    let instrs = parse_lines("ML 0\nMRXL 1 2\n").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 6);
}

#[test]
fn loss_does_not_count_as_measurement() {
    let instrs = parse_lines("LOSS(0.25) 0 1 2\nM 0\n").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 1);
}

#[test]
fn num_detectors_simple() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]").unwrap();
    assert_eq!(stats::num_detectors(&instrs), 2);
}

#[test]
fn num_detectors_with_repeat() {
    let instrs = parse_lines("REPEAT 5 {\n  M 0\n  DETECTOR rec[-1]\n}").unwrap();
    assert_eq!(stats::num_detectors(&instrs), 5);
}

#[test]
fn num_observables() {
    let instrs =
        parse_lines("M 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-2]").unwrap();
    assert_eq!(stats::num_observables(&instrs), 3);
}

#[test]
fn num_ticks() {
    let instrs = parse_lines("H 0\nTICK\nM 0\nTICK").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 2);
}

#[test]
fn num_ticks_with_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  TICK\n}").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 3);
}

#[test]
fn num_sweep_bits_tracks_highest_index() {
    let instrs = parse_lines("CX sweep[3] 0\nCX sweep[1] 2\n").unwrap();
    assert_eq!(stats::num_sweep_bits(&instrs), 4);
}

#[test]
fn num_sweep_bits_includes_repeat_bodies() {
    let instrs = parse_lines("REPEAT 2 {\n  CX sweep[4] 0\n}\nCX sweep[1] 2\n").unwrap();
    assert_eq!(stats::num_sweep_bits(&instrs), 5);
}

#[test]
fn summarize_empty_circuit() {
    let instrs = parse_lines("").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 0);
    assert_eq!(summary.repeat_blocks, 0);
    assert_eq!(summary.max_repeat_depth, 0);
    assert_eq!(summary.num_qubits, 0);
    assert_eq!(summary.num_measurements, 0);
    assert_eq!(summary.num_detectors, 0);
    assert_eq!(summary.num_observables, 0);
    assert_eq!(summary.num_ticks, 0);
    assert_eq!(summary.num_sweep_bits, 0);
}

#[test]
fn summarize_repeat_distinguishes_structure_from_expanded_counts() {
    let instrs = parse_lines("H 0\nREPEAT 3 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 5);
    assert_eq!(summary.repeat_blocks, 1);
    assert_eq!(summary.max_repeat_depth, 1);
    assert_eq!(summary.num_measurements, 3);
    assert_eq!(summary.num_detectors, 3);
    assert_eq!(summary.num_ticks, 3);
}

#[test]
fn summarize_nested_repeat_tracks_max_depth() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 5 {\n    M 0\n  }\n}\n").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.repeat_blocks, 2);
    assert_eq!(summary.max_repeat_depth, 2);
    assert_eq!(summary.instruction_count, 3);
    assert_eq!(summary.num_measurements, 10);
}

#[test]
fn summarize_counts_sibling_repeat_blocks_without_inflating_depth() {
    let instrs = parse_lines("REPEAT 2 {\n  M 0\n}\nREPEAT 3 {\n  M 1\n}\n").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 4);
    assert_eq!(summary.repeat_blocks, 2);
    assert_eq!(summary.max_repeat_depth, 1);
    assert_eq!(summary.num_qubits, 2);
    assert_eq!(summary.num_measurements, 5);
}

#[test]
fn summarize_combines_structural_and_expanded_counts_from_nested_feedback() {
    let instrs = parse_lines(
        "REPEAT 2 {\n  REPEAT 3 {\n    M 7\n    OBSERVABLE_INCLUDE(2) rec[-1]\n    CX sweep[4] 7\n  }\n}\n",
    )
    .unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 5);
    assert_eq!(summary.repeat_blocks, 2);
    assert_eq!(summary.max_repeat_depth, 2);
    assert_eq!(summary.num_qubits, 8);
    assert_eq!(summary.num_measurements, 6);
    assert_eq!(summary.num_detectors, 0);
    assert_eq!(summary.num_observables, 3);
    assert_eq!(summary.num_ticks, 0);
    assert_eq!(summary.num_sweep_bits, 5);
}
