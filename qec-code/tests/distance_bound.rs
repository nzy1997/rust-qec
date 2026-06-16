use qec_code::distance::LogicalClass;
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness, RandomizedUpperBoundOptions,
    validate_randomized_upper_bound_result,
};
use qec_code::{Pauli, QecError, StabilizerCode};

fn trivial_one_qubit_code() -> StabilizerCode {
    StabilizerCode::from_stabilizers(1, vec![]).unwrap()
}

fn one_qubit_x_witness() -> DistanceBoundWitness {
    let pauli = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    DistanceBoundWitness::from_pauli(&pauli)
}

fn valid_result() -> DistanceBoundResult {
    DistanceBoundResult::completed(
        1,
        LogicalClass::XLike,
        one_qubit_x_witness(),
        RandomizedUpperBoundOptions {
            iterations: 10,
            restarts: 1,
            seed: 7,
            target_weight: None,
        },
    )
}

#[test]
fn completed_bound_result_serializes_with_upper_bound_contract() {
    let result = valid_result();

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 1);
    assert_eq!(json["logical_class"], "x_like");
    assert_eq!(json["witness"]["x"], serde_json::json!([1]));
    assert_eq!(json["witness"]["z"], serde_json::json!([0]));
    assert_eq!(json["witness"]["weight"], 1);
    assert_eq!(json["options"]["iterations"], 10);
    assert_eq!(json["options"]["restarts"], 1);
    assert_eq!(json["options"]["seed"], 7);
    assert_eq!(json["options"]["target_weight"], serde_json::Value::Null);
    assert_eq!(json["provenance"]["tool"], "qec-code");
    assert_eq!(json["provenance"]["method_revision"], 1);
}

#[test]
fn randomized_upper_bound_options_reject_zero_iterations_restarts_and_target() {
    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 0,
            restarts: 1,
            seed: 7,
            target_weight: None,
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        })
    );

    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 1,
            restarts: 0,
            seed: 7,
            target_weight: None,
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "restarts",
            reason: "must be greater than zero".to_owned(),
        })
    );

    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 1,
            restarts: 1,
            seed: 7,
            target_weight: Some(0),
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "target_weight",
            reason: "must be greater than zero when provided".to_owned(),
        })
    );
}

#[test]
fn validator_accepts_valid_upper_bound_result() {
    let code = trivial_one_qubit_code();
    let result = valid_result();

    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code: &code,
            known_exact_distance: Some(1),
        },
    )
    .unwrap();
}

#[test]
fn validator_rejects_exact_labeled_randomized_result() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.bound_type = BoundType::Exact;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "randomized-upper-bound results must use bound_type upper".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_wrong_method() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.method = DistanceBoundMethod::Exact;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "distance bound method must be randomized-upper-bound".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_mocked_underestimate_against_known_exact_distance() {
    let code = trivial_one_qubit_code();
    let result = valid_result();

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(2),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "upper_bound 1 is below known exact distance 2".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_witness_weight_mismatch() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.upper_bound = 2;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "upper_bound must equal witness weight".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_stabilizer_span_witness() {
    let x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    let code = StabilizerCode::from_stabilizers(1, vec![x0]).unwrap();
    let result = valid_result();

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ))
    );
}

#[test]
fn provenance_uses_current_package_version_and_method_revision() {
    let provenance = DistanceBoundProvenance::current();

    assert_eq!(provenance.tool, "qec-code");
    assert_eq!(provenance.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(provenance.method_revision, 1);
}

#[test]
fn completed_status_is_serialized_as_completed() {
    assert_eq!(
        serde_json::to_value(DistanceBoundStatus::Completed).unwrap(),
        "completed"
    );
}
