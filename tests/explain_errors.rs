use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::explain_errors::explain;

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
