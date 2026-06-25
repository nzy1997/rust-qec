# Issue 172 Render SVG Sample-Shot Annotations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `rstim render_svg --sample_shot --seed <n>` through the existing QP101 sample-trace export path and render the resulting QP101 annotations in SVG.

**Architecture:** Keep sample semantics in the existing simulator/exporter path by sharing a private QP101 document builder between `export_json` and `render_svg`. The SVG renderer remains a QP101 consumer: it renders annotation label/text plus deterministic style metadata without inspecting sample contexts.

**Tech Stack:** Rust 2024, Clap derive, existing `Executor::run_with_trace`, existing `rstim::qp101::export_qp101_with_sample_trace`, existing hand-written QP101 SVG renderer, Cargo integration tests.

## Global Constraints

- `rstim render_svg --sample_shot --seed 7 --in <circuit.stim> --out <sample.svg>` must render a base SVG circuit plus visible sample-shot annotations.
- The command must also support stdin input with `--sample_shot --seed 7`.
- Reuse `export_qp101_with_sample_trace`; do not add renderer-side sampling logic.
- `--seed` is only valid with `--sample_shot`.
- `--sample_shot` cannot be combined with `--highlight_dem_error`; `render_svg` does not add a `--highlight_dem_error` flag in this issue.
- Invalid sample options and sample-export errors must not create, truncate, or replace the requested SVG output before failure.
- Render annotation `label`, `text`, and style presets deterministically and text-inspectably.
- Keep `qp101_svg::render_svg(&Qp101Document) -> Result<String, String>` as the public renderer interface.
- Do not change the QP101 JSON schema.
- Do not update Typst fixtures or docs outside the Superpowers workflow docs for this issue.
- Follow Rust 2024 style and keep touched Rust files rustfmt-clean.

---

### Task 1: CLI Sample-Shot SVG Rendering And Annotation Metadata

**Files:**
- Modify: `rstim/tests/cli_render_svg.rs`
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/src/qp101_svg.rs`

**Interfaces:**
- Consumes: `Executor::run_with_trace(&mut StdRng) -> Result<(_, SampleTrace), String>`.
- Consumes: `crate::qp101::export_qp101_with_sample_trace(instrs, trace) -> Result<Qp101Document, String>`.
- Produces: private CLI struct `Qp101BuildOptions { highlight_dem_error: Option<usize>, sample_shot: bool, seed: Option<u64> }`.
- Produces: private CLI helper `build_qp101_document(instrs: &[crate::ir::StimInstr], options: Qp101BuildOptions) -> Result<crate::qp101::Qp101Document, String>`.
- Produces: private CLI helper `run_render_svg_to_string(text: &str, options: Qp101BuildOptions) -> Result<String, String>`.
- Produces: `rstim render_svg --sample_shot --seed <n>` CLI behavior.
- Produces: SVG annotation elements with `class="annotation ..."` and `data-style-preset="..."` when `Qp101Annotation.style.preset` exists.

- [ ] **Step 1: Write the failing CLI integration test**

Modify the top of `rstim/tests/cli_render_svg.rs` so the stdin helper accepts optional render flags:

```rust
fn run_render_svg_with_stdin_args(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut cmd = rstim_cmd();
    cmd.arg("render_svg")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
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
```

Append this test after `render_svg_writes_svg_from_stdin_and_file`:

```rust
#[test]
fn render_svg_sample_shot_draws_seeded_annotations() {
    let circuit =
        "DEPOLARIZE1(1) 0\nLOSS(1) 1\nLOSS(1) 2\nM 1\nMRL 2\nDETECTOR rec[-3]\n";

    let stdout_output =
        run_render_svg_with_stdin_args(&["--sample_shot", "--seed", "7"], circuit);
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
    let bad_output =
        run_render_svg_with_stdin_args(&["--seed", "7", "--out", protected_output.path().to_str().unwrap()], "M 0\n");
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
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_sample_shot_draws_seeded_annotations -q
```

Expected: FAIL because `rstim render_svg` does not yet accept `--sample_shot` or `--seed`, and the SVG renderer does not yet expose annotation style metadata.

- [ ] **Step 3: Add render_svg sample flags and shared QP101 options**

In `rstim/src/cli.rs`, add these fields to the `Commands::RenderSvg` variant:

```rust
        #[arg(long = "sample_shot")]
        sample_shot: bool,
        #[arg(long)]
        seed: Option<u64>,
```

Add this struct near `JsonOutputFormat`:

```rust
#[derive(Clone, Copy)]
struct Qp101BuildOptions {
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
}
```

Add this helper near `parse_json_output_format`:

```rust
fn plain_qp101_build_options() -> Qp101BuildOptions {
    Qp101BuildOptions {
        highlight_dem_error: None,
        sample_shot: false,
        seed: None,
    }
}
```

- [ ] **Step 4: Route export_json and render_svg through one QP101 builder**

Replace the current `run_export_json` document-building logic in `rstim/src/cli.rs` with this structure:

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
    let doc = build_qp101_document(
        &instrs,
        Qp101BuildOptions {
            highlight_dem_error,
            sample_shot,
            seed,
        },
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
```

Replace `build_plain_qp101_document` and `run_render_svg_to_string` with:

```rust
fn build_qp101_document(
    instrs: &[crate::ir::StimInstr],
    options: Qp101BuildOptions,
) -> Result<crate::qp101::Qp101Document, String> {
    if options.seed.is_some() && !options.sample_shot {
        return Err("--seed is only supported with --sample_shot".to_string());
    }
    if options.sample_shot && options.highlight_dem_error.is_some() {
        return Err("--sample_shot cannot be combined with --highlight_dem_error".to_string());
    }

    match options.highlight_dem_error {
        Some(index) => {
            let tracked = ErrorAnalyzer::circuit_to_tracked_dem(instrs).map_err(|err| {
                if err.starts_with("tracked DEM does not yet support instruction ") {
                    format!(
                        "--highlight_dem_error currently supports a subset of noise instructions: {err}"
                    )
                } else {
                    err
                }
            })?;
            crate::qp101::export_qp101_with_highlighted_dem_error(instrs, &tracked, index)
                .map_err(|err| {
                    if err.starts_with("DEM error index ") && err.contains(" out of range ") {
                        format!("DEM error index out of range: {err}")
                    } else {
                        err
                    }
                })
        }
        None if options.sample_shot => {
            let mut ex = Executor::from_instrs(instrs.to_vec())?;
            let mut rng = make_rng(options.seed);
            let (_out, trace) = ex.run_with_trace(&mut rng)?;
            crate::qp101::export_qp101_with_sample_trace(instrs, &trace).map_err(|err| {
                if err.starts_with("sample trace visualization does not yet support instruction ")
                {
                    format!(
                        "--sample_shot currently supports a subset of sample visualization instructions: {err}"
                    )
                } else {
                    err
                }
            })
        }
        None => build_plain_qp101_document(instrs),
    }
}

fn build_plain_qp101_document(
    instrs: &[crate::ir::StimInstr],
) -> Result<crate::qp101::Qp101Document, String> {
    crate::qp101::export_qp101(instrs)
}

fn run_render_svg_to_string(text: &str, options: Qp101BuildOptions) -> Result<String, String> {
    let instrs = parse_lines(text)?;
    let doc = build_qp101_document(&instrs, options)?;
    crate::qp101_svg::render_svg(&doc)
}
```

- [ ] **Step 5: Preserve safe render_svg output ordering with the new options**

In the `run` dispatcher, update the `RenderSvg` match arm from:

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
            sample_shot,
            seed,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let svg = run_render_svg_to_string(
                &text,
                Qp101BuildOptions {
                    sample_shot,
                    seed,
                    ..plain_qp101_build_options()
                },
            )?;
            let mut w = open_output(out.as_deref())?;
            w.write_all(svg.as_bytes())
                .map_err(|e| format!("write error: {e}"))
        }
```

The call to `open_output` must remain after `run_render_svg_to_string`.

- [ ] **Step 6: Add deterministic annotation style metadata to SVG text**

In `rstim/src/qp101_svg.rs`, replace `render_annotations_with_line_offset` with:

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
        let class = annotation_class(annotation);
        let fill = annotation_fill(annotation);
        let attrs = annotation_style_attrs(annotation);
        out.push_str(&format!(
            "<text class=\"{class}\" x=\"{x}\" y=\"{}\" fill=\"{}\" text-anchor=\"middle\" font-size=\"11\"{attrs}>{content}</text>\n",
            base_y + idx as i32 * ANNOTATION_LINE_GAP,
            escape_xml(fill),
        ));
    }
}
```

Add these helpers immediately after `render_annotations_with_line_offset`:

```rust
fn annotation_class(annotation: &Qp101Annotation) -> String {
    let mut class = "annotation".to_string();
    if let Some(preset) = annotation
        .style
        .as_ref()
        .and_then(|style| style.preset.as_deref())
    {
        class.push_str(" annotation-preset-");
        class.push_str(&escape_xml(preset));
    }
    class
}

fn annotation_fill(annotation: &Qp101Annotation) -> &str {
    annotation
        .style
        .as_ref()
        .and_then(|style| style.color.as_deref())
        .unwrap_or("#7a5af8")
}

fn annotation_style_attrs(annotation: &Qp101Annotation) -> String {
    let Some(style) = annotation.style.as_ref() else {
        return String::new();
    };
    let mut attrs = String::new();
    if let Some(preset) = style.preset.as_deref() {
        attrs.push_str(" data-style-preset=\"");
        attrs.push_str(&escape_xml(preset));
        attrs.push('"');
    }
    if let Some(highlight) = style.highlight {
        attrs.push_str(" data-style-highlight=\"");
        attrs.push_str(if highlight { "true" } else { "false" });
        attrs.push('"');
    }
    attrs
}
```

- [ ] **Step 7: Run focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_sample_shot_draws_seeded_annotations -q
```

Expected: PASS.

- [ ] **Step 8: Run focused CLI regression tests**

Run:

```sh
cargo test -p rstim --test cli_render_svg -q
cargo test -p rstim --test cli_export_json export_json_sample_shot_exports_fixed_seed_sample_visualization_contract -q
cargo test -p rstim --test cli_export_json export_json_rejects_seed_without_sample_shot -q
cargo test -p rstim --test qp101_svg svg_renderer_renders_qp101_fallback_operations_and_annotations -q
```

Expected: all PASS. This verifies existing `export_json` sample behavior and existing generic annotation rendering survived the helper refactor and annotation metadata change.

- [ ] **Step 9: Format and commit the implementation**

Run:

```sh
rustfmt rstim/src/cli.rs rstim/src/qp101_svg.rs rstim/tests/cli_render_svg.rs
```

Then commit:

```sh
git add rstim/src/cli.rs rstim/src/qp101_svg.rs rstim/tests/cli_render_svg.rs
git commit -m "feat: render svg sample annotations"
```

- [ ] **Step 10: Run issue and broad verification**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_sample_shot_draws_seeded_annotations -q
cargo test -p rstim --test cli_render_svg -q
cargo test
rustfmt --check rstim/src/cli.rs rstim/src/qp101_svg.rs rstim/tests/cli_render_svg.rs
git diff --check
```

Expected: all commands exit 0.
