use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn detector_coords_include_shift() {
    let program = "SHIFT_COORDS(1,2)\nM 0\nDETECTOR(3,4) rec[-1]\n";
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.detector_coords[0], vec![4.0, 6.0]);
}
