#![allow(unexpected_cfgs)]

#[cfg(not(tarpaulin))]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[cfg(not(tarpaulin))]
use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{read_results_jsonl, BenchmarkResultRow};
#[cfg(not(tarpaulin))]
use rsinter::bench::run::run_rust_benchmark;
#[cfg(not(tarpaulin))]
use rsinter::bench::spec::BenchmarkSpec;
use rsinter::stats::fit_binomial;
use serde::Deserialize;

const FIT_FACTOR: f64 = 10_000.0;
const EXPECTED_DISTANCES: [usize; 3] = [3, 5, 7];
const EXPECTED_PS: [f64; 5] = [0.008, 0.009, 0.010, 0.011, 0.012];
const ISSUE65_RUNNERS: [&str; 3] = [
    "rmatching-memory-z-d3",
    "rmatching-memory-z-d5",
    "rmatching-memory-z-d7",
];

#[derive(Debug, Deserialize)]
struct StimFixture {
    metadata: StimMetadata,
    rows: Vec<StimRow>,
}

#[derive(Debug, Deserialize)]
struct StimMetadata {
    max_shots: u64,
    max_errors: u64,
    max_likelihood_factor: f64,
    case_count: usize,
}

#[derive(Debug, Deserialize)]
struct StimRow {
    distance: usize,
    rounds: usize,
    p: f64,
    shots: u64,
    logical_errors: u64,
    logical_error_rate: f64,
    ci_low: f64,
    ci_high: f64,
    num_detectors: usize,
    num_observables: usize,
}

#[test]
fn issue65_memory_z_stim_fixture_is_well_formed() {
    let fixture = load_stim_fixture();

    assert_eq!(fixture.metadata.max_shots, 1_000_000);
    assert_eq!(fixture.metadata.max_errors, 5_000);
    assert_eq!(fixture.metadata.max_likelihood_factor, FIT_FACTOR);
    assert_eq!(fixture.metadata.case_count, 15);
    assert_eq!(fixture.rows.len(), 15);

    let expected_cases = expected_cases();
    let actual_cases = fixture
        .rows
        .iter()
        .map(|row| case_key(row.distance, row.rounds, row.p))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_cases, expected_cases);

    for row in &fixture.rows {
        assert_eq!(row.rounds, row.distance * 3);
        assert!(row.shots <= 1_000_000);
        assert!(row.logical_errors >= 5_000 || row.shots == 1_000_000);
        assert!(row.logical_errors <= row.shots);
        assert_close(
            row.logical_error_rate,
            row.logical_errors as f64 / row.shots as f64,
            "logical_error_rate",
        );

        let fit = fit_binomial(row.shots, row.logical_errors, FIT_FACTOR);
        assert_close(row.ci_low, fit.low.unwrap(), "ci_low");
        assert_close(row.ci_high, fit.high.unwrap(), "ci_high");
    }
}

#[test]
fn issue65_memory_z_rust_result_helpers_read_runner_outputs() {
    let dir = tempfile::tempdir().unwrap();
    write_runner_result(
        dir.path(),
        "rmatching-memory-z-d3",
        RunnerRow {
            distance: 3,
            rounds: 9,
            p: 0.008,
            shots: 100.6,
            logical_errors: 5.4,
        },
    );
    write_runner_result(
        dir.path(),
        "rmatching-memory-z-d5",
        RunnerRow {
            distance: 5,
            rounds: 15,
            p: 0.012,
            shots: 200.2,
            logical_errors: 9.8,
        },
    );

    let rows = read_issue65_runner_results(
        dir.path(),
        &["rmatching-memory-z-d3", "rmatching-memory-z-d5"],
    );

    assert_eq!(rows.len(), 2);
    assert_eq!(case_key_from_rust(&rows[0]), "d3_r9_p0.008");
    assert_eq!(count_metric(&rows[0], "shots_used"), 101);
    assert_eq!(count_metric(&rows[0], "logical_errors"), 5);
    assert_eq!(case_key_from_rust(&rows[1]), "d5_r15_p0.012");
    assert_eq!(count_metric(&rows[1], "shots_used"), 200);
    assert_eq!(count_metric(&rows[1], "logical_errors"), 10);
}

struct RunnerRow {
    distance: usize,
    rounds: usize,
    p: f64,
    shots: f64,
    logical_errors: f64,
}

fn write_runner_result(root: &Path, runner: &str, row: RunnerRow) {
    let results_dir = root.join(runner).join("test-run");
    fs::create_dir_all(&results_dir).unwrap();
    let result = serde_json::json!({
        "benchmark": "issue65-memory-z-sweep",
        "runner": runner,
        "language": "rust",
        "status": "ok",
        "params": {
            "distance": row.distance,
            "rounds": row.rounds,
            "p": row.p,
        },
        "case_summary": {
            "num_dets": row.distance * row.rounds,
            "num_obs": 1,
        },
        "metrics": {
            "shots_used": row.shots,
            "logical_errors": row.logical_errors,
            "logical_error_rate": row.logical_errors / row.shots,
        },
        "artifacts": {},
        "error": null,
    });
    fs::write(
        results_dir.join("results.jsonl"),
        serde_json::to_string(&result).unwrap() + "\n",
    )
    .unwrap();
}

#[cfg(not(tarpaulin))]
#[test]
#[ignore = "heavy statistical regression: runs the full 15-point issue #65 memory-z sweep"]
fn issue65_memory_z_rstim_ler_agrees_with_stim_reference_intervals() {
    let fixture = load_stim_fixture();
    let rust_rows = run_rust_issue65_sweep();
    assert_eq!(rust_rows.len(), 15);

    let rust_by_case = rust_rows
        .iter()
        .map(|row| (case_key_from_rust(row), row))
        .collect::<BTreeMap<_, _>>();

    for stim in &fixture.rows {
        let key = case_key(stim.distance, stim.rounds, stim.p);
        let rust = rust_by_case
            .get(&key)
            .unwrap_or_else(|| panic!("missing Rust row for {key}"));
        assert_eq!(rust.status, "ok", "Rust row for {key} was not ok");
        assert_eq!(
            rust.case_summary["num_dets"],
            serde_json::json!(stim.num_detectors)
        );
        assert_eq!(
            rust.case_summary["num_obs"],
            serde_json::json!(stim.num_observables)
        );

        let rust_shots = count_metric(rust, "shots_used");
        let rust_errors = count_metric(rust, "logical_errors");
        let rust_fit = fit_binomial(rust_shots, rust_errors, FIT_FACTOR);
        let stim_fit = fit_binomial(stim.shots, stim.logical_errors, FIT_FACTOR);

        let rust_low = rust_fit.low.unwrap();
        let rust_high = rust_fit.high.unwrap();
        let stim_low = stim_fit.low.unwrap();
        let stim_high = stim_fit.high.unwrap();

        assert!(
            rust_low <= stim.logical_error_rate && stim.logical_error_rate <= rust_high,
            "Stim LER for {key}={} outside Rust CI [{}, {}]; Rust errors/shots={}/{} Stim errors/shots={}/{}",
            stim.logical_error_rate,
            rust_low,
            rust_high,
            rust_errors,
            rust_shots,
            stim.logical_errors,
            stim.shots,
        );
        assert!(
            rust_low <= stim_high && stim_low <= rust_high,
            "CI intervals do not overlap for {key}: Rust [{}, {}], Stim [{}, {}]",
            rust_low,
            rust_high,
            stim_low,
            stim_high,
        );
    }
}

fn load_stim_fixture() -> StimFixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json");
    let text = fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[cfg(not(tarpaulin))]
fn run_rust_issue65_sweep() -> Vec<BenchmarkResultRow> {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/issue65_memory_z_sweep.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();
    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();

    read_issue65_runner_results(&artifact_root, &ISSUE65_RUNNERS)
}

fn read_issue65_runner_results(artifact_root: &Path, runners: &[&str]) -> Vec<BenchmarkResultRow> {
    let mut rows = Vec::new();
    for runner in runners {
        let results_path = artifact_root
            .join(runner)
            .join("test-run")
            .join("results.jsonl");
        let data = fs::read(results_path).unwrap();
        rows.extend(read_results_jsonl(&data[..]).unwrap());
    }
    rows
}

fn expected_cases() -> BTreeSet<String> {
    let mut cases = BTreeSet::new();
    for distance in EXPECTED_DISTANCES {
        for p in EXPECTED_PS {
            cases.insert(case_key(distance, distance * 3, p));
        }
    }
    cases
}

fn count_metric(row: &BenchmarkResultRow, key: &str) -> u64 {
    row.metrics[key].round() as u64
}

fn case_key_from_rust(row: &BenchmarkResultRow) -> String {
    case_key(
        row.params["distance"].as_u64().unwrap() as usize,
        row.params["rounds"].as_u64().unwrap() as usize,
        row.params["p"].as_f64().unwrap(),
    )
}

fn case_key(distance: usize, rounds: usize, p: f64) -> String {
    format!("d{distance}_r{rounds}_p{p:.3}")
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= 1e-15,
        "{label}: expected {expected}, got {actual}",
    );
}
