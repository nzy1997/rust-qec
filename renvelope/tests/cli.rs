use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

fn case_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("cases")
        .join(name)
}

#[test]
fn positive_fixture_has_the_exact_known_answer() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "decode",
            "--in",
            case_path("single_loss_observable.json").to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
            "--backend",
            "highs",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(
        actual,
        json!({
            "schema_version": "atom-loss-envelope-result.v0",
            "status": "optimal",
            "backend": "highs",
            "selected_independent_effects": [],
            "selected_loss_candidates": [
                {"loss_id": "loss-q0-t3", "candidate_id": "d0-l0"}
            ],
            "predicted_observables": [0],
            "objective": 1.0
        })
    );
}

#[test]
fn exclusivity_fixture_is_reported_as_infeasible() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "decode",
            "--in",
            case_path("exclusivity_infeasible.json").to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
            "--backend",
            "highs",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let actual: serde_json::Value =
        serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(
        actual,
        json!({
            "schema_version": "atom-loss-envelope-result.v0",
            "status": "infeasible",
            "backend": "highs"
        })
    );
}

#[test]
fn malformed_input_is_rejected_without_writing_a_result() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("invalid.json");
    let output_path = directory.path().join("result.json");
    fs::write(
        &input_path,
        r#"{
          "schema_version": "atom-loss-envelope.v0",
          "num_detectors": 1,
          "num_observables": 0,
          "observed_detectors": [4],
          "independent_effects": [],
          "loss_envelopes": []
        }"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "decode",
            "--in",
            input_path.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
            "--backend",
            "highs",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("observed_detectors references detector")
    );
    assert!(!output_path.exists());
}

#[test]
fn matching_fixture_changes_prediction_and_groups_loss_patterns() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("matching-result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "matching",
            "--in",
            case_path("matching_known_answer.json").to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(
        actual,
        json!({
            "schema_version": "atom-loss-envelope-matching-result.v0",
            "backend": "rmatching",
            "predictions": [0, 1, 1],
            "compiled_loss_configurations": 2
        })
    );
}

#[test]
fn matching_dangling_edge_is_rejected_without_writing_a_result() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("matching-result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "matching",
            "--in",
            case_path("matching_dangling_edge.json").to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing-edge"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn matching_unmatchable_syndrome_is_rejected_without_writing_a_result() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("unmatchable.json");
    let output_path = directory.path().join("matching-result.json");
    fs::write(
        &input_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "atom-loss-envelope-matching.v0",
            "num_detectors": 2,
            "num_observables": 0,
            "edges": [{
                "id": "internal",
                "node1": 0,
                "node2": 1,
                "observable_indices": [],
                "weight": 1.0,
                "kind": "space_like"
            }],
            "loss_edge_map": [],
            "shots": [{"observed_detectors": [0], "observed_losses": []}]
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "matching",
            "--in",
            input_path.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("boundaryless graph component"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn matching_empty_shots_are_rejected_without_writing_a_result() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("empty-shots.json");
    let output_path = directory.path().join("matching-result.json");
    let mut case: serde_json::Value =
        serde_json::from_slice(&fs::read(case_path("matching_known_answer.json")).unwrap())
            .unwrap();
    case["shots"] = json!([]);
    fs::write(&input_path, serde_json::to_vec_pretty(&case).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "matching",
            "--in",
            input_path.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("at least one shot"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn prepare_bridge_fixture_flows_through_both_decoders() {
    let directory = tempfile::tempdir().unwrap();
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("shots.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, [0x03]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            case_path("bridge_single_loss.stim").to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(prepared_path.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["schema_version"],
        "atom-loss-envelope-preparation.v0"
    );
    assert_eq!(manifest["loss_readout_count"], 1);
    assert_eq!(manifest["retained_single_loss_calibration_rows"], 1);
    assert_eq!(manifest["calibrated_pattern_count"], 1);
    assert_eq!(manifest["matching_edge_count"], 1);
    assert_eq!(manifest["loss_edge_membership_count"], 1);
    assert_eq!(manifest["raw_measurement_row_bits"], 2);
    assert_eq!(manifest["compact_value_row_bits"], 1);
    assert_eq!(manifest["observable_row_bits"], 1);
    assert_eq!(manifest["observable_row_bytes"], 1);
    assert_eq!(manifest["observables_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(manifest["circuit_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(manifest["calibration_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(manifest["shots_sha256"].as_str().unwrap().len(), 64);

    let mle_path = prepared_path.join("mle/shot-000000.json");
    let mle: serde_json::Value = serde_json::from_slice(&fs::read(&mle_path).unwrap()).unwrap();
    assert_eq!(mle["observed_detectors"], json!([0]));
    assert_eq!(mle["independent_effects"].as_array().unwrap().len(), 1);
    assert_eq!(mle["independent_effects"][0]["id"], "dem-e0");
    assert_eq!(mle["independent_effects"][0]["detectors"], json!([0]));
    assert_eq!(mle["independent_effects"][0]["observables"], json!([0]));
    let weight = mle["independent_effects"][0]["weight"].as_f64().unwrap();
    assert!((weight - 9.0_f64.ln()).abs() < 1e-12);
    assert_eq!(mle["loss_envelopes"].as_array().unwrap().len(), 1);
    assert_eq!(
        mle["loss_envelopes"][0]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        mle["loss_envelopes"][0]["candidates"][0]["detectors"],
        json!([0])
    );
    assert_eq!(
        mle["loss_envelopes"][0]["candidates"][0]["observables"],
        json!([0])
    );
    assert_eq!(
        mle["loss_envelopes"][0]["candidates"][0]["weight"].as_f64(),
        Some(0.0)
    );

    let matching_path = prepared_path.join("matching.json");
    let matching: serde_json::Value =
        serde_json::from_slice(&fs::read(&matching_path).unwrap()).unwrap();
    assert_eq!(matching["edges"].as_array().unwrap().len(), 1);
    assert_eq!(matching["edges"][0]["node1"], 0);
    assert_eq!(matching["edges"][0]["node2"], serde_json::Value::Null);
    assert_eq!(matching["edges"][0]["observable_indices"], json!([0]));
    assert_eq!(matching["edges"][0]["kind"], "boundary");
    assert_eq!(matching["loss_edge_map"][0]["edge_ids"], json!(["dem-e0"]));
    assert_eq!(matching["shots"][0]["observed_detectors"], json!([0]));
    assert_eq!(matching["shots"][0]["observed_losses"], json!(["loss-m0"]));

    let mle_result_path = directory.path().join("mle-result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "decode",
            "--in",
            mle_path.to_str().unwrap(),
            "--out",
            mle_result_path.to_str().unwrap(),
            "--backend",
            "highs",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mle_result: serde_json::Value =
        serde_json::from_slice(&fs::read(mle_result_path).unwrap()).unwrap();
    assert_eq!(mle_result["predicted_observables"], json!([0]));

    let matching_result_path = directory.path().join("matching-result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "matching",
            "--in",
            matching_path.to_str().unwrap(),
            "--out",
            matching_result_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let matching_result: serde_json::Value =
        serde_json::from_slice(&fs::read(matching_result_path).unwrap()).unwrap();
    assert_eq!(matching_result["predictions"], json!([1]));
    assert_eq!(
        fs::read(prepared_path.join("observables.b8")).unwrap(),
        [0x01]
    );

    let second_prepared_path = directory.path().join("prepared-again");
    fs::create_dir(&second_prepared_path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            case_path("bridge_single_loss.stim").to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            second_prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for relative in [
        "manifest.json",
        "observables.b8",
        "mle/shot-000000.json",
        "matching.json",
    ] {
        assert_eq!(
            fs::read(prepared_path.join(relative)).unwrap(),
            fs::read(second_prepared_path.join(relative)).unwrap(),
            "prepared output {relative} was not deterministic"
        );
    }
}

#[test]
fn prepare_rejects_loss_flag_detector_before_creating_output() {
    let directory = tempfile::tempdir().unwrap();
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("shots.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, [0x03]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            case_path("bridge_loss_flag_detector.stim")
                .to_str()
                .unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid loss-flag reference"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!prepared_path.exists());
}

#[test]
fn prepare_rejects_truncated_b8_before_creating_output() {
    let directory = tempfile::tempdir().unwrap();
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("truncated.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, []).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            case_path("bridge_single_loss.stim").to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("target b8 input has 0 bytes; expected 1"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!prepared_path.exists());
}

#[test]
fn prepare_help_documents_normal_workflow_flag_names() {
    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args(["prepare", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for flag in [
        "--circuit",
        "--calibration_in",
        "--calibration_shots",
        "--in",
        "--shots",
        "--out",
    ] {
        assert!(stdout.contains(flag), "missing {flag} in help:\n{stdout}");
    }
}

#[test]
fn prepare_refuses_a_nonempty_output_directory() {
    let directory = tempfile::tempdir().unwrap();
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("shots.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, [0x03]).unwrap();
    fs::create_dir(&prepared_path).unwrap();
    fs::write(prepared_path.join("keep.txt"), b"user data").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            case_path("bridge_single_loss.stim").to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not empty"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(prepared_path.join("keep.txt")).unwrap(),
        b"user data"
    );
    assert!(!prepared_path.join("manifest.json").exists());
}

#[test]
fn prepare_rejects_generated_matching_case_that_matching_cannot_accept() {
    let directory = tempfile::tempdir().unwrap();
    let circuit_path = directory.path().join("boundaryless.stim");
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("shots.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(
        &circuit_path,
        "R 0 1\nX_ERROR(0.1) 1\nMRL 0\nM 1\nDETECTOR rec[-1]\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
    )
    .unwrap();
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, [0x03]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            circuit_path.to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("generated matching input is invalid"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("boundaryless graph component"),
        "stderr: {stderr}"
    );
    assert!(!prepared_path.exists());
}

#[test]
fn prepare_rejects_sweep_dependent_circuits_without_a_sidecar() {
    let directory = tempfile::tempdir().unwrap();
    let circuit_path = directory.path().join("sweep.stim");
    let calibration_path = directory.path().join("calibration.b8");
    let shots_path = directory.path().join("shots.b8");
    let prepared_path = directory.path().join("prepared");
    fs::write(
        &circuit_path,
        "R 0\nCX sweep[0] 0\nX_ERROR(0.1) 0\nMRL 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    fs::write(&calibration_path, [0x03]).unwrap();
    fs::write(&shots_path, [0x03]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_renvelope"))
        .args([
            "prepare",
            "--circuit",
            circuit_path.to_str().unwrap(),
            "--calibration_in",
            calibration_path.to_str().unwrap(),
            "--calibration_shots",
            "1",
            "--in",
            shots_path.to_str().unwrap(),
            "--shots",
            "1",
            "--out",
            prepared_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not support sweep-dependent circuits"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("1 sweep bit"), "stderr: {stderr}");
    assert!(!prepared_path.exists());
}
