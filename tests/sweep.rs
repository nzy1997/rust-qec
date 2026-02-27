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
    // Use a valid two-qubit instruction; sweep[99] is a legal large index
    let instrs = parse_lines("CX sweep[99] 0").unwrap();
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets[0], StimTarget::Sweep(99));

    // u32::MAX is the largest representable sweep index
    let instrs2 = parse_lines(&format!("CX sweep[{}] 0", u32::MAX)).unwrap();
    let targets2 = instrs2[0].targets().unwrap();
    assert_eq!(targets2[0], StimTarget::Sweep(u32::MAX));
}

#[test]
fn sweep_roundtrip() {
    use rstim::ir::circuit_to_string;
    let src = "CX sweep[0] 1\n";
    let instrs = parse_lines(src).unwrap();
    assert_eq!(circuit_to_string(&instrs), src);
}

#[test]
fn sweep_negative_index_rejected() {
    assert!(parse_lines("CX sweep[-1] 0").is_err());
}

#[test]
fn sweep_empty_index_rejected() {
    assert!(parse_lines("CX sweep[] 0").is_err());
}

#[test]
fn sweep_nonnumeric_rejected() {
    assert!(parse_lines("CX sweep[abc] 0").is_err());
}
