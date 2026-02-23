use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn xcz_is_cx_reversed() {
    let prog = "H 1\nXCZ 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert!(!out.detectors[0], "seed={seed}: XCZ detector fired");
    }
}

#[test]
fn xcx_entangles_from_00() {
    // XCX on |00⟩: ZI→ZX, IZ→XZ. Measuring q1 in Z is random.
    let prog = "XCX 0 1\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut zeros = 0;
    for seed in 0..200 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if !out.measurements[0] { zeros += 1; }
    }
    assert!(zeros > 60 && zeros < 140, "XCX: {zeros}/200 zeros");
}

#[test]
fn xcy_entangles_from_00() {
    // XCY on |00⟩: ZI→ZY, IZ→XZ. Measuring q1 in Z is random.
    let prog = "XCY 0 1\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut zeros = 0;
    for seed in 0..200 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if !out.measurements[0] { zeros += 1; }
    }
    assert!(zeros > 60 && zeros < 140, "XCY: {zeros}/200 zeros on q1");
}

#[test]
fn ycx_entangles_from_00() {
    // YCX on |00⟩: ZI→ZX, IZ→YZ. Measuring q0 in Z anticommutes with YZ → random.
    let prog = "YCX 0 1\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut zeros = 0;
    for seed in 0..200 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        if !out.measurements[0] { zeros += 1; }
    }
    assert!(zeros > 60 && zeros < 140, "YCX: {zeros}/200 zeros on q0");
}

#[test]
fn ycz_is_cy_reversed() {
    let prog = "H 1\nYCZ 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert!(!out.detectors[0], "seed={seed}: YCZ detector fired");
    }
}

#[test]
fn ycy_is_involutory() {
    let prog = "H 0\nYCY 0 1\nYCY 0 1\nMX 0\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, false]);
}

// --- CXSWAP / SWAPCX / CZSWAP ---

#[test]
fn cxswap_on_10() {
    let prog = "X 0\nCXSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true, true]);
}

#[test]
fn swapcx_on_10() {
    let prog = "X 0\nSWAPCX 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}

#[test]
fn czswap_on_10() {
    let prog = "X 0\nCZSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}
