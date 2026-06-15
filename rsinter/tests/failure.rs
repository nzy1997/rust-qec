use std::str::FromStr;

use rsinter::failure::{classify_completed, classify_error, combine_failure_kind, FailureKind};

#[test]
fn failure_kind_public_string_matrix_is_stable() {
    assert_eq!(FailureKind::Ok.as_str(), "ok");
    assert_eq!(FailureKind::LogicalFailure.as_str(), "logical_failure");
    assert_eq!(FailureKind::Timeout.as_str(), "timeout");
    assert_eq!(FailureKind::SolverFailure.as_str(), "solver_failure");
    assert_eq!(FailureKind::Unsupported.as_str(), "unsupported");
    assert_eq!(FailureKind::SamplerError.as_str(), "sampler_error");

    assert_eq!(FailureKind::Ok.status(), "ok");
    assert_eq!(FailureKind::LogicalFailure.status(), "ok");
    assert_eq!(FailureKind::Timeout.status(), "ok");
    assert_eq!(FailureKind::SolverFailure.status(), "error");
    assert_eq!(FailureKind::Unsupported.status(), "error");
    assert_eq!(FailureKind::SamplerError.status(), "error");

    assert_eq!(FailureKind::default(), FailureKind::Ok);
    assert_eq!(FailureKind::Ok.to_string(), "ok");
    assert_eq!(FailureKind::LogicalFailure.to_string(), "logical_failure");
    assert_eq!(FailureKind::Timeout.to_string(), "timeout");
    assert_eq!(FailureKind::SolverFailure.to_string(), "solver_failure");
    assert_eq!(FailureKind::Unsupported.to_string(), "unsupported");
    assert_eq!(FailureKind::SamplerError.to_string(), "sampler_error");
}

#[test]
fn failure_kind_json_round_trips_each_variant() {
    let ok_json = serde_json::to_string(&FailureKind::Ok).unwrap();
    let logical_json = serde_json::to_string(&FailureKind::LogicalFailure).unwrap();
    let timeout_json = serde_json::to_string(&FailureKind::Timeout).unwrap();
    let solver_json = serde_json::to_string(&FailureKind::SolverFailure).unwrap();
    let unsupported_json = serde_json::to_string(&FailureKind::Unsupported).unwrap();
    let sampler_json = serde_json::to_string(&FailureKind::SamplerError).unwrap();

    assert_eq!(ok_json, "\"ok\"");
    assert_eq!(logical_json, "\"logical_failure\"");
    assert_eq!(timeout_json, "\"timeout\"");
    assert_eq!(solver_json, "\"solver_failure\"");
    assert_eq!(unsupported_json, "\"unsupported\"");
    assert_eq!(sampler_json, "\"sampler_error\"");

    let ok: FailureKind = serde_json::from_str(&ok_json).unwrap();
    let logical: FailureKind = serde_json::from_str(&logical_json).unwrap();
    let timeout: FailureKind = serde_json::from_str(&timeout_json).unwrap();
    let solver: FailureKind = serde_json::from_str(&solver_json).unwrap();
    let unsupported: FailureKind = serde_json::from_str(&unsupported_json).unwrap();
    let sampler: FailureKind = serde_json::from_str(&sampler_json).unwrap();

    assert_eq!(ok, FailureKind::Ok);
    assert_eq!(logical, FailureKind::LogicalFailure);
    assert_eq!(timeout, FailureKind::Timeout);
    assert_eq!(solver, FailureKind::SolverFailure);
    assert_eq!(unsupported, FailureKind::Unsupported);
    assert_eq!(sampler, FailureKind::SamplerError);
}

#[test]
fn failure_kind_from_str_accepts_only_snake_case_names() {
    assert_eq!(FailureKind::from_str("ok").unwrap(), FailureKind::Ok);
    assert_eq!(
        FailureKind::from_str("logical_failure").unwrap(),
        FailureKind::LogicalFailure
    );
    assert_eq!(
        FailureKind::from_str("timeout").unwrap(),
        FailureKind::Timeout
    );
    assert_eq!(
        FailureKind::from_str("solver_failure").unwrap(),
        FailureKind::SolverFailure
    );
    assert_eq!(
        FailureKind::from_str("unsupported").unwrap(),
        FailureKind::Unsupported
    );
    assert_eq!(
        FailureKind::from_str("sampler_error").unwrap(),
        FailureKind::SamplerError
    );

    assert!(FailureKind::from_str("").is_err());
    assert!(FailureKind::from_str("logical-failure").is_err());
    assert!(FailureKind::from_str("sampler error").is_err());
    assert!(FailureKind::from_str("unknown")
        .unwrap_err()
        .contains("unknown failure_kind"));
}

#[test]
fn failure_classifiers_cover_completion_error_and_priority_rules() {
    assert_eq!(classify_completed(0, false), FailureKind::Ok);
    assert_eq!(classify_completed(7, false), FailureKind::LogicalFailure);
    assert_eq!(classify_completed(0, true), FailureKind::Timeout);
    assert_eq!(classify_completed(7, true), FailureKind::Timeout);

    assert_eq!(
        classify_error("BackendUnavailable: gurobi", FailureKind::SolverFailure),
        FailureKind::Unsupported
    );
    assert_eq!(
        classify_error("backend unavailable: highs", FailureKind::SolverFailure),
        FailureKind::Unsupported
    );
    assert_eq!(
        classify_error("backend is unavailable", FailureKind::SolverFailure),
        FailureKind::Unsupported
    );
    assert_eq!(
        classify_error("no ILP backend is available", FailureKind::SolverFailure),
        FailureKind::Unsupported
    );
    assert_eq!(
        classify_error(
            "unsupported detector error model",
            FailureKind::SolverFailure
        ),
        FailureKind::Unsupported
    );
    assert_eq!(
        classify_error("solver iteration limit reached", FailureKind::SolverFailure),
        FailureKind::SolverFailure
    );
    assert_eq!(
        classify_error("sampling failed", FailureKind::SamplerError),
        FailureKind::SamplerError
    );

    assert_eq!(
        combine_failure_kind(FailureKind::Ok, FailureKind::LogicalFailure),
        FailureKind::LogicalFailure
    );
    assert_eq!(
        combine_failure_kind(FailureKind::Timeout, FailureKind::SamplerError),
        FailureKind::SamplerError
    );
    assert_eq!(
        combine_failure_kind(FailureKind::SolverFailure, FailureKind::Unsupported),
        FailureKind::Unsupported
    );
    assert_eq!(
        combine_failure_kind(FailureKind::Unsupported, FailureKind::SolverFailure),
        FailureKind::Unsupported
    );
}
