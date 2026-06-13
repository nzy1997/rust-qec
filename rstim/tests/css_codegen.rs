use rstim::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::NoiseParams;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::circuit_to_string;
use rstim::stats;

fn repetition_like_css_config(rounds: usize, basis: MemoryBasis) -> CssMemoryConfig {
    CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds,
        noise: NoiseParams::none(),
        basis,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    }
}

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
fn sequential_css_memory_x_emits_detectors_observable_and_dem() {
    let circuit = css_memory(repetition_like_css_config(2, MemoryBasis::X)).unwrap();
    let text = circuit_to_string(&circuit);

    assert!(text.contains("QUBIT_COORDS(0) 0"));
    assert!(text.contains("RX 0"));
    assert!(text.contains("H 2"));
    assert!(text.contains("CX 2 0"));
    assert!(text.contains("MX 0"));
    assert_eq!(stats::num_detectors(&circuit), 3);
    assert_eq!(stats::num_observables(&circuit), 1);

    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn css_memory_rejects_non_orthogonal_checks() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0]],
            hz: vec![vec![0]],
            num_data_qubits: 1,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("CSS X/Z checks are not orthogonal"),
        "error was: {err}"
    );
}

#[test]
fn css_memory_rejects_out_of_range_observable_support() {
    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.observables = CssObservableSource::Explicit(vec![vec![2]]);

    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("observable 0 references data qubit 2"),
        "error was: {err}"
    );
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
