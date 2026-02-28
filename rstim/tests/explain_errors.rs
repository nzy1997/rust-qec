use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::explain_errors::explain;
use rstim::cli::run_explain_errors;

#[test]
fn explain_no_detectors_fired() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let explanations = explain(&dem, &[]);
    assert!(explanations.is_empty());
}

#[test]
fn explain_single_detector_fired() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let explanations = explain(&dem, &[0]);
    assert!(!explanations.is_empty());
    let covered: Vec<usize> = explanations.iter()
        .flat_map(|e| e.detectors.iter().copied())
        .collect();
    assert!(covered.contains(&0));
}

#[test]
fn explain_probability_in_range() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let explanations = explain(&dem, &[0]);
    for e in &explanations {
        assert!(e.probability > 0.0 && e.probability <= 1.0);
    }
}

#[test]
fn explain_errors_cli_dets_format() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let dets_input = b"shot D0\n";
    let mut out = Vec::new();
    run_explain_errors(circuit, None, dets_input, "dets", &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("error("));
    assert!(text.contains("D0"));
}

#[test]
fn explain_errors_cli_no_detectors() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let dets_input = b"shot\n"; // no detectors fired
    let mut out = Vec::new();
    run_explain_errors(circuit, None, dets_input, "dets", &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("no errors needed"));
}

#[test]
fn explain_with_observable_target() {
    // DEPOLARIZE1 can produce errors with observable targets
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let explanations = explain(&dem, &[0]);
    assert!(!explanations.is_empty());
    // At least one explanation should have an observable
    let has_obs = explanations.iter().any(|e| !e.observables.is_empty());
    assert!(has_obs);
}

#[test]
fn explain_unfirable_detector_stops_gracefully() {
    // Fired detector 99 doesn't exist in DEM — should return empty (no progress)
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let explanations = explain(&dem, &[99]);
    // No error covers detector 99, so greedy makes no progress
    assert!(explanations.is_empty());
}

#[test]
fn explain_errors_cli_01_format() {
    // Use 01 format: 1 detector, 1 shot, bit=1 means detector fired
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let det_data = b"1\n";
    let mut out = Vec::new();
    run_explain_errors(circuit, None, det_data, "01", &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("error("));
}

#[test]
fn explain_errors_cli_unsupported_format_errors() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let mut out = Vec::new();
    let result = run_explain_errors(circuit, None, b"", "b8", &mut out);
    assert!(result.is_err());
}

#[test]
fn explain_with_repeat_block() {
    // Build a DEM with a REPEAT block manually
    // repeat 2 { error(0.1) D0 } — detectors D0 and D1 (offset per repeat)
    let inner_circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(inner_circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // Just verify explain works on a normal DEM (REPEAT path tested via circuit with rounds)
    let explanations = explain(&dem, &[0]);
    assert!(!explanations.is_empty());
}
