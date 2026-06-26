use std::fs;
use std::process::Command;

#[test]
fn rsinter_cli_help_mentions_bench_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("bench"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("merge"));
    assert!(stdout.contains("plot"));
}

#[test]
fn rsinter_bench_run_help_mentions_resume_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args(["bench", "run", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--resume"), "{stdout}");
}

#[test]
fn rsinter_bench_run_writes_artifacts_from_fixture_spec() {
    let dir = tempfile::tempdir().unwrap();
    let spec = "tests/fixtures/bench/minimal_surface_decoder.toml";

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "run",
            "--spec",
            spec,
            "--language",
            "rust",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert!(!entries.is_empty());
}

#[test]
fn rsinter_bench_run_writes_artifacts_from_css_fixture_spec() {
    let dir = tempfile::tempdir().unwrap();
    let spec = "tests/fixtures/bench/minimal_css_decoder.toml";

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "run",
            "--spec",
            spec,
            "--language",
            "rust",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert!(!entries.is_empty());
}

#[test]
fn rsinter_bench_merge_writes_combined_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.jsonl");
    let second = dir.path().join("second.jsonl");
    let out = dir.path().join("merged").join("merged.jsonl");

    fs::write(
        &first,
        concat!(
            "{\"benchmark\":\"surface_decoder\",\"runner\":\"z\",\"language\":\"rust\",\"status\":\"ok\",",
            "\"params\":{\"distance\":5,\"p\":0.005},\"case_summary\":{},\"metrics\":{\"logical_error_rate\":0.01},",
            "\"artifacts\":{},\"error\":null}\n"
        ),
    )
    .unwrap();
    fs::write(
        &second,
        concat!(
            "{\"benchmark\":\"surface_decoder\",\"runner\":\"a\",\"language\":\"python\",\"status\":\"ok\",",
            "\"params\":{\"distance\":3,\"p\":0.002},\"case_summary\":{},\"metrics\":{\"logical_error_rate\":0.001},",
            "\"artifacts\":{},\"error\":null}\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "merge",
            "--spec",
            "tests/fixtures/bench/minimal_surface_decoder.toml",
            "--input",
            first.to_str().unwrap(),
            "--input",
            second.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let merged = fs::read_to_string(out).unwrap();
    assert!(merged.contains("\"runner\":\"a\""));
    assert!(merged.contains("\"runner\":\"z\""));
}

#[test]
fn rsinter_bench_plot_writes_svg_from_jsonl_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("rows.jsonl");
    let out = dir.path().join("plots").join("plot.svg");
    fs::write(
        &input,
        concat!(
            "{\"benchmark\":\"surface_decoder\",\"runner\":\"rmatching\",\"language\":\"rust\",\"status\":\"ok\",",
            "\"params\":{\"distance\":3,\"p\":0.002},\"case_summary\":{},",
            "\"metrics\":{\"logical_error_rate\":0.001,\"decode_us_per_shot\":12.0,\"shots_used\":2000,\"logical_errors\":2},",
            "\"artifacts\":{},\"error\":null}\n",
            "{\"benchmark\":\"surface_decoder\",\"runner\":\"rmatching\",\"language\":\"rust\",\"status\":\"ok\",",
            "\"params\":{\"distance\":3,\"p\":0.005},\"case_summary\":{},",
            "\"metrics\":{\"logical_error_rate\":0.01,\"decode_us_per_shot\":18.0,\"shots_used\":2000,\"logical_errors\":20},",
            "\"artifacts\":{},\"error\":null}\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "plot",
            "--spec",
            "tests/fixtures/bench/minimal_surface_decoder.toml",
            "--input",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert!(
        fs::read_to_string(dir.path().join("plots").join("plot.svg"))
            .unwrap()
            .contains("<svg")
    );
}

#[test]
fn rsinter_bench_plot_surface_compare_csv_writes_png_from_legacy_csv() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plots").join("surface_decoder_compare.png");

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "plot-surface-compare-csv",
            "--spec",
            "../benchmarks/surface_decoder/spec.toml",
            "--input",
            "../benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let png = fs::read(out).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn rsinter_bb90_circuit_bposd_memory_prints_four_column_result_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--code-id",
            "bb90",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "1",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fields: Vec<_> = stdout.trim().split('\t').collect();
    assert_eq!(fields, vec!["0.000000000001", "1", "1", "0"]);
}

#[test]
fn rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--code-id",
            "bb72",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "12345",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
            "--json-compare-case",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["code_id"], "bb72");
    assert_eq!(json["num_trials"], 1);
    assert_eq!(json["max_bp_iterations"], 10);
    assert_eq!(json["osd_order"], 0);
    assert_eq!(json["trials"].as_array().unwrap().len(), 1);
    assert_eq!(json["z_model"]["first_logical_row"], 36 * 3);
    assert_eq!(json["x_model"]["first_logical_row"], 36 * 3);
    assert_eq!(json["trials"][0]["z_logical"].as_array().unwrap().len(), 12);
    assert_eq!(json["trials"][0]["x_logical"].as_array().unwrap().len(), 12);
    assert!(json["rust_result"]["profile"]["setup_seconds"].is_number());
    assert!(json["z_model"]["sparse_rows"].as_array().unwrap().len() > 0);
}

#[test]
fn rsinter_json_compare_case_accepts_ldpc_osd_method_and_exports_trial_predictions() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--code-id",
            "bb72",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "12345",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
            "--osd-method",
            "osd_cs",
            "--json-compare-case",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let trial = &json["trials"][0];
    assert_eq!(trial["z_logical_prediction"].as_array().unwrap().len(), 12);
    assert_eq!(trial["z_profile"]["decode_call_count"], 1);
    assert!(trial["z_profile"]["decode_seconds"].as_f64().unwrap() >= 0.0);
    assert!(trial["x_logical_prediction"].as_array().is_some());
    assert_eq!(trial["x_profile"]["decode_call_count"], 1);
}

#[test]
fn rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--physical-error-rate",
            "-0.1",
            "--num-cycles",
            "12",
            "--num-trials",
            "100",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "{output:?}");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stdout.trim().is_empty(),
        "invalid command should not print a completed result line: {stdout:?}"
    );
    assert!(stderr.contains("physical_error_rate must be finite and lie in [0, 1)"));
    assert!(
        !stdout
            .lines()
            .any(|line| line.split_whitespace().count() == 4),
        "invalid command printed a four-column result line: {stdout:?}"
    );
}

#[test]
fn bb144_reproduction_evidence_note_records_required_context() {
    let note = include_str!("../../docs/bb144_circuit_bposd_reproduction.md");

    for required in [
        "0.003\t12\t5\t0",
        "--num-trials 50000",
        "--seed 12345",
        "95% one-sided Clopper-Pearson upper bound",
        "does not claim statistical agreement",
        "small_ldpc.png",
        "red [[144,12,12]] LDPC curve",
        "ldpc_vs_surface.png",
        "red-diamond LDPC [[144,12,12]] curve",
        "--max-bp-iterations 10000",
        "--osd-order 7",
        "physical_error_rate must be finite and lie in [0, 1)",
    ] {
        assert!(
            note.contains(required),
            "missing evidence token: {required}"
        );
    }
}
