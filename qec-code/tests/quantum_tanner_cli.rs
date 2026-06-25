use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::tempdir;

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn run_quantum_tanner(matrix: &str, fixture: &str) -> Output {
    let spec = workspace_root().join(fixture);
    Command::new(qec_code_bin())
        .args(["code", "css", "quantum-tanner", "--spec"])
        .arg(&spec)
        .arg(matrix)
        .output()
        .expect("qec-code binary should run")
}

const QUANTUM_TANNER_CLI_DOC: &str = include_str!("../doc/quantum_tanner_cli.md");
const DOC_COMMAND_PREFIX: &str = "cargo run -q -p qec-code -- ";

#[derive(Debug)]
struct DocumentedCommand {
    args: Vec<String>,
    redirect: Option<String>,
}

fn documented_command_block(marker: &str) -> &str {
    let marker_text = format!("<!-- {marker} -->");
    let after_marker = QUANTUM_TANNER_CLI_DOC
        .split_once(&marker_text)
        .map(|(_, after)| after)
        .unwrap_or_else(|| panic!("missing doc marker {marker_text}"));
    let fence_start = after_marker
        .find("```bash")
        .unwrap_or_else(|| panic!("missing bash fence after {marker_text}"));
    let command_start = fence_start + "```bash".len();
    let command_tail = &after_marker[command_start..];
    let fence_end = command_tail
        .find("```")
        .unwrap_or_else(|| panic!("missing closing bash fence after {marker_text}"));
    &command_tail[..fence_end]
}

fn documented_qec_code_commands(marker: &str) -> Vec<DocumentedCommand> {
    documented_command_block(marker)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(DOC_COMMAND_PREFIX))
        .map(parse_documented_command)
        .collect()
}

fn parse_documented_command(line: &str) -> DocumentedCommand {
    let command = line.strip_prefix(DOC_COMMAND_PREFIX).unwrap_or_else(|| {
        panic!("documented command must start with {DOC_COMMAND_PREFIX}: {line}")
    });
    let (args_text, redirect) = match command.split_once(" > ") {
        Some((before_redirect, redirect)) => (before_redirect, Some(redirect.to_owned())),
        None => (command, None),
    };

    DocumentedCommand {
        args: args_text.split_whitespace().map(str::to_owned).collect(),
        redirect,
    }
}

fn workspace_path_arg(arg: &str, output_map: &HashMap<String, PathBuf>) -> String {
    output_map
        .get(arg)
        .cloned()
        .unwrap_or_else(|| workspace_root().join(arg))
        .display()
        .to_string()
}

fn materialize_doc_args(args: &[String], output_map: &HashMap<String, PathBuf>) -> Vec<String> {
    args.iter()
        .map(|arg| {
            if arg.starts_with("qec-code/tests/fixtures/") || output_map.contains_key(arg) {
                workspace_path_arg(arg, output_map)
            } else {
                arg.to_owned()
            }
        })
        .collect()
}

fn run_documented_command(
    command: &DocumentedCommand,
    output_map: &HashMap<String, PathBuf>,
) -> Output {
    let args = materialize_doc_args(&command.args, output_map);
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("documented qec-code command should run")
}

fn assert_quantum_tanner_sparse_rows_output(stdout: &[u8]) -> serde_json::Value {
    let stdout = String::from_utf8(stdout.to_vec()).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 16);
    assert_eq!(rows.len(), 8);
    assert!(
        rows.iter().all(|row| {
            row.as_array()
                .is_some_and(|cols| cols.is_empty() || cols.len() == 4)
        }),
        "all non-empty quantum Tanner rows should have weight 4: {rows:?}"
    );

    json
}

fn json_output(stdout: &[u8]) -> serde_json::Value {
    let stdout = String::from_utf8(stdout.to_vec()).expect("stdout should be valid utf-8");
    serde_json::from_str(&stdout).expect("stdout should be JSON")
}

fn is_sparse_rows_json(stdout: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout)
        .is_ok_and(|json| json["format"] == "sparse_rows" && json["rows"].is_array())
}

fn is_distance_json(stdout: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout)
        .is_ok_and(|json| json["status"] == "completed" && json["distance"].is_number())
}

#[test]
fn code_css_quantum_tanner_hx_prints_sparse_rows_json() {
    let output = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_quantum_tanner_sparse_rows_output(&output.stdout);
}

#[test]
fn code_css_quantum_tanner_hz_prints_sparse_rows_json() {
    let output = run_quantum_tanner("hz", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_quantum_tanner_sparse_rows_output(&output.stdout);
}

#[test]
fn code_css_quantum_tanner_hx_and_hz_are_exported_separately() {
    let hx = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");
    let hz = run_quantum_tanner("hz", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(hx.status.success());
    assert!(hz.status.success());

    let hx_json = assert_quantum_tanner_sparse_rows_output(&hx.stdout);
    let hz_json = assert_quantum_tanner_sparse_rows_output(&hz.stdout);
    assert_ne!(hx_json, hz_json);
}

#[test]
fn code_css_quantum_tanner_invalid_spec_fails_without_stdout() {
    let output = run_quantum_tanner(
        "hx",
        "qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json",
    );

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("invalid quantum Tanner generator set A"),
        "stderr was: {stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be valid sparse-row JSON: {stderr}"
    );
}

#[test]
fn quantum_tanner_cli_doc_commands_stay_current() {
    assert!(QUANTUM_TANNER_CLI_DOC.contains("Rust consumes explicit finite-group specs"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("does not search for groups"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("does not call GAP or Oscar"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/codes/quantum.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/objects.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/codes/quantum_test.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/qLDPCOrg/qLDPC"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/QuantumSavory/QuantumExpanders.jl"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/RebKatRad/qTanner"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("Apache-2.0"));
    assert!(
        QUANTUM_TANNER_CLI_DOC.contains("reference-only unless a compatible license is confirmed")
    );

    let inspect_command =
        documented_command_block("quantum_tanner_cli:inspect_toric_d4_fixture").trim();
    assert_eq!(
        inspect_command,
        "sed -n '1,80p' qec-code/tests/fixtures/quantum_tanner/toric_d4.json"
    );
    let inspect_output = Command::new("sed")
        .args(["-n", "1,80p"])
        .arg(workspace_root().join("qec-code/tests/fixtures/quantum_tanner/toric_d4.json"))
        .output()
        .expect("documented fixture inspection command should run");
    assert!(inspect_output.status.success());
    assert_eq!(inspect_output.stderr, b"");
    let inspected =
        String::from_utf8(inspect_output.stdout).expect("fixture inspection should be utf-8");
    assert!(inspected.contains(r#""fixture_id": "toric_d4""#));

    let tempdir = tempdir().expect("temporary output directory should be created");
    let hx_path = tempdir.path().join("toric_d4_hx.json");
    let hz_path = tempdir.path().join("toric_d4_hz.json");
    let output_map = HashMap::from([
        (
            "target/qec-code-workflow/toric_d4_hx.json".to_owned(),
            hx_path.clone(),
        ),
        (
            "target/qec-code-workflow/toric_d4_hz.json".to_owned(),
            hz_path.clone(),
        ),
    ]);

    let commands = documented_qec_code_commands("quantum_tanner_cli:toric_d4_commands");
    assert!(
        !commands.is_empty(),
        "workflow doc should contain qec-code commands"
    );

    for command in &commands {
        let output = run_documented_command(command, &output_map);
        assert!(
            output.status.success(),
            "documented command failed: {command:?}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stderr, b"");

        if let Some(redirect) = &command.redirect {
            let path = output_map
                .get(redirect)
                .unwrap_or_else(|| panic!("unexpected redirect path {redirect}"));
            fs::write(path, &output.stdout).expect("redirected output should be writable");
            assert_quantum_tanner_sparse_rows_output(&output.stdout);
        }

        if command.args.iter().any(|arg| arg == "--json") {
            let json = json_output(&output.stdout);
            assert_eq!(json["status"], "completed");
            assert_eq!(json["distance"], 4);
        }
    }

    assert!(hx_path.exists(), "documented hx export should write a file");
    assert!(hz_path.exists(), "documented hz export should write a file");

    let invalid_commands = documented_qec_code_commands("quantum_tanner_cli:invalid_spec_command");
    assert_eq!(invalid_commands.len(), 1);
    let output = run_documented_command(&invalid_commands[0], &HashMap::new());
    assert!(!output.status.success());
    assert!(!is_sparse_rows_json(&output.stdout));
    assert!(!is_distance_json(&output.stdout));
}

#[test]
fn code_css_quantum_tanner_missing_spec_fails_without_stdout() {
    let output = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/missing.json");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("failed to read CSS matrix"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("missing.json"), "stderr was: {stderr}");
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be valid sparse-row JSON: {stderr}"
    );
}
