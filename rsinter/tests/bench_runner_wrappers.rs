use std::collections::BTreeMap;

use rsinter::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use rsinter::bench::runners::rbposd::RbposdRunner;
use rsinter::bench::runners::rilpqec::RilpqecRunner;

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

#[test]
fn rbposd_runner_preflight_defaults_lsd_method_when_order_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(0),
    )]));

    runner.preflight_point(&point).unwrap();
}

#[test]
fn rbposd_runner_preflight_defaults_lsd_order_when_method_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_method".into(),
        toml::Value::String("localized_statistics".into()),
    )]));

    runner.preflight_point(&point).unwrap();
}

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
