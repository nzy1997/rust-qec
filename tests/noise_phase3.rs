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

#[test]
fn correlated_error_deterministic() {
    let m = run("CORRELATED_ERROR(1) X0\nM 0\n");
    assert_eq!(m, vec![true]);
}

#[test]
fn correlated_error_zero_prob() {
    let m = run("CORRELATED_ERROR(0) X0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn correlated_error_multi_pauli() {
    let m = run("CORRELATED_ERROR(1) X0\nM 0 1 2\n");
    assert_eq!(m, vec![true, false, false]);
}

#[test]
fn else_correlated_error_skipped_when_first_fires() {
    let m = run("CORRELATED_ERROR(1) X0\nELSE_CORRELATED_ERROR(1) X1\nM 0 1\n");
    assert_eq!(m, vec![true, false]);
}

#[test]
fn else_correlated_error_fires_when_first_doesnt() {
    let m = run("CORRELATED_ERROR(0) X0\nELSE_CORRELATED_ERROR(1) X1\nM 0 1\n");
    assert_eq!(m, vec![false, true]);
}

#[test]
fn correlated_error_chain_three() {
    let m = run("CORRELATED_ERROR(0) X0\nELSE_CORRELATED_ERROR(1) X1\nELSE_CORRELATED_ERROR(1) X2\nM 0 1 2\n");
    assert_eq!(m, vec![false, true, false]);
}

#[test]
fn correlated_error_resets_flag() {
    let m = run(
        "CORRELATED_ERROR(1) X0\n\
         ELSE_CORRELATED_ERROR(1) X1\n\
         CORRELATED_ERROR(1) X2\n\
         ELSE_CORRELATED_ERROR(1) X3\n\
         M 0 1 2 3\n"
    );
    assert_eq!(m, vec![true, false, true, false]);
}

#[test]
fn correlated_error_multi_qubit_pauli() {
    let m = run("CORRELATED_ERROR(1) Y0 Z1\nM 0 1\n");
    assert_eq!(m, vec![true, false]);
}

#[test]
fn pauli_channel_1_deterministic_x() {
    let m = run("PAULI_CHANNEL_1(1,0,0) 0\nM 0\n");
    assert_eq!(m, vec![true]);
}

#[test]
fn pauli_channel_1_deterministic_z() {
    // Z|0⟩ = |0⟩
    let m = run("PAULI_CHANNEL_1(0,0,1) 0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn pauli_channel_1_no_error() {
    let m = run("PAULI_CHANNEL_1(0,0,0) 0\nM 0\n");
    assert_eq!(m, vec![false]);
}

#[test]
fn pauli_channel_1_multi_qubit() {
    let m = run("PAULI_CHANNEL_1(1,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![true, true]);
}

#[test]
fn pauli_channel_2_deterministic_xx() {
    // p_xx=1 (index 4), all others 0
    // Order: IX IY IZ XI XX XY XZ YI YX YY YZ ZI ZX ZY ZZ
    let m = run("PAULI_CHANNEL_2(0,0,0,0,1,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![true, true]);
}

#[test]
fn pauli_channel_2_deterministic_zi() {
    // p_zi=1 (index 11): Z on first qubit, I on second
    let m = run("PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,1,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]); // Z|0⟩=|0⟩
}

#[test]
fn pauli_channel_2_no_error() {
    let m = run("PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n");
    assert_eq!(m, vec![false, false]);
}
