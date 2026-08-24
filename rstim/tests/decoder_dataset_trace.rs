use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_cli(args: &[String]) -> Output {
    rstim_cmd().args(args).output().expect("run rstim")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn export_args(
    circuit: &Path,
    mode: &str,
    shots: usize,
    public_out: &Path,
    private_out: &Path,
    extra: &[&str],
) -> Vec<String> {
    let mut args = vec![
        "export_decoder_dataset".to_string(),
        "--circuit".to_string(),
        circuit.display().to_string(),
        "--shots".to_string(),
        shots.to_string(),
        "--mode".to_string(),
        mode.to_string(),
        "--public_out".to_string(),
        public_out.display().to_string(),
        "--private_out".to_string(),
        private_out.display().to_string(),
    ];
    args.extend(extra.iter().map(|value| value.to_string()));
    args
}

/// Reads the single row bit of a shot in a 1-bit-wide b8 table
/// (bytes_per_shot is 1, so each shot occupies one byte, LSB first).
fn b8_bit(bytes: &[u8], shot: usize) -> bool {
    bytes[shot] & 1 == 1
}

fn trace_lines(private_out: &Path, shots: usize) -> Vec<Value> {
    let text = fs::read_to_string(private_out.join("trace.jsonl")).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), shots, "one trace line per shot");
    for (shot, line) in lines.iter().enumerate() {
        assert_eq!(line["schema_version"], "rstim.error-trace.v1");
        assert_eq!(line["shot"], shot);
        assert!(line["events"].is_array());
    }
    lines
}

fn event_ops(line: &Value) -> Vec<&str> {
    line["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["op"].as_str().unwrap())
        .collect()
}

fn assert_manifest_trace_entry(private_out: &Path, shots: usize) {
    let manifest: Value =
        serde_json::from_slice(&fs::read(private_out.join("manifest.json")).unwrap()).unwrap();
    let entry = &manifest["trace_file"];
    assert_eq!(entry["file"], "trace.jsonl");
    assert_eq!(entry["schema"], "rstim.error-trace.v1");
    assert_eq!(entry["lines"], shots);
    let bytes = fs::read(private_out.join("trace.jsonl")).unwrap();
    assert_eq!(
        entry["sha256"].as_str().unwrap(),
        rstim::decoder_dataset::sha256_hex(&bytes)
    );
}

#[test]
fn detectors_trace_records_pauli_and_loss_events_per_shot() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(0.5) 0\nLOSS(0.5) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let shots = 32;
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            shots,
            &public_out,
            &private_out,
            &["--seed", "7", "--error_trace"],
        )),
        "detectors trace export",
    );

    let shots_bytes = fs::read(public_out.join("shots.b8")).unwrap();
    let answers_bytes = fs::read(private_out.join("answers.b8")).unwrap();
    let lines = trace_lines(&private_out, shots);
    let mut saw_x = false;
    let mut saw_loss = false;
    for (shot, line) in lines.iter().enumerate() {
        let ops = event_ops(line);
        saw_x |= ops.contains(&"X_ERROR");
        saw_loss |= ops.contains(&"LOSS");
        // R resets to 0; an X error flips the readout, and a lost qubit reads 1.
        let expected = ops.contains(&"X_ERROR") || ops.contains(&"LOSS");
        assert_eq!(
            b8_bit(&shots_bytes, shot),
            expected,
            "shot {shot} detector row disagrees with trace events {ops:?}"
        );
        assert_eq!(
            b8_bit(&answers_bytes, shot),
            b8_bit(&shots_bytes, shot),
            "shot {shot} answer disagrees with the single detector row"
        );
        for event in line["events"].as_array().unwrap() {
            assert!(
                event["targets"]
                    .as_array()
                    .unwrap()
                    .contains(&Value::from(0))
            );
            let branch = event["branch"].as_str().unwrap();
            match event["op"].as_str().unwrap() {
                "X_ERROR" => assert_eq!(branch, "X"),
                "LOSS" => assert_eq!(branch, "L"),
                other => panic!("unexpected traced op {other}"),
            }
        }
    }
    assert!(saw_x, "expected at least one X error in 32 shots");
    assert!(saw_loss, "expected at least one loss in 32 shots");
    assert_manifest_trace_entry(&private_out, shots);
}

#[test]
fn blinded_trace_unmasks_answers_and_stays_private() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let shots = 32;
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "measurements_blinded",
            shots,
            &public_out,
            &private_out,
            &["--logical_x_qubits", "0", "--seed", "11", "--error_trace"],
        )),
        "blinded trace export",
    );

    let shots_bytes = fs::read(public_out.join("shots.b8")).unwrap();
    let answers_bytes = fs::read(private_out.join("answers.b8")).unwrap();
    let masks_bytes = fs::read(private_out.join("masks.b8")).unwrap();
    let lines = trace_lines(&private_out, shots);
    let mut saw_flip = false;
    for (shot, line) in lines.iter().enumerate() {
        let x_fired = event_ops(line).contains(&"X_ERROR");
        let mask = b8_bit(&masks_bytes, shot);
        saw_flip |= mask;
        // The executed measurement reads x_fired ^ mask; the answer is the
        // unmasked public observable, i.e. exactly whether the X error fired.
        assert_eq!(
            b8_bit(&shots_bytes, shot),
            x_fired ^ mask,
            "shot {shot} measurement row disagrees with trace and mask"
        );
        assert_eq!(
            b8_bit(&answers_bytes, shot),
            x_fired,
            "shot {shot} answer is not the unmasked observable"
        );
    }
    assert!(saw_flip, "expected at least one hidden flip in 32 shots");
    assert_manifest_trace_entry(&private_out, shots);

    // The trace must stay out of the public bundle.
    let public_manifest = fs::read_to_string(public_out.join("manifest.json")).unwrap();
    assert!(
        !public_manifest.contains("trace"),
        "public manifest leaks trace metadata: {public_manifest}"
    );
    assert!(!public_out.join("trace.jsonl").exists());
}

#[test]
fn seeded_trace_export_is_deterministic_and_opt_in() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();

    let public_a = root.path().join("public-a");
    let private_a = root.path().join("private-a");
    let public_b = root.path().join("public-b");
    let private_b = root.path().join("private-b");
    for (public, private) in [(&public_a, &private_a), (&public_b, &private_b)] {
        assert_success(
            &run_cli(&export_args(
                &circuit,
                "detectors",
                16,
                public,
                private,
                &["--seed", "17", "--error_trace"],
            )),
            "deterministic trace export",
        );
    }
    assert_eq!(
        fs::read(private_a.join("trace.jsonl")).unwrap(),
        fs::read(private_b.join("trace.jsonl")).unwrap(),
        "same seed must reproduce trace bytes"
    );
    assert_eq!(
        fs::read(public_a.join("shots.b8")).unwrap(),
        fs::read(public_b.join("shots.b8")).unwrap(),
        "same seed must reproduce traced shots"
    );

    // Without --error_trace nothing trace-related is written.
    let public_c = root.path().join("public-c");
    let private_c = root.path().join("private-c");
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            16,
            &public_c,
            &private_c,
            &["--seed", "17"],
        )),
        "untraced export",
    );
    assert!(!private_c.join("trace.jsonl").exists());
    let manifest: Value =
        serde_json::from_slice(&fs::read(private_c.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest.get("trace_file").is_none());
}

#[test]
fn traced_export_batches_keep_global_shot_indices() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();

    // One-shot batches force several chunks; trace indices must stay global.
    let public_chunked = root.path().join("public-chunked");
    let private_chunked = root.path().join("private-chunked");
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            8,
            &public_chunked,
            &private_chunked,
            &["--seed", "23", "--batch_shots", "1", "--error_trace"],
        )),
        "chunked traced export",
    );
    let lines = trace_lines(&private_chunked, 8);

    let public_full = root.path().join("public-full");
    let private_full = root.path().join("private-full");
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            8,
            &public_full,
            &private_full,
            &["--seed", "23", "--error_trace"],
        )),
        "single-batch traced export",
    );
    assert_eq!(
        fs::read(private_chunked.join("trace.jsonl")).unwrap(),
        fs::read(private_full.join("trace.jsonl")).unwrap(),
        "chunk size must not change traced output for a fixed seed"
    );
    assert_eq!(
        fs::read(public_chunked.join("shots.b8")).unwrap(),
        fs::read(public_full.join("shots.b8")).unwrap(),
        "chunk size must not change traced shots for a fixed seed"
    );
    assert_eq!(lines[0]["shot"], 0);
    assert_eq!(lines[7]["shot"], 7);
}
