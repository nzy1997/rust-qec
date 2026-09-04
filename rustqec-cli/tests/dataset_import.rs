//! Integration tests for `rustqec dataset import`, the entry point for
//! datasets produced by third-party tooling: a circuit plus a shot payload
//! (and optionally a loss sidecar) are packaged into a public decoder
//! dataset only after passing the same validation `decode` performs.

use std::fs;
use std::path::Path;
use std::process::Command;

use rstim::decoder_dataset::{
    DecoderDatasetMode, ExportDecoderDatasetLogicalFlipConfig, LogicalFlip, LogicalPauli,
    export_decoder_dataset_with_logical_flip,
};
use serde_json::Value;

const TINY_CIRCUIT: &str = concat!(
    "QUBIT_COORDS(0,0) 0\n",
    "QUBIT_COORDS(1,0) 1\n",
    "R 0 1\n",
    "X_ERROR(0.1) 0\n",
    "X_ERROR(0.01) 1\n",
    "LOSS(0.1) 0\n",
    "H 0\n",
    "H 0\n",
    "CX 0 1\n",
    "X_ERROR(0.02) 0\n",
    "LOSS(0.1) 1\n",
    "ML 0 1\n",
    "DETECTOR(0,0,0) rec[-3]\n",
    "DETECTOR(1,0,0) rec[-1]\n",
    "OBSERVABLE_INCLUDE(0) rec[-3]\n",
);

fn rustqec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rustqec"))
}

fn run_import(root: &Path, extra: &[&str]) -> std::process::Output {
    let circuit = root.join("circuit.stim");
    fs::write(&circuit, TINY_CIRCUIT).unwrap();
    let shots = root.join("shots.01");
    fs::write(&shots, "0100\n1000\n1000\n0000\n").unwrap();
    let mut args = vec![
        "dataset".to_string(),
        "import".to_string(),
        "--circuit".to_string(),
        circuit.display().to_string(),
        "--shots".to_string(),
        shots.display().to_string(),
        "--out".to_string(),
        root.join("bundle").display().to_string(),
    ];
    args.extend(extra.iter().map(|arg| arg.to_string()));
    rustqec().args(args).output().unwrap()
}

fn stderr_json(output: &std::process::Output) -> Value {
    assert!(output.stdout.is_empty());
    serde_json::from_slice(&output.stderr).unwrap()
}

#[test]
fn import_packages_third_party_payloads_and_decode_reads_them() {
    let root = tempfile::tempdir().unwrap();
    let output = run_import(root.path(), &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "dataset.import");
    assert_eq!(value["result"]["shots"], 4);
    assert_eq!(value["result"]["measurements"], 4);
    assert_eq!(value["result"]["loss_flags"], 2);
    let bundle = root.path().join("bundle");
    for file in ["manifest.json", "circuit.stim", "shots.b8"] {
        assert!(bundle.join(file).exists(), "missing {file}");
    }

    let output = rustqec()
        .args([
            "decode",
            "--decoder",
            "envelope-mle",
            "--dataset",
            bundle.to_str().unwrap(),
            "--out",
            root.path().join("preds.b8").to_str().unwrap(),
            "--stats-out",
            root.path().join("stats.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The decode unit suite independently checks these canonical detector
    // predictions against renvelope's exact MLE.
    assert_eq!(
        fs::read(root.path().join("preds.b8")).unwrap(),
        [1, 1, 1, 0]
    );
}

#[test]
fn import_accepts_consistent_loss_logs_and_rejects_drift() {
    let root = tempfile::tempdir().unwrap();
    let log = root.path().join("loss.json");
    fs::write(
        &log,
        r#"{"schema_version":"rustqec.loss-log.v1","shots":[[],[0],[0],[]]}"#,
    )
    .unwrap();
    let flag = log.display().to_string();
    let output = run_import(root.path(), &["--loss-log", &flag]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let root = tempfile::tempdir().unwrap();
    let log = root.path().join("loss.json");
    fs::write(
        &log,
        r#"{"schema_version":"rustqec.loss-log.v1","shots":[[],[0],[],[]]}"#,
    )
    .unwrap();
    let flag = log.display().to_string();
    let output = run_import(root.path(), &["--loss-log", &flag]);
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "loss_log_mismatch");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("shot 2"),
        "{}",
        value["error"]["message"]
    );
    assert!(
        !root.path().join("bundle").exists(),
        "a failed import must publish nothing"
    );
}

#[test]
fn import_rejects_bad_payloads_and_unsupported_circuits_without_publishing() {
    // Row width mismatch in the 01 payload.
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("circuit.stim"), TINY_CIRCUIT).unwrap();
    fs::write(root.path().join("shots.01"), "010\n").unwrap();
    let output = rustqec()
        .args([
            "dataset",
            "import",
            "--circuit",
            root.path().join("circuit.stim").to_str().unwrap(),
            "--shots",
            root.path().join("shots.01").to_str().unwrap(),
            "--out",
            root.path().join("bundle").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "invalid_arguments");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("exactly 4 bits")
    );
    assert!(!root.path().join("bundle").exists());

    // Circuit outside the loss-visible subset (CZ is not in v1).
    let root = tempfile::tempdir().unwrap();
    let unsupported = TINY_CIRCUIT.replace("CX 0 1\n", "CZ 0 1\n");
    fs::write(root.path().join("circuit.stim"), &unsupported).unwrap();
    fs::write(root.path().join("shots.01"), "0100\n").unwrap();
    let output = rustqec()
        .args([
            "dataset",
            "import",
            "--circuit",
            root.path().join("circuit.stim").to_str().unwrap(),
            "--shots",
            root.path().join("shots.01").to_str().unwrap(),
            "--out",
            root.path().join("bundle").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "unsupported_circuit");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("outside the supported"),
        "{}",
        value["error"]["message"]
    );
    assert!(!root.path().join("bundle").exists());

    // Nonzero padding bits in a b8 payload.
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("circuit.stim"), TINY_CIRCUIT).unwrap();
    fs::write(root.path().join("shots.b8"), [0x82u8]).unwrap();
    let output = rustqec()
        .args([
            "dataset",
            "import",
            "--circuit",
            root.path().join("circuit.stim").to_str().unwrap(),
            "--shots",
            root.path().join("shots.b8").to_str().unwrap(),
            "--shots-format",
            "b8",
            "--out",
            root.path().join("bundle").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = stderr_json(&output);
    assert_eq!(value["error"]["code"], "invalid_arguments");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("padding")
    );
}

/// An exported dataset and an imported repackaging of the same circuit and
/// shots decode to identical predictions: the import path is a faithful
/// interchange route, not a lossy one.
#[test]
fn imported_bundle_decodes_identically_to_exported_bundle() {
    const FIXTURE: &str = include_str!("fixtures/stim_rotated_memory_z_d3_r2_loss_visible.stim");
    let root = tempfile::tempdir().unwrap();
    let exported = root.path().join("exported");
    let private = root.path().join("private");
    export_decoder_dataset_with_logical_flip(ExportDecoderDatasetLogicalFlipConfig {
        circuit_text: FIXTURE.to_string(),
        shots: 8,
        mode: DecoderDatasetMode::MeasurementsBlinded,
        logical_flip: Some(LogicalFlip {
            pauli: LogicalPauli::X,
            qubits: vec![1, 8, 15],
        }),
        public_out: exported.clone(),
        private_out: private,
        seed: Some(99),
        error_trace: false,
    })
    .unwrap();

    let imported = root.path().join("imported");
    let output = rustqec()
        .args([
            "dataset",
            "import",
            "--circuit",
            exported.join("circuit.stim").to_str().unwrap(),
            "--shots",
            exported.join("shots.b8").to_str().unwrap(),
            "--shots-format",
            "b8",
            "--out",
            imported.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, dataset) in [("exported", &exported), ("imported", &imported)] {
        let output = rustqec()
            .args([
                "decode",
                "--decoder",
                "envelope-mle",
                "--dataset",
                dataset.to_str().unwrap(),
                "--out",
                root.path().join(format!("{name}.b8")).to_str().unwrap(),
                "--stats-out",
                root.path().join(format!("{name}.json")).to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read(root.path().join("exported.b8")).unwrap(),
        fs::read(root.path().join("imported.b8")).unwrap(),
    );
}
