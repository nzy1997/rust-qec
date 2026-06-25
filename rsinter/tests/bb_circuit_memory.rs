use rsinter::bb_circuit_memory::{
    OperationKind, SimulationConfig, bb_circuit_bposd_result_row, build_code,
    build_effective_models, build_syndrome_cycle, build_upstream_code,
    export_comparison_case_for_code, run_simulation, run_simulation_for_code, sample_seeded_trial,
    validate_bposd_profile_result_row,
};

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
fn build_code_supports_bb90_and_preserves_bb144_defaults() {
    let bb90 = build_code("bb90").unwrap();
    assert_eq!(bb90.ell(), 15);
    assert_eq!(bb90.m(), 3);
    assert_eq!(bb90.n2(), 45);
    assert_eq!(bb90.n(), 90);
    assert_eq!(bb90.k(), 8);

    let bb144 = build_code("bb144").unwrap();
    let upstream = build_upstream_code().unwrap();
    assert_eq!(bb144, upstream);
}

#[test]
fn build_code_supports_bb72_smoke_shape() {
    let bb72 = build_code("bb72").unwrap();
    assert_eq!(bb72.ell(), 6);
    assert_eq!(bb72.m(), 6);
    assert_eq!(bb72.n2(), 36);
    assert_eq!(bb72.n(), 72);
    assert_eq!(bb72.k(), 12);
    assert!(bb72.hx_rows().iter().all(|row| row.len() == 6));
    assert!(bb72.hz_rows().iter().all(|row| row.len() == 6));
}

#[test]
fn build_code_rejects_unknown_code_id_with_supported_values() {
    let error = build_code("bb999").unwrap_err();
    assert!(error.contains("bb999"), "{error}");
    assert!(error.contains("bb72"), "{error}");
    assert!(error.contains("bb90"), "{error}");
    assert!(error.contains("bb144"), "{error}");
}

#[test]
fn comparison_case_export_contains_models_samples_and_profile() {
    let export = export_comparison_case_for_code(
        "bb72",
        SimulationConfig {
            physical_error_rate: 1.0e-12,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(12345),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    assert_eq!(export.code_id, "bb72");
    assert_eq!(export.num_trials, 1);
    assert_eq!(export.seed, Some(12345));
    assert_eq!(export.max_bp_iterations, 10);
    assert_eq!(export.osd_order, 0);
    assert_eq!(export.z_model.num_checks, 36 * 3);
    assert_eq!(export.x_model.num_checks, 36 * 3);
    assert_eq!(export.z_model.first_logical_row, 36 * 3);
    assert_eq!(export.x_model.first_logical_row, 36 * 3);
    assert_eq!(export.trials.len(), 1);
    assert_eq!(export.trials[0].z_syndrome.len(), export.z_model.num_checks);
    assert_eq!(export.trials[0].x_syndrome.len(), export.x_model.num_checks);
    assert_eq!(export.trials[0].z_logical.len(), bb72_logical_len());
    assert_eq!(export.trials[0].x_logical.len(), bb72_logical_len());
    assert!(export.rust_result.profile.setup_seconds.is_finite());
    assert!(export.rust_result.profile.decode_seconds.is_finite());
}

fn bb72_logical_len() -> usize {
    build_code("bb72").unwrap().k()
}

#[test]
fn sample_seeded_trial_rejects_zero_cycles() {
    let code = build_code("bb90").unwrap();
    let cycle = build_syndrome_cycle(&code);

    let error = sample_seeded_trial(&code, &cycle, 0, 0.006, 12345).unwrap_err();

    assert_eq!(error, "num_cycles must be greater than zero");
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

    assert!(
        operations[..72]
            .iter()
            .all(|operation| operation.kind() == OperationKind::PrepX)
    );
    assert!(
        operations[72..144]
            .iter()
            .all(|operation| operation.kind() == OperationKind::Cnot)
    );
    assert!(operations[144..216].iter().all(|operation| {
        operation.kind() == OperationKind::Idle
            && operation.qubits().len() == 1
            && (data_start..=data_end).contains(&operation.qubits()[0])
    }));

    for round in 0..5 {
        let start = 216 + round * 144;
        let end = start + 144;
        assert!(
            operations[start..end]
                .iter()
                .all(|operation| operation.kind() == OperationKind::Cnot)
        );
    }

    let round6 = 216 + 5 * 144;
    assert!(
        operations[round6..round6 + 72]
            .iter()
            .all(|operation| operation.kind() == OperationKind::MeasZ)
    );
    assert!(
        operations[round6 + 72..round6 + 144]
            .iter()
            .all(|operation| operation.kind() == OperationKind::Cnot)
    );
    assert!(
        operations[round6 + 144..round6 + 216]
            .iter()
            .all(|operation| {
                operation.kind() == OperationKind::Idle
                    && operation.qubits().len() == 1
                    && (data_start..=data_end).contains(&operation.qubits()[0])
            })
    );

    let final_layer = round6 + 216;
    assert!(
        operations[final_layer..final_layer + 144]
            .iter()
            .all(|operation| {
                operation.kind() == OperationKind::Idle
                    && operation.qubits().len() == 1
                    && (data_start..=data_end).contains(&operation.qubits()[0])
            })
    );
    assert!(
        operations[final_layer + 144..final_layer + 216]
            .iter()
            .all(|operation| operation.kind() == OperationKind::MeasX)
    );
    assert!(
        operations[final_layer + 216..final_layer + 288]
            .iter()
            .all(|operation| operation.kind() == OperationKind::PrepZ)
    );

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

#[test]
fn one_cycle_effective_models_have_expected_syndrome_rows() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);
    let config = SimulationConfig {
        physical_error_rate: 0.003,
        num_cycles: 1,
        num_trials: 1,
        seed: Some(7),
        max_bp_iterations: 10,
        osd_order: 0,
    };

    let models = build_effective_models(&code, &cycle, &config).unwrap();

    assert_eq!(models.z_faults.decoder.num_checks(), 72 * 3);
    assert_eq!(models.x_faults.decoder.num_checks(), 72 * 3);
    assert_eq!(models.z_faults.first_logical_row, 72 * 3);
    assert_eq!(models.x_faults.first_logical_row, 72 * 3);
    assert!(!models.z_faults.channel_probs.is_empty());
    assert!(!models.x_faults.channel_probs.is_empty());
    assert_eq!(
        models.z_faults.decoder.num_bits(),
        models.z_faults.channel_probs.len()
    );
    assert_eq!(
        models.x_faults.decoder.num_bits(),
        models.x_faults.channel_probs.len()
    );
}

#[test]
fn tiny_seeded_smoke_run_reports_zero_failures_without_sampled_faults() {
    let result = run_simulation(SimulationConfig {
        physical_error_rate: 1.0e-12,
        num_cycles: 1,
        num_trials: 2,
        seed: Some(1),
        max_bp_iterations: 10,
        osd_order: 0,
    })
    .unwrap();

    assert_eq!(result.physical_error_rate, 1.0e-12);
    assert_eq!(result.num_cycles, 1);
    assert_eq!(result.num_trials, 2);
    assert_eq!(result.num_failed_trials, 0);
}

#[test]
fn zero_noise_smoke_run_reports_zero_failures() {
    let result = run_simulation(SimulationConfig {
        physical_error_rate: 0.0,
        num_cycles: 1,
        num_trials: 2,
        seed: Some(1),
        max_bp_iterations: 10,
        osd_order: 0,
    })
    .unwrap();

    assert_eq!(result.physical_error_rate, 0.0);
    assert_eq!(result.num_cycles, 1);
    assert_eq!(result.num_trials, 2);
    assert_eq!(result.num_failed_trials, 0);
}

#[test]
fn effective_models_only_use_basis_specific_logical_rows() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);
    let config = SimulationConfig {
        physical_error_rate: 0.003,
        num_cycles: 1,
        num_trials: 1,
        seed: Some(7),
        max_bp_iterations: 10,
        osd_order: 0,
    };

    let models = build_effective_models(&code, &cycle, &config).unwrap();

    for model in [&models.z_faults, &models.x_faults] {
        let first_logical_row = model.first_logical_row;
        let logical_rows_end = first_logical_row + code.k();
        let logical_rows = model
            .augmented_columns
            .iter()
            .flat_map(|column| column.iter().copied())
            .filter(|&row| row >= first_logical_row)
            .collect::<Vec<_>>();

        assert!(
            !logical_rows.is_empty(),
            "expected at least one augmented column with logical support"
        );
        assert!(logical_rows.iter().all(|&row| row < logical_rows_end));
        assert!(
            model
                .augmented_columns
                .iter()
                .any(|column| column.iter().any(|&row| row >= first_logical_row))
        );
    }
}

#[test]
fn bb_circuit_bposd_timing_counters_partition_decode_work() {
    let result = run_simulation_for_code(
        "bb90",
        SimulationConfig {
            physical_error_rate: 0.003,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(1),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    let profile = &result.profile;
    assert!(profile.setup_seconds.is_finite());
    assert!(profile.sample_seconds.is_finite());
    assert!(profile.decode_seconds.is_finite());
    assert!(profile.decode_call_count > 0);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    assert_eq!(profile.osd_candidate_count, 0);
    assert!(
        profile.bp_iteration_count >= profile.decode_call_count,
        "bp iterations {} should cover decode calls {} for this nontrivial sampled trial",
        profile.bp_iteration_count,
        profile.decode_call_count
    );

    let row = bb_circuit_bposd_result_row("bb90", &result);
    validate_bposd_profile_result_row(&row).unwrap();
    for key in [
        "setup_seconds",
        "sample_seconds",
        "decode_seconds",
        "bp_seconds",
        "osd_seconds",
        "decode_call_count",
        "z_decode_call_count",
        "x_decode_call_count",
        "bp_iteration_count",
        "osd_use_count",
        "osd_candidate_count",
        "gf2_solve_count",
        "gf2_full_elimination_count",
    ] {
        assert!(row.metrics.contains_key(key), "missing metric {key}");
    }
}

#[test]
fn bb_circuit_bposd_timing_counters_reject_incomplete_rows() {
    let mut result = run_simulation_for_code(
        "bb90",
        SimulationConfig {
            physical_error_rate: 1.0e-12,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(1),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    let mut missing = bb_circuit_bposd_result_row("bb90", &result);
    missing.metrics.remove("decode_call_count");
    assert!(validate_bposd_profile_result_row(&missing).is_err());

    let mut non_finite = bb_circuit_bposd_result_row("bb90", &result);
    non_finite
        .metrics
        .insert("decode_seconds".to_string(), f64::NAN);
    assert!(validate_bposd_profile_result_row(&non_finite).is_err());

    let mut negative = bb_circuit_bposd_result_row("bb90", &result);
    negative.metrics.insert("decode_seconds".to_string(), -1.0);
    assert!(validate_bposd_profile_result_row(&negative).is_err());

    let mut fractional_counter = bb_circuit_bposd_result_row("bb90", &result);
    fractional_counter
        .metrics
        .insert("osd_candidate_count".to_string(), 1.5);
    assert!(validate_bposd_profile_result_row(&fractional_counter).is_err());

    result.profile.x_decode_call_count += 1;
    let mismatched = bb_circuit_bposd_result_row("bb90", &result);
    assert!(validate_bposd_profile_result_row(&mismatched).is_err());
}
