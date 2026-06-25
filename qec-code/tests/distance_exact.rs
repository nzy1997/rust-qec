use qec_code::distance::{DistanceResult, LogicalClass};
use qec_code::distance_exact::{
    ExactCssDistanceBackend, ExactCssDistanceInput, ExactCssDistanceOptions,
    ExactCssDistanceProvenance, ExactCssDistanceResult, ExactCssDistanceSolverOptions,
    ExactCssDistanceSolverReport, ExactCssDistanceSolverStatus, ExactCssDistanceStatus,
    ExactDistanceBoundType,
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
            solver: ExactCssDistanceSolverOptions::default(),
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
    assert_eq!(
        json["provenance"]["tool_version"],
        env!("CARGO_PKG_VERSION")
    );
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
            solver: ExactCssDistanceSolverOptions::default(),
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

#[test]
fn exact_css_distance_result_serializes_solver_provenance_for_completed_runs() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: Some(300.0),
                mip_gap: Some(0.0),
                threads: Some(2),
                verbose_solver: true,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::Optimal,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "optimal");
    assert_eq!(json["time_limit_seconds"], 300.0);
    assert_eq!(json["mip_gap"], 0.0);
    assert_eq!(json["threads"], 2);
    assert_eq!(json["verbose_solver"], true);
    assert_eq!(json["options"]["backend"], "highs");
    assert_eq!(json["options"]["time_limit_seconds"], 300.0);
    assert_eq!(json["options"]["mip_gap"], 0.0);
    assert_eq!(json["options"]["threads"], 2);
    assert_eq!(json["options"]["verbose_solver"], true);
}

#[test]
fn exact_css_distance_result_serializes_positive_mip_gap_optimal_as_incomplete_upper_bound() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: None,
                mip_gap: Some(0.001),
                threads: None,
                verbose_solver: false,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::Optimal,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "incomplete");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["solver_status"], "optimal");
    assert_eq!(json["mip_gap"], 0.001);
}

#[test]
fn exact_css_distance_result_serializes_time_limited_incumbent_as_upper_bound() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: Some(0.001),
                mip_gap: None,
                threads: None,
                verbose_solver: false,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::TimeLimit,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "timeout");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "time_limit");
    assert_eq!(json["time_limit_seconds"], 0.001);
}

#[test]
fn exact_css_distance_result_serializes_solution_limited_incumbent_as_incomplete_upper_bound() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: None,
                mip_gap: Some(0.01),
                threads: Some(1),
                verbose_solver: false,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::SolutionLimit,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "incomplete");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "solution_limit");
    assert_eq!(json["mip_gap"], 0.01);
    assert_eq!(json["threads"], 1);
}

#[test]
fn exact_css_distance_result_serializes_suboptimal_incumbent_as_incomplete_upper_bound() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: None,
                verbose_solver: false,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::SubOptimal,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "incomplete");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "sub_optimal");
}

#[test]
fn exact_css_distance_result_deserializes_legacy_json_with_default_solver_options() {
    let legacy_json = serde_json::json!({
        "status": "completed",
        "distance": 2,
        "method": "rstim-ilp-exact",
        "bound_type": "exact",
        "logical_class": "x_like",
        "witness": {
            "x": [1, 0, 1],
            "z": [0, 0, 0],
            "weight": 2
        },
        "options": {
            "input": "code_id",
            "code_id": "surface_rotated:d=3"
        },
        "provenance": {
            "tool": "qec-code",
            "tool_version": "0.1.0",
            "method_revision": 1
        }
    });

    let result: ExactCssDistanceResult = serde_json::from_value(legacy_json).unwrap();

    assert_eq!(result.status, ExactCssDistanceStatus::Completed);
    assert_eq!(result.bound_type, ExactDistanceBoundType::Exact);
    assert_eq!(result.requested_backend, ExactCssDistanceBackend::Auto);
    assert_eq!(result.backend, None);
    assert_eq!(result.solver_status, None);
    assert_eq!(result.time_limit_seconds, None);
    assert_eq!(result.mip_gap, None);
    assert_eq!(result.threads, None);
    assert_eq!(
        result.options.solver,
        ExactCssDistanceSolverOptions::default()
    );
}
