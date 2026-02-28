use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn detector_xor() {
    let program = "M 0\nM 0\nDETECTOR rec[-1] rec[-2]\n";
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.detectors.len(), 1);
    assert_eq!(out.detectors[0], out.measurements[0] ^ out.measurements[1]);
}
