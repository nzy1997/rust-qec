use rsinter::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use rsinter::bench::runners::rbposd::RbposdRunner;
use rsinter::bench::runners::rilpqec::RilpqecRunner;

#[test]
fn rbposd_runner_handles_zero_shot_benchmark_points() {
    let runner = RbposdRunner;
    let point = BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
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
fn rilpqec_runner_handles_zero_shot_benchmark_points() {
    let runner = RilpqecRunner;
    let point = BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
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
