use rstim::parser::parse_lines;

#[test]
fn parses_observable_args() {
    let instrs = parse_lines("OBSERVABLE_INCLUDE(2) rec[-1]\n").unwrap();
    assert_eq!(instrs[0].args().unwrap()[0], 2.0);
}
