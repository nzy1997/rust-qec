use rstim::ir::{StimInstr, StimTarget, PauliBasis};
use rstim::parser::parse_lines;

#[test]
fn parse_mpp_single_product() {
    let prog = "MPP X0*Z1";
    let instrs = parse_lines(prog).unwrap();
    assert_eq!(instrs.len(), 1);
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, false));
        assert_eq!(targets[1], StimTarget::Combiner);
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_multiple_products() {
    let prog = "MPP X0*X1 Z2*Z3";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 6);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, false));
        assert_eq!(targets[1], StimTarget::Combiner);
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::X, false));
        assert_eq!(targets[3], StimTarget::pauli(2, PauliBasis::Z, false));
        assert_eq!(targets[4], StimTarget::Combiner);
        assert_eq!(targets[5], StimTarget::pauli(3, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_inverted() {
    let prog = "MPP !Y0*Z1";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::Y, true));
        assert_eq!(targets[2], StimTarget::pauli(1, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpp_with_args() {
    let prog = "MPP(0.01) Z0*Z1";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { args, targets, .. } = &instrs[0] {
        assert_eq!(args, &[0.01]);
        assert_eq!(targets.len(), 3);
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_mpad() {
    let prog = "MPAD 0 1 0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], StimTarget::Qubit(0));
        assert_eq!(targets[1], StimTarget::Qubit(1));
        assert_eq!(targets[2], StimTarget::Qubit(0));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_spp_single_qubit() {
    let prog = "SPP Z0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::Z, false));
    } else {
        panic!("expected Op");
    }
}

#[test]
fn parse_spp_inverted() {
    let prog = "SPP !X0";
    let instrs = parse_lines(prog).unwrap();
    if let StimInstr::Op { targets, .. } = &instrs[0] {
        assert_eq!(targets[0], StimTarget::pauli(0, PauliBasis::X, true));
    } else {
        panic!("expected Op");
    }
}
