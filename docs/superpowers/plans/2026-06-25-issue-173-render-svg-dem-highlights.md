# Issue 173 `rstim render_svg` DEM Highlights Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rstim render_svg --highlight_dem_error <index>` so DEM-origin QP101 annotations render visibly in built-in SVG output.

**Architecture:** Keep provenance in the existing tracked-DEM QP101 export path and share CLI document construction between `export_json` and `render_svg`. The SVG renderer remains a `Qp101Document -> SVG` renderer, with deterministic annotation style attributes added to its existing annotation text output.

**Tech Stack:** Rust 2024, Clap derive, existing `ErrorAnalyzer::circuit_to_tracked_dem`, existing `rstim::qp101::export_qp101_with_highlighted_dem_error`, existing `rstim::qp101_svg::render_svg`, Cargo integration tests.

## Global Constraints

- Add `--highlight_dem_error <index>` to `rstim render_svg`.
- Reuse `ErrorAnalyzer::circuit_to_tracked_dem` and `export_qp101_with_highlighted_dem_error`; do not add renderer-side DEM provenance logic.
- Keep option compatibility with `export_json`: `--seed` is only valid with `--sample_shot`, and `--sample_shot` cannot be combined with `--highlight_dem_error`.
- Issue #173 does not add `render_svg --sample_shot`; call shared construction with `sample_shot = false` and `seed = None` for the SVG highlight path.
- Keep safe file-output behavior: do not open or truncate `--out` until parse, tracked-DEM construction, highlighted QP101 export, and SVG rendering have all succeeded.
- Render annotation labels, text, and style presets deterministically in SVG text/attributes.
- Do not change QP101 JSON schema, Typst fixtures, or DEM provenance semantics.
- Use the acceptance fixture `X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n`.
- The out-of-range negative control must preserve an existing output file and report `DEM error index out of range`.

---

### Task 1: CLI Highlight Document Path

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_render_svg.rs`

**Interfaces:**
- Consumes: `ErrorAnalyzer::circuit_to_tracked_dem(instrs: &[StimInstr]) -> Result<TrackedDemResult, String>`.
- Consumes: `crate::qp101::export_qp101_with_highlighted_dem_error(instrs, tracked, index) -> Result<Qp101Document, String>`.
- Produces: private helper `build_qp101_document_for_visualization(instrs, highlight_dem_error, sample_shot, seed) -> Result<Qp101Document, String>`.
- Produces: private helper `run_render_svg_to_string(text, highlight_dem_error) -> Result<String, String>`.
- Produces: CLI interface `rstim render_svg --highlight_dem_error <index>`.

- [ ] **Step 1: Write the failing CLI test**

Append this test to `rstim/tests/cli_render_svg.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline
```

Expected: FAIL because `render_svg` does not yet accept `--highlight_dem_error`.

- [ ] **Step 3: Add the `render_svg` CLI option**

Change the `Commands::RenderSvg` variant in `rstim/src/cli.rs` from:

```rust
    RenderSvg {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
```

to:

```rust
    RenderSvg {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long = "highlight_dem_error")]
        highlight_dem_error: Option<usize>,
    },
```

- [ ] **Step 4: Forward the option through safe-output dispatch**

Change the `RenderSvg` match arm in `rstim/src/cli.rs` from:

```rust
        Some(Commands::RenderSvg { r#in, out }) => {
            let text = read_input(r#in.as_deref())?;
            let svg = run_render_svg_to_string(&text)?;
            let mut w = open_output(out.as_deref())?;
            w.write_all(svg.as_bytes())
                .map_err(|e| format!("write error: {e}"))
        }
```

to:

```rust
        Some(Commands::RenderSvg {
            r#in,
            out,
            highlight_dem_error,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let svg = run_render_svg_to_string(&text, highlight_dem_error)?;
            let mut w = open_output(out.as_deref())?;
            w.write_all(svg.as_bytes())
                .map_err(|e| format!("write error: {e}"))
        }
```

The call to `open_output` must stay after `run_render_svg_to_string`.

- [ ] **Step 5: Extract shared visualization document construction**

Replace the current `run_export_json`, `build_plain_qp101_document`, and
`run_render_svg_to_string` helper block in `rstim/src/cli.rs` with this shape,
preserving the existing JSON serialization at the end of `run_export_json`:

```rust
fn run_export_json(
    text: &str,
    format: JsonOutputFormat,
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
    w: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let doc = build_qp101_document_for_visualization(
        &instrs,
        highlight_dem_error,
        sample_shot,
        seed,
    )?;
    match format {
        JsonOutputFormat::Pretty => {
            serde_json::to_writer_pretty(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
        JsonOutputFormat::Compact => {
            serde_json::to_writer(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
    }
    w.write_all(b"\n")
        .map_err(|e| format!("write error: {e}"))?;
    Ok(())
}

fn build_qp101_document_for_visualization(
    instrs: &[crate::ir::StimInstr],
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
) -> Result<crate::qp101::Qp101Document, String> {
    if seed.is_some() && !sample_shot {
        return Err("--seed is only supported with --sample_shot".to_string());
    }
    if sample_shot && highlight_dem_error.is_some() {
        return Err("--sample_shot cannot be combined with --highlight_dem_error".to_string());
    }
    match highlight_dem_error {
        Some(index) => build_highlighted_dem_qp101_document(instrs, index),
        None if sample_shot => build_sample_qp101_document(instrs, seed),
        None => build_plain_qp101_document(instrs),
    }
}

fn build_highlighted_dem_qp101_document(
    instrs: &[crate::ir::StimInstr],
    index: usize,
) -> Result<crate::qp101::Qp101Document, String> {
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(instrs).map_err(|err| {
        if err.starts_with("tracked DEM does not yet support instruction ") {
            format!(
                "--highlight_dem_error currently supports a subset of noise instructions: {err}"
            )
        } else {
            err
        }
    })?;
    crate::qp101::export_qp101_with_highlighted_dem_error(instrs, &tracked, index).map_err(
        |err| {
            if err.starts_with("DEM error index ") && err.contains(" out of range ") {
                format!("DEM error index out of range: {err}")
            } else {
                err
            }
        },
    )
}

fn build_sample_qp101_document(
    instrs: &[crate::ir::StimInstr],
    seed: Option<u64>,
) -> Result<crate::qp101::Qp101Document, String> {
    let mut ex = Executor::from_instrs(instrs.to_vec())?;
    let mut rng = make_rng(seed);
    let (_out, trace) = ex.run_with_trace(&mut rng)?;
    crate::qp101::export_qp101_with_sample_trace(instrs, &trace).map_err(|err| {
        if err.starts_with("sample trace visualization does not yet support instruction ") {
            format!(
                "--sample_shot currently supports a subset of sample visualization instructions: {err}"
            )
        } else {
            err
        }
    })
}

fn build_plain_qp101_document(
    instrs: &[crate::ir::StimInstr],
) -> Result<crate::qp101::Qp101Document, String> {
    crate::qp101::export_qp101(instrs)
}

fn run_render_svg_to_string(
    text: &str,
    highlight_dem_error: Option<usize>,
) -> Result<String, String> {
    let instrs = parse_lines(text)?;
    let doc =
        build_qp101_document_for_visualization(&instrs, highlight_dem_error, false, None)?;
    crate::qp101_svg::render_svg(&doc)
}
```

Keep `build_qp101_document_for_visualization` private to `cli.rs`.

- [ ] **Step 6: Run focused CLI tests to verify GREEN**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q --offline
```

Expected: both PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```sh
git add rstim/src/cli.rs rstim/tests/cli_render_svg.rs
git commit -m "feat: render dem highlights in svg CLI"
```

---

### Task 2: Deterministic SVG Annotation Style Attributes

**Files:**
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/cli_render_svg.rs`

**Interfaces:**
- Consumes: `Qp101Annotation.style`.
- Consumes: `Qp101Annotation.tags`.
- Produces: SVG annotation `<text>` elements with stable `class`, `data-style-preset`, `data-style-highlight`, `data-annotation-tags`, and style-aware `fill` attributes.

- [ ] **Step 1: Extend the CLI test to require style metadata**

In `render_svg_highlight_dem_error_draws_query_markers`, after the base marker
loop, add:

```rust
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
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline
```

Expected: FAIL because highlighted annotation text renders but style metadata
attributes are absent.

- [ ] **Step 3: Add annotation attribute helpers**

In `rstim/src/qp101_svg.rs`, replace `render_annotations_with_line_offset`
with:

```rust
fn render_annotations_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    let base_lane = lanes.first().copied().unwrap_or(0);
    let base_y = below_gate_text_y(base_lane) + line_offset as i32 * ANNOTATION_LINE_GAP;
    for (idx, annotation) in annotations.iter().enumerate() {
        let mut parts = Vec::new();
        parts.push(annotation.kind.clone());
        if let Some(label) = annotation.label.as_deref() {
            parts.push(label.to_string());
        }
        if let Some(text) = annotation.text.as_deref() {
            parts.push(text.to_string());
        }
        let content = escape_xml(&parts.join(": "));
        let attrs = annotation_svg_attrs(annotation);
        out.push_str(&format!(
            "<text {attrs} x=\"{x}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\">{content}</text>\n",
            base_y + idx as i32 * ANNOTATION_LINE_GAP
        ));
    }
}
```

Add these helpers immediately below it:

```rust
fn annotation_svg_attrs(annotation: &Qp101Annotation) -> String {
    let mut classes = vec!["annotation".to_string()];
    let mut attrs = Vec::new();
    if let Some(style) = annotation.style.as_ref() {
        if let Some(preset) = style.preset.as_deref() {
            classes.push(format!("annotation-preset-{}", css_token(preset)));
            attrs.push(format!("data-style-preset=\"{}\"", escape_xml(preset)));
        }
        if let Some(highlight) = style.highlight {
            attrs.push(format!("data-style-highlight=\"{highlight}\""));
        }
    }
    if !annotation.tags.is_empty() {
        attrs.push(format!(
            "data-annotation-tags=\"{}\"",
            escape_xml(&annotation.tags.join(" "))
        ));
    }
    attrs.insert(0, format!("class=\"{}\"", classes.join(" ")));
    attrs.push(format!(
        "fill=\"{}\"",
        escape_xml(&annotation_fill(annotation))
    ));
    attrs.join(" ")
}

fn annotation_fill(annotation: &Qp101Annotation) -> String {
    annotation
        .style
        .as_ref()
        .and_then(|style| style.color.as_deref())
        .map(annotation_color)
        .or_else(|| {
            annotation
                .style
                .as_ref()
                .and_then(|style| style.preset.as_deref())
                .map(annotation_color)
        })
        .unwrap_or("#7a5af8")
        .to_string()
}

fn annotation_color(value: &str) -> &str {
    match value {
        "danger" | "red" => "#dc2626",
        "info" | "blue" => "#2563eb",
        "warning" | "yellow" => "#ca8a04",
        "success" | "green" => "#16a34a",
        other => other,
    }
}

fn css_token(value: &str) -> String {
    let mut token = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            token.push(ch);
        } else {
            token.push('-');
        }
    }
    if token.is_empty() {
        "custom".to_string()
    } else {
        token
    }
}
```

- [ ] **Step 4: Run focused renderer and CLI tests**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline
cargo test --manifest-path rstim/Cargo.toml --test qp101_svg svg_renderer_renders_qp101_fallback_operations_and_annotations -q --offline
```

Expected: both PASS.

- [ ] **Step 5: Run broader focused checks**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg -q --offline
cargo test --manifest-path rstim/Cargo.toml --test qp101_svg -q --offline
```

Expected: both PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```sh
git add rstim/src/qp101_svg.rs rstim/tests/cli_render_svg.rs
git commit -m "feat: expose svg annotation styles"
```

---

### Task 3: Final Verification And PR Prep

**Files:**
- Verify only unless formatting changes are required.

**Interfaces:**
- Produces: clean working tree except committed branch changes.
- Produces: verification evidence for the pull request.

- [ ] **Step 1: Run rustfmt check**

Run:

```sh
cargo fmt --check --manifest-path rstim/Cargo.toml
```

Expected: PASS. If it fails, run `cargo fmt --manifest-path rstim/Cargo.toml`,
inspect the diff, and commit formatting with the relevant source changes.

- [ ] **Step 2: Run the issue verification command**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q
```

Expected: PASS when online registry access is available. If the sandbox blocks
crates.io access, record the exact failure and rerun:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline
```

Expected: PASS.

- [ ] **Step 3: Run focused offline regression checks**

Run:

```sh
cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg -q --offline
cargo test --manifest-path rstim/Cargo.toml --test qp101_svg -q --offline
```

Expected: both PASS.

- [ ] **Step 4: Run broad requested verification**

Run:

```sh
cargo test
```

Expected: PASS when online registry access is available. If the sandbox blocks
crates.io access before tests run, record the exact dependency/network failure
and run the broadest feasible offline crate-level check:

```sh
cargo test --manifest-path rstim/Cargo.toml --offline
```

Expected: PASS.

- [ ] **Step 5: Run diff hygiene check**

Run:

```sh
git diff --check
```

Expected: PASS.

- [ ] **Step 6: Prepare PR summary**

Use this PR body:

```markdown
Closes #173

## Summary
- Add `rstim render_svg --highlight_dem_error <index>` and route it through the existing tracked-DEM QP101 highlight export path.
- Preserve safe SVG file output for invalid DEM highlight queries.
- Add deterministic SVG annotation style attributes for highlighted QP101 markers.
- Cover highlighted source/symptom markers and out-of-range safe-output behavior in CLI tests.

## Verification
- `cargo test -p rstim --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q`
- `cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q --offline`
- `cargo test --manifest-path rstim/Cargo.toml --test cli_render_svg -q --offline`
- `cargo test --manifest-path rstim/Cargo.toml --test qp101_svg -q --offline`
- `cargo test --manifest-path rstim/Cargo.toml --offline`
- `cargo fmt --check --manifest-path rstim/Cargo.toml`
- `git diff --check`

## Notes
- The exact online `cargo test` commands may fail in this sandbox before tests run because Cargo attempts to reach crates.io for the workspace dependency index. Offline `rstim` crate checks are the fallback evidence.
```
