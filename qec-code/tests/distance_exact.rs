use qec_code::distance::{DistanceResult, LogicalClass};
use qec_code::distance_exact::{
    ExactCssDistanceInput, ExactCssDistanceOptions, ExactCssDistanceProvenance,
    ExactCssDistanceResult,
};
use qec_code::Pauli;

fn sample_distance_result() -> DistanceResult {
    let witness = Pauli::from_xz_bits(vec![1, 0, 1], vec![0, 0, 0]).unwrap();
    DistanceResult {
        distance: 2,
        witness,
        logical_class: LogicalClass::XLike,
    }
}

#[test]
fn exact_css_distance_result_serializes_completed_contract() {
    let result = ExactCssDistanceResult::completed(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "surface_rotated:d=3".to_owned(),
            },
        },
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 2);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["logical_class"], "x_like");
    assert_eq!(json["witness"]["x"], serde_json::json!([1, 0, 1]));
    assert_eq!(json["witness"]["z"], serde_json::json!([0, 0, 0]));
    assert_eq!(json["witness"]["weight"], 2);
    assert_eq!(json["options"]["input"], "code_id");
    assert_eq!(json["options"]["code_id"], "surface_rotated:d=3");
    assert_eq!(json["provenance"]["tool"], "qec-code");
    assert_eq!(json["provenance"]["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["provenance"]["method_revision"], 1);
}

#[test]
fn exact_css_distance_file_options_serialize_input_paths() {
    let result = ExactCssDistanceResult::completed(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::Files {
                hx: "input/hx.json".to_owned(),
                hz: "input/hz.json".to_owned(),
            },
        },
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["options"]["input"], "files");
    assert_eq!(json["options"]["hx"], "input/hx.json");
    assert_eq!(json["options"]["hz"], "input/hz.json");
}

#[test]
fn exact_css_distance_provenance_uses_current_package_version() {
    let provenance = ExactCssDistanceProvenance::current();

    assert_eq!(provenance.tool, "qec-code");
    assert_eq!(provenance.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(provenance.method_revision, 1);
}
