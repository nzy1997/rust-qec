use rand::{SeedableRng, rngs::StdRng};
use rstim::executor::Executor;
use rstim::near_clifford::NearCliffordExecutor;
use rstim::parser::parse_lines;

#[test]
fn two_t_circuit_samples_coherent_x_parity() {
    let circuit = NearCliffordExecutor::compile_text("H 0\nCX 0 1\nT 0\nH 0\nT 0\nMX 0 1").unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    let samples = circuit.sample(4096, &mut rng).unwrap();
    let even = samples
        .iter()
        .filter(|shot| shot.measurements[0] == shot.measurements[1])
        .count();
    let frequency = even as f64 / samples.len() as f64;
    assert!((frequency - 0.75).abs() < 0.03, "frequency={frequency}");
    assert!(samples.iter().all(|shot| shot.measurements.len() == 2));
}

#[test]
fn t_then_t_dag_restores_clifford_measurement() {
    let circuit = NearCliffordExecutor::compile_text("H 0\nT 0\nT_DAG 0\nH 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(31);
    let samples = circuit.sample(64, &mut rng).unwrap();
    assert!(samples.iter().all(|shot| shot.measurements == [false]));
}

#[test]
fn repeat_and_inverted_terminal_target_work() {
    let repeated = NearCliffordExecutor::compile_text(
        "H 0\nREPEAT 2 {\n  T 0\n}\nT_DAG 0\nT_DAG 0\nH 0\nM !0",
    )
    .unwrap();
    let mut rng = StdRng::seed_from_u64(31);
    let shot = repeated.run(&mut rng).unwrap();
    assert_eq!(shot.measurements, vec![true]);
}

#[test]
fn pure_clifford_route_is_preserved() {
    let instructions = parse_lines("H 0\nCX 0 1\nM 0 1").unwrap();
    let near = NearCliffordExecutor::compile(instructions.clone()).unwrap();
    let mut existing = Executor::from_instrs(instructions).unwrap();
    let mut near_rng = StdRng::seed_from_u64(3);
    let mut old_rng = StdRng::seed_from_u64(3);
    for _ in 0..128 {
        let near_shot = near.run(&mut near_rng).unwrap();
        let old_shot = existing.run(&mut old_rng).unwrap();
        assert_eq!(near_shot.measurements[0], near_shot.measurements[1]);
        assert_eq!(old_shot.measurements[0], old_shot.measurements[1]);
    }
}

#[test]
fn unsupported_rotation_and_existing_reset_name_are_distinct() {
    let error =
        NearCliffordExecutor::compile_text("H 0\nROT_Z(0.4487989505128276) 0\nM 0").unwrap_err();
    assert!(error.contains("unsupported gate ROT_Z"), "{error}");
    let reset = NearCliffordExecutor::compile_text("H 0\nRZ 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(3);
    assert_eq!(reset.run(&mut rng).unwrap().measurements, [false]);
    let error = NearCliffordExecutor::compile_text("T(0.2) 0").unwrap_err();
    assert!(error.contains("takes no arguments"), "{error}");
}

#[test]
fn mid_circuit_measurement_and_invalid_targets() {
    let circuit = NearCliffordExecutor::compile_text("M 0\nT 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(3);
    assert_eq!(circuit.run(&mut rng).unwrap().measurements, [false, false]);
    let error = NearCliffordExecutor::compile_text("CX 0").unwrap_err();
    assert!(error.contains("requires qubit pairs"), "{error}");
    let error = NearCliffordExecutor::compile_text("T !0").unwrap_err();
    assert!(error.contains("requires qubit targets"), "{error}");
    let repeated = NearCliffordExecutor::compile_text("REPEAT 2 {\n  M 0\n}").unwrap();
    assert_eq!(repeated.run(&mut rng).unwrap().measurements, [false, false]);
}

#[test]
fn active_limit_is_reported_during_run() {
    let instructions = parse_lines("H 0\nT 0\nM 0").unwrap();
    let circuit = NearCliffordExecutor::compile_with_limit(instructions, 0).unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    let error = circuit.run(&mut rng).unwrap_err();
    assert!(error.contains("active-state limit"), "{error}");
}

#[test]
fn physical_qubit_limit_is_checked_before_allocation() {
    let error = NearCliffordExecutor::compile_text("H 4294967295").unwrap_err();
    assert!(error.contains("exceeds 4096 physical qubits"), "{error}");
}

#[test]
fn empty_measurement_does_not_start_terminal_region() {
    let circuit = NearCliffordExecutor::compile_text("M\nH 0\nH 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(1);
    assert_eq!(circuit.run(&mut rng).unwrap().measurements, [false]);
}
