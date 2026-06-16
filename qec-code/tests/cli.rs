use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, CodeCommands, Commands, CssArgs, CssMatrixKind, run};
use tempfile::tempdir;

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_fixture(rel_path: &str) -> String {
    std::fs::read_to_string(workspace_root().join(rel_path)).expect("fixture should be readable")
}

fn run_qec_code(args: &[&str]) -> Output {
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("qec-code binary should run")
}

fn run_qec_code_in_process(args: &[&str]) -> Result<String, QecError> {
    run_qec_code_in_process_os(args.iter().map(OsString::from).collect())
}

fn run_qec_code_in_process_os(args: Vec<OsString>) -> Result<String, QecError> {
    let mut argv = vec![OsString::from("qec-code")];
    argv.extend(args);
    run(Cli::parse_from(argv))
}

fn write_matrix_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).expect("matrix fixture should be writable");
    path
}

#[derive(Debug, Clone, Copy)]
struct BuiltInCssFixtureCase {
    code_id: &'static str,
    matrix: &'static str,
    fixture: &'static str,
}

const BUILT_IN_CSS_FIXTURE_CASES: &[BuiltInCssFixtureCase] = &[
    BuiltInCssFixtureCase {
        code_id: "steane",
        matrix: "hx",
        fixture: "steane_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "steane",
        matrix: "hz",
        fixture: "steane_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "repetition_x:d=5",
        matrix: "hx",
        fixture: "repetition_x_d5_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "repetition_z:d=5",
        matrix: "hz",
        fixture: "repetition_z_d5_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "bb72",
        matrix: "hx",
        fixture: "bb72_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "bb72",
        matrix: "hz",
        fixture: "bb72_hz.json",
    },
];

#[test]
fn steane_summary_reports_basic_code_parameters() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "summary"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("n: 7"), "stdout was: {stdout}");
    assert!(
        stdout.contains("stabilizer_rank: 6"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("k: 1"), "stdout was: {stdout}");
}

#[test]
fn steane_distance_reports_distance_and_logical_class() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "distance"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("distance: 3"), "stdout was: {stdout}");
    assert!(stdout.contains("logical_class:"), "stdout was: {stdout}");
}

#[test]
fn steane_stabilizers_reports_generator_lines() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "stabilizers"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("g1:"), "stdout was: {stdout}");
    assert!(stdout.contains("g6:"), "stdout was: {stdout}");
}

#[test]
fn steane_logicals_reports_logical_sections() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "logicals"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("logical_x:"), "stdout was: {stdout}");
    assert!(stdout.contains("logical_z:"), "stdout was: {stdout}");
    assert!(stdout.contains("  1:"), "stdout was: {stdout}");
    assert!(stdout.contains("weight="), "stdout was: {stdout}");
}

#[test]
fn code_css_steane_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_export_subcommand_steane_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "export", "steane", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_steane_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_unknown_id_fails() {
    let output = run_qec_code(&["code", "css", "unknown", "hx"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");

    assert!(
        stderr.contains("unknown built-in CSS code: unknown"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_bb72_hx_prints_sparse_rows_json() {
    let output = run_qec_code(&["code", "css", "bb72", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 72);
    assert_eq!(rows.len(), 36);
    assert!(
        rows.iter()
            .all(|row| row.as_array().is_some_and(|cols| cols.len() == 6)),
        "all bb72 hx rows should have weight 6: {rows:?}"
    );
}

#[test]
fn code_css_list_includes_supported_built_ins() {
    let output = run_qec_code(&["code", "css", "list"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(
        stdout.contains("Built-in CSS codes:"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("steane"), "stdout was: {stdout}");
    assert!(stdout.contains("bb72"), "stdout was: {stdout}");
    assert!(
        stdout.contains("repetition_x:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("repetition_z:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("surface_rotated:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("distance >= 2"), "stdout was: {stdout}");
}

#[test]
fn code_css_list_rejects_unexpected_extra_arguments() {
    let output = run_qec_code(&["code", "css", "list", "extra"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");

    assert!(
        stderr.contains("extra") || stderr.contains("Usage:"),
        "stderr was: {stderr}"
    );
}

#[test]
fn built_in_css_fixture_manifest_exports_match_pinned_json() {
    for case in BUILT_IN_CSS_FIXTURE_CASES {
        let output = run_qec_code(&["code", "css", case.code_id, case.matrix]);

        assert!(output.status.success());
        assert!(output.stderr.is_empty());

        let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
        let fixture_path = format!("qec-code/tests/fixtures/css/{}", case.fixture);
        let expected = read_fixture(&fixture_path);

        assert_eq!(
            stdout, expected,
            "case {case:?} stdout differed from fixture {}",
            case.fixture
        );
    }
}

#[test]
fn run_code_css_steane_matrices_return_fixture_json_without_newline() {
    let hx = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export("steane".to_owned(), CssMatrixKind::Hx)),
        },
    })
    .unwrap();
    let hz = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export("steane".to_owned(), CssMatrixKind::Hz)),
        },
    })
    .unwrap();

    let expected_hx = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");
    let expected_hz = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(hx, expected_hx.trim_end_matches('\n'));
    assert_eq!(hz, expected_hz.trim_end_matches('\n'));
}

#[test]
fn run_code_css_list_returns_catalog_without_newline() {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::list()),
        },
    })
    .unwrap();

    let expected = "Built-in CSS codes:\n  steane                        fixed [[7,1,3]] CSS code\n  bb72                          fixed [[72,12,6]] bivariate-bicycle CSS code\n  repetition_x:d=<distance>     X-check chain, distance >= 2\n  repetition_z:d=<distance>     Z-check chain, distance >= 2\n  surface_rotated:d=<distance>  rotated surface CSS code, distance >= 2";
    assert_eq!(output, expected);
}

#[test]
fn run_code_css_export_subcommand_returns_fixture_json_without_newline() {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export_subcommand(
                "steane".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();

    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");
    assert_eq!(output, expected.trim_end_matches('\n'));
}

#[test]
fn run_code_css_unknown_id_returns_registry_error() {
    let result = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export("unknown".to_owned(), CssMatrixKind::Hx)),
        },
    });

    assert_eq!(
        result,
        Err(QecError::UnknownBuiltInCssCode {
            code_id: "unknown".to_owned(),
        })
    );
}

#[test]
fn run_code_css_distance_randomized_upper_bound_code_id_returns_json() {
    let stdout = run_qec_code_in_process(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "500",
        "--restarts",
        "4",
        "--seed",
        "7",
        "--target-weight",
        "3",
        "--json",
    ])
    .unwrap();

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
}

#[test]
fn run_code_css_distance_randomized_upper_bound_files_return_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let stdout = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("randomized-upper-bound"),
        OsString::from("--hx"),
        hx.into_os_string(),
        OsString::from("--hz"),
        hz.into_os_string(),
        OsString::from("--iterations"),
        OsString::from("500"),
        OsString::from("--restarts"),
        OsString::from("4"),
        OsString::from("--seed"),
        OsString::from("7"),
        OsString::from("--target-weight"),
        OsString::from("3"),
        OsString::from("--json"),
    ])
    .unwrap();

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["upper_bound"], 3);
}

#[test]
fn run_code_css_distance_randomized_upper_bound_rejects_input_errors() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let conflicting_inputs = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("randomized-upper-bound"),
        OsString::from("--code-id"),
        OsString::from("steane"),
        OsString::from("--hx"),
        hx.clone().into_os_string(),
        OsString::from("--hz"),
        hz.into_os_string(),
        OsString::from("--iterations"),
        OsString::from("10"),
        OsString::from("--seed"),
        OsString::from("7"),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        conflicting_inputs,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains("use either --code-id or --hx/--hz")
    ));

    let missing_pair = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("randomized-upper-bound"),
        OsString::from("--hx"),
        hx.into_os_string(),
        OsString::from("--iterations"),
        OsString::from("10"),
        OsString::from("--seed"),
        OsString::from("7"),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        missing_pair,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains("--hx and --hz must be provided together")
    ));

    let missing_source = run_qec_code_in_process(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--iterations",
        "10",
        "--seed",
        "7",
        "--json",
    ]);
    assert!(matches!(
        missing_source,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains("provide --code-id or both --hx and --hz")
    ));
}

#[test]
fn run_code_css_distance_randomized_upper_bound_rejects_output_and_file_errors() {
    assert_eq!(
        run_qec_code_in_process(&[
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--code-id",
            "steane",
            "--iterations",
            "10",
            "--seed",
            "7",
        ]),
        Err(QecError::JsonOutputRequired {
            command: "code css-distance randomized-upper-bound",
        })
    );

    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,1]]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":4,"rows":[[2,3]]}"#,
    );
    let mismatched_widths = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("randomized-upper-bound"),
        OsString::from("--hx"),
        hx.into_os_string(),
        OsString::from("--hz"),
        hz.into_os_string(),
        OsString::from("--iterations"),
        OsString::from("10"),
        OsString::from("--seed"),
        OsString::from("7"),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        mismatched_widths,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains("hx width 3 does not match hz width 4")
    ));

    let missing_hx = dir.path().join("missing-hx.json");
    let readable_hz = write_matrix_file(
        dir.path(),
        "readable-hz.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[]}"#,
    );
    let read_failure = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("randomized-upper-bound"),
        OsString::from("--hx"),
        missing_hx.into_os_string(),
        OsString::from("--hz"),
        readable_hz.into_os_string(),
        OsString::from("--iterations"),
        OsString::from("10"),
        OsString::from("--seed"),
        OsString::from("7"),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        read_failure,
        Err(QecError::CssMatrixReadFailed { .. })
    ));
}

#[cfg(not(feature = "distance-ilp-highs"))]
#[test]
fn large_distance_errors_render_configuration_message() {
    let stderr = qec_code::QecError::DistanceComputationUnsupported {
        n: 32,
        reason: "enable a distance ILP feature or use a smaller code".into(),
    }
    .to_string();

    assert!(stderr.contains("distance computation is unsupported"));
}

#[test]
fn css_distance_randomized_upper_bound_code_id_outputs_json() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "500",
        "--restarts",
        "4",
        "--seed",
        "7",
        "--target-weight",
        "3",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
    assert_eq!(json["options"]["seed"], 7);
}

#[test]
fn css_distance_randomized_upper_bound_hx_hz_files_output_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "randomized-upper-bound", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args([
            "--iterations",
            "500",
            "--restarts",
            "4",
            "--seed",
            "7",
            "--target-weight",
            "3",
            "--json",
        ])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
}

#[test]
fn css_distance_randomized_upper_bound_requires_json_flag() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "10",
        "--seed",
        "7",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("JSON output is required for code css-distance randomized-upper-bound"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_rejects_code_id_and_file_input_together() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--code-id",
            "steane",
            "--hx",
        ])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args(["--iterations", "10", "--seed", "7", "--json"])
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("use either --code-id or --hx/--hz, not both"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_rejects_missing_matrix_pair() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "randomized-upper-bound", "--hx"])
        .arg(hx)
        .args(["--iterations", "10", "--seed", "7", "--json"])
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--hx and --hz must be provided together"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_rejects_missing_input_source() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--iterations",
        "10",
        "--seed",
        "7",
        "--json",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("provide --code-id or both --hx and --hz"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_rejects_mismatched_file_widths() {
    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,1]]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":4,"rows":[[2,3]]}"#,
    );
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "randomized-upper-bound", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args(["--iterations", "10", "--seed", "7", "--json"])
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("hx width 3 does not match hz width 4"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_reports_matrix_read_failures() {
    let dir = tempdir().unwrap();
    let hx = dir.path().join("missing-hx.json");
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[]}"#,
    );
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "randomized-upper-bound", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args(["--iterations", "10", "--seed", "7", "--json"])
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to read CSS matrix"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("missing-hx.json"), "stderr was: {stderr}");
}

#[test]
fn css_distance_randomized_upper_bound_rejects_zero_iterations_without_stdout() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "0",
        "--seed",
        "7",
        "--json",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid distance bound option iterations"),
        "stderr was: {stderr}"
    );
}
