use std::io::Write;
use std::process::{Command, Stdio};

fn rustqec_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rustqec"))
}

fn run_with_stdin(args: &[&str], input: &str) -> std::process::Output {
    let mut child = rustqec_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_json(output: &std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn stderr_json(output: &std::process::Output) -> serde_json::Value {
    assert!(output.stdout.is_empty());
    serde_json::from_slice(&output.stderr).unwrap()
}

#[test]
fn circuit_gen_midswap_writes_a_loss_visible_circuit() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("midswap.stim");
    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--noise",
            "0.001",
            "--operation-loss-probability",
            "0.005",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    assert_eq!(value["command"], "circuit.gen");
    assert_eq!(value["result"]["task"], "rotated_memory_z_midswap");
    assert_eq!(
        value["artifacts"],
        serde_json::json!([out.display().to_string()])
    );

    let circuit = std::fs::read_to_string(&out).unwrap();
    assert!(circuit.contains("MRL"));
    assert!(circuit.contains("TICK[rstim:logical_flip_point]"));
}

#[test]
fn circuit_gen_rejects_unknown_code_and_misplaced_loss_flags() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.stim");
    for args in [
        vec![
            "circuit",
            "gen",
            "--code",
            "nope",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
        ],
        vec![
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--operation-loss-probability",
            "0.1",
        ],
        vec![
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--before-round-data-loss",
            "0.1",
        ],
    ] {
        let mut full = args.clone();
        full.extend(["--out", out.to_str().unwrap()]);
        let output = rustqec_cmd().args(&full).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "args: {args:?}");
        let value = stderr_json(&output);
        assert_eq!(value["command"], "circuit.gen", "args: {args:?}");
        assert_eq!(
            value["error"]["code"], "invalid_arguments",
            "args: {args:?}"
        );
        assert!(!out.exists(), "args: {args:?}");
    }
}

#[test]
fn circuit_gen_rotated_memory_z_with_loss_routes_to_loss_visible_builder() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("loss.stim");
    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--before-round-data-depolarization",
            "0.001",
            "--noise",
            "0.002",
            "--before-measure-flip-probability",
            "0.003",
            "--after-reset-flip-probability",
            "0.004",
            "--after-clifford-loss-probability",
            "0.005",
            "--operation-loss-probability",
            "0.006",
            "--measurement-loss-probability",
            "0.007",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    assert_eq!(value["command"], "circuit.gen");
    assert_eq!(value["result"]["task"], "rotated_memory_z");

    let circuit = std::fs::read_to_string(&out).unwrap();
    assert!(circuit.contains("MRL"));
    assert!(circuit.contains("ML"));
    assert!(circuit.contains("LOSS(0.005)"));
    assert!(circuit.contains("TICK[rstim:logical_flip_point]"));
    // This generator path emits explicit detector time coordinates. The
    // decoder also accepts SHIFT_COORDS from earlier RStim revisions.
    assert!(!circuit.contains("SHIFT_COORDS"));
}

#[test]
fn circuit_gen_noise_flag_only_drives_the_after_clifford_channel() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("noise.stim");
    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--noise",
            "0.1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    stdout_json(&output);

    let circuit = std::fs::read_to_string(&out).unwrap();
    assert!(circuit.contains("DEPOLARIZE1(0.1)"));
    assert!(circuit.contains("DEPOLARIZE2(0.1)"));
    // --noise must not broadcast into the reset/measurement flip channels.
    assert!(!circuit.contains("X_ERROR(0.1)"));
}

#[test]
fn circuit_gen_rejects_out_of_range_probabilities() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.stim");
    for (flag, value) in [
        ("--before-measure-flip-probability", "2"),
        ("--before-round-data-loss-probability", "2"),
        ("--noise", "nan"),
        ("--after-reset-flip-probability", "1.5"),
    ] {
        let output = rustqec_cmd()
            .args([
                "circuit",
                "gen",
                "--code",
                "surface_code",
                "--task",
                "rotated_memory_z",
                "--distance",
                "3",
                "--rounds",
                "2",
                flag,
                value,
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag} {value}");
        let json = stderr_json(&output);
        assert_eq!(json["command"], "circuit.gen", "{flag} {value}");
        assert_eq!(json["error"]["code"], "invalid_arguments", "{flag} {value}");
        assert!(!out.exists(), "{flag} {value}");
    }
}

#[test]
fn circuit_gen_midswap_applies_the_explicit_noise_channel_flags() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.stim");
    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--before-round-data-depolarization",
            "0.01",
            "--before-round-data-loss",
            "0.05",
            "--noise",
            "0.02",
            "--before-measure-flip-probability",
            "0.03",
            "--after-reset-flip-probability",
            "0.04",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let circuit = std::fs::read_to_string(out).unwrap();
    assert!(circuit.contains("DEPOLARIZE1(0.01)"));
    assert!(circuit.contains("DEPOLARIZE1(0.02)"));
    assert!(circuit.contains("DEPOLARIZE2(0.02)"));
    assert!(circuit.contains("X_ERROR(0.03)"));
    assert!(circuit.contains("X_ERROR(0.04)"));
    assert!(!circuit.contains("X_ERROR(0.02)"));
    assert!(circuit.contains("LOSS(0.05)"));
}

#[test]
fn circuit_sample_writes_b8_with_the_expected_row_width() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("shots.b8");
    let output = run_with_stdin(
        &[
            "circuit",
            "sample",
            "--shots",
            "10",
            "--out-format",
            "b8",
            "--seed",
            "7",
            "--out",
            out.to_str().unwrap(),
        ],
        "X_ERROR(0.1) 0\nM 0\nM 1\nDETECTOR rec[-2]\n",
    );
    let value = stdout_json(&output);
    assert_eq!(value["command"], "circuit.sample");
    assert_eq!(value["result"]["shots"], 10);
    assert_eq!(value["result"]["num_measurements"], 2);
    // 10 shots * ceil(2 measurement bits / 8) = 10 bytes.
    assert_eq!(std::fs::metadata(&out).unwrap().len(), 10);
}

#[test]
fn circuit_sample_seeded_runs_are_reproducible() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("a.b8");
    let second = dir.path().join("b.b8");
    for path in [&first, &second] {
        let output = run_with_stdin(
            &[
                "circuit",
                "sample",
                "--shots",
                "32",
                "--out-format",
                "b8",
                "--seed",
                "42",
                "--out",
                path.to_str().unwrap(),
            ],
            "X_ERROR(0.2) 0\nM 0\n",
        );
        assert!(output.status.success());
    }
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
}

#[test]
fn circuit_sample_invalid_circuit_and_missing_input_use_stable_codes() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("shots.b8");

    let output = run_with_stdin(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            out.to_str().unwrap(),
        ],
        "NOT_A_GATE 0\n",
    );
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["command"], "circuit.sample");
    assert_eq!(value["error"]["code"], "invalid_circuit");

    let missing = dir.path().join("missing.stim");
    let output = rustqec_cmd()
        .args([
            "circuit",
            "sample",
            "--shots",
            "1",
            "--in",
            missing.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "input_error");
}

#[test]
fn circuit_detect_writes_detections_and_separate_observables() {
    let dir = tempfile::tempdir().unwrap();
    let dets = dir.path().join("dets.b8");
    let obs = dir.path().join("obs.01");
    let output = run_with_stdin(
        &[
            "circuit",
            "detect",
            "--shots",
            "5",
            "--out-format",
            "b8",
            "--seed",
            "3",
            "--out",
            dets.to_str().unwrap(),
            "--obs-out",
            obs.to_str().unwrap(),
        ],
        "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let value = stdout_json(&output);
    assert_eq!(value["command"], "circuit.detect");
    assert_eq!(value["result"]["num_detectors"], 1);
    assert_eq!(
        value["result"]["observables_out"],
        serde_json::json!(obs.display().to_string())
    );
    assert_eq!(std::fs::metadata(&dets).unwrap().len(), 5);
    let obs_text = std::fs::read_to_string(&obs).unwrap();
    assert_eq!(obs_text.lines().count(), 5);
}

#[test]
fn circuit_dem_extracts_a_detector_error_model() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("model.dem");
    let output = run_with_stdin(
        &["circuit", "dem", "--out", out.to_str().unwrap()],
        "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n",
    );
    let value = stdout_json(&output);
    assert_eq!(value["command"], "circuit.dem");
    assert_eq!(value["result"]["num_detectors"], 1);
    let dem = std::fs::read_to_string(&out).unwrap();
    assert!(dem.contains("error(0.1)"), "dem: {dem}");
}

#[test]
fn dataset_export_then_decode_closes_the_midswap_loop() {
    let dir = tempfile::tempdir().unwrap();
    let circuit = dir.path().join("midswap.stim");
    let public = dir.path().join("public");
    let private = dir.path().join("private");
    let predictions = dir.path().join("preds.b8");
    let stats = dir.path().join("stats.json");

    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--noise",
            "0.001",
            "--operation-loss-probability",
            "0.005",
            "--out",
            circuit.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = rustqec_cmd()
        .args([
            "dataset",
            "export",
            "--circuit",
            circuit.to_str().unwrap(),
            "--shots",
            "8",
            "--mode",
            "measurements_blinded",
            "--public-out",
            public.to_str().unwrap(),
            "--private-out",
            private.to_str().unwrap(),
            "--seed",
            "7",
            "--logical-x-qubits",
            "1,8,15",
        ])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    assert_eq!(value["command"], "dataset.export");
    assert_eq!(value["result"]["mode"], "measurements_blinded");
    for file in ["manifest.json", "circuit.stim", "shots.b8"] {
        assert!(public.join(file).exists(), "missing public/{file}");
    }

    let output = rustqec_cmd()
        .args([
            "decode",
            "--decoder",
            "envelope-matching",
            "--dataset",
            public.to_str().unwrap(),
            "--out",
            predictions.to_str().unwrap(),
            "--stats-out",
            stats.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // 8 shots * 1 observable bit = 8 bytes.
    assert_eq!(std::fs::metadata(&predictions).unwrap().len(), 8);
    let stats: serde_json::Value = serde_json::from_slice(&std::fs::read(&stats).unwrap()).unwrap();
    assert_eq!(stats["shot_count"], 8);
}

#[test]
fn dataset_export_rejects_unknown_mode_and_missing_circuit() {
    let dir = tempfile::tempdir().unwrap();
    let circuit = dir.path().join("circuit.stim");
    std::fs::write(&circuit, "M 0\nDETECTOR rec[-1]\n").unwrap();
    let public = dir.path().join("public");
    let private = dir.path().join("private");

    let output = rustqec_cmd()
        .args([
            "dataset",
            "export",
            "--circuit",
            circuit.to_str().unwrap(),
            "--shots",
            "4",
            "--mode",
            "nope",
            "--public-out",
            public.to_str().unwrap(),
            "--private-out",
            private.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["command"], "dataset.export");
    assert_eq!(value["error"]["code"], "invalid_arguments");

    let missing = dir.path().join("missing.stim");
    let output = rustqec_cmd()
        .args([
            "dataset",
            "export",
            "--circuit",
            missing.to_str().unwrap(),
            "--shots",
            "4",
            "--mode",
            "detectors",
            "--public-out",
            public.to_str().unwrap(),
            "--private-out",
            private.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "input_error");
}

#[test]
fn dataset_export_error_trace_writes_private_trace_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let circuit = dir.path().join("circuit.stim");
    std::fs::write(
        &circuit,
        "R 0\nX_ERROR(0.5) 0\nLOSS(0.5) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public = dir.path().join("public");
    let private = dir.path().join("private");

    let output = rustqec_cmd()
        .args([
            "dataset",
            "export",
            "--circuit",
            circuit.to_str().unwrap(),
            "--shots",
            "8",
            "--mode",
            "detectors",
            "--public-out",
            public.to_str().unwrap(),
            "--private-out",
            private.to_str().unwrap(),
            "--seed",
            "7",
            "--error-trace",
        ])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    assert_eq!(value["command"], "dataset.export");

    let trace = std::fs::read_to_string(private.join("trace.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = trace
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 8, "one trace line per shot");
    for (shot, line) in lines.iter().enumerate() {
        assert_eq!(line["schema_version"], "rstim.error-trace.v1");
        assert_eq!(line["shot"], shot);
    }
    assert!(
        lines.iter().any(|line| line["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["branch"] == "L")),
        "expected at least one heralding loss event"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(private.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["trace_file"]["file"], "trace.jsonl");
    assert_eq!(manifest["trace_file"]["schema"], "rstim.error-trace.v1");
    assert!(!public.join("trace.jsonl").exists());
}

#[test]
fn pipeline_commands_default_to_json_and_support_human() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("circuit.stim");

    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--noise",
            "0.01",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    assert_eq!(value["schema_version"], "rustqec.cli.v1");

    let output = rustqec_cmd()
        .args([
            "circuit",
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--noise",
            "0.01",
            "--out",
            out.to_str().unwrap(),
            "--format",
            "human",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("status: ok\n"), "text: {text}");
    assert!(text.contains("code: repetition_code"), "text: {text}");
    assert!(serde_json::from_slice::<serde_json::Value>(&text.into_bytes()).is_err());
}

#[test]
fn capabilities_lists_the_pipeline_verbs_with_contracts() {
    let output = rustqec_cmd()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    let value = stdout_json(&output);
    let commands = value["commands"].as_array().unwrap();
    for (name, argv) in [
        ("circuit.gen", serde_json::json!(["circuit", "gen"])),
        ("circuit.sample", serde_json::json!(["circuit", "sample"])),
        ("circuit.detect", serde_json::json!(["circuit", "detect"])),
        ("circuit.dem", serde_json::json!(["circuit", "dem"])),
        ("dataset.export", serde_json::json!(["dataset", "export"])),
        ("dataset.import", serde_json::json!(["dataset", "import"])),
    ] {
        let entry = commands
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("missing capability {name}"));
        assert_eq!(entry["argv"], argv, "capability {name}");
        assert_eq!(entry["success_exit_code"], 0, "capability {name}");
        assert!(
            !entry["artifacts"].as_array().unwrap().is_empty(),
            "capability {name} declares no artifacts"
        );
        let errors = entry["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|error| error["code"] == "invalid_arguments"),
            "capability {name} lacks invalid_arguments"
        );
    }
    let export = commands
        .iter()
        .find(|entry| entry["name"] == "dataset.export")
        .unwrap();
    let export_errors = export["errors"].as_array().unwrap();
    assert!(
        !export_errors
            .iter()
            .any(|error| error["code"] == "output_error"),
        "dataset.export performs no direct file writes; output_error is unreachable"
    );
    let gen_entry = commands
        .iter()
        .find(|entry| entry["name"] == "circuit.gen")
        .unwrap();
    let gen_arguments = gen_entry["arguments"].as_array().unwrap();
    for flag in [
        "--after-clifford-loss-probability",
        "--before-round-data-depolarization",
        "--before-measure-flip-probability",
        "--after-reset-flip-probability",
    ] {
        assert!(
            gen_arguments.iter().any(|argument| argument["flag"] == flag),
            "circuit.gen must declare {flag}"
        );
    }
    let mode = export["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|argument| argument["name"] == "mode")
        .unwrap();
    assert_eq!(
        mode["values"],
        serde_json::json!(["detectors", "measurements_blinded"])
    );
    let trace_argument = export["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|argument| argument["name"] == "error_trace")
        .expect("dataset.export must declare --error-trace");
    assert_eq!(trace_argument["flag"], "--error-trace");
    assert_eq!(trace_argument["required"], false);
    assert_eq!(trace_argument["default"], "false");
}

#[test]
fn unknown_output_format_is_invalid_arguments_not_execution_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("shots.out");
    let output = run_with_stdin(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out-format",
            "yaml",
            "--out",
            out.to_str().unwrap(),
        ],
        "M 0\n",
    );
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["command"], "circuit.sample");
    assert_eq!(value["error"]["code"], "invalid_arguments");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown output format")
    );
    assert!(!out.exists());
}
