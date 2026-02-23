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

fn run_many(prog: &str, n: usize) -> Vec<Vec<bool>> {
    (0..n).map(|seed| {
        let instrs = parse_lines(prog).unwrap();
        let mut exec = Executor::from_instrs(instrs).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        exec.run(&mut rng).unwrap().measurements
    }).collect()
}

#[test]
fn mxx_bell_deterministic() {
    let m = run("H 0\nCX 0 1\nMXX 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn myy_bell_deterministic() {
    let m = run("H 0\nCX 0 1\nMYY 0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mzz_bell_deterministic() {
    let m = run("H 0\nCX 0 1\nMZZ 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mxx_random_on_product_state() {
    let results = run_many("MXX 0 1", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mzz_product_state_deterministic() {
    let m = run("MZZ 0 1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mxx_inverted() {
    let m = run("H 0\nCX 0 1\nMXX !0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mxx_multiple_pairs() {
    let m = run("H 0\nCX 0 1\nH 2\nCX 2 3\nMXX 0 1 2 3");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mzz_noisy() {
    let m = run("MZZ(1.0) 0 1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mxx_equivalent_to_mpp() {
    let m1 = run("H 0\nCX 0 1\nMXX 0 1");
    let m2 = run("H 0\nCX 0 1\nMPP X0*X1");
    assert_eq!(m1, m2);
}
