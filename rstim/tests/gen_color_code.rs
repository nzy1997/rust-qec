use rstim::codegen::color_code::memory_xyz;
use rstim::stats;

#[test]
fn color_code_d3_r2() {
    // d=3 (minimum), r=2 (minimum)
    let instrs = memory_xyz(3, 2, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn color_code_d3_r3() {
    let instrs = memory_xyz(3, 3, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn color_code_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = memory_xyz(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}

#[test]
fn color_code_with_noise() {
    use rstim::ir::StimInstr;
    let instrs = memory_xyz(3, 2, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}
