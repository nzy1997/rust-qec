use qec_ilp_core::BinaryIlpError;
use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint, ModelVar,
};
#[cfg(not(feature = "gurobi"))]
use qec_ilp_core::BackendConfig;

fn simple_model() -> BinaryIlpModel {
    BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "x".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![ModelVar {
            name: "t".into(),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        }],
        constraints: vec![LinearConstraint {
            name: "parity".into(),
            sense: ConstraintSense::Eq,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![(0, -2.0)],
            rhs: 1.0,
        }],
        solution_binary_prefix_len: 1,
    }
}

#[test]
fn auto_backend_falls_back_to_highs() {
    let mut backend = build_binary_backend(&simple_model(), &BinaryIlpConfig::default()).unwrap();

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[test]
fn binary_backend_trait_objects_have_a_stable_debug_name() {
    let mut config = BinaryIlpConfig::default();
    config.backend.kind = BackendKind::Highs;

    let backend = build_binary_backend(&simple_model(), &config).unwrap();

    assert_eq!(format!("{backend:?}"), "BinaryBackend(..)");
}

#[cfg(not(feature = "gurobi"))]
#[test]
fn explicit_gurobi_selection_reports_unavailable_without_feature() {
    let err = build_binary_backend(
        &simple_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Gurobi,
                time_limit_seconds: None,
                mip_gap: None,
                threads: None,
                verbose: false,
            },
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        BinaryIlpError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        }
    );
}

#[test]
fn unknown_binary_variable_references_are_rejected_before_backend_build() {
    let mut model = simple_model();
    model.constraints[0].binary_terms[0].0 = 1;

    let err = build_binary_backend(&model, &BinaryIlpConfig::default()).unwrap_err();

    assert_eq!(err, BinaryIlpError::UnknownBinaryVar(1));
}

#[test]
fn unknown_integer_variable_references_are_rejected_before_backend_build() {
    let mut model = simple_model();
    model.constraints[0].integer_terms[0].0 = 1;

    let err = build_binary_backend(&model, &BinaryIlpConfig::default()).unwrap_err();

    assert_eq!(err, BinaryIlpError::UnknownIntegerVar(1));
}

#[test]
fn oversized_solution_prefix_is_rejected_before_backend_build() {
    let mut model = simple_model();
    model.solution_binary_prefix_len = 2;

    let err = build_binary_backend(&model, &BinaryIlpConfig::default()).unwrap_err();

    assert_eq!(err, BinaryIlpError::UnknownBinaryVar(2));
}

#[test]
fn invalid_binary_variable_bounds_are_rejected_before_backend_build() {
    let mut model = simple_model();
    model.binary_vars[0].upper = 2.0;

    let err = build_binary_backend(&model, &BinaryIlpConfig::default()).unwrap_err();

    assert_eq!(
        err,
        BinaryIlpError::InvalidBinaryVarBounds {
            index: 0,
            lower: 0.0,
            upper: 2.0,
        }
    );
}
