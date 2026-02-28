use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn reset_z_always_gives_zero() {
    let prog = "H 0\nR 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_x_always_gives_plus() {
    let prog = "RX 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_y_always_gives_plus_i() {
    let prog = "H 0\nRY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements, vec![false], "seed={seed}");
    }
}

#[test]
fn reset_does_not_record_measurement() {
    let prog = "R 0\nM 0\nDETECTOR rec[-1]\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements.len(), 1);
    assert_eq!(out.measurements, vec![false]);
}

// --- MR / MRX / MRY ---

#[test]
fn mr_records_and_resets() {
    let prog = "H 0\nMR 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        assert!(!out.measurements[1], "seed={seed}: M after MR should be 0");
    }
}

#[test]
fn mrx_records_and_resets_to_plus() {
    let prog = "MRX 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        assert!(!out.measurements[1], "seed={seed}: MX after MRX should be 0");
    }
}

#[test]
fn mry_records_and_resets_to_plus_i() {
    let prog = "MRY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert_eq!(out.measurements.len(), 2);
        assert!(!out.measurements[1], "seed={seed}: MY after MRY should be 0");
    }
}
