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
fn spp_z_equals_s() {
    let m1 = run("SPP Z0\nM 0");
    let m2 = run("S 0\nM 0");
    assert_eq!(m1, m2);
    assert_eq!(m1, vec![false]);
}

#[test]
fn spp_z_on_one_state() {
    let m = run("X 0\nSPP Z0\nM 0");
    assert_eq!(m, vec![true]);
}

#[test]
fn spp_x_equals_sqrt_x() {
    let m1 = run("SPP X0\nH_YZ 0\nM 0");
    let m2 = run("SQRT_X 0\nH_YZ 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_dag_z_equals_s_dag() {
    let m1 = run("X 0\nSPP_DAG Z0\nM 0");
    let m2 = run("X 0\nS_DAG 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_xx_phase() {
    let results = run_many("SPP X0*X1\nMPP X0*X1", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected random after SPP XX on |00>");
}

#[test]
fn spp_inverted_equals_spp_dag() {
    let m1 = run("H 0\nSPP !Z0\nH 0\nM 0");
    let m2 = run("H 0\nSPP_DAG Z0\nH 0\nM 0");
    assert_eq!(m1, m2);
}

#[test]
fn spp_preserves_stabilizer() {
    let m = run("H 0\nCX 0 1\nSPP Z0*Z1\nMPP Z0*Z1");
    assert_eq!(m, vec![false]);
}

#[test]
fn spp_multiple_products() {
    let m1 = run("SPP Z0 Z1\nM 0 1");
    let m2 = run("S 0\nS 1\nM 0 1");
    assert_eq!(m1, m2);
}
