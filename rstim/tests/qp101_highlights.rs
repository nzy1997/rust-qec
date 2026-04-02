use rstim::dem::DetectorErrorModel;
use rstim::dem::DemTarget;
use rstim::dem_provenance::{SourceBranch, TrackedDemResult, TrackedErrorTerm, TrackedSource};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use rstim::qp101::export_qp101_with_highlighted_dem_error;

#[test]
fn qp101_export_includes_dem_origin_highlights() {
    let circuit = parse_lines(
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    let doc = export_qp101_with_highlighted_dem_error(&circuit, &tracked, 0).unwrap();
    let value = serde_json::to_value(doc).unwrap();

    assert_eq!(
        value["extensions"]["rstim_query_highlights"]["query"]["kind"],
        "dem_error_origin"
    );
    assert_eq!(value["extensions"]["rstim_query_highlights"]["version"], "1");
    assert_eq!(
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["target_slots"],
        serde_json::json!([0])
    );
    assert_eq!(
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["label"],
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["branch"]
    );
    assert!(
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["repeat_iterations"]
            .is_array()
    );
}

#[test]
fn qp101_export_rejects_out_of_range_dem_error_index_directly() {
    let circuit = parse_lines("R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    let err = export_qp101_with_highlighted_dem_error(&circuit, &tracked, 99).unwrap_err();

    assert!(err.contains("DEM error index 99 out of range"));
}

#[test]
fn qp101_export_rejects_missing_tracked_source_entry() {
    let circuit = parse_lines("R 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let tracked = TrackedDemResult {
        dem: DetectorErrorModel::new(),
        sources: Vec::new(),
        dem_error_to_sources: vec![vec![0]],
        source_to_dem_errors: Vec::new(),
    };

    let err = export_qp101_with_highlighted_dem_error(&circuit, &tracked, 0).unwrap_err();

    assert!(err.contains("tracked source index 0 missing for DEM error 0"));
}

#[test]
fn qp101_export_dedupes_equivalent_source_highlights() {
    let circuit = parse_lines("R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let tracked = TrackedDemResult::from_terms_and_sources(
        vec![
            TrackedSource {
                source_id: 0,
                op_path: vec![1],
                repeat_iterations: vec![2],
                instr_name: "X_ERROR".to_string(),
                target_slots: vec![0],
                target_qubits: vec![0],
                branch: SourceBranch::X,
                probability_fragment: 0.1,
            },
            TrackedSource {
                source_id: 1,
                op_path: vec![1],
                repeat_iterations: vec![2],
                instr_name: "X_ERROR".to_string(),
                target_slots: vec![0],
                target_qubits: vec![0],
                branch: SourceBranch::X,
                probability_fragment: 0.1,
            },
        ],
        vec![TrackedErrorTerm {
            probability: 0.1,
            targets: vec![DemTarget::Detector(0)],
            source_ids: vec![0, 1],
        }],
    );

    let doc = export_qp101_with_highlighted_dem_error(&circuit, &tracked, 0).unwrap();
    let highlights = serde_json::to_value(doc).unwrap()["extensions"]["rstim_query_highlights"]
        ["highlights"]
        .as_array()
        .unwrap()
        .clone();

    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0]["repeat_iterations"], serde_json::json!([2]));
    assert_eq!(highlights[0]["target_qubits"], serde_json::json!([0]));
}
