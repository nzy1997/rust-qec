use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn sqrt_z_is_s() {
    let prog = "H 0\nSQRT_Z 0\nSQRT_Z_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn mz_is_m() {
    let prog = "MZ 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn cnot_alias_works() {
    let prog = "H 0\nCNOT 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements[0], out.measurements[1]);
}

#[test]
fn rz_alias_works() {
    let prog = "H 0\nRZ 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..50 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert!(!out.measurements[0], "seed={seed}");
    }
}

#[test]
fn mrz_alias_works() {
    let prog = "H 0\nMRZ 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..50 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert!(!out.measurements[1], "seed={seed}: M after MRZ should be 0");
    }
}
