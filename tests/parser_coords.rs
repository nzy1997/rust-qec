use rstim::parser::parse_lines;
use rstim::ir::StimInstr;

#[test]
fn parses_coord_instructions() {
    let program = "QUBIT_COORDS(1,2) 0 1\nSHIFT_COORDS(3,4)\nTICK\n";
    let instrs = parse_lines(program).unwrap();
    assert_eq!(instrs.len(), 3);
    match &instrs[0] {
        StimInstr::Op { name, args, .. } => {
            assert_eq!(name, "QUBIT_COORDS");
            assert_eq!(args.as_slice(), &[1.0, 2.0]);
        }
        _ => panic!("expected Op"),
    }
}
