use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::LogicalClass;
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness, Issue225LadderCase,
    RandomizedUpperBoundOptions, randomized_css_upper_bound,
    validate_randomized_upper_bound_result, verify_issue_225_ladder_case,
};
use qec_code::{Pauli, QecError, StabilizerCode};

fn css_from_sparse_rows(num_cols: usize, hx: Vec<Vec<usize>>, hz: Vec<Vec<usize>>) -> CssCode {
    let hx = SparseRowsMatrix::new(num_cols, hx).unwrap().to_dense_rows();
    let hz = SparseRowsMatrix::new(num_cols, hz).unwrap().to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

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

fn issue_225_ladder_cases() -> Vec<Issue225LadderCase> {
    serde_json::from_str(include_str!("fixtures/distance/issue_225_ladder.json"))
        .expect("issue-225 ladder fixture should deserialize")
}

fn issue_225_case(case_id: &str) -> Issue225LadderCase {
    issue_225_ladder_cases()
        .into_iter()
        .find(|case| case.case_id == case_id)
        .expect("requested issue-225 ladder case should exist")
}

fn css_from_built_in_code_id(code_id: &str) -> CssCode {
    let checks = built_in_css_checks(code_id).unwrap();
    css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz)
}

fn x_only_witness(num_qubits: usize, support: &[usize]) -> DistanceBoundWitness {
    let mut x = vec![0; num_qubits];
    for &qubit in support {
        x[qubit] = 1;
    }
    let pauli = Pauli::from_xz_bits(x, vec![0; num_qubits]).unwrap();
    DistanceBoundWitness::from_pauli(&pauli)
}

fn surface_rotated_d5_result_with_x_support(support: &[usize]) -> DistanceBoundResult {
    DistanceBoundResult::completed(
        support.len(),
        LogicalClass::XLike,
        x_only_witness(25, support),
        RandomizedUpperBoundOptions {
            iterations: 5000,
            restarts: 8,
            seed: 225,
            target_weight: Some(5),
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
            "distance bound results must use bound_type upper".to_owned(),
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
            "expected method randomized-upper-bound, got exact".to_owned(),
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
fn validator_rejects_serialized_witness_weight_that_disagrees_with_pauli_weight() {
    let code = StabilizerCode::from_stabilizers(2, vec![]).unwrap();
    let mut result = valid_result();
    result.witness = DistanceBoundWitness {
        x: vec![1, 1],
        z: vec![0, 0],
        weight: 1,
    };

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness weight field must equal Pauli weight".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_witness_width_that_differs_from_code_length() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.witness = DistanceBoundWitness {
        x: vec![1, 0],
        z: vec![0, 0],
        weight: 1,
    };

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness width must match code length".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_invalid_result_options() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.options.iterations = 0;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        })
    );
}

#[test]
fn validator_rejects_logical_class_that_disagrees_with_witness_support() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.logical_class = LogicalClass::ZLike;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "logical_class must match witness support".to_owned(),
        ))
    );
}

#[test]
fn validator_accepts_z_like_and_mixed_witness_classes() {
    let code = trivial_one_qubit_code();
    let mut z_like = valid_result();
    z_like.logical_class = LogicalClass::ZLike;
    z_like.witness = DistanceBoundWitness {
        x: vec![0],
        z: vec![1],
        weight: 1,
    };

    validate_randomized_upper_bound_result(
        &z_like,
        BoundValidationContext {
            code: &code,
            known_exact_distance: Some(1),
        },
    )
    .unwrap();

    let mut mixed = valid_result();
    mixed.logical_class = LogicalClass::Mixed;
    mixed.witness = DistanceBoundWitness {
        x: vec![1],
        z: vec![1],
        weight: 1,
    };

    validate_randomized_upper_bound_result(
        &mixed,
        BoundValidationContext {
            code: &code,
            known_exact_distance: Some(1),
        },
    )
    .unwrap();
}

#[test]
fn validator_rejects_zero_completed_upper_bound() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.upper_bound = 0;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "completed upper_bound must be positive".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_identity_witness_even_with_positive_declared_weight() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.witness = DistanceBoundWitness {
        x: vec![0],
        z: vec![0],
        weight: 1,
    };

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_noncommuting_witness() {
    let z0 = Pauli::from_xz_bits(vec![0], vec![1]).unwrap();
    let code = StabilizerCode::from_stabilizers(1, vec![z0]).unwrap();
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
            "witness does not commute with stabilizers".to_owned(),
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
fn issue_225_ladder_verifier_accepts_exact_upper_bounds_and_rejects_loose_bounds() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let exact = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);

    verify_issue_225_ladder_case(
        &case,
        &exact,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .unwrap();

    let loose = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4, 9, 14]);
    let error = verify_issue_225_ladder_case(
        &case,
        &loose,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected loose bound rejection");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 expected upper_bound <= 5, got 7".to_owned(),
        )
    );
}

#[test]
fn issue_225_ladder_verifier_rejects_unvalidated_witness() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);

    let stabilizer_span = surface_rotated_d5_result_with_x_support(&[0, 5]);
    let span_error = verify_issue_225_ladder_case(
        &case,
        &stabilizer_span,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected stabilizer-span witness rejection");
    assert_eq!(
        span_error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 witness lies in stabilizer span".to_owned(),
        )
    );

    let mut mismatched_weight = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);
    mismatched_weight.witness.weight = 4;
    let weight_error = verify_issue_225_ladder_case(
        &case,
        &mismatched_weight,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected serialized witness weight rejection");
    assert_eq!(
        weight_error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 upper_bound must equal witness weight".to_owned(),
        )
    );
}

#[test]
fn issue_225_ladder_verifier_rejects_wrong_method_label() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let result = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);

    let error = verify_issue_225_ladder_case(
        &case,
        &result,
        &css,
        DistanceBoundMethod::RandomWindowUpperBound,
    )
    .expect_err("expected method mismatch rejection");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 expected method random-window-upper-bound, got randomized-upper-bound"
                .to_owned(),
        )
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

#[test]
fn randomized_upper_bound_reproducible_for_same_seed() {
    let checks = built_in_css_checks("steane").unwrap();
    let css = css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz);
    let options = RandomizedUpperBoundOptions {
        iterations: 500,
        restarts: 4,
        seed: 7,
        target_weight: Some(3),
    };

    let first = randomized_css_upper_bound(&css, options.clone()).unwrap();
    let second = randomized_css_upper_bound(&css, options).unwrap();

    assert_eq!(first, second);
}

#[test]
fn randomized_upper_bound_finds_steane_distance_under_pinned_options() {
    let checks = built_in_css_checks("steane").unwrap();
    let css = css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz);

    let result = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: 500,
            restarts: 4,
            seed: 7,
            target_weight: Some(3),
        },
    )
    .unwrap();

    assert_eq!(result.upper_bound, 3);
    assert_eq!(result.witness.weight, 3);
}

#[test]
fn randomized_upper_bound_finds_repetition_css_distance_under_pinned_options() {
    let css = css_from_sparse_rows(3, vec![vec![0, 1], vec![1, 2]], vec![]);

    let result = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: 20,
            restarts: 1,
            seed: 11,
            target_weight: Some(1),
        },
    )
    .unwrap();

    assert_eq!(result.upper_bound, 1);
    assert_eq!(result.logical_class, LogicalClass::XLike);
}

#[test]
fn randomized_upper_bound_returns_best_witness_after_exhausting_iterations() {
    let css = css_from_sparse_rows(3, vec![vec![0, 1], vec![1, 2]], vec![]);

    let result = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: 20,
            restarts: 1,
            seed: 11,
            target_weight: None,
        },
    )
    .unwrap();

    assert_eq!(result.upper_bound, 1);
    assert_eq!(result.options.target_weight, None);
}

#[test]
fn randomized_upper_bound_rejects_invalid_options_before_search() {
    let css = css_from_sparse_rows(1, vec![vec![0]], vec![]);

    assert_eq!(
        randomized_css_upper_bound(
            &css,
            RandomizedUpperBoundOptions {
                iterations: 0,
                restarts: 1,
                seed: 7,
                target_weight: None,
            },
        ),
        Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        })
    );
}

#[test]
fn randomized_upper_bound_rejects_zero_logical_qubit_code() {
    let css = css_from_sparse_rows(1, vec![vec![0]], vec![]);

    assert_eq!(
        randomized_css_upper_bound(
            &css,
            RandomizedUpperBoundOptions {
                iterations: 10,
                restarts: 1,
                seed: 7,
                target_weight: None,
            },
        ),
        Err(QecError::DistanceWitnessNotFound)
    );
}
