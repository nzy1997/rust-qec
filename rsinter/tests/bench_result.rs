use rsinter::bench::result::{
    read_results_jsonl,
    write_results_jsonl,
    BenchmarkResultRow,
    CaseSummary,
    MetricMap,
    PairMapExt,
    ParamMap,
    RunManifest,
};

#[test]
fn result_row_serializes_round_trip_as_json() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
        ]),
        metrics: MetricMap::from_pairs([
            ("shots_used", 2000.0),
            ("logical_error_rate", 0.001),
        ]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let encoded = serde_json::to_string(&row).unwrap();
    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.runner, "rmatching");
    assert_eq!(decoded.metrics["logical_error_rate"], 0.001);
}

#[test]
fn run_manifest_serializes_round_trip() {
    let manifest = RunManifest::new(
        "surface_decoder".into(),
        1,
        "rmatching".into(),
        "rust".into(),
        "benchmarks/out/surface_decoder/rmatching/test".into(),
    );

    let encoded = serde_json::to_string(&manifest).unwrap();
    let decoded: RunManifest = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.benchmark, "surface_decoder");
    assert_eq!(decoded.runner, "rmatching");
}

#[test]
fn results_jsonl_round_trip_multiple_rows() {
    let rows = vec![
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([("distance", serde_json::json!(3))]),
            case_summary: CaseSummary::from_pairs([("num_dets", serde_json::json!(24))]),
            metrics: MetricMap::from_pairs([("shots_used", 2000.0)]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        },
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "pymatching".into(),
            language: "python".into(),
            status: "error".into(),
            params: ParamMap::from_pairs([("distance", serde_json::json!(5))]),
            case_summary: CaseSummary::from_pairs([("num_dets", serde_json::json!(120))]),
            metrics: MetricMap::from_pairs([("shots_used", 0.0)]),
            artifacts: std::collections::BTreeMap::new(),
            error: Some("solver failed".into()),
        },
    ];

    let mut buf = Vec::new();
    write_results_jsonl(&rows, &mut buf).unwrap();
    let decoded = read_results_jsonl(&buf[..]).unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[1].status, "error");
    assert_eq!(decoded[1].error.as_deref(), Some("solver failed"));
}
