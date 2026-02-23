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
fn mpp_zz_bell_deterministic() {
    // Bell state |Φ+> has Z0⊗Z1 = +1, so MPP Z0*Z1 → 0
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_xx_bell_deterministic() {
    // Bell state |Φ+> has X0⊗X1 = +1, so MPP X0*X1 → 0
    let m = run("H 0\nCX 0 1\nMPP X0*X1");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_yy_bell_deterministic() {
    // Bell state |Φ+> has Y0⊗Y1 = -1, so MPP Y0*Y1 → 1
    let m = run("H 0\nCX 0 1\nMPP Y0*Y1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mpp_single_qubit_z() {
    // MPP Z0 on |0> is deterministic 0
    let m = run("MPP Z0");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_single_qubit_x_random() {
    // MPP X0 on |0> is random
    let results = run_many("MPP X0", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mpp_inverted() {
    // !Z0*Z1 on Bell state: inverts the result (0→1)
    let m = run("H 0\nCX 0 1\nMPP !Z0*Z1");
    assert_eq!(m, vec![true]);
}

#[test]
fn mpp_multiple_products() {
    // Two products in one MPP: Z0*Z1 and X0*X1 on Bell state
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1 X0*X1");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mpp_three_qubit_product() {
    // Prepare GHZ: (|000>+|111>)/√2, then MPP Z0*Z1*Z2 → 0
    let m = run("H 0\nCX 0 1\nCX 0 2\nMPP Z0*Z1*Z2");
    assert_eq!(m, vec![false]);
}

#[test]
fn mpp_preserves_state() {
    // MPP should not disturb the state. Two consecutive MPP Z0*Z1 on Bell state → same result.
    let m = run("H 0\nCX 0 1\nMPP Z0*Z1\nMPP Z0*Z1");
    assert_eq!(m, vec![false, false]);
}

#[test]
fn mpp_mixed_xyz_product() {
    // Prepare |0,+,0>, measure X0*X1*Z2
    // Not an eigenstate, result is random.
    let results = run_many("H 1\nMPP X0*X1*Z2", 200);
    let ones: usize = results.iter().filter(|m| m[0]).count();
    assert!(ones > 20 && ones < 180, "expected ~50/50, got {ones}/200");
}

#[test]
fn mpp_noisy() {
    // With p=1.0, deterministic result flips
    let m = run("H 0\nCX 0 1\nMPP(1.0) Z0*Z1");
    assert_eq!(m, vec![true]); // flipped from false
}
