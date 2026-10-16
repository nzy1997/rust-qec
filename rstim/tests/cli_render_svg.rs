use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_render_svg_with_stdin_args(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut cmd = rstim_cmd();
    let mut child = cmd
        .arg("render_svg")
        .args(args)
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

fn run_render_svg_with_stdin(stdin_data: &str) -> std::process::Output {
    run_render_svg_with_stdin_args(&[], stdin_data)
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

    std::fs::write(protected_output.path(), "existing output should remain").unwrap();
    let incompatible_output = rstim_cmd()
        .arg("render_svg")
        .arg("--highlight_dem_error")
        .arg("0")
        .arg("--sample_shot")
        .arg("--seed")
        .arg("7")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(protected_output.path())
        .output()
        .unwrap();
    assert!(
        !incompatible_output.status.success(),
        "highlight DEM and sample-shot modes should be mutually exclusive"
    );
    let stderr = String::from_utf8_lossy(&incompatible_output.stderr);
    assert!(
        stderr.contains("--sample_shot cannot be combined with --highlight_dem_error"),
        "stderr should explain highlight/sample compatibility: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing output should remain");
}

#[test]
fn render_svg_sample_shot_draws_seeded_annotations() {
    let circuit = "DEPOLARIZE1(1) 0\nLOSS(1) 1\nLOSS(1) 2\nM 1\nMRL 2\nDETECTOR rec[-3]\n";

    let stdout_output = run_render_svg_with_stdin_args(&["--sample_shot", "--seed", "7"], circuit);
    assert!(
        stdout_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stdout_output.stderr)
    );
    let stdout_svg = String::from_utf8(stdout_output.stdout).unwrap();
    assert!(
        stdout_svg.starts_with("<svg"),
        "sample-shot stdout should start with <svg: {stdout_svg}"
    );

    for marker in [
        "q0",
        ">D1</text>",
        ">LOSS</text>",
        ">M</text>",
        ">MRL</text>",
        ">DETECTOR</text>",
    ] {
        assert!(
            stdout_svg.contains(marker),
            "sample-shot SVG missing base circuit marker {marker}: {stdout_svg}"
        );
    }
    for marker in [
        "marker: X",
        "marker: L",
        "marker: 1[L]",
        "marker: L=1 | M=1[L]",
        "marker: D0",
    ] {
        assert!(
            stdout_svg.contains(marker),
            "sample-shot SVG missing annotation marker {marker}: {stdout_svg}"
        );
    }
    for marker in [
        "class=\"annotation annotation-preset-danger\"",
        "class=\"annotation annotation-preset-info\"",
        "data-style-preset=\"danger\"",
        "data-style-preset=\"info\"",
    ] {
        assert!(
            stdout_svg.contains(marker),
            "sample-shot SVG missing annotation style marker {marker}: {stdout_svg}"
        );
    }

    let input = tempfile::NamedTempFile::new().unwrap();
    let first_output = tempfile::NamedTempFile::new().unwrap();
    let second_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), circuit).unwrap();

    for out_path in [first_output.path(), second_output.path()] {
        let file_output = rstim_cmd()
            .arg("render_svg")
            .arg("--sample_shot")
            .arg("--seed")
            .arg("7")
            .arg("--in")
            .arg(input.path())
            .arg("--out")
            .arg(out_path)
            .output()
            .unwrap();
        assert!(
            file_output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&file_output.stderr)
        );
        assert!(
            file_output.stdout.is_empty(),
            "sample-shot file-output run should not write stdout: {}",
            String::from_utf8_lossy(&file_output.stdout)
        );
    }

    let first_svg = std::fs::read_to_string(first_output.path()).unwrap();
    let second_svg = std::fs::read_to_string(second_output.path()).unwrap();
    assert_eq!(
        first_svg, second_svg,
        "same seed and input should produce deterministic SVG annotations"
    );
    assert_eq!(
        stdout_svg, first_svg,
        "stdin/stdout and --in/--out sample-shot paths should render the same SVG"
    );

    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(protected_output.path(), "existing svg should remain").unwrap();
    let bad_output = run_render_svg_with_stdin_args(
        &[
            "--seed",
            "7",
            "--out",
            protected_output.path().to_str().unwrap(),
        ],
        "M 0\n",
    );
    assert!(
        !bad_output.status.success(),
        "--seed without --sample_shot should fail"
    );
    let stderr = String::from_utf8_lossy(&bad_output.stderr);
    assert!(
        stderr.contains("--seed is only supported with --sample_shot"),
        "stderr should explain sample-shot seed compatibility: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing svg should remain");
}

#[test]
fn render_svg_sample_export_errors_preserve_existing_output() {
    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(protected_output.path(), "existing svg should remain").unwrap();

    let output = run_render_svg_with_stdin_args(
        &[
            "--sample_shot",
            "--seed",
            "7",
            "--out",
            protected_output.path().to_str().unwrap(),
        ],
        "HERALDED_ERASE(1) 0\nDETECTOR rec[-1]\n",
    );

    assert!(
        !output.status.success(),
        "unsupported sample visualization instruction should fail"
    );
    assert!(
        output.stdout.is_empty(),
        "failing sample-shot export should not write stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "--sample_shot currently supports a subset of sample visualization instructions"
        ),
        "stderr should explain sample-shot instruction support limits: {stderr}"
    );
    assert!(
        stderr.contains("HERALDED_ERASE"),
        "stderr should name the unsupported instruction: {stderr}"
    );

    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing svg should remain");
}

#[test]
fn render_svg_documented_workflow_matches_cli() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rstim crate should live under repository root");
    let read_doc = |path: &str| -> String {
        std::fs::read_to_string(repo_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };
    let readme = read_doc("README.md");
    let cli_doc = read_doc("rstim/doc/cli.md");

    for (name, doc) in [("README.md", &readme), ("rstim/doc/cli.md", &cli_doc)] {
        assert!(doc.contains("render_svg"), "{name} should document render_svg");
        assert!(
            doc.contains("export_json"),
            "{name} should still document export_json for QP101 data export"
        );
        assert!(
            !doc.contains("rstim svg_render"),
            "{name} should not contain stale svg_render command spelling"
        );
    }

    for required in [
        "rstim render_svg --in circuit.stim --out circuit.svg",
        "--sample_shot --seed 7",
        "--highlight_dem_error 0",
        "--seed is only supported with --sample_shot",
    ] {
        assert!(
            cli_doc.contains(required),
            "CLI docs missing documented render_svg workflow marker {required}"
        );
    }

    let circuit = "H 0\nCX 0 1\nTICK\nM 0\n";
    let input = tempfile::NamedTempFile::new().unwrap();
    let output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), circuit).unwrap();

    let plain_output = rstim_cmd()
        .arg("render_svg")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(output.path())
        .output()
        .unwrap();
    assert!(
        plain_output.status.success(),
        "documented plain render command should succeed, stderr: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );
    assert!(
        plain_output.stdout.is_empty(),
        "documented file-output render should not write stdout: {}",
        String::from_utf8_lossy(&plain_output.stdout)
    );
    let svg = std::fs::read_to_string(output.path()).unwrap();
    assert!(svg.starts_with("<svg"), "documented command produced non-SVG: {svg}");
    for marker in ["q0", "H", "M"] {
        assert!(
            svg.contains(marker),
            "documented command SVG missing marker {marker}: {svg}"
        );
    }

    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(protected_output.path(), "existing svg should remain").unwrap();
    let bad_output = run_render_svg_with_stdin_args(
        &["--seed", "7", "--out", protected_output.path().to_str().unwrap()],
        "M 0\n",
    );
    assert!(
        !bad_output.status.success(),
        "documented --seed without --sample_shot failure should fail"
    );
    let stderr = String::from_utf8_lossy(&bad_output.stderr);
    assert!(
        stderr.contains("--seed is only supported with --sample_shot"),
        "stderr should match documented seed compatibility error: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing svg should remain");
}
