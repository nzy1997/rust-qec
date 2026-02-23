use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn bell_pair_measurements_match() {
    let program = "H 0\nCNOT 0 1\nM 0 1\n";
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements[0], out.measurements[1]);
}

#[test]
fn i_gate_is_noop() {
    let prog = "H 0\nI 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn s_dag_undoes_s() {
    let prog = "H 0\nS 0\nS_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}
