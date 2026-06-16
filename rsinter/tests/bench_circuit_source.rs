use std::collections::BTreeMap;
use std::path::Path;

use rsinter::bench::circuit_source::build_circuit_for_point;
use rsinter::bench::registry::BenchCasePoint;
use rstim::ir::StimInstr;

fn surface_point(input_type: &str) -> BenchCasePoint {
    BenchCasePoint {
        input_type: input_type.into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params: BTreeMap::new(),
    }
}

fn has_op(circuit: &[StimInstr], op_name: &str) -> bool {
    circuit
        .iter()
        .any(|instr| matches!(instr, StimInstr::Op { name, .. } if name == op_name))
}

#[test]
fn build_circuit_for_point_dispatches_rotated_memory_z() {
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let memory_x = build_circuit_for_point(&surface_point("surface_rotated_memory_x"), spec_dir)
        .unwrap();
    let memory_z = build_circuit_for_point(&surface_point("surface_rotated_memory_z"), spec_dir)
        .unwrap();

    assert_eq!(
        memory_x.params["input_type"],
        serde_json::json!("surface_rotated_memory_x")
    );
    assert_eq!(
        memory_z.params["input_type"],
        serde_json::json!("surface_rotated_memory_z")
    );

    assert!(has_op(&memory_x.circuit, "RX"));
    assert!(has_op(&memory_x.circuit, "MX"));
    assert!(has_op(&memory_z.circuit, "R"));
    assert!(has_op(&memory_z.circuit, "M"));
    assert!(!has_op(&memory_z.circuit, "MX"));
}
