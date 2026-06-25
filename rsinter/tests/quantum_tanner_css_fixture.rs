use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{BenchmarkResultRow, read_results_jsonl};
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::{
    AxisSpec, BenchmarkMode, BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotSpec, RunnerSpec,
    SeriesSpec,
};
use rsinter::failure::FailureKind;
use toml::Value;

const TORIC_D4_HX: &str = "tests/fixtures/css/quantum_tanner_toric_d4_hx.json";
const TORIC_D4_HZ: &str = "tests/fixtures/css/quantum_tanner_toric_d4_hz.json";

#[test]
fn quantum_tanner_toric_d4_css_fixture_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = run_quantum_tanner_css_benchmark(TORIC_D4_HZ, dir.path(), spec_dir).unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.status, "ok");
    assert_eq!(row.failure_kind, FailureKind::Ok);
    assert_eq!(row.error, None);
    assert_eq!(row.params["input_type"], serde_json::json!("css"));
    assert_eq!(
        row.params["code_id"],
        serde_json::json!("quantum_tanner_toric_d4")
    );
    assert_eq!(row.params["hx"], serde_json::json!(TORIC_D4_HX));
    assert_eq!(row.params["hz"], serde_json::json!(TORIC_D4_HZ));
    assert_eq!(row.params["decoder_impl"], serde_json::json!("rmatching"));
    assert_eq!(row.case_summary["num_obs"], serde_json::json!(2));
    assert_eq!(
        row.case_summary["logical_observable_count"],
        serde_json::json!(2)
    );
    assert_eq!(row.metrics["shots_used"], 4.0);
    assert_eq!(row.metrics["logical_errors"], 0.0);

    let non_orthogonal_hz = write_non_orthogonal_hz_fixture(dir.path());
    let err = run_quantum_tanner_css_benchmark(
        &non_orthogonal_hz.display().to_string(),
        dir.path(),
        spec_dir,
    )
    .unwrap_err();
    assert!(
        err.contains("CSS X/Z checks are not orthogonal"),
        "expected CSS orthogonality rejection, got: {err}"
    );
}

fn run_quantum_tanner_css_benchmark(
    hz_path: &str,
    out_root: &Path,
    spec_dir: &Path,
) -> Result<Vec<BenchmarkResultRow>, String> {
    let registry = build_default_rust_runner_registry();
    let artifact_root = run_rust_benchmark(
        &quantum_tanner_css_spec(hz_path),
        "rust",
        out_root,
        &registry,
        spec_dir,
    )?;
    let data = fs::read(
        artifact_root
            .join("rmatching_quantum_tanner_toric_d4")
            .join("test-run")
            .join("results.jsonl"),
    )
    .map_err(|error| error.to_string())?;
    read_results_jsonl(&data[..])
}

fn quantum_tanner_css_spec(hz_path: &str) -> BenchmarkSpec {
    let mut params = BTreeMap::new();
    params.insert("input_type".into(), Value::String("css".into()));
    params.insert(
        "code_id".into(),
        Value::String("quantum_tanner_toric_d4".into()),
    );
    params.insert("hx".into(), Value::String(TORIC_D4_HX.into()));
    params.insert("hz".into(), Value::String(hz_path.into()));
    params.insert("basis".into(), Value::String("x".into()));
    params.insert("schedule".into(), Value::String("greedy".into()));
    params.insert("rounds".into(), Value::Array(vec![Value::Integer(1)]));
    params.insert("p".into(), Value::Array(vec![Value::Float(0.0)]));
    params.insert("max_shots".into(), Value::Integer(4));
    params.insert("max_errors".into(), Value::Integer(4));
    params.insert("batch_size".into(), Value::Integer(4));

    BenchmarkSpec {
        name: "quantum_tanner_toric_d4_css".into(),
        version: 1,
        mode: BenchmarkMode::Independent,
        runners: vec![RunnerSpec {
            name: "rmatching_quantum_tanner_toric_d4".into(),
            language: "rust".into(),
            impl_key: "rmatching".into(),
            params,
        }],
        plot: PlotSpec {
            title: "Quantum Tanner toric_d4 CSS Smoke".into(),
            logical_rate_unit: LogicalRateUnit::PerShot,
            x: AxisSpec {
                field: "params.p".into(),
                scale: "linear".into(),
                label: "Physical Error Rate".into(),
            },
            series: SeriesSpec {
                group_by: vec!["runner".into(), "params.code_id".into()],
                label_template: "{{runner}} {{params.code_id}}".into(),
            },
            panels: vec![PanelSpec {
                metric: "metrics.logical_error_rate".into(),
                scale: "linear".into(),
                label: "Logical Error Rate".into(),
            }],
        },
    }
}

fn write_non_orthogonal_hz_fixture(dir: &Path) -> PathBuf {
    let mut hz: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/css/quantum_tanner_toric_d4_hz.json")).unwrap();
    hz["rows"][0] = serde_json::json!([1, 4, 5, 6]);
    let path = dir.join("quantum_tanner_toric_d4_non_orthogonal_hz.json");
    fs::write(&path, serde_json::to_string(&hz).unwrap()).unwrap();
    path
}
