use std::fs;
use std::process::Command;

#[cfg(feature = "plotting")]
use rsinter::bench::bb_compare_csv::read_bb_compare_csv;
#[cfg(feature = "plotting")]
use rsinter::bench::plot::logical_rate_fit_for_plot;
#[cfg(feature = "rbposd-runner")]
use rsinter::bench::result::read_results_jsonl;
#[cfg(feature = "plotting")]
use rsinter::bench::spec::LogicalRateUnit;

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
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rbposd-runner")]
fn rsinter_bench_run_minimal_steane_css_rbposd_fixture_writes_one_ok_row() {
    let dir = tempfile::tempdir().unwrap();
    let spec = "tests/fixtures/bench/minimal_steane_css_rbposd.toml";

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
    let results_path = dir
        .path()
        .join("rbposd-steane")
        .join("test-run")
        .join("results.jsonl");
    let rows = read_results_jsonl(&fs::read(results_path).unwrap()[..]).unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.params["input_type"], serde_json::json!("css"));
    assert_eq!(row.params["code_id"], serde_json::json!("steane"));
    assert_eq!(row.params["decoder_impl"], serde_json::json!("rbposd"));
    assert_eq!(row.status, "ok");
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
#[cfg(feature = "plotting")]
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
#[cfg(all(feature = "rbposd-runner", not(feature = "plotting")))]
fn rsinter_bench_plot_requires_plotting_feature_before_reading_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let input = dir.path().join("missing-results.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "plot",
            "--spec",
            "tests/fixtures/bench/minimal_steane_css_rbposd.toml",
            "--input",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("requires Cargo feature 'plotting'"),
        "{stderr}"
    );
    assert!(!out.exists());
}

#[test]
#[cfg(feature = "plotting")]
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
#[cfg(feature = "plotting")]
fn rsinter_bench_plot_bb_compare_csv_writes_png_from_batched_csv() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bb_results.csv");
    let out = dir
        .path()
        .join("plots")
        .join("bb_circuit_bposd_compare.png");
    fs::write(
        &input,
        "case_id,runner,decoder_impl,code_id,p,num_cycles,shots_budget,errors_budget,shots_used,seed,bp_method,max_iter,osd_method,osd_order,batch_size,batches_completed,setup_seconds,sample_seconds,decode_seconds,run_seconds,logical_errors,logical_error_rate,bp_seconds,osd_seconds,decode_call_count,bp_iteration_count,osd_use_count,osd_candidate_count,gf2_solve_count,gf2_full_elimination_count,status,stop_reason,error\n\
bb72-p001-c6-t10-seed12345,batched_compare,rbposd,bb72,0.001,6,10,200,10,12345,ms,10000,osd_cs,7,5,2,0.1,0.2,0.4,0.7,0,0.0,0.2,0.1,20,10,0,0,0,0,ok,completed,\n\
bb72-p001-c6-t10-seed12345,batched_compare,ldpc_bposd,bb72,0.001,6,10,200,10,12345,ms,10000,osd_cs,7,5,2,0.1,0.0,0.5,0.6,1,0.1,,,,,,,,,ok,completed,\n\
bb72-p002-c6-t10-seed12345,batched_compare,rbposd,bb72,0.002,6,10,200,10,12345,ms,10000,osd_cs,7,5,2,0.1,0.2,0.5,0.8,1,0.1,0.3,0.2,20,10,1,16,1,1,partial,wall_budget_exhausted,\n\
bb72-p002-c6-t10-seed12345,batched_compare,ldpc_bposd,bb72,0.002,6,10,200,10,12345,ms,10000,osd_cs,7,5,2,0.1,0.0,0.7,0.8,2,0.2,,,,,,,,,partial,wall_budget_exhausted,\n\
bb72-skipped,batched_compare,ldpc_bposd,bb72,0.003,6,10,,0,12345,ms,10000,osd_cs,7,5,0,0.0,0.0,0.0,0.0,0,0.0,,,,,,,,,skipped,python_dependency_missing,missing ldpc package\n\
bb72-error,batched_compare,legacy_decoder,bb72,0.003,6,10,,0,12345,ms,10000,osd_cs,7,5,0,0.0,0.0,0.0,0.0,0,0.0,,,,,,,,,error,rust_error,unsupported decoder\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "plot-bb-compare-csv",
            "--spec",
            "../benchmarks/bb_circuit_bposd_compare/plot.toml",
            "--input",
            input.to_str().unwrap(),
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
#[cfg(feature = "plotting")]
fn bb_compare_csv_adapter_preserves_trial_level_ler_for_plot_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bb_results.csv");
    fs::write(
        &input,
        "case_id,runner,decoder_impl,code_id,p,num_cycles,shots_budget,errors_budget,shots_used,seed,bp_method,max_iter,osd_method,osd_order,batch_size,batches_completed,setup_seconds,sample_seconds,decode_seconds,run_seconds,logical_errors,logical_error_rate,bp_seconds,osd_seconds,decode_call_count,bp_iteration_count,osd_use_count,osd_candidate_count,gf2_solve_count,gf2_full_elimination_count,status,stop_reason,error\n\
bb144-p0030-c12-t1000000-seed12345,batched_compare,rbposd,bb144,0.003,12,1000000,200,40000,12345,ms,10000,osd_cs,7,500,80,1.0,2.0,3.0,6.0,200,0.005,1.0,2.0,20,10,1,16,1,1,ok,errors_budget_reached,\n",
    )
    .unwrap();

    let rows = read_bb_compare_csv(&input, "bb_circuit_bposd_compare").unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.params["rounds"], serde_json::json!(12));
    assert_eq!(
        row.case_summary["logical_observable_count"],
        serde_json::json!(1)
    );
    assert_eq!(row.metrics["logical_errors"], 200.0);
    assert_eq!(row.metrics["shots_used"], 40000.0);
    assert_eq!(row.metrics["logical_error_rate"], 0.005);

    let fit = logical_rate_fit_for_plot(row, LogicalRateUnit::PerShot).unwrap();
    assert_eq!(fit.best, Some(0.005));
    assert_ne!(fit.best, Some(200.0 / (40000.0 * 12.0)));
}

#[test]
#[cfg(not(feature = "rbposd-runner"))]
fn rsinter_bb_circuit_bposd_memory_requires_rbposd_runner_feature() {
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
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("requires Cargo feature 'rbposd-runner'"),
        "{stderr}"
    );
}

#[test]
#[cfg(feature = "rbposd-runner")]
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
#[cfg(feature = "rbposd-runner")]
fn rsinter_bb_circuit_bposd_memory_accepts_ldpc_osd_method_for_result_line() {
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
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fields: Vec<_> = stdout.trim().split('\t').collect();
    assert_eq!(fields, vec!["0.000000000001", "1", "1", "0"]);
}

#[test]
#[cfg(feature = "rbposd-runner")]
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
#[cfg(feature = "rbposd-runner")]
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
            "10000",
            "--osd-order",
            "7",
            "--osd-method",
            "osd_cs",
            "--json-compare-case",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["max_bp_iterations"], 10000);
    assert_eq!(json["osd_order"], 7);
    let trial = &json["trials"][0];
    assert_eq!(trial["z_logical_prediction"].as_array().unwrap().len(), 12);
    assert!(trial["z_correction"].as_array().is_some());
    assert_eq!(
        trial["z_correction"].as_array().unwrap().len(),
        json["z_model"]["num_bits"].as_u64().unwrap() as usize
    );
    assert_eq!(trial["z_profile"]["decode_call_count"], 1);
    assert!(trial["z_profile"]["decode_seconds"].as_f64().unwrap() >= 0.0);
    assert!(trial["x_logical_prediction"].as_array().is_some());
    assert!(trial["x_correction"].as_array().is_some());
    assert_eq!(
        trial["x_correction"].as_array().unwrap().len(),
        json["x_model"]["num_bits"].as_u64().unwrap() as usize
    );
    assert_eq!(trial["x_profile"]["decode_call_count"], 1);
}

#[test]
#[cfg(feature = "rbposd-runner")]
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
