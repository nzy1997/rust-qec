use rsinter::bench::merge::merge_result_rows;
use rsinter::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap};
use rsinter::failure::FailureKind;

#[test]
fn merge_result_rows_concatenates_and_sorts_by_runner_then_distance_then_p() {
    let rows = merge_result_rows(vec![
        vec![BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "pymatching".into(),
            language: "python".into(),
            status: "ok".into(),
            failure_kind: FailureKind::Ok,
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
            failure_kind: FailureKind::Ok,
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.002)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([("logical_error_rate", 0.001)]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        }],
    ])
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].runner, "pymatching");
    assert_eq!(rows[1].runner, "rmatching");
}

#[test]
fn benchmark_merge_combines_rows_with_same_identity() {
    let first = ok_row(
        serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap(),
        100.0,
        1.0,
        300.0,
        0.5,
    );
    let second = ok_row(
        serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap(),
        300.0,
        5.0,
        900.0,
        1.5,
    );
    assert_eq!(first.identity().unwrap(), second.identity().unwrap());

    let rows = merge_result_rows(vec![vec![first], vec![second]]).unwrap();

    assert_eq!(rows.len(), 1);
    let metrics = &rows[0].metrics;
    assert_eq!(metrics["shots_used"], 400.0);
    assert_eq!(metrics["logical_errors"], 6.0);
    assert_eq!(metrics["total_decode_us"], 1200.0);
    assert_eq!(metrics["wall_seconds"], 2.0);
    assert_eq!(metrics["logical_error_rate"], 0.015);
    assert_eq!(metrics["decode_us_per_shot"], 3.0);
    assert_eq!(
        rows[0].case_summary["num_shots_generated"],
        serde_json::json!(400)
    );

    let different_decoder = ok_row(serde_json::json!({"a": 1, "b": 3}), 50.0, 0.0, 50.0, 0.2);
    let distinct = merge_result_rows(vec![vec![rows[0].clone()], vec![different_decoder]]).unwrap();
    assert_eq!(distinct.len(), 2);

    let mut incompatible = rows[0].clone();
    incompatible.status = "error".into();
    incompatible.failure_kind = FailureKind::SolverFailure;
    incompatible.error = Some("solver failed".into());
    let err = merge_result_rows(vec![vec![rows[0].clone()], vec![incompatible]])
        .expect_err("same identity with conflicting status must fail");
    assert!(err.contains("conflicting status"), "{err}");
}

fn ok_row(
    decoder_options: serde_json::Value,
    shots_used: f64,
    logical_errors: f64,
    total_decode_us: f64,
    wall_seconds: f64,
) -> BenchmarkResultRow {
    BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: FailureKind::LogicalFailure,
        params: ParamMap::from_pairs([
            ("decoder_options", decoder_options),
            ("decoder_impl", serde_json::json!("rmatching")),
            ("distance", serde_json::json!(3)),
            ("max_errors", serde_json::json!(100)),
            ("max_shots", serde_json::json!(1000)),
            ("p", serde_json::json!(0.002)),
            ("seed", serde_json::json!(12345)),
        ]),
        case_summary: CaseSummary::from_pairs([
            ("num_dets", serde_json::json!(24)),
            ("num_obs", serde_json::json!(1)),
            ("num_shots_generated", serde_json::json!(shots_used as u64)),
        ]),
        metrics: MetricMap::from_pairs([
            ("shots_used", shots_used),
            ("logical_errors", logical_errors),
            ("logical_error_rate", logical_errors / shots_used),
            ("total_decode_us", total_decode_us),
            ("wall_seconds", wall_seconds),
            ("decode_us_per_shot", total_decode_us / shots_used),
        ]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    }
}
