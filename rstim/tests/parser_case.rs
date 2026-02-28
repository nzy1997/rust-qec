use rstim::parser::parse_lines;
use rstim::ir::StimInstr;

#[test]
fn parses_case_insensitive_names() {
    let instrs = parse_lines("h 0\nDeTeCtOr rec[-1]\n").unwrap();
    match &instrs[0] {
        StimInstr::Op { name, .. } => assert_eq!(name, "H"),
        _ => panic!("expected Op"),
    }
    match &instrs[1] {
        StimInstr::Op { name, .. } => assert_eq!(name, "DETECTOR"),
        _ => panic!("expected Op"),
    }
}

#[test]
fn rejects_non_negative_rec() {
    let err = parse_lines("DETECTOR rec[0]\n").unwrap_err();
    assert!(err.contains("rec"));
}
