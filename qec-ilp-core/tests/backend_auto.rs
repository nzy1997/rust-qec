#[cfg(not(feature = "gurobi"))]
use qec_ilp_core::BinaryIlpError;
use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelVar,
};

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
