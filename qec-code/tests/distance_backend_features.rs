#![cfg(any(
    all(feature = "distance-ilp-highs", not(feature = "distance-ilp-gurobi")),
    all(feature = "distance-ilp-gurobi", not(feature = "distance-ilp-highs"))
))]

use qec_code::QecError;
use qec_code::codes::steane::Steane;
use qec_code::distance::compute_distance_with_solver_options;
use qec_code::distance_exact::{ExactCssDistanceBackend, ExactCssDistanceSolverOptions};

fn options(backend: ExactCssDistanceBackend) -> ExactCssDistanceSolverOptions {
    ExactCssDistanceSolverOptions {
        backend,
        ..ExactCssDistanceSolverOptions::default()
    }
}

#[cfg(all(feature = "distance-ilp-highs", not(feature = "distance-ilp-gurobi")))]
#[test]
fn highs_only_build_reports_explicit_gurobi_request_as_unavailable() {
    let code = Steane::new().unwrap();

    let err =
        compute_distance_with_solver_options(code.code(), options(ExactCssDistanceBackend::Gurobi))
            .unwrap_err();

    assert_eq!(err, QecError::IlpBackendUnavailable("Gurobi".into()));
}

#[cfg(all(feature = "distance-ilp-gurobi", not(feature = "distance-ilp-highs")))]
#[test]
fn gurobi_only_build_reports_explicit_highs_request_as_unavailable() {
    let code = Steane::new().unwrap();

    let err =
        compute_distance_with_solver_options(code.code(), options(ExactCssDistanceBackend::Highs))
            .unwrap_err();

    assert_eq!(err, QecError::IlpBackendUnavailable("Highs".into()));
}
