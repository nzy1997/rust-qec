use rstim::codegen::NoiseParams;
use rstim::codegen::css::{
    CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis, css_memory,
    parse_css_matrix_json, parse_css_observable_json,
};
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
fn sequential_css_memory_z_emits_detectors_observable_and_dem() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![],
            hz: vec![vec![0, 1]],
            num_data_qubits: 2,
        },
        rounds: 2,
        noise: NoiseParams::none(),
        basis: MemoryBasis::Z,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    };

    let circuit = css_memory(config).unwrap();
    let text = circuit_to_string(&circuit);

    assert!(text.contains("R 0"));
    assert!(text.contains("CX 0 2"));
    assert!(text.contains("M 0"));
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
fn explicit_observables_allow_redundant_orthogonal_checks() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1], vec![0, 1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    };

    let circuit = css_memory(config).unwrap();

    assert_eq!(stats::num_observables(&circuit), 1);
    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
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
fn parser_rejects_shape_and_format_edge_cases() {
    let empty_dense = r#"{"format":"dense","rows":[]}"#;
    let err = parse_css_matrix_json(empty_dense).unwrap_err().to_string();
    assert!(err.contains("width must be positive"), "error was: {err}");

    let ragged_dense = r#"{"format":"dense","rows":[[1,0],[1]]}"#;
    let err = parse_css_matrix_json(ragged_dense).unwrap_err().to_string();
    assert!(err.contains("dense row 1 has width 1"), "error was: {err}");

    let missing_sparse_width = r#"{"format":"sparse_rows","rows":[[]]}"#;
    let err = parse_css_matrix_json(missing_sparse_width)
        .unwrap_err()
        .to_string();
    assert!(err.contains("width must be positive"), "error was: {err}");

    let zero_sparse_width = r#"{"format":"sparse_rows","num_cols":0,"rows":[]}"#;
    let err = parse_css_matrix_json(zero_sparse_width)
        .unwrap_err()
        .to_string();
    assert!(err.contains("width must be positive"), "error was: {err}");

    let unknown_format = r#"{"format":"csr","rows":[]}"#;
    let err = parse_css_matrix_json(unknown_format)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("unknown CSS matrix format"),
        "error was: {err}"
    );

    let empty_observables = r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#;
    let err = parse_css_observable_json(empty_observables)
        .unwrap_err()
        .to_string();
    assert!(err.contains("width must be positive"), "error was: {err}");
}

#[test]
fn observable_json_uses_sparse_support_rows() {
    let logicals = r#"{"format":"sparse_rows","num_cols":4,"rows":[[0,2],[1,3]]}"#;
    let parsed = parse_css_observable_json(logicals).unwrap();

    assert_eq!(parsed.num_cols, 4);
    assert_eq!(parsed.rows, vec![vec![0, 2], vec![1, 3]]);
}

#[test]
fn css_memory_rejects_invalid_supports_and_empty_explicit_observables() {
    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.checks.num_data_qubits = 0;
    let err = css_memory(config).unwrap_err().to_string();
    assert!(err.contains("at least one data qubit"), "error was: {err}");

    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.checks.hx = vec![vec![0, 0]];
    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("hx row 0 repeats column 0"),
        "error was: {err}"
    );

    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.checks.hx.clear();
    config.checks.hz = vec![vec![2]];
    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("hz row 0 contains out-of-range column 2"),
        "error was: {err}"
    );

    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.observables = CssObservableSource::Explicit(vec![]);
    let err = css_memory(config).unwrap_err().to_string();
    assert!(err.contains("produced no observables"), "error was: {err}");
}

#[test]
fn greedy_schedule_packs_disjoint_cnots() {
    let sequential = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0], vec![1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    };
    let mut greedy = sequential.clone();
    greedy.schedule = CssSchedule::Greedy;

    let sequential_text = circuit_to_string(&css_memory(sequential).unwrap());
    let greedy_text = circuit_to_string(&css_memory(greedy).unwrap());

    assert!(sequential_text.contains("CX 2 0\nTICK\nCX 3 1"));
    assert!(greedy_text.contains("CX 2 0 3 1"));
}

#[test]
fn css_memory_places_requested_noise_channels() {
    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.noise = NoiseParams::uniform(0.125);

    let text = circuit_to_string(&css_memory(config).unwrap());

    assert!(text.contains("DEPOLARIZE1(0.125) 0"));
    assert!(text.contains("DEPOLARIZE2(0.125) 2 0"));
    assert!(text.contains("X_ERROR(0.125) 2"));
}

fn steane_h() -> Vec<Vec<usize>> {
    vec![vec![0, 3, 5, 6], vec![1, 3, 4, 6], vec![2, 4, 5, 6]]
}

#[test]
fn canonical_fallback_adds_steane_observable() {
    let h = steane_h();
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: h.clone(),
            hz: h,
            num_data_qubits: 7,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::CanonicalFallback,
    };

    let circuit = css_memory(config).unwrap();

    assert_eq!(stats::num_observables(&circuit), 1);
    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn explicit_or_canonical_prefers_explicit_observables() {
    let h = steane_h();
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: h.clone(),
            hz: h,
            num_data_qubits: 7,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::ExplicitOrCanonical(vec![vec![0, 1, 2]]),
    };

    let text = circuit_to_string(&css_memory(config).unwrap());

    assert!(text.contains("OBSERVABLE_INCLUDE(0) rec[-7] rec[-6] rec[-5]"));
}
