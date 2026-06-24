# Issue 167 `rstim render_svg` CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `rstim render_svg` CLI command that renders plain Stim circuits to SVG through QP101.

**Architecture:** Extend the existing `rstim/src/cli.rs` command dispatcher with a narrow `render_svg` path. The command will read Stim text, reuse a shared plain QP101 document builder, render SVG in memory through `rstim::qp101_svg::render_svg`, and only then open `--out` for writing.

**Tech Stack:** Rust 2024, Clap derive, existing `rstim::parser::parse_lines`, existing `rstim::qp101::export_qp101`, existing `rstim::qp101_svg::render_svg`, Cargo integration tests.

## Global Constraints

- Add a `render_svg` subcommand to the `rstim` CLI for plain circuit rendering.
- Support `rstim render_svg --in <circuit.stim> --out <circuit.svg>`.
- Support `rstim render_svg` reading Stim text from stdin and writing SVG to stdout.
- The command must parse Stim input, export it through the existing QP101 path, then render SVG through the built-in renderer from issue #166.
- Share document-building logic with `export_json` instead of duplicating parser/export behavior.
- Keep this issue focused on plain rendering; do not add sample-shot or DEM-highlight flags.
- Do not open or truncate the `--out` file before parse, QP101 export, and SVG rendering have all succeeded.
- Add CLI docs in a later documentation issue, not here.
- A successful stdout run returns a valid SVG document on stdout.
- A successful file-output run leaves stdout empty.
- Invalid input exits nonzero with clear stderr.
- Existing output file content remains unchanged when invalid input is passed with `--out`.

---

### Task 1: CLI Command, Safe Output, And Integration Test

**Files:**
- Modify: `rstim/src/cli.rs`
- Create: `rstim/tests/cli_render_svg.rs`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines(text: &str) -> Result<Vec<StimInstr>, String>`.
- Consumes: `rstim::qp101::export_qp101(instrs: &[StimInstr]) -> Result<Qp101Document, String>`.
- Consumes: `rstim::qp101_svg::render_svg(doc: &Qp101Document) -> Result<String, String>`.
- Produces: `rstim render_svg --in <path> --out <path>`.
- Produces: `rstim render_svg` stdin-to-stdout mode.
- Produces: private CLI helper `build_plain_qp101_document(instrs: &[crate::ir::StimInstr]) -> Result<crate::qp101::Qp101Document, String>`.
- Produces: private CLI helper `run_render_svg_to_string(text: &str) -> Result<String, String>`.

- [ ] **Step 1: Write the failing CLI integration test**

Create `rstim/tests/cli_render_svg.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q --offline
```

Expected: FAIL because Clap does not recognize the `render_svg` subcommand or because the new test file fails to compile before the command exists.

- [ ] **Step 3: Add the `render_svg` Clap variant**

Modify `rstim/src/cli.rs` in the `Commands` enum after `ExportJson`:

```rust
    /// Render a circuit as SVG through QP101
    #[command(name = "render_svg")]
    RenderSvg {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
```

- [ ] **Step 4: Add safe-output dispatch logic**

Modify `rstim/src/cli.rs` in `pub fn run(cli: Cli) -> Result<(), String>` immediately after the `ExportJson` match arm:

```rust
        Some(Commands::RenderSvg { r#in, out }) => {
            let text = read_input(r#in.as_deref())?;
            let svg = run_render_svg_to_string(&text)?;
            let mut w = open_output(out.as_deref())?;
            w.write_all(svg.as_bytes())
                .map_err(|e| format!("write error: {e}"))
        }
```

This match arm must render the SVG string before calling `open_output`.

- [ ] **Step 5: Extract the shared plain QP101 builder and SVG helper**

Modify `rstim/src/cli.rs` near `run_export_json` so the plain QP101 construction is shared:

```rust
fn build_plain_qp101_document(
    instrs: &[crate::ir::StimInstr],
) -> Result<crate::qp101::Qp101Document, String> {
    crate::qp101::export_qp101(instrs)
}

fn run_render_svg_to_string(text: &str) -> Result<String, String> {
    let instrs = parse_lines(text)?;
    let doc = build_plain_qp101_document(&instrs)?;
    crate::qp101_svg::render_svg(&doc)
}
```

Then change the plain branch in `run_export_json` from:

```rust
        None => crate::qp101::export_qp101(&instrs)?,
```

to:

```rust
        None => build_plain_qp101_document(&instrs)?,
```

Do not move or alter the existing sample-shot or DEM-highlight branches.

- [ ] **Step 6: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q --offline
```

Expected: PASS.

- [ ] **Step 7: Run the issue verification command**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q
```

Expected: PASS when online registry access is available. If the sandbox blocks crates.io access, record the failure and rerun:

```sh
cargo test -p rstim --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q --offline
```

Expected: PASS.

- [ ] **Step 8: Run broader CLI and renderer checks**

Run:

```sh
cargo test -p rstim --test cli_export_json -q --offline
cargo test -p rstim --test qp101_svg -q --offline
```

Expected: PASS.

- [ ] **Step 9: Run full requested verification**

Run:

```sh
cargo test
```

Expected: PASS when online registry access is available. If the sandbox blocks crates.io access, record the failure and run:

```sh
cargo test --offline
```

Expected: PASS.

- [ ] **Step 10: Run diff hygiene**

Run:

```sh
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 11: Commit**

Run:

```sh
git add rstim/src/cli.rs rstim/tests/cli_render_svg.rs docs/superpowers/plans/2026-06-24-issue-167-rstim-render-svg-cli.md
git commit -m "feat: add rstim render_svg cli"
```
