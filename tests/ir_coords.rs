use rstim::ir::StimInstr;

#[test]
fn builds_qubit_coords_instr() {
    let instr = StimInstr::new("QUBIT_COORDS", vec![1.0, 2.0], vec![]);
    assert_eq!(instr.name().unwrap(), "QUBIT_COORDS");
}
