use std::fs;
use std::path::{Path, PathBuf};

use rstim::codegen::{repetition_code_memory, surface_code};
use rstim::qp101::{export_qp101, Qp101Document};

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("qp101")
        .join(file_name)
}

fn load_fixture(file_name: &str) -> Qp101Document {
    let path = fixture_path(file_name);
    assert!(path.exists(), "fixture file does not exist: {}", path.display());

    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse fixture {} as Qp101Document: {err}", path.display()))
}

fn assert_common_markers(doc: &Qp101Document) {
    assert_eq!(doc.standard, "QP101-ZY");
    assert_eq!(doc.version, "1.0");
    assert!(!doc.operations.is_empty(), "operations should be non-empty");

    let serialized = serde_json::to_value(doc).expect("Qp101Document should serialize");
    let ops = serialized["operations"]
        .as_array()
        .expect("operations should serialize to a JSON array");
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
fn repetition_code_fixture_has_expected_qp101_markers() {
    // Regenerate with: rstim gen ... && rstim export_json ... (Task 5 fixture flow).
    let generated = export_qp101(&repetition_code_memory(3, 3, 0.0))
        .expect("export of repetition code should succeed");
    let fixture = load_fixture("repetition_code_memory_d3_r3.json");

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}

#[test]
fn surface_code_fixture_has_expected_qp101_markers() {
    let generated = export_qp101(&surface_code::rotated_memory_x(3, 3, 0.0))
        .expect("export of rotated surface code should succeed");
    let fixture = load_fixture("surface_code_rotated_memory_x_d3_r3.json");

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}
