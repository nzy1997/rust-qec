use rand::rngs::StdRng;
use rand::SeedableRng;
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

// --- SQRT_X / SQRT_X_DAG ---

#[test]
fn sqrt_x_preserves_x_eigenstate() {
    let prog = "H 0\nSQRT_X 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_x_dag_undoes_sqrt_x() {
    let prog = "H 0\nSQRT_X 0\nSQRT_X_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_x_squared_is_x() {
    let prog = "SQRT_X 0\nSQRT_X 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

// --- SQRT_Y / SQRT_Y_DAG ---

#[test]
fn sqrt_y_squared_is_y() {
    let prog = "SQRT_Y 0\nSQRT_Y 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

#[test]
fn sqrt_y_dag_undoes_sqrt_y() {
    let prog = "H 0\nSQRT_Y 0\nSQRT_Y_DAG 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sqrt_y_maps_z_eigenstate_to_x_eigenstate() {
    let prog = "SQRT_Y 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

// --- H_XY / H_YZ ---

#[test]
fn h_xy_swaps_x_and_y() {
    let prog = "H 0\nH_XY 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn h_xy_negates_z() {
    let prog = "H_XY 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

#[test]
fn h_yz_swaps_y_and_z() {
    let prog = "H_YZ 0\nMY 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn h_yz_negates_x() {
    let prog = "H 0\nH_YZ 0\nMX 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![true]);
}

// --- CY / SWAP ---

#[test]
fn cy_creates_bell_pair() {
    let prog = "H 0\nCY 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(prog).unwrap();
    for seed in 0..100 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        assert!(!out.detectors[0], "seed={seed}: CY Bell pair detector fired");
    }
}

#[test]
fn swap_exchanges_qubits() {
    let prog = "X 0\nSWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}

// --- ISWAP / ISWAP_DAG ---

#[test]
fn iswap_dag_undoes_iswap() {
    let prog = "H 0\nISWAP 0 1\nISWAP_DAG 0 1\nMX 0\nM 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, false]);
}

#[test]
fn iswap_on_computational_basis() {
    let prog = "X 0\nISWAP 0 1\nM 0 1\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false, true]);
}
