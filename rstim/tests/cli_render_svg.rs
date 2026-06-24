use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_render_svg_with_stdin(stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .arg("render_svg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_data.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn render_svg_writes_svg_from_stdin_and_file() {
    let circuit = "H 0\nCX 0 1\nTICK\nM 0\n";

    let stdout_output = run_render_svg_with_stdin(circuit);
    assert!(
        stdout_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stdout_output.stderr)
    );
    assert!(
        stdout_output.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&stdout_output.stderr)
    );
    let stdout_svg = String::from_utf8(stdout_output.stdout).unwrap();
    assert!(
        stdout_svg.starts_with("<svg"),
        "stdout should start with <svg: {stdout_svg}"
    );
    for marker in ["q0", "H", "M"] {
        assert!(
            stdout_svg.contains(marker),
            "stdout SVG missing marker {marker}: {stdout_svg}"
        );
    }

    let input = tempfile::NamedTempFile::new().unwrap();
    let output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), circuit).unwrap();
    let file_output = rstim_cmd()
        .arg("render_svg")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(output.path())
        .output()
        .unwrap();
    assert!(
        file_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&file_output.stderr)
    );
    assert!(
        file_output.stdout.is_empty(),
        "file-output run should not write stdout: {}",
        String::from_utf8_lossy(&file_output.stdout)
    );
    let file_svg = std::fs::read_to_string(output.path()).unwrap();
    assert!(
        file_svg.starts_with("<svg"),
        "file SVG should start with <svg: {file_svg}"
    );
    for marker in ["q0", "H", "M"] {
        assert!(
            file_svg.contains(marker),
            "file SVG missing marker {marker}: {file_svg}"
        );
    }

    let bad_input = tempfile::NamedTempFile::new().unwrap();
    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(bad_input.path(), "REPEAT nope {\n  M 0\n}\n").unwrap();
    std::fs::write(protected_output.path(), "existing output should remain").unwrap();
    let bad_output = rstim_cmd()
        .arg("render_svg")
        .arg("--in")
        .arg(bad_input.path())
        .arg("--out")
        .arg(protected_output.path())
        .output()
        .unwrap();
    assert!(
        !bad_output.status.success(),
        "invalid Stim syntax should fail"
    );
    let stderr = String::from_utf8_lossy(&bad_output.stderr);
    assert!(
        stderr.contains("bad repeat count") || stderr.contains("line 1"),
        "stderr should name the parse error: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing output should remain");
}
