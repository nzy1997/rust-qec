use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use rstim::codegen::repetition_code_memory;

#[test]
fn decompose_already_graphlike_unchanged() {
    // Rep code with X_ERROR only produces graphlike errors
    let circuit = parse_lines("
        R 0 1 2
        X_ERROR(0.1) 0 1
        M 0 1 2
        DETECTOR rec[-3] rec[-2]
        DETECTOR rec[-2] rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
    ").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let text = dem.to_string();
    // All errors should have <=2 detectors per component
    assert_all_graphlike(&text);
}

#[test]
fn decompose_rep_code() {
    let circuit = repetition_code_memory(5, 3, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    assert_all_graphlike(&dem.to_string());
}

#[test]
fn decompose_no_errors_is_fine() {
    let circuit = parse_lines("R 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    assert_eq!(dem.to_string().matches("error").count(), 0);
}

fn assert_all_graphlike(text: &str) {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("error") {
            // Extract everything after the probability
            if let Some(targets_part) = line.split(')').nth(1) {
                let components: Vec<&str> = targets_part.split('^').collect();
                for comp in &components {
                    let det_count = comp.matches('D').count();
                    assert!(det_count <= 2, "non-graphlike component in: {}", line);
                }
            }
        }
    }
}
