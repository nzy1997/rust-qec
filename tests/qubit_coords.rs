use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::{executor::Executor, parser::parse_lines};

#[test]
fn stores_qubit_coords() {
    let program = "QUBIT_COORDS(1,2) 0 1\nM 0\n";
    let instrs = parse_lines(program).unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    let out = ex.run(&mut rng).unwrap();
    assert_eq!(out.qubit_coords.get(&0).unwrap(), &vec![1.0, 2.0]);
}
