use std::collections::BTreeMap;

use rsinter::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
#[cfg(feature = "rbposd-runner")]
use rsinter::bench::runners::rbposd::RbposdRunner;
#[cfg(feature = "ilp-runner")]
use rsinter::bench::runners::rilpqec::RilpqecRunner;

#[cfg(feature = "rbposd-runner")]
fn rbposd_point_with_decoder_params(
    decoder_params: BTreeMap<String, toml::Value>,
) -> BenchCasePoint {
    BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
        seed: 12_345,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params,
    }
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_handles_zero_shot_benchmark_points() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::new());
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "rbposd_alias".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let row = runner.run_point(&point, &ctx).unwrap();

    assert_eq!(runner.name(), "rbposd");
    assert_eq!(row.runner, "rbposd_alias");
    assert_eq!(row.failure_kind, rsinter::failure::FailureKind::Ok);
    assert_eq!(row.metrics["shots_used"], 0.0);
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_accepts_lsd_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "lsd_method".into(),
            toml::Value::String("localized_statistics".into()),
        ),
        ("lsd_order".into(), toml::Value::Integer(1)),
    ]));

    runner.preflight_point(&point).unwrap();
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_accepts_ldpc_osd_cs_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "osd_method".into(),
            toml::Value::String("ldpc_osd_cs".into()),
        ),
        ("osd_order".into(), toml::Value::Integer(7)),
    ]));

    runner.preflight_point(&point).unwrap();
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_runs_ldpc_osd_cs_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "osd_method".into(),
            toml::Value::String("ldpc_osd_cs".into()),
        ),
        ("osd_order".into(), toml::Value::Integer(7)),
    ]));
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "rbposd_ldpc".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let row = runner.run_point(&point, &ctx).unwrap();

    assert_eq!(row.runner, "rbposd_ldpc");
    assert_eq!(row.failure_kind, rsinter::failure::FailureKind::Ok);
    assert_eq!(row.params["osd_method"], serde_json::json!("ldpc_osd_cs"));
    assert_eq!(row.params["osd_order"], serde_json::json!(7));
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_defaults_lsd_method_when_order_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(0),
    )]));

    runner.preflight_point(&point).unwrap();
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_defaults_lsd_order_when_method_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_method".into(),
        toml::Value::String("localized_statistics".into()),
    )]));

    runner.preflight_point(&point).unwrap();
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_accepts_bp_method_and_schedule_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "bp_method".into(),
            toml::Value::String("product_sum".into()),
        ),
        ("schedule".into(), toml::Value::String("serial".into())),
    ]));

    runner.preflight_point(&point).unwrap();
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_rejects_unknown_bp_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "bp_method".into(),
        toml::Value::String("sum_product".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd bp_method must be \"minimum_sum\" or \"product_sum\", got \"sum_product\""
    );
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_rejects_unknown_bp_schedule() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "schedule".into(),
        toml::Value::String("flooding".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd schedule must be \"parallel\" or \"serial\", got \"flooding\""
    );
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_rejects_unsupported_lsd_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_method".into(),
        toml::Value::String("unknown_method".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd lsd_method must be \"localized_statistics\", got \"unknown_method\""
    );
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_rejects_unsupported_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(2),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "rbposd lsd_order must be <= 1, got 2");
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_rejects_negative_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(-1),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "lsd_order must be non-negative");
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_rejects_non_integer_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Float(1.0),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "lsd_order must be an integer");
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_runner_preflight_rejects_mixed_osd_and_lsd_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        ("osd_order".into(), toml::Value::Integer(10)),
        ("lsd_order".into(), toml::Value::Integer(1)),
    ]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "rbposd params must not mix OSD and LSD decoder params");
}

#[cfg(feature = "rbposd-runner")]
#[test]
fn rbposd_lsd_runner_order_changes_benchmark_logical_error_rate() {
    let runner = RbposdRunner;
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "rbposd_lsd".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let mut order0 = rbposd_point_with_decoder_params(BTreeMap::from([
        ("bp_iters".into(), toml::Value::Integer(0)),
        ("lsd_order".into(), toml::Value::Integer(0)),
    ]));
    order0.rounds = 1;
    order0.p = 0.02;
    order0.seed = 1;
    order0.max_shots = 64;
    order0.max_errors = 64;
    order0.batch_size = 16;

    let mut order1 = rbposd_point_with_decoder_params(BTreeMap::from([
        ("bp_iters".into(), toml::Value::Integer(0)),
        ("lsd_order".into(), toml::Value::Integer(1)),
    ]));
    order1.rounds = order0.rounds;
    order1.p = order0.p;
    order1.seed = order0.seed;
    order1.max_shots = order0.max_shots;
    order1.max_errors = order0.max_errors;
    order1.batch_size = order0.batch_size;

    let order0_row = runner.run_point(&order0, &ctx).unwrap();
    let order1_row = runner.run_point(&order1, &ctx).unwrap();

    assert_eq!(order0_row.params["lsd_order"], serde_json::json!(0));
    assert_eq!(order1_row.params["lsd_order"], serde_json::json!(1));
    assert_eq!(order0_row.metrics["shots_used"], 64.0);
    assert_eq!(order1_row.metrics["shots_used"], 64.0);
    assert_ne!(
        order1_row.metrics["logical_errors"], order0_row.metrics["logical_errors"],
        "expected parsed lsd_order=1 to change logical errors: order0={}, order1={}",
        order0_row.metrics["logical_errors"], order1_row.metrics["logical_errors"]
    );
    assert_ne!(
        order1_row.metrics["logical_error_rate"], order0_row.metrics["logical_error_rate"],
        "expected parsed lsd_order=1 to change runner LER: order0={}, order1={}",
        order0_row.metrics["logical_error_rate"], order1_row.metrics["logical_error_rate"]
    );
}

#[cfg(feature = "ilp-runner")]
#[test]
fn rilpqec_runner_handles_zero_shot_benchmark_points() {
    let runner = RilpqecRunner;
    let point = BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
        seed: 12_345,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params: std::collections::BTreeMap::new(),
    };
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "rilpqec_alias".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let row = runner.run_point(&point, &ctx).unwrap();

    assert_eq!(runner.name(), "rilpqec");
    assert_eq!(row.runner, "rilpqec_alias");
    assert_eq!(row.failure_kind, rsinter::failure::FailureKind::Ok);
    assert_eq!(row.metrics["shots_used"], 0.0);
}
