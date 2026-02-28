use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn sample_01_format() {
    let output = run_with_stdin(&["sample", "--shots", "3", "--out_format", "01"], "R 0\nM 0");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3);
    for line in &lines {
        assert_eq!(line.len(), 1);
        assert!(line == &"0" || line == &"1");
    }
}

#[test]
fn sample_default_format_is_01() {
    let output = run_with_stdin(&["sample", "--shots", "1"], "R 0\nM 0");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim() == "0" || stdout.trim() == "1");
}

#[test]
fn sample_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    std::fs::write(&circuit_path, "R 0\nX 0\nM 0").unwrap();
    let output = rstim_cmd()
        .args(["sample", "--shots", "1", "--in", circuit_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
}

#[test]
fn sample_hits_format() {
    let output = run_with_stdin(&["sample", "--shots", "1", "--out_format", "hits"], "R 0\nX 0\nM 0");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "0");
}

#[test]
fn sample_b8_format() {
    let output = run_with_stdin(&["sample", "--shots", "1", "--out_format", "b8"], "R 0\nX 0\nM 0");
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0x01]);
}

#[test]
fn sample_seed_deterministic() {
    let out1 = run_with_stdin(&["sample", "--shots", "10", "--seed", "42"], "H 0\nM 0");
    let out2 = run_with_stdin(&["sample", "--shots", "10", "--seed", "42"], "H 0\nM 0");
    assert_eq!(out1.stdout, out2.stdout);
}
