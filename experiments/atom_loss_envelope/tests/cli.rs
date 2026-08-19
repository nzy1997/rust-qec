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
    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_atom-loss-envelope"))
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
