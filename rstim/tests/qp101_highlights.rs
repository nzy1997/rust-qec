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
    assert_eq!(
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["target_slots"],
        serde_json::json!([0])
    );
    assert!(
        value["extensions"]["rstim_query_highlights"]["highlights"][0]["repeat_iterations"]
            .is_array()
    );
}
