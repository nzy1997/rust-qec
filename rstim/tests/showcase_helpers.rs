use rstim::dem::DetectorErrorModel;
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary, median_duration_ns, render_markdown_table, showcase_cases,
    strip_comment_preamble, structural_circuit_summary,
};
use std::time::Duration;

#[test]
fn showcase_cases_cover_expected_matrix() {
    let labels: Vec<String> = showcase_cases().into_iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), 6);
    assert!(labels.contains(&"repetition_code/memory d=5 r=5".to_string()));
    assert!(labels.contains(&"repetition_code/memory d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=13 r=13".to_string()));
}

#[test]
fn strip_comment_preamble_drops_leading_stim_header_only() {
    let text = "# header\n# header\nR 0\n# inline stays comment to parser\nM 0\n";
    assert_eq!(
        strip_comment_preamble(text),
        "R 0\n# inline stays comment to parser\nM 0\n"
    );
}

#[test]
fn structural_circuit_summary_counts_repeat_and_annotations() {
    let instrs = parse_lines(
        "QUBIT_COORDS(1, 2) 0\nR 0\nREPEAT 2 {\n    M 0\n    DETECTOR(1, 0) rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let summary = structural_circuit_summary(&instrs);
    assert_eq!(summary.measurements, 2);
    assert_eq!(summary.detectors, 2);
    assert_eq!(summary.observables, 1);
    assert_eq!(summary.opcode_counts["M"], 2);
    assert!(summary.qubit_coords.contains("QUBIT_COORDS(1,2) 0"));
}

#[test]
fn dem_semantic_summary_flattens_repeat_blocks_and_shifted_detectors() {
    let dem = DetectorErrorModel::parse(
        "error(0.125) D0\nrepeat 2 {\n    error(0.25) D0 D1\n    shift_detectors 2\n    detector(5, 0) D0\n}\n",
    )
    .unwrap();
    let summary = dem_semantic_summary(&dem);
    assert!(summary.error_probabilities.contains_key("D0"));
    assert!(summary.error_probabilities.contains_key("D0 D1"));
    assert!(summary
        .annotation_lines
        .iter()
        .any(|line| line.starts_with("detector(5,0) D2")));
}

#[test]
fn median_duration_ns_picks_middle_value() {
    let values = vec![
        Duration::from_millis(30),
        Duration::from_millis(10),
        Duration::from_millis(20),
    ];
    assert_eq!(median_duration_ns(&values), 20_000_000);
}

#[test]
fn render_markdown_table_includes_expected_headers() {
    let table = render_markdown_table(&[vec![
        "repetition_code/memory d=5 r=5".to_string(),
        "exact".to_string(),
        "match".to_string(),
        "0".to_string(),
        "1.0".to_string(),
        "1.1".to_string(),
        "2.0".to_string(),
        "2.4".to_string(),
        "1.10x".to_string(),
        "1.20x".to_string(),
    ]]);
    assert!(table.contains("| Case | Gen | DEM |"));
    assert!(table.contains("repetition_code/memory d=5 r=5"));
}
