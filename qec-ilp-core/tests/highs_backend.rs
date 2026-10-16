use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelVar,
};

fn single_column_flip_model() -> BinaryIlpModel {
    BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "e0".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![ModelVar {
            name: "t0".into(),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        }],
        constraints: vec![LinearConstraint {
            name: "row0".into(),
            sense: ConstraintSense::Eq,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![(0, -2.0)],
            rhs: 1.0,
        }],
        solution_binary_prefix_len: 1,
    }
}

#[test]
fn highs_solves_a_single_binary_parity_problem() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    assert_eq!(backend.kind(), BackendKind::Highs);

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[test]
fn highs_respects_optional_solver_settings() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: Some(1.0),
                mip_gap: Some(0.05),
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[test]
fn highs_backend_supports_mutating_one_rhs_between_solves() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    assert_eq!(backend.solve().unwrap().binary_values, vec![true]);
    backend.set_rhs(0, 0.0).unwrap();
    assert_eq!(backend.solve().unwrap().binary_values, vec![false]);
}

#[test]
fn highs_backend_supports_mutating_le_rhs_between_solves() {
    let model = BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "x".into(),
            objective: -1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![],
        constraints: vec![LinearConstraint {
            name: "cap".into(),
            sense: ConstraintSense::Le,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        }],
        solution_binary_prefix_len: 1,
    };
    let mut backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    assert_eq!(backend.solve().unwrap().binary_values, vec![false]);
    backend.set_rhs(0, 1.0).unwrap();
    assert_eq!(backend.solve().unwrap().binary_values, vec![true]);
}

#[test]
fn highs_backend_supports_mutating_ge_rhs_between_solves() {
    let model = BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "x".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![],
        constraints: vec![LinearConstraint {
            name: "floor".into(),
            sense: ConstraintSense::Ge,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![],
            rhs: 1.0,
        }],
        solution_binary_prefix_len: 1,
    };
    let mut backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    assert_eq!(backend.solve().unwrap().binary_values, vec![true]);
    backend.set_rhs(0, 0.0).unwrap();
    assert_eq!(backend.solve().unwrap().binary_values, vec![false]);
}

#[test]
fn highs_backend_remains_usable_after_a_solve_error() {
    let model = BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "x".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![ModelVar {
            name: "bad".into(),
            objective: 0.0,
            lower: 1.0,
            upper: 0.0,
        }],
        constraints: vec![LinearConstraint {
            name: "row0".into(),
            sense: ConstraintSense::Eq,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![(0, 0.0)],
            rhs: 0.0,
        }],
        solution_binary_prefix_len: 1,
    };
    let mut backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let first = backend.solve().unwrap_err();
    let second = backend.solve().unwrap_err();

    let first_message = format!("{first}");
    assert!(
        !first_message.contains("model already in use"),
        "unexpected poisoned backend error: {first_message}"
    );
    let second_message = format!("{second}");
    assert!(
        !second_message.contains("model already in use"),
        "unexpected poisoned backend error: {second_message}"
    );
}
