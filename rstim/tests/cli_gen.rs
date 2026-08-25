use std::process::Command;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn gen_repetition_code() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.001",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("R "));
    assert!(s.contains("CX "));
    assert!(s.contains("M "));
    assert!(s.contains("DETECTOR"));
    assert!(s.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_noiseless() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(!s.contains("DEPOLARIZE"));
}

#[test]
fn gen_unknown_code_fails() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "unknown",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn gen_surface_code_atom_loss_is_opt_in_from_cli() {
    use std::fs;

    let atom_loss_path = std::env::temp_dir().join(format!(
        "rstim-surface-atom-loss-{}.stim",
        std::process::id()
    ));
    let depol_only_path = std::env::temp_dir().join(format!(
        "rstim-surface-depol-only-{}.stim",
        std::process::id()
    ));

    let atom_loss = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_loss_probability",
            "0.01",
            "--out",
            atom_loss_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen atom-loss command should run");
    assert!(
        atom_loss.status.success(),
        "atom-loss command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&atom_loss.stdout),
        String::from_utf8_lossy(&atom_loss.stderr)
    );
    let atom_loss_text = fs::read_to_string(&atom_loss_path).expect("atom-loss output exists");
    assert!(
        atom_loss_text.contains("LOSS(0.01)"),
        "atom-loss output should contain LOSS(0.01):\n{atom_loss_text}"
    );
    assert!(
        atom_loss_text.contains("H") && atom_loss_text.contains("CX"),
        "positive control should include Clifford layers:\n{atom_loss_text}"
    );

    let depol_only = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_depolarization",
            "0.01",
            "--out",
            depol_only_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen depolarization command should run");
    assert!(
        depol_only.status.success(),
        "depolarization command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&depol_only.stdout),
        String::from_utf8_lossy(&depol_only.stderr)
    );
    let depol_text = fs::read_to_string(&depol_only_path).expect("depol output exists");
    assert!(depol_text.contains("DEPOLARIZE1(0.01)"));
    assert!(depol_text.contains("DEPOLARIZE2(0.01)"));
    assert!(
        !depol_text.contains("LOSS(0.01)"),
        "depolarization-only generation must not emit loss:\n{depol_text}"
    );

    let _ = fs::remove_file(atom_loss_path);
    let _ = fs::remove_file(depol_only_path);
}

#[test]
fn gen_midswap_is_parseable_and_sampleable_from_cli() {
    let directory = tempfile::tempdir().unwrap();
    let circuit_path = directory.path().join("midswap.stim");
    let shots_path = directory.path().join("midswap.b8");
    let generated = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.002",
            "--operation_loss_probability",
            "0.002",
            "--measurement_loss_probability",
            "0.003",
            "--out",
            circuit_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let text = std::fs::read_to_string(&circuit_path).unwrap();
    assert_eq!(text.matches("# MIDSWAP_SHUTTLE").count(), 2);
    assert_eq!(text.matches("# RSTIM_LOGICAL_FLIP_POINT").count(), 1);
    assert_eq!(
        text.lines().filter(|line| line.starts_with("MRL ")).count(),
        2
    );
    assert_eq!(
        text.lines().filter(|line| line.starts_with("ML ")).count(),
        1
    );
    assert!(text.contains("DEPOLARIZE1(0.002)"));
    assert!(text.contains("DEPOLARIZE2(0.002)"));
    assert!(text.contains("X_ERROR(0.002)"));
    assert!(text.contains("LOSS(0.001)"));
    assert!(text.contains("LOSS(0.003)"));

    let sampled = rstim_cmd()
        .args([
            "sample",
            "--in",
            circuit_path.to_str().unwrap(),
            "--shots",
            "8",
            "--seed",
            "7",
            "--out_format",
            "b8",
            "--out",
            shots_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        sampled.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sampled.stderr)
    );
    assert_eq!(std::fs::metadata(shots_path).unwrap().len(), 8 * 7);
}

#[test]
fn gen_midswap_rejects_invalid_input_without_touching_output() {
    let directory = tempfile::tempdir().unwrap();
    let circuit_path = directory.path().join("midswap.stim");
    for (flag, value, expected_error) in [
        ("--distance", "4", "odd and at least 3"),
        (
            "--operation_loss_probability",
            "NaN",
            "finite and in [0, 1]",
        ),
        (
            "--measurement_loss_probability",
            "1.01",
            "finite and in [0, 1]",
        ),
    ] {
        std::fs::write(&circuit_path, "keep me").unwrap();
        let mut command = rstim_cmd();
        command.args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--rounds",
            "2",
        ]);
        if flag != "--distance" {
            command.args(["--distance", "3"]);
        }
        let output = command
            .args([flag, value, "--out", circuit_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read_to_string(&circuit_path).unwrap(), "keep me");
    }
}

#[test]
fn new_loss_flags_do_not_change_conventional_surface_generation() {
    let base = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.002",
        ])
        .output()
        .unwrap();
    let explicit_zero = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.002",
            "--operation_loss_probability",
            "0",
            "--measurement_loss_probability",
            "0",
        ])
        .output()
        .unwrap();
    assert!(base.status.success());
    assert!(explicit_zero.status.success());
    assert_eq!(base.stdout, explicit_zero.stdout);
}

#[test]
fn gen_common_without_distance_does_not_touch_out() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.stim");
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("distance is required for common generators"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
}

#[test]
fn gen_css_memory_from_sparse_json_files() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    std::fs::write(
        &hx,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
    )
    .unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#).unwrap();
    std::fs::write(
        &obs,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0]]}"#,
    )
    .unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "2",
            "--schedule",
            "greedy",
            "--observables",
            obs.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DETECTOR"));
    assert!(stdout.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_css_memory_requires_hx_and_hz() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(&hx, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();

    for (omitted_arg, provided_arg, provided_path, expected) in [
        ("--hx", "--hz", hz.as_path(), "--hx is required"),
        ("--hz", "--hx", hx.as_path(), "--hz is required"),
    ] {
        let output = rstim_cmd()
            .args([
                "gen",
                "--code",
                "css",
                "--task",
                "memory",
                provided_arg,
                provided_path.to_str().unwrap(),
                "--rounds",
                "1",
            ])
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "omitting {omitted_arg} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "stderr: {stderr}");
    }
}

#[test]
fn gen_css_memory_requires_basis() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    std::fs::write(
        &hx,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
    )
    .unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#).unwrap();
    std::fs::write(
        &obs,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
    )
    .unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--rounds",
            "1",
            "--observables",
            obs.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--basis is required"), "stderr: {stderr}");
}

#[test]
fn gen_css_memory_validation_error_does_not_touch_out() {
    let dir = tempfile::tempdir().unwrap();
    let hz = dir.path().join("hz.json");
    let out = dir.path().join("out.stim");
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--hx is required"), "stderr: {stderr}");
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
}

#[test]
fn gen_css_memory_reports_non_orthogonal_checks() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(&hx, r#"{"format":"dense","rows":[[1]]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"dense","rows":[[1]]}"#).unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CSS X/Z checks are not orthogonal"),
        "stderr: {stderr}"
    );
}

#[test]
fn gen_css_memory_rejects_non_logical_observable_and_preserves_out() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    let out = dir.path().join("out.stim");
    let steane_h =
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
    std::fs::write(&hx, steane_h).unwrap();
    std::fs::write(&hz, steane_h).unwrap();
    std::fs::write(
        &obs,
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0]]}"#,
    )
    .unwrap();
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
            "--observables",
            obs.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("observable 0 is not an X logical"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
}

#[test]
fn gen_rotated_memory_z_explicit_noise_and_loss() {
    use rstim::ir::StimTarget;
    use rstim::parser::parse_lines;

    let directory = tempfile::tempdir().unwrap();
    let circuit_path = directory.path().join("conventional-loss.stim");

    // Positive command: every Pauli-noise channel and both loss channels set
    // to distinct values.
    let generated = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--before_round_data_depolarization",
            "0.011",
            "--after_clifford_depolarization",
            "0.022",
            "--before_measure_flip_probability",
            "0.033",
            "--after_reset_flip_probability",
            "0.044",
            "--operation_loss_probability",
            "0.055",
            "--measurement_loss_probability",
            "0.066",
            "--out",
            circuit_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let text = std::fs::read_to_string(&circuit_path).unwrap();
    let circuit = parse_lines(&text).expect("generated circuit must parse");

    let name_at = |index: usize| circuit[index].name().unwrap_or("<none>");
    let args_at = |index: usize| circuit[index].args().unwrap_or(&[]).to_vec();
    let targets_at = |index: usize| circuit[index].targets().unwrap_or(&[]).to_vec();
    let qubits_at = |index: usize| -> Vec<u32> {
        targets_at(index)
            .iter()
            .map(|target| match target {
                StimTarget::Qubit(q) => *q,
                other => panic!("expected qubit target, got {other:?}"),
            })
            .collect()
    };

    // 1. Each Pauli rate occurs only in its named channel, verified by
    //    instruction position.
    let data_qubits = vec![1, 2, 3, 7, 8, 9, 13, 14, 15];
    let x_ancilla = vec![4, 6, 10, 12];
    for index in 0..circuit.len() {
        let name = name_at(index);
        let args = args_at(index);
        if name == "DEPOLARIZE1" && args == [0.011] {
            assert_eq!(
                name_at(index.wrapping_sub(1)),
                "TICK",
                "before-round data depolarization must open a round"
            );
            assert_eq!(
                qubits_at(index),
                data_qubits,
                "0.011 may only depolarize data qubits before the round"
            );
        }
        if name == "DEPOLARIZE1" && args == [0.022] {
            assert_eq!(
                name_at(index - 1),
                "H",
                "after-Clifford DEPOLARIZE1 must follow H"
            );
            assert_eq!(
                qubits_at(index),
                x_ancilla,
                "0.022 DEPOLARIZE1 may only target X ancilla after H"
            );
        }
        if name == "DEPOLARIZE2" {
            assert_eq!(
                args,
                [0.022],
                "DEPOLARIZE2 only carries the after-Clifford rate"
            );
            assert_eq!(name_at(index - 1), "CX");
            assert_eq!(
                qubits_at(index),
                qubits_at(index - 1),
                "DEPOLARIZE2 targets must match its CX layer"
            );
        }
        if name == "X_ERROR" && args == [0.033] {
            assert!(
                matches!(name_at(index + 1), "MRL" | "ML"),
                "before-measure flips must immediately precede a readout, found {}",
                name_at(index + 1)
            );
        }
        if name == "X_ERROR" && args == [0.044] {
            assert!(
                matches!(name_at(index - 1), "R" | "MRL"),
                "after-reset flips must immediately follow a reset, found {}",
                name_at(index - 1)
            );
        }
        if name == "X_ERROR" {
            assert!(
                args == [0.033] || args == [0.044],
                "X_ERROR may only carry 0.033 or 0.044, found {args:?}"
            );
        }
        if name == "DEPOLARIZE1" {
            assert!(
                args == [0.011] || args == [0.022],
                "DEPOLARIZE1 may only carry 0.011 or 0.022, found {args:?}"
            );
        }
        if name == "LOSS" {
            assert!(
                args == [0.055] || args == [0.0275] || args == [0.066],
                "LOSS may only carry the loss rates, found {args:?}"
            );
        }
    }
    assert_eq!(
        circuit
            .iter()
            .filter(|instr| instr.name() == Some("DEPOLARIZE1") && instr.args() == Some(&[0.011][..]))
            .count(),
        2,
        "one before-round data depolarization layer per round"
    );

    // 2. Operation and measurement loss follow the conventional loss
    //    contract: Mid-SWAP reset/H/two-qubit semantics plus measurement-stage
    //    LOSS before every loss-visible readout.
    for index in 0..circuit.len() {
        match name_at(index) {
            "H" => {
                assert_eq!(name_at(index + 1), "DEPOLARIZE1");
                assert_eq!(name_at(index + 2), "LOSS");
                assert_eq!(args_at(index + 2), [0.055]);
                assert_eq!(qubits_at(index + 2), qubits_at(index));
            }
            "CX" => {
                assert_eq!(name_at(index + 1), "DEPOLARIZE2");
                assert_eq!(name_at(index + 2), "LOSS");
                assert_eq!(
                    args_at(index + 2),
                    [0.0275],
                    "two-qubit operation loss is split across the pair"
                );
                assert_eq!(qubits_at(index + 2), qubits_at(index));
            }
            "MRL" | "ML" => {
                assert_eq!(name_at(index - 2), "LOSS");
                assert_eq!(args_at(index - 2), [0.066]);
                assert_eq!(qubits_at(index - 2), qubits_at(index));
                assert_eq!(name_at(index - 1), "X_ERROR");
            }
            _ => {}
        }
    }
    assert_eq!(
        text.lines().filter(|line| line.starts_with("MRL ")).count(),
        2
    );
    assert_eq!(
        text.lines().filter(|line| line.starts_with("ML ")).count(),
        1
    );

    // 3. Measurements are loss-visible: every DETECTOR/OBSERVABLE_INCLUDE
    //    record reference lands on a value bit (odd distance back from an
    //    even-sized record block boundary) and stays in range.
    let mut record_count = 0_i64;
    for instruction in &circuit {
        let name = instruction.name().unwrap();
        let targets = instruction.targets().unwrap();
        if matches!(name, "DETECTOR" | "OBSERVABLE_INCLUDE") {
            for target in targets {
                let StimTarget::Rec(offset) = target else {
                    panic!("{name} contained a non-record target: {target:?}");
                };
                assert!(
                    offset % 2 == -1,
                    "{name} referenced a loss-flag record via rec[{offset}]"
                );
                let referenced = record_count + i64::from(*offset);
                assert!(
                    (0..record_count).contains(&referenced),
                    "{name} used invalid rec[{offset}] after {record_count} records"
                );
            }
        }
        if matches!(name, "MRL" | "ML") {
            record_count += 2 * targets.len() as i64;
        }
    }
    assert_eq!(record_count, 50);

    // Record counts must remain valid for blinded dataset export.
    let public_out = directory.path().join("public");
    let private_out = directory.path().join("private");
    let exported = rstim_cmd()
        .args([
            "export_decoder_dataset",
            "--circuit",
            circuit_path.to_str().unwrap(),
            "--shots",
            "8",
            "--mode",
            "measurements_blinded",
            "--logical_x_qubits",
            "1,2,3",
            "--public_out",
            public_out.to_str().unwrap(),
            "--private_out",
            private_out.to_str().unwrap(),
            "--seed",
            "7",
        ])
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "measurements_blinded export failed\nstderr: {}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(public_out.join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["circuit"]["measurements"], 50);
    assert_eq!(manifest["row"]["bits"], 50);

    // 4. Round 0 and round 1 use the same fixed CNOT layer order.
    let cx_layers: Vec<Vec<u32>> = circuit
        .iter()
        .enumerate()
        .filter(|(_, instr)| instr.name() == Some("CX"))
        .map(|(index, _)| qubits_at(index))
        .collect();
    assert_eq!(cx_layers.len(), 8, "two rounds of four CX layers");
    assert_eq!(
        &cx_layers[..4],
        &cx_layers[4..],
        "conventional control must keep the fixed CNOT layer order in every round"
    );

    // 5. Exactly one logical-flip marker and zero shuttle/remapping events.
    assert_eq!(text.matches("# RSTIM_LOGICAL_FLIP_POINT").count(), 1);
    assert!(!text.contains("MIDSWAP_SHUTTLE"));
    assert!(!text.contains("SHUTTLE"));
    let lines: Vec<&str> = text.lines().collect();
    let data_reset = lines
        .iter()
        .position(|line| line.starts_with("R "))
        .unwrap();
    assert_eq!(lines[data_reset + 1], "# RSTIM_LOGICAL_FLIP_POINT");

    // The circuit must also be sampleable.
    let shots_path = directory.path().join("shots.b8");
    let sampled = rstim_cmd()
        .args([
            "sample",
            "--in",
            circuit_path.to_str().unwrap(),
            "--shots",
            "8",
            "--seed",
            "7",
            "--out_format",
            "b8",
            "--out",
            shots_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        sampled.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sampled.stderr)
    );
    assert_eq!(std::fs::metadata(&shots_path).unwrap().len(), 8 * 7);

    // Negative control 1: only --after_clifford_depolarization touches the
    // after-Clifford channel; nothing is broadcast and no LOSS appears.
    let depol_only_path = directory.path().join("depol-only.stim");
    let depol_only = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.022",
            "--out",
            depol_only_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        depol_only.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&depol_only.stderr)
    );
    let depol_text = std::fs::read_to_string(&depol_only_path).unwrap();
    assert!(depol_text.contains("DEPOLARIZE1(0.022)"));
    assert!(depol_text.contains("DEPOLARIZE2(0.022)"));
    assert!(
        !depol_text.contains("X_ERROR(0.022)"),
        "after_clifford_depolarization must not broadcast into flip channels"
    );
    assert!(
        !depol_text.contains("LOSS"),
        "no loss may appear without explicit loss flags"
    );
    let depol_circuit = parse_lines(&depol_text).unwrap();
    for (index, instruction) in depol_circuit.iter().enumerate() {
        if instruction.name() == Some("DEPOLARIZE1") {
            let targets: Vec<u32> = instruction
                .targets()
                .unwrap()
                .iter()
                .map(|target| match target {
                    StimTarget::Qubit(q) => *q,
                    other => panic!("expected qubit target, got {other:?}"),
                })
                .collect();
            assert!(
                targets.iter().all(|q| x_ancilla.contains(q)),
                "DEPOLARIZE1(0.022) at instruction {index} targeted data qubits: {targets:?}"
            );
        }
    }

    // Negative control 2: there is no uniform CLI shortcut.
    let uniform = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--uniform_noise",
            "0.022",
        ])
        .output()
        .unwrap();
    assert!(
        !uniform.status.success(),
        "--uniform_noise must be rejected"
    );
    assert!(
        String::from_utf8_lossy(&uniform.stderr).contains("unexpected argument"),
        "stderr: {}",
        String::from_utf8_lossy(&uniform.stderr)
    );
}

#[test]
fn gen_css_uses_explicit_noise_channels() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(
        &hx,
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#,
    )
    .unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":7,"rows":[]}"#).unwrap();

    // Only the after-Clifford channel set: nothing may broadcast into the
    // reset or measurement flip channels.
    let depol_only = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.123",
        ])
        .output()
        .unwrap();
    assert!(
        depol_only.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&depol_only.stderr)
    );
    let depol_text = String::from_utf8(depol_only.stdout).unwrap();
    assert!(depol_text.contains("DEPOLARIZE"));
    assert!(
        !depol_text.contains("X_ERROR(0.123)"),
        "after_clifford_depolarization must not broadcast into CSS flip channels:\n{depol_text}"
    );

    // Explicitly named channels land in their own slots.
    let explicit = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "2",
            "--before_measure_flip_probability",
            "0.05",
            "--after_reset_flip_probability",
            "0.06",
        ])
        .output()
        .unwrap();
    assert!(
        explicit.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    let explicit_text = String::from_utf8(explicit.stdout).unwrap();
    assert!(explicit_text.contains("X_ERROR(0.05)"));
    assert!(explicit_text.contains("X_ERROR(0.06)"));
    assert!(!explicit_text.contains("DEPOLARIZE"));
}

#[test]
fn gen_rejects_out_of_range_probabilities() {
    for (flag, value) in [
        ("--before_round_data_depolarization", "2"),
        ("--after_clifford_depolarization", "NaN"),
        ("--before_measure_flip_probability", "-0.1"),
        ("--after_reset_flip_probability", "1.5"),
        ("--after_clifford_loss_probability", "2"),
        ("--before_measure_flip_probability", "inf"),
    ] {
        let output = rstim_cmd()
            .args([
                "gen",
                "--code",
                "repetition_code",
                "--task",
                "memory",
                "--distance",
                "3",
                "--rounds",
                "1",
                &format!("{flag}={value}"),
            ])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "{flag} {value} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("finite and in [0, 1]"),
            "{flag} {value}: stderr: {stderr}"
        );
    }
}
