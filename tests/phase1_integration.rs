use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn noiseless_repetition_code_no_detections() {
    let circuit = "\
R 0 1 2
TICK
CX 0 1
CX 2 1
TICK
MR 1
DETECTOR rec[-1]
TICK
CX 0 1
CX 2 1
TICK
MR 1
DETECTOR rec[-1] rec[-2]
TICK
M 0 2
DETECTOR rec[-1] rec[-3]
OBSERVABLE_INCLUDE(0) rec[-2]
";
    let instrs = parse_lines(circuit).unwrap();
    for seed in 0..50 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let out = ex.run(&mut rng).unwrap();
        for (i, d) in out.detectors.iter().enumerate() {
            assert!(!d, "seed={seed}, detector {i} fired");
        }
    }
}

#[test]
fn all_new_gates_parse_and_execute() {
    let circuit = "\
I 0
S_DAG 0
SQRT_X 0
SQRT_X_DAG 0
SQRT_Y 0
SQRT_Y_DAG 0
H_XY 0
H_YZ 0
CY 0 1
SWAP 0 1
ISWAP 0 1
ISWAP_DAG 0 1
XCX 0 1
XCY 0 1
XCZ 0 1
YCX 0 1
YCY 0 1
YCZ 0 1
CXSWAP 0 1
SWAPCX 0 1
CZSWAP 0 1
R 0
RX 0
RY 0
MR 0
MRX 0
MRY 0
Y_ERROR(0.0) 0
M 0 1
";
    let instrs = parse_lines(circuit).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = ex.run(&mut rng).unwrap();
    // MR + MRX + MRY + M(0) + M(1) = 5 measurement records
    assert_eq!(out.measurements.len(), 5);
}

#[test]
fn y_error_flips_at_expected_rate() {
    let prog = "Y_ERROR(0.3) 0\nM 0\n";
    let instrs = parse_lines(prog).unwrap();
    let mut ones = 0;
    let shots = 5000;
    for seed in 0..shots {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed as u64);
        let out = ex.run(&mut rng).unwrap();
        if out.measurements[0] { ones += 1; }
    }
    let rate = ones as f64 / shots as f64;
    assert!((rate - 0.3).abs() < 0.05, "Y_ERROR rate {rate} not near 0.3");
}
