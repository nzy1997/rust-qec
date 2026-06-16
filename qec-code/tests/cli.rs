use std::path::PathBuf;
use std::process::{Command, Output};

use qec_code::cli::{run, Cli, CodeCommands, Commands, CssArgs, CssMatrixKind};
use qec_code::QecError;

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

    let expected = "Built-in CSS codes:\n  steane                     fixed [[7,1,3]] CSS code\n  bb72                       fixed [[72,12,6]] bivariate-bicycle CSS code\n  repetition_x:d=<distance>  X-check chain, distance >= 2\n  repetition_z:d=<distance>  Z-check chain, distance >= 2";
    assert_eq!(output, expected);
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
