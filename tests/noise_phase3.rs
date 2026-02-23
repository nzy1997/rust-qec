use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

fn run(program: &str) -> Vec<bool> {
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    ex.run(&mut rng).unwrap().measurements
}

#[test]
fn i_error_is_noop() {
    let m = run("I_ERROR(0.1) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn ii_error_is_noop() {
    let m = run("II_ERROR(0.1) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn parser_tag_round_trip() {
    let instrs = parse_lines("I_ERROR[LEAKAGE:0.1](0.05) 0\n").unwrap();
    match &instrs[0] {
        rstim::ir::StimInstr::Op { name, tag, args, .. } => {
            assert_eq!(name, "I_ERROR");
            assert_eq!(tag.as_deref(), Some("LEAKAGE:0.1"));
            assert_eq!(args, &[0.05]);
        }
        _ => panic!("expected Op"),
    }
}

#[test]
fn parser_tag_no_args() {
    let instrs = parse_lines("I_ERROR[TAG] 0\n").unwrap();
    match &instrs[0] {
        rstim::ir::StimInstr::Op { name, tag, args, .. } => {
            assert_eq!(name, "I_ERROR");
            assert_eq!(tag.as_deref(), Some("TAG"));
            assert!(args.is_empty());
        }
        _ => panic!("expected Op"),
    }
}
