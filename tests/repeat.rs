use rstim::parser::parse_lines;

#[test]
fn parses_repeat_block() {
    let program = "REPEAT 2 {\nH 0\n}\n";
    let instrs = parse_lines(program).unwrap();
    assert_eq!(instrs.len(), 1);
}
