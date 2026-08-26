use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_trace_export(circuit: &Path, root: &Path, suffix: &str) -> (Output, [PathBuf; 3]) {
    let detections = root.join(format!("detections-{suffix}.b8"));
    let observables = root.join(format!("observables-{suffix}.b8"));
    let trace = root.join(format!("trace-{suffix}.jsonl"));
    let output = rstim_cmd()
        .args([
            "detect",
            "--in",
            circuit.to_str().unwrap(),
            "--shots",
            "2",
            "--seed",
            "7",
            "--out",
            detections.to_str().unwrap(),
            "--out_format",
            "b8",
            "--obs_out",
            observables.to_str().unwrap(),
            "--obs_out_format",
            "b8",
            "--trace_out",
            trace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    (output, [detections, observables, trace])
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_no_transaction_files(root: &Path) {
    let leaked: Vec<_> = fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.contains(".rstim-") && (name.ends_with(".tmp") || name.ends_with(".bak"))
        })
        .collect();
    assert!(leaked.is_empty(), "transaction files leaked: {leaked:?}");
}

#[test]
fn trace_export_contract() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("training.stim");
    fs::write(
        &circuit,
        concat!(
            "R 0 1\n",
            "X_ERROR(1) 0\n",
            "LOSS(1) 1\n",
            "X_ERROR(1) 1\n",
            "M 0 1\n",
            "DETECTOR rec[-2]\n",
            "DETECTOR rec[-1]\n",
            "OBSERVABLE_INCLUDE(0) rec[-2]\n",
        ),
    )
    .unwrap();

    let (first_output, first_paths) = run_trace_export(&circuit, root.path(), "a");
    assert_success(&first_output, "first trace export");
    let [detections, observables, trace] = &first_paths;
    assert_eq!(fs::read(detections).unwrap(), [0b0000_0011, 0b0000_0011]);
    assert_eq!(fs::read(observables).unwrap(), [1, 1]);

    let lines: Vec<Value> = fs::read_to_string(trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["record_type"], "manifest");
    assert_eq!(lines[0]["schema_version"], "rstim.sample_trace.v1");
    assert_eq!(lines[0]["seed"], 7);
    assert_eq!(lines[0]["shots"], 2);
    assert_eq!(lines[0]["num_measurements"], 2);
    assert_eq!(lines[0]["num_detectors"], 2);
    assert_eq!(lines[0]["num_observables"], 1);
    assert_eq!(lines[0]["circuit_sha256"].as_str().unwrap().len(), 64);

    for (shot_index, shot) in lines[1..].iter().enumerate() {
        assert_eq!(shot["record_type"], "shot");
        assert_eq!(shot["shot_index"], shot_index);
        assert_eq!(shot["measurements"], serde_json::json!([true, true]));
        assert_eq!(shot["detectors"], serde_json::json!([true, true]));
        assert_eq!(shot["observables"], serde_json::json!([true]));

        let noise = shot["noise_events"].as_array().unwrap();
        assert!(noise.iter().any(|event| {
            event["instr_name"] == "X_ERROR"
                && event["target_qubits"] == serde_json::json!([0])
                && event["occurred"] == true
                && event["branch_label"] == "X"
        }));
        assert!(noise.iter().any(|event| {
            event["instr_name"] == "LOSS"
                && event["target_qubits"] == serde_json::json!([1])
                && event["occurred"] == true
                && event["branch_label"] == "L"
        }));
        assert!(
            shot["measurement_events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["target_qubit"] == 1 && event["loss_cause"] == true)
        );
        assert!(
            shot["inapplicable_noise_events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["op_path"] == serde_json::json!([3]))
        );
    }

    let (second_output, second_paths) = run_trace_export(&circuit, root.path(), "b");
    assert_success(&second_output, "second trace export");
    for (first, second) in first_paths.iter().zip(&second_paths) {
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    }

    let loader = Path::new(env!("CARGO_MANIFEST_DIR")).join("doc/examples/load_training_data.py");
    let loader_output = Command::new("python3")
        .args([
            loader.to_str().unwrap(),
            "--detectors",
            detections.to_str().unwrap(),
            "--observables",
            observables.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&loader_output, "training-data Python loader");
    assert_eq!(
        String::from_utf8(loader_output.stdout).unwrap().trim(),
        "PASS training alignment shots=2"
    );

    let collision = root.path().join("collision.bin");
    fs::write(&collision, b"sentinel-collision").unwrap();
    let collision_output = rstim_cmd()
        .args([
            "detect",
            "--in",
            circuit.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            collision.to_str().unwrap(),
            "--trace_out",
            collision.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!collision_output.status.success());
    assert_eq!(fs::read(&collision).unwrap(), b"sentinel-collision");

    let malformed = root.path().join("malformed.stim");
    fs::write(&malformed, "REPEAT 2 {\nM 0\n").unwrap();
    let malformed_paths = [
        root.path().join("malformed-dets.b8"),
        root.path().join("malformed-obs.b8"),
        root.path().join("malformed-trace.jsonl"),
    ];
    for path in &malformed_paths {
        fs::write(path, b"sentinel-malformed").unwrap();
    }
    let malformed_output = rstim_cmd()
        .args([
            "detect",
            "--in",
            malformed.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            malformed_paths[0].to_str().unwrap(),
            "--obs_out",
            malformed_paths[1].to_str().unwrap(),
            "--trace_out",
            malformed_paths[2].to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!malformed_output.status.success());
    for path in &malformed_paths {
        assert_eq!(fs::read(path).unwrap(), b"sentinel-malformed");
    }

    let invalid_format_paths = [
        root.path().join("invalid-format-dets.b8"),
        root.path().join("invalid-format-obs.b8"),
        root.path().join("invalid-format-trace.jsonl"),
    ];
    for path in &invalid_format_paths {
        fs::write(path, b"sentinel-format").unwrap();
    }
    let invalid_format_output = rstim_cmd()
        .args([
            "detect",
            "--in",
            circuit.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            invalid_format_paths[0].to_str().unwrap(),
            "--obs_out",
            invalid_format_paths[1].to_str().unwrap(),
            "--obs_out_format",
            "dets",
            "--trace_out",
            invalid_format_paths[2].to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!invalid_format_output.status.success());
    assert!(
        String::from_utf8_lossy(&invalid_format_output.stderr).contains("--obs_out_format=dets")
    );
    for path in &invalid_format_paths {
        assert_eq!(fs::read(path).unwrap(), b"sentinel-format");
    }

    for existing_destinations in [false, true] {
        let max_rename = if existing_destinations { 6 } else { 3 };
        for fail_at in 1..=max_rename {
            let prefix = format!(
                "rename-{}-{fail_at}",
                if existing_destinations {
                    "existing"
                } else {
                    "absent"
                }
            );
            let paths = [
                root.path().join(format!("{prefix}-dets.b8")),
                root.path().join(format!("{prefix}-obs.b8")),
                root.path().join(format!("{prefix}-trace.jsonl")),
            ];
            if existing_destinations {
                for (index, path) in paths.iter().enumerate() {
                    fs::write(path, format!("sentinel-{index}")).unwrap();
                }
            }
            let output = rstim_cmd()
                .args([
                    "detect",
                    "--in",
                    circuit.to_str().unwrap(),
                    "--shots",
                    "1",
                    "--out",
                    paths[0].to_str().unwrap(),
                    "--obs_out",
                    paths[1].to_str().unwrap(),
                    "--trace_out",
                    paths[2].to_str().unwrap(),
                ])
                .env("RSTIM_TEST_RSMP_FAIL_RENAME_AT", fail_at.to_string())
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "rename {fail_at} unexpectedly succeeded"
            );
            for (index, path) in paths.iter().enumerate() {
                if existing_destinations {
                    assert_eq!(
                        fs::read_to_string(path).unwrap(),
                        format!("sentinel-{index}")
                    );
                } else {
                    assert!(!path.exists(), "{} was partially published", path.display());
                }
            }
            assert_no_transaction_files(root.path());
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let real = root.path().join("real");
        let alias = root.path().join("alias");
        fs::create_dir(&real).unwrap();
        symlink(&real, &alias).unwrap();

        let shared = real.join("shared-output");
        fs::write(&shared, b"sentinel-symlink").unwrap();
        let alias_output = alias.join("shared-output");
        let symlink_collision = rstim_cmd()
            .args([
                "detect",
                "--in",
                circuit.to_str().unwrap(),
                "--shots",
                "1",
                "--out",
                shared.to_str().unwrap(),
                "--trace_out",
                alias_output.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!symlink_collision.status.success());
        assert_eq!(fs::read(&shared).unwrap(), b"sentinel-symlink");

        let child = real.join("child");
        let traversal_base = root.path().join("traversal-base");
        fs::create_dir(&child).unwrap();
        fs::create_dir(&traversal_base).unwrap();
        symlink(&child, traversal_base.join("link")).unwrap();
        let traversal_alias = traversal_base.join("link/../shared-output");
        let traversal_collision = rstim_cmd()
            .args([
                "detect",
                "--in",
                circuit.to_str().unwrap(),
                "--shots",
                "1",
                "--out",
                shared.to_str().unwrap(),
                "--trace_out",
                traversal_alias.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!traversal_collision.status.success());
        assert_eq!(fs::read(&shared).unwrap(), b"sentinel-symlink");

        let aliased_input = alias.join("input.stim");
        let real_input = real.join("input.stim");
        fs::copy(&circuit, &real_input).unwrap();
        let before_input = fs::read(&real_input).unwrap();
        let input_collision = rstim_cmd()
            .args([
                "detect",
                "--in",
                real_input.to_str().unwrap(),
                "--shots",
                "1",
                "--out",
                aliased_input.to_str().unwrap(),
                "--trace_out",
                real.join("input-collision-trace.jsonl").to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!input_collision.status.success());
        assert_eq!(fs::read(&real_input).unwrap(), before_input);
        assert_no_transaction_files(root.path());
    }

    println!(
        "PASS trace export shots=2 manifest=1 deterministic=1 aligned=1 loss=1 negative_cases=2"
    );
}
