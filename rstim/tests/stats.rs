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
    let instrs = parse_lines("M 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-2]").unwrap();
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
