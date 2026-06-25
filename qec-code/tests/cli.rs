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

fn quantum_tanner_fixture_path(name: &str) -> PathBuf {
    workspace_root()
        .join("qec-code/tests/fixtures/quantum_tanner")
        .join(name)
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

const BB72_PARAMETERIZED_SPEC: &str = "bb:lx=6,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";
const BB144_PARAMETERIZED_SPEC: &str = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";
const BB_FAMILY_CATALOG_SPEC: &str =
    "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...";
const BB_INVALID_LX_ERROR: &str = "out-of-range built-in CSS integer parameter lx for family bb: 0";

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
    BuiltInCssFixtureCase {
        code_id: BB72_PARAMETERIZED_SPEC,
        matrix: "hx",
        fixture: "bb72_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: BB72_PARAMETERIZED_SPEC,
        matrix: "hz",
        fixture: "bb72_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "surface_rotated:d=3",
        matrix: "hx",
        fixture: "surface_rotated_d3_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "surface_rotated:d=3",
        matrix: "hz",
        fixture: "surface_rotated_d3_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "toric:d=3",
        matrix: "hx",
        fixture: "toric_d3_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "toric:d=3",
        matrix: "hz",
        fixture: "toric_d3_hz.json",
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
fn code_css_bb_parameterized_hx_matches_bb72_fixture() {
    let output = run_qec_code(&["code", "css", BB72_PARAMETERIZED_SPEC, "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/bb72_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_bb_parameterized_hz_matches_bb72_fixture() {
    let output = run_qec_code(&["code", "css", BB72_PARAMETERIZED_SPEC, "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/bb72_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_bb144_parameterized_hx_prints_sparse_rows_shape() {
    // This is qec-code construction/export coverage only; circuit-level
    // BB144 work and benchmark reproduction remain downstream in #110/#124.
    let output = run_qec_code(&["code", "css", BB144_PARAMETERIZED_SPEC, "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 144);
    assert_eq!(rows.len(), 72);
    assert!(
        rows.iter()
            .all(|row| row.as_array().is_some_and(|cols| cols.len() == 6)),
        "all BB144 hx rows should have weight 6: {rows:?}"
    );
}

#[test]
fn code_css_bb_parameterized_invalid_lattice_dimension_fails_without_json() {
    let output = run_qec_code(&["code", "css", "bb:lx=0,ly=6,a=3:0,b=0:3", "hx"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(stderr.contains(BB_INVALID_LX_ERROR), "stderr was: {stderr}");
}

#[test]
fn code_css_bb_parameterized_malformed_shift_term_fails_without_json() {
    let output = run_qec_code(&["code", "css", "bb:lx=12,ly=6,a=3:0|,b=0:3", "hx"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("invalid built-in CSS integer parameter a for family bb"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_surface_rotated_d3_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "surface_rotated:d=3", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_surface_rotated_d3_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "surface_rotated:d=3", "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_toric_d3_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "toric:d=3", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/toric_d3_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_toric_d3_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "toric:d=3", "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/toric_d3_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_toric_missing_or_bad_distance_fails() {
    #[derive(Debug)]
    struct FailureCase {
        args: &'static [&'static str],
        stderr_fragment: &'static str,
    }

    const CASES: &[FailureCase] = &[
        FailureCase {
            args: &["code", "css", "toric", "hx"],
            stderr_fragment: "missing built-in CSS parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=nope", "hx"],
            stderr_fragment: "invalid built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=1", "hx"],
            stderr_fragment: "out-of-range built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=3", "foo"],
            stderr_fragment: "invalid value 'foo'",
        },
    ];

    for case in CASES {
        let output = run_qec_code(case.args);

        assert!(
            !output.status.success(),
            "case {case:?} unexpectedly succeeded"
        );
        assert_eq!(output.stdout, b"", "case {case:?} should not print stdout");

        let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
        assert!(
            stderr.contains(case.stderr_fragment),
            "case {case:?} stderr was: {stderr}"
        );
    }
}

#[test]
fn code_css_surface_rotated_missing_or_bad_distance_fails() {
    #[derive(Debug)]
    struct FailureCase {
        args: &'static [&'static str],
        stderr_fragment: &'static str,
    }

    const CASES: &[FailureCase] = &[
        FailureCase {
            args: &["code", "css", "surface_rotated", "hx"],
            stderr_fragment: "missing built-in CSS parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=nope", "hx"],
            stderr_fragment: "invalid built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=1", "hx"],
            stderr_fragment: "out-of-range built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=3", "foo"],
            stderr_fragment: "invalid value 'foo'",
        },
    ];

    for case in CASES {
        let output = run_qec_code(case.args);

        assert!(
            !output.status.success(),
            "case {case:?} unexpectedly succeeded"
        );
        assert_eq!(output.stdout, b"", "case {case:?} should not print stdout");

        let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
        assert!(
            stderr.contains(case.stderr_fragment),
            "case {case:?} stderr was: {stderr}"
        );
    }
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
        stdout.contains(BB_FAMILY_CATALOG_SPEC),
        "stdout was: {stdout}"
    );
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
    assert!(
        stdout.contains("toric:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("distance >= 2"), "stdout was: {stdout}");
}

#[test]
fn apm_kasai_css_export() {
    let list = run_qec_code(&["code", "css", "list"]);
    assert!(list.status.success());
    assert_eq!(list.stderr, b"");

    let list_stdout = String::from_utf8(list.stdout).expect("stdout should be valid utf-8");
    assert!(
        list_stdout.contains("apm_kasai:p=96"),
        "stdout was: {list_stdout}"
    );
    assert!(
        list_stdout.contains("apm_kasai:p=192"),
        "stdout was: {list_stdout}"
    );

    for (code_id, expected_num_cols) in [("apm_kasai:p=96", 1152), ("apm_kasai:p=192", 2304)] {
        for matrix in ["hx", "hz"] {
            let output = run_qec_code(&["code", "css", code_id, matrix]);
            assert!(output.status.success());
            assert_eq!(output.stderr, b"");

            let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
            let json: serde_json::Value =
                serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
            assert_eq!(json["format"], "sparse_rows");
            assert_eq!(json["num_cols"], expected_num_cols);
            assert!(
                json["rows"].as_array().is_some_and(|rows| !rows.is_empty()),
                "rows should be non-empty: {json}"
            );
        }
    }

    let p128 = run_qec_code(&["code", "css", "apm_kasai:p=128", "hx"]);
    assert!(!p128.status.success());
    assert_eq!(p128.stdout, b"");
    let p128_stderr = String::from_utf8(p128.stderr).expect("stderr should be valid utf-8");
    assert!(
        p128_stderr
            .contains("unsupported built-in CSS integer parameter p for family apm_kasai: 128"),
        "stderr was: {p128_stderr}"
    );
    assert!(
        p128_stderr.contains("supported: 96, 192"),
        "stderr was: {p128_stderr}"
    );
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
fn run_code_css_surface_rotated_d3_matrices_return_fixture_json_without_newline() {
    let hx = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let hz = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();

    let expected_hx = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hx.json");
    let expected_hz = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hz.json");

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

    let width = "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...".len();
    let expected = format!(
        "Built-in CSS codes:\n  {steane:width$}  fixed [[7,1,3]] CSS code\n  {bb72:width$}  fixed [[72,12,6]] bivariate-bicycle CSS code\n  {apm96:width$}  fixed Table A1 P=96 APM-CSS code\n  {apm192:width$}  fixed Table A1 P=192 APM-CSS code\n  {bb:width$}  bivariate-bicycle CSS family over periodic lattice\n  {rep_x:width$}  X-check chain, distance >= 2\n  {rep_z:width$}  Z-check chain, distance >= 2\n  {surf:width$}  rotated surface CSS code, distance >= 2\n  {toric:width$}  periodic square-lattice toric CSS code, distance >= 2",
        steane = "steane",
        bb72 = "bb72",
        apm96 = "apm_kasai:p=96",
        apm192 = "apm_kasai:p=192",
        bb = "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...",
        rep_x = "repetition_x:d=<distance>",
        rep_z = "repetition_z:d=<distance>",
        surf = "surface_rotated:d=<distance>",
        toric = "toric:d=<distance>",
        width = width,
    );
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
fn run_code_css_surface_rotated_export_subcommand_returns_fixture_json_without_newline() {
    let hx = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export_subcommand(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let hz = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export_subcommand(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();

    let expected_hx = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hx.json");
    let expected_hz = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hz.json");

    assert_eq!(hx, expected_hx.trim_end_matches('\n'));
    assert_eq!(hz, expected_hz.trim_end_matches('\n'));
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
            if message.contains(
                "use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz",
            )
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
            if message
                .contains("provide --code-id, --quantum-tanner-spec, or both --hx and --hz")
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

#[test]
fn code_css_distance_exact_code_id_returns_exact_json() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "code_id");
    assert_eq!(json["options"]["code_id"], "steane");
    assert_eq!(json["provenance"]["tool"], "qec-code");
}

#[test]
fn run_code_css_distance_exact_code_id_returns_exact_json() {
    let output = run_qec_code_in_process(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--json",
    ])
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "code_id");
    assert_eq!(json["options"]["code_id"], "steane");
    assert_eq!(json["provenance"]["tool"], "qec-code");
}

#[test]
fn code_css_distance_exact_hx_hz_files_return_exact_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(&hx)
        .arg("--hz")
        .arg(&hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "files");
    assert_eq!(json["options"]["hx"], hx.display().to_string());
    assert_eq!(json["options"]["hz"], hz.display().to_string());
}

#[test]
fn code_css_distance_exact_quantum_tanner_spec_returns_exact_json() {
    let spec = quantum_tanner_fixture_path("toric_d4.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--quantum-tanner-spec"])
        .arg(&spec)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Fixture follows qLDPC's toric Tanner test: Z_d x Z_d has distance d.
    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 4);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 4);
    assert_eq!(json["options"]["input"], "quantum_tanner_spec");
    assert_eq!(
        json["options"]["quantum_tanner_spec"],
        spec.display().to_string()
    );
}

#[test]
fn code_css_distance_exact_quantum_tanner_invalid_spec_fails_before_distance_result() {
    let spec = quantum_tanner_fixture_path("invalid_non_symmetric_a.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--quantum-tanner-spec"])
        .arg(spec)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid quantum Tanner generator set A"),
        "stderr was: {stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be a distance result: {stderr}"
    );
}

#[test]
fn run_code_css_distance_exact_files_return_exact_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("exact"),
        OsString::from("--hx"),
        hx.clone().into_os_string(),
        OsString::from("--hz"),
        hz.clone().into_os_string(),
        OsString::from("--json"),
    ])
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "files");
    assert_eq!(json["options"]["hx"], hx.display().to_string());
    assert_eq!(json["options"]["hz"], hz.display().to_string());
}

#[test]
fn code_css_distance_exact_requires_json_flag() {
    let output = run_qec_code(&["code", "css-distance", "exact", "--code-id", "steane"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("JSON output is required for code css-distance exact"),
        "stderr was: {stderr}"
    );
}

#[test]
fn run_code_css_distance_exact_rejects_input_errors() {
    let missing_json =
        run_qec_code_in_process(&["code", "css-distance", "exact", "--code-id", "steane"]);
    assert!(matches!(
        missing_json,
        Err(QecError::JsonOutputRequired { command })
            if command == "code css-distance exact"
    ));

    let missing_source = run_qec_code_in_process(&["code", "css-distance", "exact", "--json"]);
    assert!(matches!(
        missing_source,
        Err(QecError::InvalidCssDistanceInput(message))
            if message
                .contains("provide --code-id, --quantum-tanner-spec, or both --hx and --hz")
    ));

    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[]}"#,
    );

    let mixed_input = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("exact"),
        OsString::from("--code-id"),
        OsString::from("steane"),
        OsString::from("--hx"),
        hx.clone().into_os_string(),
        OsString::from("--hz"),
        hz.into_os_string(),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        mixed_input,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains(
                "use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz",
            )
    ));

    let missing_pair = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css-distance"),
        OsString::from("exact"),
        OsString::from("--hx"),
        hx.into_os_string(),
        OsString::from("--json"),
    ]);
    assert!(matches!(
        missing_pair,
        Err(QecError::InvalidCssDistanceInput(message))
            if message.contains("--hx and --hz must be provided together")
    ));
}

#[test]
fn code_css_distance_exact_rejects_code_id_and_file_input_together() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "exact",
            "--code-id",
            "steane",
            "--hx",
        ])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr
            .contains("use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_missing_matrix_pair() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--json")
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
fn code_css_distance_exact_rejects_missing_input_source() {
    let output = run_qec_code(&["code", "css-distance", "exact", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("provide --code-id, --quantum-tanner-spec, or both --hx and --hz"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_mismatched_file_widths() {
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
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
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
fn code_css_distance_exact_rejects_non_commuting_css_before_solving() {
    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":1,"rows":[[0]]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":1,"rows":[[0]]}"#,
    );
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("CSS X/Z checks are not orthogonal"),
        "stderr was: {stderr}"
    );
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_surface_rotated_known_distances_with_ilp() {
    for (code_id, expected_distance) in [
        ("surface_rotated:d=3", 3),
        ("surface_rotated:d=5", 5),
        ("surface_rotated:d=7", 7),
    ] {
        let output = run_qec_code(&[
            "code",
            "css-distance",
            "exact",
            "--code-id",
            code_id,
            "--json",
        ]);

        assert!(output.status.success(), "case {code_id} failed");
        assert_eq!(output.stderr, b"", "case {code_id} printed stderr");

        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

        assert_eq!(json["distance"], expected_distance);
        assert_eq!(json["bound_type"], "exact");
        assert_eq!(json["method"], "rstim-ilp-exact");
        assert_eq!(json["witness"]["weight"], expected_distance);
    }
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_bb72_known_distance_with_ilp() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "bb72",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["distance"], 6);
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["witness"]["weight"], 6);
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
fn css_distance_randomized_upper_bound_quantum_tanner_spec_outputs_json() {
    let spec = quantum_tanner_fixture_path("toric_d4.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--quantum-tanner-spec",
        ])
        .arg(spec)
        .args([
            "--iterations",
            "1000",
            "--restarts",
            "8",
            "--seed",
            "7",
            "--target-weight",
            "4",
            "--json",
        ])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert!(json["upper_bound"].as_u64().unwrap() <= 4);
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
        stderr
            .contains("use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz"),
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
        stderr.contains("provide --code-id, --quantum-tanner-spec, or both --hx and --hz"),
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

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_accepts_highs_backend_and_solver_limits() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--backend",
        "highs",
        "--time-limit-seconds",
        "300",
        "--mip-gap",
        "0.001",
        "--threads",
        "1",
        "--verbose-solver",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "optimal");
    assert_eq!(json["time_limit_seconds"], 300.0);
    assert_eq!(json["mip_gap"], 0.001);
    assert_eq!(json["threads"], 1);
    assert_eq!(json["verbose_solver"], true);
    assert_eq!(json["options"]["backend"], "highs");
}

#[test]
fn code_css_distance_exact_rejects_gurobi_backend_without_feature() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--backend",
        "gurobi",
        "--json",
    ]);

    if cfg!(feature = "distance-ilp-gurobi") {
        assert!(output.status.success());
    } else {
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("ILP backend is unavailable"),
            "stderr was: {stderr}"
        );
        assert!(stderr.contains("Gurobi"), "stderr was: {stderr}");
    }
}

#[test]
fn run_code_css_distance_exact_rejects_invalid_solver_options() {
    for (flag, value, expected) in [
        ("--time-limit-seconds", "0", "time_limit_seconds"),
        ("--time-limit-seconds", "NaN", "time_limit_seconds"),
        ("--mip-gap", "-0.1", "mip_gap"),
        ("--mip-gap", "NaN", "mip_gap"),
        ("--threads", "0", "threads"),
    ] {
        let result = run_qec_code_in_process(&[
            "code",
            "css-distance",
            "exact",
            "--code-id",
            "steane",
            flag,
            value,
            "--json",
        ]);
        assert!(
            matches!(
                result,
                Err(QecError::InvalidCssDistanceInput(ref message)) if message.contains(expected)
            ),
            "expected invalid {expected} error for {flag} {value}, got {result:?}",
        );
    }
}
