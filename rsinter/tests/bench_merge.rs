use rsinter::bench::merge::merge_result_rows;
use rsinter::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap};

#[test]
fn merge_result_rows_concatenates_and_sorts_by_runner_then_distance_then_p() {
    let rows = merge_result_rows(vec![
        vec![BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "pymatching".into(),
            language: "python".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(5)),
                ("p", serde_json::json!(0.005)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([("logical_error_rate", 0.01)]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        }],
        vec![BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.002)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([("logical_error_rate", 0.001)]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        }],
    ]);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].runner, "pymatching");
    assert_eq!(rows[1].runner, "rmatching");
}
