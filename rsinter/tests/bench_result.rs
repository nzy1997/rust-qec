use rsinter::bench::result::{
    BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap, RunManifest,
    read_results_jsonl, write_results_jsonl,
};
use rsinter::failure::FailureKind;

#[test]
fn result_row_serializes_round_trip_as_json() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::Ok,
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
        ]),
        metrics: MetricMap::from_pairs([("shots_used", 2000.0), ("logical_error_rate", 0.001)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let encoded = serde_json::to_string(&row).unwrap();
    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.runner, "rmatching");
    assert_eq!(decoded.metrics["logical_error_rate"], 0.001);
}

#[test]
fn result_row_serializes_stable_identity_field() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::Ok,
        params: ParamMap::from_pairs([
            (
                "decoder_options",
                serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap(),
            ),
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
            ("num_shots_generated", serde_json::json!(2000)),
        ]),
        metrics: MetricMap::from_pairs([("shots_used", 2000.0)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let identity = row.identity().unwrap();
    assert!(identity.starts_with("sha256:"));
    assert_eq!(identity.len(), "sha256:".len() + 64);

    let encoded = serde_json::to_string(&row).unwrap();
    let encoded_value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(encoded_value["identity"], serde_json::json!(identity));

    let mut reordered = row.clone();
    reordered.params.insert(
        "decoder_options".into(),
        serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap(),
    );
    assert_eq!(row.identity().unwrap(), reordered.identity().unwrap());

    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.identity().unwrap(), identity);
}

#[test]
fn result_row_serializes_failure_kind_as_snake_case() {
    let row = BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::LogicalFailure,
        params: ParamMap::from_pairs([("distance", serde_json::json!(3))]),
        case_summary: CaseSummary::new(),
        metrics: MetricMap::from_pairs([("logical_errors", 2.0)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    };

    let encoded = serde_json::to_string(&row).unwrap();
    assert!(encoded.contains("\"failure_kind\":\"logical_failure\""));

    let decoded: BenchmarkResultRow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.failure_kind, FailureKind::LogicalFailure);
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
            failure_kind: FailureKind::Ok,
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
            failure_kind: FailureKind::SolverFailure,
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

#[test]
fn results_jsonl_ignores_blank_lines() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"rmatching\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"distance\":3},\"case_summary\":{},\"metrics\":{\"shots_used\":2.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "\n",
        " \n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"pymatching\",\"language\":\"python\",\"status\":\"ok\",",
        "\"params\":{\"distance\":5},\"case_summary\":{},\"metrics\":{\"shots_used\":4.0},",
        "\"artifacts\":{},\"error\":null}\n"
    );

    let decoded = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].runner, "rmatching");
    assert_eq!(decoded[1].runner, "pymatching");
}

#[test]
fn results_jsonl_infers_missing_failure_kind_from_legacy_rows() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"clean\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"logical\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{\"logical_errors\":3.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"solver\",\"language\":\"rust\",\"status\":\"error\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":\"HiGHS backend error: solve failed\"}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"unsupported\",\"language\":\"rust\",\"status\":\"error\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":\"no ILP backend is available for kind Gurobi\"}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Ok);
    assert_eq!(rows[1].failure_kind, FailureKind::LogicalFailure);
    assert_eq!(rows[2].failure_kind, FailureKind::SolverFailure);
    assert_eq!(rows[3].failure_kind, FailureKind::Unsupported);
}

#[test]
fn results_jsonl_preserves_explicit_failure_kind_over_legacy_inference() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"explicit_timeout\",\"language\":\"rust\",",
        "\"status\":\"ok\",\"failure_kind\":\"timeout\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"explicit_sampler\",\"language\":\"rust\",",
        "\"status\":\"error\",\"failure_kind\":\"sampler_error\",",
        "\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":\"sample failed\"}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].failure_kind, FailureKind::Timeout);
    assert_eq!(rows[1].status, "error");
    assert_eq!(rows[1].failure_kind, FailureKind::SamplerError);
    assert_eq!(rows[1].error.as_deref(), Some("sample failed"));
}

#[test]
fn results_jsonl_infers_missing_failure_kind_from_legacy_timeout_row() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"timeout\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"max_wall_seconds\":0.25},\"case_summary\":{},",
        "\"metrics\":{\"wall_seconds\":0.25,\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Timeout);
}

#[test]
fn results_jsonl_does_not_infer_timeout_without_wall_seconds_metric() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"missing_wall\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"max_wall_seconds\":0.25},\"case_summary\":{},",
        "\"metrics\":{\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Ok);
}

#[test]
fn results_jsonl_keeps_more_legacy_timeout_edges_as_non_timeouts() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"no_limit\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{},\"case_summary\":{},",
        "\"metrics\":{\"wall_seconds\":99.0,\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"under_limit\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"max_wall_seconds\":0.25},\"case_summary\":{},",
        "\"metrics\":{\"wall_seconds\":0.24,\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"error_without_message\",\"language\":\"rust\",",
        "\"status\":\"error\",\"params\":{},\"case_summary\":{},\"metrics\":{},",
        "\"artifacts\":{},\"error\":null}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Ok);
    assert_eq!(rows[1].failure_kind, FailureKind::Ok);
    assert_eq!(rows[2].failure_kind, FailureKind::SolverFailure);
}

#[test]
fn results_jsonl_does_not_infer_timeout_when_legacy_rows_hit_caps() {
    let input = concat!(
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"shot_cap\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"max_wall_seconds\":0.25,\"max_shots\":100,\"max_errors\":20},",
        "\"case_summary\":{},",
        "\"metrics\":{\"wall_seconds\":0.25,\"shots_used\":100.0,\"logical_errors\":0.0},",
        "\"artifacts\":{},\"error\":null}\n",
        "{\"benchmark\":\"surface_decoder\",\"runner\":\"error_cap\",\"language\":\"rust\",\"status\":\"ok\",",
        "\"params\":{\"max_wall_seconds\":0.25,\"max_shots\":100,\"max_errors\":20},",
        "\"case_summary\":{},",
        "\"metrics\":{\"wall_seconds\":0.25,\"shots_used\":40.0,\"logical_errors\":20.0},",
        "\"artifacts\":{},\"error\":null}\n"
    );

    let rows = read_results_jsonl(input.as_bytes()).unwrap();

    assert_eq!(rows[0].failure_kind, FailureKind::Ok);
    assert_eq!(rows[1].failure_kind, FailureKind::LogicalFailure);
}
