use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use rstim::codegen::repetition_code_memory;
use rstim::codegen::surface_code::rotated_memory_x;
use std::collections::BTreeMap;

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

#[test]
fn decompose_errors_rejects_non_graphlike_error_without_graphlike_basis() {
    let circuit = parse_lines(
        "\
R 0 1 2
X_ERROR(0.1) 0
CX 0 1
CX 1 2
M 0 1 2
DETECTOR rec[-3]
DETECTOR rec[-2]
DETECTOR rec[-1]
",
    )
    .unwrap();

    let err = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap_err();
    assert!(err.contains("decompose") || err.contains("graphlike"), "{err}");
}

#[test]
fn decompose_errors_matches_stim_semantics_for_minimal_rep_code_boundary_case() {
    let circuit = repetition_code_memory(3, 1, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let expected = "\
error(0.009304831745666962) D0 D1
error(0.02236793284649183) D0 D2
error(0.005333333333333312) D0 D3
error(0.009304831745666962) D0 L0
error(0.009304831745666962) D1
error(0.02236793284649183) D1 D3
error(0.01262033963927737) D2 D3
error(0.01262033963927737) D2 L0
error(0.002673815958446298) D2 L0 ^ D0 L0
error(0.01262033963927737) D3
error(0.002673815958446298) D3 ^ D1
detector(1, 0, 0) D0
detector(3, 0, 0) D1
detector(1, 0, 1) D2
detector(3, 0, 1) D3
";
    let expected_map = parse_error_multiset(expected);
    let actual_map = parse_error_multiset(&dem.to_string());

    assert_eq!(
        actual_map.keys().collect::<Vec<_>>(),
        expected_map.keys().collect::<Vec<_>>(),
        "{}",
        dem,
    );
    assert_prob_multimaps_close(
        &expected_map,
        &actual_map,
        &dem.to_string(),
    );
}

#[test]
fn decompose_errors_merges_duplicate_decomposed_targets_for_small_surface_code() {
    let circuit = rotated_memory_x(3, 2, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let errors = parse_error_multiset(&dem.to_string());

    assert_eq!(errors.get("D11 ^ D14"), Some(&vec![0.007318633932043431]));
    assert_eq!(errors.get("D3 ^ D9"), Some(&vec![0.005333333333333312]));
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

fn parse_error_multiset(text: &str) -> BTreeMap<String, Vec<f64>> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("error(") {
            if let Some(end) = rest.find(')') {
                let prob: f64 = rest[..end].parse().unwrap();
                let targets = canonicalize_error_targets(rest[end + 1..].trim());
                out.entry(targets).or_insert_with(Vec::new).push(prob);
            }
        }
    }
    for probs in out.values_mut() {
        probs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    out
}

fn assert_prob_multimaps_close(
    expected: &BTreeMap<String, Vec<f64>>,
    actual: &BTreeMap<String, Vec<f64>>,
    context: &str,
) {
    assert_eq!(expected.len(), actual.len(), "{context}");
    for (key, expected_probs) in expected {
        let actual_probs = actual.get(key).unwrap_or_else(|| panic!("{context}"));
        assert_eq!(expected_probs.len(), actual_probs.len(), "{context}");
        for (expected_prob, actual_prob) in expected_probs.iter().zip(actual_probs.iter()) {
            let scale = expected_prob.abs().max(actual_prob.abs()).max(1.0);
            let diff = (expected_prob - actual_prob).abs();
            assert!(diff <= 1e-12 * scale, "{context}");
        }
    }
}

fn canonicalize_error_targets(targets: &str) -> String {
    let mut components: Vec<String> = targets
        .split('^')
        .map(|component| {
            let mut terms: Vec<&str> = component.split_whitespace().collect();
            terms.sort();
            terms.join(" ")
        })
        .filter(|component| !component.is_empty())
        .collect();
    components.sort();
    components.join(" ^ ")
}
