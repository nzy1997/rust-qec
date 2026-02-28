use rstim::ir::{StimInstr, StimTarget, Annotation};

#[test]
fn build_simple_instr() {
    let instr = StimInstr::new("H", vec![], vec![StimTarget::Qubit(0)]);
    assert_eq!(instr.name().unwrap(), "H");
}

#[test]
fn build_detector_annotation() {
    let ann = Annotation::detector(vec![0.0, 1.0], vec![-1, -2]);
    assert_eq!(ann.rec_offsets.len(), 2);
}
