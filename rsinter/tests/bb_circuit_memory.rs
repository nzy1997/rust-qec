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

#[test]
fn upstream_syndrome_cycle_has_expected_layer_order() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);
    let operations = cycle.operations();
    let checks = code.n2();
    let data = code.data_qubits();
    let data_start = data[0];
    let data_end = data[data.len() - 1];

    assert_eq!(operations.len(), 216 + 5 * 144 + 216 + 288);

    assert!(operations[..72]
        .iter()
        .all(|operation| operation.kind() == OperationKind::PrepX));
    assert!(operations[72..144]
        .iter()
        .all(|operation| operation.kind() == OperationKind::Cnot));
    assert!(operations[144..216].iter().all(|operation| {
        operation.kind() == OperationKind::Idle
            && operation.qubits().len() == 1
            && (data_start..=data_end).contains(&operation.qubits()[0])
    }));

    for round in 0..5 {
        let start = 216 + round * 144;
        let end = start + 144;
        assert!(operations[start..end]
            .iter()
            .all(|operation| operation.kind() == OperationKind::Cnot));
    }

    let round6 = 216 + 5 * 144;
    assert!(operations[round6..round6 + 72]
        .iter()
        .all(|operation| operation.kind() == OperationKind::MeasZ));
    assert!(operations[round6 + 72..round6 + 144]
        .iter()
        .all(|operation| operation.kind() == OperationKind::Cnot));
    assert!(operations[round6 + 144..round6 + 216].iter().all(|operation| {
        operation.kind() == OperationKind::Idle
            && operation.qubits().len() == 1
            && (data_start..=data_end).contains(&operation.qubits()[0])
    }));

    let final_layer = round6 + 216;
    assert!(operations[final_layer..final_layer + 144].iter().all(|operation| {
        operation.kind() == OperationKind::Idle
            && operation.qubits().len() == 1
            && (data_start..=data_end).contains(&operation.qubits()[0])
    }));
    assert!(operations[final_layer + 144..final_layer + 216]
        .iter()
        .all(|operation| operation.kind() == OperationKind::MeasX));
    assert!(operations[final_layer + 216..final_layer + 288]
        .iter()
        .all(|operation| operation.kind() == OperationKind::PrepZ));

    assert_eq!(checks, 72);
}

#[test]
fn upstream_syndrome_cycle_idles_only_data_qubits() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);

    for operation in cycle.operations() {
        if operation.kind() == OperationKind::Idle {
            assert_eq!(operation.qubits().len(), 1);
            assert!(code.data_qubits().contains(&operation.qubits()[0]));
        }
    }
}
