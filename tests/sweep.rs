use rstim::parser::parse_lines;
use rstim::ir::StimTarget;

#[test]
fn parse_sweep_target() {
    let instrs = parse_lines("CX sweep[0] 1").unwrap();
    assert_eq!(instrs.len(), 1);
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets[0], StimTarget::Sweep(0));
    assert_eq!(targets[1], StimTarget::Qubit(1));
}

#[test]
fn parse_sweep_large_index() {
    let instrs = parse_lines("M sweep[99]").unwrap();
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets[0], StimTarget::Sweep(99));
}

#[test]
fn sweep_roundtrip() {
    use rstim::ir::circuit_to_string;
    let src = "CX sweep[0] 1\n";
    let instrs = parse_lines(src).unwrap();
    assert_eq!(circuit_to_string(&instrs), src);
}
