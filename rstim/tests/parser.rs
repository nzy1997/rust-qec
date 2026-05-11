use rstim::parser::parse_lines;
use rstim::ir::{StimTarget, StimInstr};

#[test]
fn parses_simple_gate() {
    let instrs = parse_lines("H 0\n").unwrap();
    assert_eq!(instrs.len(), 1);
    match &instrs[0] {
        StimInstr::Op { name, targets, .. } => {
            assert_eq!(name, "H");
            assert_eq!(targets, &vec![StimTarget::Qubit(0)]);
        }
        _ => panic!("expected Op"),
    }
}

#[test]
fn parses_detector_with_rec() {
    let instrs = parse_lines("DETECTOR rec[-1]\n").unwrap();
    match &instrs[0] {
        StimInstr::Op { name, .. } => assert_eq!(name, "DETECTOR"),
        _ => panic!("expected Op"),
    }
}

#[test]
fn parses_loss_instruction() {
    let instrs = parse_lines("LOSS(0.125) 0 2\n").unwrap();
    match &instrs[0] {
        StimInstr::Op { name, args, targets, .. } => {
            assert_eq!(name, "LOSS");
            assert_eq!(args, &[0.125]);
            assert_eq!(targets, &vec![StimTarget::Qubit(0), StimTarget::Qubit(2)]);
        }
        _ => panic!("expected Op"),
    }
}

#[test]
fn parses_loss_visible_measurements() {
    let instrs = parse_lines("ML 0\nMRXL !1\n").unwrap();
    match &instrs[0] {
        StimInstr::Op { name, targets, .. } => {
            assert_eq!(name, "ML");
            assert_eq!(targets, &vec![StimTarget::Qubit(0)]);
        }
        _ => panic!("expected Op"),
    }
    match &instrs[1] {
        StimInstr::Op { name, targets, .. } => {
            assert_eq!(name, "MRXL");
            assert_eq!(targets, &vec![StimTarget::QubitInv(1)]);
        }
        _ => panic!("expected Op"),
    }
}
