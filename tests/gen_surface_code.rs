use rstim::codegen::surface_code::{rotated_memory_x, rotated_memory_z};
use rstim::stats;

#[test]
fn rotated_memory_x_d3_r1_qubit_count() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    // d=3 rotated: 9 data + 8 ancilla = 17 qubits
    assert_eq!(stats::num_qubits(&instrs), 17);
}

#[test]
fn rotated_memory_x_d3_r1_measurement_count() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    // 1 round: 8 ancilla MR + 9 final data M = 17 measurements
    assert_eq!(stats::num_measurements(&instrs), 17);
}

#[test]
fn rotated_memory_x_has_observable() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn rotated_memory_z_d3_r1() {
    let instrs = rotated_memory_z(3, 1, 0.0);
    assert_eq!(stats::num_qubits(&instrs), 17);
    assert_eq!(stats::num_measurements(&instrs), 17);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn rotated_memory_x_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = rotated_memory_x(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}

#[test]
fn rotated_memory_x_with_noise() {
    use rstim::ir::StimInstr;
    let instrs = rotated_memory_x(3, 1, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}
