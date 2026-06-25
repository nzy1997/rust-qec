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

#[test]
fn render_svg_highlight_dem_error_draws_query_markers() {
    let circuit = "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n";

    let plain_output = run_render_svg_with_stdin(circuit);
    assert!(
        plain_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );
    let plain_svg = String::from_utf8(plain_output.stdout).unwrap();
    assert!(
        plain_svg.starts_with("<svg"),
        "plain SVG should start with <svg: {plain_svg}"
    );
    assert!(
        !plain_svg.contains("marker: X"),
        "plain SVG should not contain source highlight marker text: {plain_svg}"
    );
    assert!(
        !plain_svg.contains("marker: D0"),
        "plain SVG should not contain symptom highlight marker text: {plain_svg}"
    );

    let input = tempfile::NamedTempFile::new().unwrap();
    let output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), circuit).unwrap();
    let highlighted_output = rstim_cmd()
        .arg("render_svg")
        .arg("--highlight_dem_error")
        .arg("0")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(output.path())
        .output()
        .unwrap();
    assert!(
        highlighted_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&highlighted_output.stderr)
    );
    assert!(
        highlighted_output.stdout.is_empty(),
        "file-output run should not write stdout: {}",
        String::from_utf8_lossy(&highlighted_output.stdout)
    );

    let highlighted_svg = std::fs::read_to_string(output.path()).unwrap();
    assert!(
        highlighted_svg.starts_with("<svg"),
        "highlighted SVG should start with <svg: {highlighted_svg}"
    );
    for marker in ["q0", "XE", "M", "DETECTOR", "marker: X", "marker: D0"] {
        assert!(
            highlighted_svg.contains(marker),
            "highlighted SVG missing marker {marker}: {highlighted_svg}"
        );
    }
    for marker in [
        "class=\"annotation annotation-preset-danger\"",
        "data-style-preset=\"danger\"",
        "data-style-highlight=\"true\"",
        "data-annotation-tags=\"dem-origin query-result\"",
        "data-annotation-tags=\"dem-symptom query-result\"",
    ] {
        assert!(
            highlighted_svg.contains(marker),
            "highlighted SVG missing style marker {marker}: {highlighted_svg}"
        );
        assert!(
            !plain_svg.contains(marker),
            "plain SVG should not contain highlight style marker {marker}: {plain_svg}"
        );
    }
    assert!(
        !plain_svg.contains("marker: X") && highlighted_svg.contains("marker: X"),
        "source highlight text should only appear in highlighted SVG"
    );
    assert!(
        !plain_svg.contains("marker: D0") && highlighted_svg.contains("marker: D0"),
        "symptom highlight text should only appear in highlighted SVG"
    );

    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(protected_output.path(), "existing output should remain").unwrap();
    let invalid_output = rstim_cmd()
        .arg("render_svg")
        .arg("--highlight_dem_error")
        .arg("99")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(protected_output.path())
        .output()
        .unwrap();
    assert!(
        !invalid_output.status.success(),
        "out-of-range DEM query should fail"
    );
    let stderr = String::from_utf8_lossy(&invalid_output.stderr);
    assert!(
        stderr.contains("DEM error index out of range"),
        "stderr should report out-of-range DEM index: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing output should remain");
}
