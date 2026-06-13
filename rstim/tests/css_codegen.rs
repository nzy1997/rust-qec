use rstim::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::NoiseParams;

#[test]
fn css_memory_rejects_zero_rounds() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0]],
            hz: vec![],
            num_data_qubits: 1,
        },
        rounds: 0,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();
    assert!(err.contains("rounds must be >= 1"), "error was: {err}");
}

#[test]
fn dense_and_sparse_json_normalize_to_same_supports() {
    let dense = r#"{"format":"dense","rows":[[1,0,1],[0,1,1]]}"#;
    let sparse = r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,2],[1,2]]}"#;

    let dense_matrix = parse_css_matrix_json(dense).unwrap();
    let sparse_matrix = parse_css_matrix_json(sparse).unwrap();

    assert_eq!(dense_matrix.num_cols, 3);
    assert_eq!(dense_matrix.rows, vec![vec![0, 2], vec![1, 2]]);
    assert_eq!(dense_matrix, sparse_matrix);
}

#[test]
fn parser_rejects_bad_dense_and_sparse_inputs() {
    let bad_dense = r#"{"format":"dense","rows":[[1,2]]}"#;
    let err = parse_css_matrix_json(bad_dense).unwrap_err().to_string();
    assert!(err.contains("non-binary entry 2"), "error was: {err}");

    let repeated_sparse = r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,0]]}"#;
    let err = parse_css_matrix_json(repeated_sparse)
        .unwrap_err()
        .to_string();
    assert!(err.contains("repeats column 0"), "error was: {err}");

    let out_of_range = r#"{"format":"sparse_rows","num_cols":3,"rows":[[3]]}"#;
    let err = parse_css_matrix_json(out_of_range).unwrap_err().to_string();
    assert!(err.contains("out-of-range column 3"), "error was: {err}");
}

#[test]
fn observable_json_uses_sparse_support_rows() {
    let logicals = r#"{"format":"sparse_rows","num_cols":4,"rows":[[0,2],[1,3]]}"#;
    let parsed = parse_css_observable_json(logicals).unwrap();

    assert_eq!(parsed.num_cols, 4);
    assert_eq!(parsed.rows, vec![vec![0, 2], vec![1, 3]]);
}
