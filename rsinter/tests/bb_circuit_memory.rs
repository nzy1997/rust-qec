use rsinter::bb_circuit_memory::{OperationKind, build_syndrome_cycle, build_upstream_code};

#[test]
fn upstream_bb144_code_has_expected_shape() {
    let code = build_upstream_code().unwrap();

    assert_eq!(code.ell(), 12);
    assert_eq!(code.m(), 6);
    assert_eq!(code.n2(), 72);
    assert_eq!(code.n(), 144);
    assert_eq!(code.k(), 12);
    assert_eq!(code.x_checks().len(), 72);
    assert_eq!(code.z_checks().len(), 72);
    assert_eq!(code.data_qubits().len(), 144);
    assert_eq!(code.num_circuit_qubits(), 288);

    assert!(code.hx_rows().iter().all(|row| row.len() == 6));
    assert!(code.hz_rows().iter().all(|row| row.len() == 6));
    assert_eq!(code.logical_x_rows().len(), 12);
    assert_eq!(code.logical_z_rows().len(), 12);
}

#[test]
fn upstream_syndrome_cycle_has_expected_schedule_counts() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);

    assert_eq!(cycle.operations().len(), 1440);
    assert_eq!(cycle.count(OperationKind::Cnot), 864);
    assert_eq!(cycle.count(OperationKind::Idle), 288);
    assert_eq!(cycle.count(OperationKind::PrepX), 72);
    assert_eq!(cycle.count(OperationKind::PrepZ), 72);
    assert_eq!(cycle.count(OperationKind::MeasX), 72);
    assert_eq!(cycle.count(OperationKind::MeasZ), 72);
    assert_eq!(cycle.sx_labels(), ["idle", "1", "4", "3", "5", "0", "2"]);
    assert_eq!(cycle.sz_labels(), ["3", "5", "0", "1", "2", "4", "idle"]);
}
