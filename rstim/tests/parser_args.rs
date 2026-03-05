use rstim::parser::parse_lines;

#[test]
fn parses_observable_args() {
    let instrs = parse_lines("OBSERVABLE_INCLUDE(2) rec[-1]\n").unwrap();
    assert_eq!(instrs[0].args().unwrap()[0], 2.0);
}

#[test]
fn parses_args_with_spaces_in_parens() {
    let instrs = parse_lines("DETECTOR(2, 4, 0) rec[-1]").unwrap();
    let args = instrs[0].args().unwrap();
    assert_eq!(args, &[2.0, 4.0, 0.0]);
}

#[test]
fn parses_args_with_spaces_and_tabs_in_parens() {
    let instrs = parse_lines("QUBIT_COORDS(1, 3) 0").unwrap();
    let args = instrs[0].args().unwrap();
    assert_eq!(args, &[1.0, 3.0]);
}

#[test]
fn parses_multiple_detectors_with_spaces() {
    let instrs = parse_lines("DETECTOR(2, 4, 0) rec[-1]\nDETECTOR(3, 5, 0) rec[-2]").unwrap();
    assert_eq!(instrs.len(), 2);
    assert_eq!(instrs[0].args().unwrap(), &[2.0, 4.0, 0.0]);
    assert_eq!(instrs[1].args().unwrap(), &[3.0, 5.0, 0.0]);
}

#[test]
fn parses_error_channel_with_spaces() {
    let instrs = parse_lines("DEPOLARIZE1(0.001) 0 1 2").unwrap();
    assert_eq!(instrs[0].args().unwrap(), &[0.001]);
}

#[test]
fn parses_no_space_in_parens_still_works() {
    let instrs = parse_lines("DETECTOR(2,4,0) rec[-1]").unwrap();
    assert_eq!(instrs[0].args().unwrap(), &[2.0, 4.0, 0.0]);
}

#[test]
fn parses_repeat_with_spaced_args_inside() {
    let circuit = "REPEAT 2 {\nDETECTOR(1, 2, 0) rec[-1]\n}";
    let instrs = parse_lines(circuit).unwrap();
    if let rstim::ir::StimInstr::Repeat { body, count } = &instrs[0] {
        assert_eq!(*count, 2);
        assert_eq!(body[0].args().unwrap(), &[1.0, 2.0, 0.0]);
    } else {
        panic!("expected REPEAT");
    }
}
