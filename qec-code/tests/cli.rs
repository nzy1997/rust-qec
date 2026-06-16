use std::path::PathBuf;
use std::process::{Command, Output};

use qec_code::QecError;
use qec_code::cli::{Cli, CodeCommands, Commands, CssMatrixKind, run};

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
fn run_code_css_steane_matrices_return_fixture_json_without_newline() {
    let hx = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css {
                code_id: "steane".to_owned(),
                matrix: CssMatrixKind::Hx,
            },
        },
    })
    .unwrap();
    let hz = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css {
                code_id: "steane".to_owned(),
                matrix: CssMatrixKind::Hz,
            },
        },
    })
    .unwrap();

    let expected_hx = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");
    let expected_hz = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(hx, expected_hx.trim_end_matches('\n'));
    assert_eq!(hz, expected_hz.trim_end_matches('\n'));
}

#[test]
fn run_code_css_unknown_id_returns_registry_error() {
    let result = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css {
                code_id: "unknown".to_owned(),
                matrix: CssMatrixKind::Hx,
            },
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
