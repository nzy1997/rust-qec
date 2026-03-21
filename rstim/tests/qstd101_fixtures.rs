use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("qstd101")
        .join(file_name)
}

fn load_fixture(file_name: &str) -> Value {
    let path = fixture_path(file_name);
    assert!(path.exists(), "fixture file does not exist: {}", path.display());

    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse fixture {} as JSON: {err}", path.display()))
}

fn assert_common_markers(doc: &Value) {
    assert_eq!(doc["standard"], "QSTD101-ZY");
    assert_eq!(doc["version"], "1.0");

    let ops = doc["operations"]
        .as_array()
        .expect("operations should be a JSON array");
    assert!(!ops.is_empty(), "operations should be non-empty");

    let has_qubit_coords = ops.iter().any(|op| op["type"] == "qubit_coords");
    let has_tick = ops.iter().any(|op| op["type"] == "tick");
    let has_detector = ops.iter().any(|op| op["type"] == "detector");
    let has_observable_include = ops.iter().any(|op| op["type"] == "observable_include");

    assert!(has_qubit_coords, "expected at least one qubit_coords operation");
    assert!(has_tick, "expected at least one tick operation");
    assert!(has_detector, "expected at least one detector operation");
    assert!(
        has_observable_include,
        "expected at least one observable_include operation"
    );
}

#[test]
fn repetition_code_fixture_has_expected_qstd101_markers() {
    let doc = load_fixture("repetition_code_memory_d3_r3.json");
    assert_common_markers(&doc);
}

#[test]
fn surface_code_fixture_has_expected_qstd101_markers() {
    let doc = load_fixture("surface_code_rotated_memory_x_d3_r3.json");
    assert_common_markers(&doc);
}
