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

#[test]
fn benchmark_merge_recomputes_completed_failure_kind_from_merged_counters() {
    let mut clean = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 300.0, 0.5);
    clean.failure_kind = FailureKind::Ok;
    let logical_failure = ok_row(serde_json::json!({"a": 1}), 300.0, 5.0, 900.0, 1.5);
    assert_eq!(
        clean.identity().unwrap(),
        logical_failure.identity().unwrap()
    );

    let rows = merge_result_rows(vec![vec![clean], vec![logical_failure]]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].failure_kind, FailureKind::LogicalFailure);
    let metrics = &rows[0].metrics;
    assert_eq!(metrics["shots_used"], 400.0);
    assert_eq!(metrics["logical_errors"], 5.0);
    assert_eq!(metrics["logical_error_rate"], 0.0125);
}

#[test]
fn benchmark_merge_rejects_unknown_metrics_for_same_identity() {
    let first = ok_row(serde_json::json!({"a": 1}), 100.0, 1.0, 300.0, 0.5);
    let mut second = ok_row(serde_json::json!({"a": 1}), 300.0, 5.0, 900.0, 1.5);
    second.metrics.insert("median_decode_us".into(), 42.0);

    let err = merge_result_rows(vec![vec![first], vec![second]])
        .expect_err("same identity with unknown metrics must fail");
    assert!(
        err.contains("conflicting metrics.median_decode_us"),
        "{err}"
    );
}

#[test]
fn benchmark_merge_checks_failure_kind_compatibility() {
    let first_error = error_row(FailureKind::SolverFailure);
    let second_error = error_row(FailureKind::SolverFailure);

    let merged = merge_result_rows(vec![vec![first_error.clone()], vec![second_error]]).unwrap();

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].status, "error");
    assert_eq!(merged[0].failure_kind, FailureKind::SolverFailure);

    let mut conflicting_error = first_error.clone();
    conflicting_error.failure_kind = FailureKind::SamplerError;
    let err = merge_result_rows(vec![vec![first_error.clone()], vec![conflicting_error]])
        .expect_err("same identity with conflicting error failure kind must fail");
    assert!(err.contains("conflicting failure_kind"), "{err}");

    let mut invalid_completed = first_error;
    invalid_completed.status = "ok".into();
    invalid_completed.failure_kind = FailureKind::SamplerError;
    invalid_completed.error = None;
    let valid_completed = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 100.0, 0.1);
    let err = merge_result_rows(vec![vec![valid_completed], vec![invalid_completed]])
        .expect_err("ok rows with error failure kinds must fail");
    assert!(err.contains("conflicting failure_kind"), "{err}");
}

#[test]
fn benchmark_merge_handles_optional_additive_case_summary_values() {
    let mut missing_counter = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 100.0, 0.1);
    missing_counter.case_summary.remove("num_shots_generated");
    let with_counter = ok_row(serde_json::json!({"a": 1}), 300.0, 0.0, 300.0, 0.3);

    let merged = merge_result_rows(vec![vec![missing_counter], vec![with_counter]]).unwrap();

    assert_eq!(
        merged[0].case_summary["num_shots_generated"],
        serde_json::json!(300)
    );

    let with_counter = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 100.0, 0.1);
    let mut missing_counter = ok_row(serde_json::json!({"a": 1}), 300.0, 0.0, 300.0, 0.3);
    missing_counter.case_summary.remove("num_shots_generated");

    let merged = merge_result_rows(vec![vec![with_counter], vec![missing_counter]]).unwrap();

    assert_eq!(
        merged[0].case_summary["num_shots_generated"],
        serde_json::json!(100)
    );
}

#[test]
fn benchmark_merge_rejects_invalid_additive_case_summary_values() {
    let mut first = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 100.0, 0.1);
    first
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!(u64::MAX));
    let second = ok_row(serde_json::json!({"a": 1}), 1.0, 0.0, 1.0, 0.1);

    let err = merge_result_rows(vec![vec![first], vec![second]])
        .expect_err("overflowing additive case summary values must fail");
    assert!(
        err.contains("conflicting case_summary.num_shots_generated"),
        "{err}"
    );

    let mut first = ok_row(serde_json::json!({"a": 1}), 100.0, 0.0, 100.0, 0.1);
    first
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!("100"));
    let second = ok_row(serde_json::json!({"a": 1}), 1.0, 0.0, 1.0, 0.1);

    let err = merge_result_rows(vec![vec![first], vec![second]])
        .expect_err("non-numeric additive case summary values must fail");
    assert!(
        err.contains("conflicting case_summary.num_shots_generated"),
        "{err}"
    );
}

#[test]
fn benchmark_merge_sums_signed_and_float_case_summary_values() {
    let mut first = ok_row(serde_json::json!({"signed": true}), 1.0, 0.0, 1.0, 0.1);
    first
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!(-5));
    let mut second = ok_row(serde_json::json!({"signed": true}), 1.0, 0.0, 1.0, 0.1);
    second
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!(-7));

    let merged = merge_result_rows(vec![vec![first], vec![second]]).unwrap();

    assert_eq!(
        merged[0].case_summary["num_shots_generated"],
        serde_json::json!(-12)
    );

    let mut first = ok_row(serde_json::json!({"float": true}), 1.0, 0.0, 1.0, 0.1);
    first
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!(1.25));
    let mut second = ok_row(serde_json::json!({"float": true}), 1.0, 0.0, 1.0, 0.1);
    second
        .case_summary
        .insert("num_shots_generated".into(), serde_json::json!(2.5));

    let merged = merge_result_rows(vec![vec![first], vec![second]]).unwrap();

    assert_eq!(
        merged[0].case_summary["num_shots_generated"],
        serde_json::json!(3.75)
    );
}

#[test]
fn benchmark_merge_removes_stale_derived_metrics_without_required_counters() {
    let mut zero_shots = ok_row(serde_json::json!({"a": 1}), 0.0, 0.0, 0.0, 0.0);
    zero_shots
        .metrics
        .insert("logical_error_rate".into(), 123.0);
    zero_shots
        .metrics
        .insert("decode_us_per_shot".into(), 456.0);
    let second_zero_shots = zero_shots.clone();

    let merged = merge_result_rows(vec![vec![zero_shots], vec![second_zero_shots]]).unwrap();

    assert!(!merged[0].metrics.contains_key("logical_error_rate"));
    assert!(!merged[0].metrics.contains_key("decode_us_per_shot"));

    let mut missing_counters = ok_row(serde_json::json!({"b": 2}), 100.0, 0.0, 100.0, 0.1);
    missing_counters.metrics.clear();
    missing_counters
        .metrics
        .insert("logical_error_rate".into(), 123.0);
    missing_counters
        .metrics
        .insert("decode_us_per_shot".into(), 456.0);
    let second_missing_counters = missing_counters.clone();

    let merged =
        merge_result_rows(vec![vec![missing_counters], vec![second_missing_counters]]).unwrap();

    assert!(!merged[0].metrics.contains_key("logical_error_rate"));
    assert!(!merged[0].metrics.contains_key("decode_us_per_shot"));
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

fn error_row(failure_kind: FailureKind) -> BenchmarkResultRow {
    let mut row = ok_row(serde_json::json!({"a": 1}), 1.0, 0.0, 1.0, 0.1);
    row.status = "error".into();
    row.failure_kind = failure_kind;
    row.metrics.clear();
    row.error = Some("decoder failed".into());
    row
}
