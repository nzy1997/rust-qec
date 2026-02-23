use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn run(prog: &str) -> Vec<bool> {
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    exec.run(&mut rng).unwrap().measurements
}

#[test]
fn mpad_pushes_fixed_bits() {
    let m = run("MPAD 0 1 0 1 1");
    assert_eq!(m, vec![false, true, false, true, true]);
}

#[test]
fn mpad_before_measurement() {
    let m = run("MPAD 1\nM 0");
    assert_eq!(m, vec![true, false]);
}

#[test]
fn mpad_noisy() {
    let m = run("MPAD(1.0) 0 1 0");
    assert_eq!(m, vec![true, false, true]);
}

#[test]
fn mpad_interacts_with_detector() {
    let prog = "MPAD 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(prog).unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = exec.run(&mut rng).unwrap();
    assert_eq!(out.detectors, vec![false]);
}
