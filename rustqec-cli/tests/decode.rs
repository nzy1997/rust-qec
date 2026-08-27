use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rstim::decoder_dataset::{
    DecoderDatasetMode, ExportDecoderDatasetLogicalFlipConfig, LogicalFlip, LogicalPauli,
    export_decoder_dataset_with_logical_flip,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CIRCUIT: &str = concat!(
    "QUBIT_COORDS(0,0) 0\n",
    "QUBIT_COORDS(1,0) 1\n",
    "R 0 1\n",
    "TICK[rstim:logical_flip_point]\n",
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
const SHOTS: &[u8] = &[0x02, 0x01, 0x01, 0x00];
const EXPECTED_PREDICTIONS: &[u8] = &[1, 0, 0, 0];
const PLACEHOLDER_INVARIANCE_CIRCUIT: &str = concat!(
    "QUBIT_COORDS(0,0) 0\n",
    "R 0\n",
    "X_ERROR(0.1) 0\n",
    "LOSS(0.1) 0\n",
    "MRL 0\n",
    "X_ERROR(0.02) 0\n",
    "LOSS(0.1) 0\n",
    "ML 0\n",
    "DETECTOR(0,0,0) rec[-3]\n",
    "DETECTOR(0,0,1) rec[-3] rec[-1]\n",
    "OBSERVABLE_INCLUDE(0) rec[-1]\n",
);

fn rustqec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rustqec"))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn dataset_id(circuit_sha: &str, shots_sha: &str, shots: usize, row_bits: usize) -> String {
    sha256(
        format!(
            "format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\ncircuit_sha256={circuit_sha}\nshots={shots}\nrow_bits={row_bits}\nshots_b8_sha256={shots_sha}\n"
        )
        .as_bytes(),
    )
}

fn write_dataset(root: &Path, circuit: &str, shots: &[u8]) -> PathBuf {
    let dataset = root.join("dataset");
    fs::create_dir(&dataset).unwrap();
    fs::write(dataset.join("circuit.stim"), circuit).unwrap();
    fs::write(dataset.join("shots.b8"), shots).unwrap();
    let circuit_sha = sha256(circuit.as_bytes());
    let shots_sha = sha256(shots);
    let manifest = json!({
        "format": "rstim_decoder_dataset",
        "schema_version": 1,
        "dataset_id": dataset_id(&circuit_sha, &shots_sha, shots.len(), 4),
        "mode": "measurements_blinded",
        "shots": shots.len(),
        "row": {
            "kind": "measurements",
            "bits": 4,
            "encoding": "b8",
            "bit_order": "lsb_first",
            "bytes_per_shot": 1
        },
        "circuit": {
            "file": "circuit.stim",
            "sha256": circuit_sha,
            "measurements": 4,
            "detectors": 2,
            "observables": 1,
            "sweep_bits": 0
        },
        "shots_file": {
            "file": "shots.b8",
            "sha256": shots_sha,
            "bits": 4,
            "bytes_per_shot": 1
        }
    });
    fs::write(
        dataset.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    dataset
}

fn write_decoder_server_dataset(root: &Path, circuit: &str, shots: &[u8]) -> PathBuf {
    let dataset = root.join("decoder-server-dataset");
    fs::create_dir(&dataset).unwrap();
    fs::write(dataset.join("circuit.stim"), circuit).unwrap();
    fs::write(dataset.join("shots.b8"), shots).unwrap();
    let circuit_sha = sha256(circuit.as_bytes());
    let shots_sha = sha256(shots);
    let manifest = json!({
        "format": "qude_decoder_dataset",
        "schema_version": 3,
        "benchmark_id": "surface_d3_r1_p001_loss_midswap_measurements",
        "code_family": "surface_code",
        "task": "rotated_memory_z_midswap_alt",
        "code_params": {"distance": 3},
        "noise_model": "circuit_depolarization_and_atom_loss",
        "p": 0.001,
        "rounds": 1,
        "mode": "measurements_blinded",
        "shots": shots.len(),
        "num_detectors": 2,
        "num_measurements": 4,
        "num_observables": 1,
        "row": {
            "kind": "measurements",
            "bits": 4,
            "encoding": "b8",
            "bit_order": "little_endian",
            "bytes_per_shot": 1
        },
        "circuit": {
            "file": "circuit.stim",
            "sha256": circuit_sha,
            "measurements": 4,
            "detectors": 2,
            "observables": 1
        },
        "shots_file": {
            "file": "shots.b8",
            "sha256": shots_sha,
            "encoding": "b8",
            "bit_order": "little_endian",
            "bits_per_shot": 4,
            "bytes_per_shot": 1
        },
        "predictions": {
            "encoding": "b8",
            "bit_order": "little_endian",
            "bits_per_shot": 1,
            "bytes_per_shot": 1,
            "padding": "zero"
        }
    });
    fs::write(
        dataset.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    dataset
}

fn run_decode(dataset: &Path, decoder: &str, root: &Path) -> std::process::Output {
    rustqec()
        .args([
            "decode",
            "--decoder",
            decoder,
            "--dataset",
            dataset.to_str().unwrap(),
            "--out",
            root.join(format!("{decoder}.b8")).to_str().unwrap(),
            "--stats-out",
            root.join(format!("{decoder}.json")).to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

#[test]
fn both_loss_decoders_run_public_only_and_reuse_compiled_state() {
    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
    for decoder in ["envelope-matching", "envelope-mle"] {
        let output = run_decode(&dataset, decoder, root.path());
        assert!(
            output.status.success(),
            "{decoder}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        let predictions = fs::read(root.path().join(format!("{decoder}.b8"))).unwrap();
        assert_eq!(predictions, EXPECTED_PREDICTIONS, "{decoder}");
        assert!(predictions.iter().all(|byte| byte & 0xfe == 0));
        let stats: Value =
            serde_json::from_slice(&fs::read(root.path().join(format!("{decoder}.json"))).unwrap())
                .unwrap();
        assert_eq!(stats["schema_version"], "rustqec.decode-stats.v1");
        assert_eq!(stats["shot_count"], SHOTS.len());
        assert_eq!(stats["attempted_shot_count"], SHOTS.len());
        assert_eq!(stats["circuit_compilations"], 1);
        assert_eq!(stats["distinct_loss_patterns"], 2);
        if decoder == "envelope-matching" {
            assert_eq!(stats["matching_graph_builds"], 2);
            assert_eq!(stats["cache_hits"], 2);
        } else {
            assert_eq!(stats["mle_model_builds"], 2);
        }
    }
}

#[test]
fn real_cli_predictions_are_invariant_to_lost_measurement_placeholders() {
    let root = tempfile::tempdir().unwrap();
    // Both shots herald loss at record 0 and have the same known final value.
    // They differ only at lost value record 1 (0x09 versus 0x0b).
    let dataset = write_dataset(root.path(), PLACEHOLDER_INVARIANCE_CIRCUIT, &[0x09, 0x0b]);
    for decoder in ["envelope-matching", "envelope-mle"] {
        let output = run_decode(&dataset, decoder, root.path());
        assert!(
            output.status.success(),
            "{decoder}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let predictions = fs::read(root.path().join(format!("{decoder}.b8"))).unwrap();
        assert_eq!(predictions, [1, 1], "{decoder}");
        assert_eq!(predictions[0], predictions[1], "{decoder}");
        let stats: Value =
            serde_json::from_slice(&fs::read(root.path().join(format!("{decoder}.json"))).unwrap())
                .unwrap();
        assert_eq!(stats["distinct_loss_patterns"], 1);
        if decoder == "envelope-matching" {
            assert_eq!(stats["matching_graph_builds"], 1);
        } else {
            assert_eq!(stats["mle_model_builds"], 1);
        }
    }
}

#[test]
fn decoder_server_v3_public_bundle_decodes_without_translation() {
    let root = tempfile::tempdir().unwrap();
    let dataset = write_decoder_server_dataset(root.path(), CIRCUIT, SHOTS);
    for decoder in ["envelope-matching", "envelope-mle"] {
        let output = run_decode(&dataset, decoder, root.path());
        assert!(
            output.status.success(),
            "{decoder}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(root.path().join(format!("{decoder}.b8"))).unwrap(),
            EXPECTED_PREDICTIONS,
        );
    }
}

#[test]
fn decoder_server_v3_contract_mismatches_are_rejected() {
    for (name, mutate, expected) in [
        (
            "counts",
            (|manifest: &mut Value| manifest["num_measurements"] = json!(5)) as fn(&mut Value),
            "counts disagree",
        ),
        (
            "bit-order",
            (|manifest: &mut Value| manifest["shots_file"]["bit_order"] = json!("msb_first"))
                as fn(&mut Value),
            "little_endian b8",
        ),
        (
            "predictions",
            (|manifest: &mut Value| manifest["predictions"]["bits_per_shot"] = json!(2))
                as fn(&mut Value),
            "prediction row width",
        ),
        (
            "unknown",
            (|manifest: &mut Value| manifest["private_seed"] = json!(123)) as fn(&mut Value),
            "unknown field",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let dataset = write_decoder_server_dataset(root.path(), CIRCUIT, SHOTS);
        rewrite_manifest(&dataset, mutate);
        let output = run_decode(&dataset, "envelope-mle", root.path());
        assert_json_error(&output, "invalid_dataset");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn rewrite_manifest(dataset: &Path, mutate: impl FnOnce(&mut Value)) {
    let path = dataset.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    mutate(&mut manifest);
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn assert_json_error(output: &std::process::Output, code: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["status"], "error");
    assert_eq!(value["command"], "decode");
    assert_eq!(value["error"]["code"], code);
}

#[test]
fn malformed_and_detector_mode_datasets_use_structured_errors() {
    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
    rewrite_manifest(&dataset, |manifest| {
        manifest["mode"] = json!("detectors");
        manifest["row"]["kind"] = json!("detectors");
    });
    assert_json_error(
        &run_decode(&dataset, "envelope-mle", root.path()),
        "unsupported_dataset_mode",
    );

    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, &[0x82]);
    let output = run_decode(&dataset, "envelope-mle", root.path());
    assert_json_error(&output, "invalid_dataset");
    assert!(String::from_utf8_lossy(&output.stderr).contains("padding"));
}

#[test]
fn missing_files_and_manifest_circuit_mismatch_are_rejected() {
    for file in ["manifest.json", "circuit.stim", "shots.b8"] {
        let root = tempfile::tempdir().unwrap();
        let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
        fs::remove_file(dataset.join(file)).unwrap();
        assert_json_error(
            &run_decode(&dataset, "envelope-mle", root.path()),
            "missing_dataset_file",
        );
    }

    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
    rewrite_manifest(&dataset, |manifest| manifest["row"]["bits"] = json!(7));
    assert_json_error(
        &run_decode(&dataset, "envelope-mle", root.path()),
        "invalid_dataset",
    );
}

#[test]
fn unsupported_layout_timeout_and_infeasible_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let unsupported = CIRCUIT.replace("DETECTOR(0,0,0) rec[-3]", "DETECTOR(0,0,0) rec[-4]");
    let dataset = write_dataset(root.path(), &unsupported, SHOTS);
    assert_json_error(
        &run_decode(&dataset, "envelope-mle", root.path()),
        "unsupported_circuit",
    );

    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
    let output = rustqec()
        .args([
            "decode",
            "--decoder",
            "envelope-mle",
            "--dataset",
            dataset.to_str().unwrap(),
            "--out",
            root.path().join("timeout.b8").to_str().unwrap(),
            "--stats-out",
            root.path().join("timeout.json").to_str().unwrap(),
            "--shot-timeout-ms",
            "0",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_json_error(&output, "decode_timeout");
    assert!(!root.path().join("timeout.b8").exists());
    let stats: Value =
        serde_json::from_slice(&fs::read(root.path().join("timeout.json")).unwrap()).unwrap();
    assert_eq!(stats["timeout_count"], 1);
    assert_eq!(stats["infeasible_shot_count"], 0);
    assert_eq!(stats["attempted_shot_count"], 1);

    let root = tempfile::tempdir().unwrap();
    let infeasible = CIRCUIT
        .replace("X_ERROR(0.01) 1\n", "")
        .replace("X_ERROR(0.02) 0\n", "");
    let dataset = write_dataset(root.path(), &infeasible, &[0x08]);
    let output = run_decode(&dataset, "envelope-mle", root.path());
    assert_eq!(output.status.code(), Some(3));
    assert_json_error(&output, "decode_infeasible");
    assert!(!root.path().join("envelope-mle.b8").exists());
    let stats: Value =
        serde_json::from_slice(&fs::read(root.path().join("envelope-mle.json")).unwrap()).unwrap();
    assert_eq!(stats["timeout_count"], 0);
    assert_eq!(stats["infeasible_shot_count"], 1);
    assert_eq!(stats["attempted_shot_count"], 1);
}

#[test]
fn unsupported_gates_readouts_and_overlapping_cx_pairs_are_rejected() {
    for circuit in [
        CIRCUIT.replace("H 0\n", "SQRT_X 0\n"),
        CIRCUIT.replace("ML 0 1\n", "MXL 0 1\n"),
        CIRCUIT.replace("CX 0 1\n", "CX 0 1 1 0\n"),
        CIRCUIT.replace("CX 0 1\n", "CZ 0 1\n"),
        CIRCUIT.replace("ML 0 1\n", "ML 0\nX_ERROR(0.1) 0\nML 1\n"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let dataset = write_dataset(root.path(), &circuit, SHOTS);
        assert_json_error(
            &run_decode(&dataset, "envelope-mle", root.path()),
            "unsupported_circuit",
        );
    }

    let root = tempfile::tempdir().unwrap();
    let dataset = write_dataset(root.path(), CIRCUIT, SHOTS);
    let output = rustqec()
        .args([
            "decode",
            "--decoder",
            "envelope-matching",
            "--dataset",
            dataset.to_str().unwrap(),
            "--out",
            root.path().join("matching.b8").to_str().unwrap(),
            "--stats-out",
            root.path().join("matching.json").to_str().unwrap(),
            "--shot-timeout-ms",
            "1",
        ])
        .output()
        .unwrap();
    assert_json_error(&output, "invalid_arguments");
}

#[test]
fn huge_repeat_is_rejected_before_flattening() {
    let huge = "REPEAT 999999999999 {\n    H 0\n    H 0\n}\n".to_string();
    let mut deep = "H 0\nH 0\n".to_string();
    for _ in 0..64 {
        deep = format!("REPEAT 1 {{\n{deep}}}\n");
    }
    for body in [huge, deep] {
        let repeated = CIRCUIT.replace("H 0\nH 0\n", &body);
        let root = tempfile::tempdir().unwrap();
        let dataset = write_dataset(root.path(), &repeated, SHOTS);
        let output = run_decode(&dataset, "envelope-mle", root.path());
        assert_json_error(&output, "unsupported_circuit");
        assert!(String::from_utf8_lossy(&output.stderr).contains("REPEAT"));
    }
}

#[test]
fn native_d3_and_d5_exports_decode_via_cli_against_private_answers() {
    for distance in [3, 5] {
        let circuit = rstim::codegen::rotated_memory_z_midswap(rstim::codegen::MidSwapConfig {
            distance,
            rounds: 1,
            before_round_data_depolarization: 1e-9,
            before_round_data_loss_probability: 0.0,
            after_clifford_depolarization: 1e-9,
            before_measure_flip_probability: 1e-9,
            after_reset_flip_probability: 1e-9,
            operation_loss_probability: 1e-9,
            measurement_loss_probability: 1e-9,
        })
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        let public = root.path().join("public");
        let private = root.path().join("private");
        let stride = 2 * distance + 1;
        export_decoder_dataset_with_logical_flip(ExportDecoderDatasetLogicalFlipConfig {
            circuit_text: circuit,
            shots: 4,
            mode: DecoderDatasetMode::MeasurementsBlinded,
            logical_flip: Some(LogicalFlip {
                pauli: LogicalPauli::X,
                qubits: (0..distance).map(|row| (1 + row * stride) as u32).collect(),
            }),
            public_out: public.clone(),
            private_out: private.clone(),
            seed: Some(0x630 + distance as u64),
            error_trace: false,
        })
        .unwrap();
        let answers = fs::read(private.join("answers.b8")).unwrap();

        for decoder in ["envelope-matching", "envelope-mle"] {
            let output = run_decode(&public, decoder, root.path());
            assert!(
                output.status.success(),
                "d={distance} {decoder}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let predictions = fs::read(root.path().join(format!("{decoder}.b8"))).unwrap();
            assert_eq!(predictions.len(), answers.len());
            for shot in 0..answers.len() {
                assert_eq!(
                    predictions[shot] & 1,
                    answers[shot] & 1,
                    "d={distance} {decoder} shot={shot}"
                );
            }
        }
    }
}

#[test]
fn conventional_loss_visible_rotated_memory_z_decodes_via_cli_against_private_answers() {
    let circuit = rstim::codegen::surface_code::rotated_memory_z_loss_visible(
        3,
        1,
        rstim::codegen::surface_code::RotatedMemoryZLossConfig {
            before_round_data_depolarization: 1e-9,
            after_clifford_depolarization: 1e-9,
            before_measure_flip_probability: 1e-9,
            after_reset_flip_probability: 1e-9,
            operation_loss_probability: 1e-9,
            measurement_loss_probability: 1e-9,
            after_clifford_loss_probability: 1e-9,
        },
    )
    .unwrap();
    assert!(!circuit.contains("SHIFT_COORDS"));
    let root = tempfile::tempdir().unwrap();
    let public = root.path().join("public");
    let private = root.path().join("private");
    export_decoder_dataset_with_logical_flip(ExportDecoderDatasetLogicalFlipConfig {
        circuit_text: circuit,
        shots: 4,
        mode: DecoderDatasetMode::MeasurementsBlinded,
        logical_flip: Some(LogicalFlip {
            pauli: LogicalPauli::X,
            qubits: vec![1, 2, 3],
        }),
        public_out: public.clone(),
        private_out: private.clone(),
        seed: Some(0x631),
        error_trace: false,
    })
    .unwrap();
    let answers = fs::read(private.join("answers.b8")).unwrap();

    for decoder in ["envelope-matching", "envelope-mle"] {
        let output = run_decode(&public, decoder, root.path());
        assert!(
            output.status.success(),
            "{decoder}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let predictions = fs::read(root.path().join(format!("{decoder}.b8"))).unwrap();
        assert_eq!(predictions.len(), answers.len());
        for shot in 0..answers.len() {
            assert_eq!(
                predictions[shot] & 1,
                answers[shot] & 1,
                "{decoder} shot={shot}"
            );
        }
    }
}

#[test]
fn capabilities_advertises_decode_contract() {
    let output = rustqec()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let decode = value["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["name"] == "decode")
        .unwrap();
    assert_eq!(
        decode["decoders"],
        json!(["envelope-matching", "envelope-mle"])
    );
    assert!(
        decode["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg["flag"] == "--shot-timeout-ms")
    );
    assert!(
        decode["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["flag"] == "--out" && artifact["format"] == "b8")
    );
    assert!(
        decode["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "decode_infeasible" && error["exit_code"] == 3)
    );
    assert!(
        decode["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "decode_error" && error["exit_code"] == 2)
    );
}
