# qec-code CSS CLI Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `qec-code code css <code-id> <hx|hz>` so built-in CSS parity-check matrices export as the existing `sparse_rows` JSON document.

**Architecture:** Keep the CLI as a thin adapter over existing library APIs. `qec-code/src/cli.rs` parses the generic CSS command, calls `built_in_css_checks(...)`, selects one matrix, and serializes it through `SparseRowsMatrix`; `qec-code/tests/cli.rs` verifies the binary output against pinned workspace fixtures.

**Tech Stack:** Rust 2024, `qec-code`, `clap` derive/`ValueEnum`, `SparseRowsMatrix`, binary integration tests with `std::process::Command`, `cargo test`

---

## File Structure

- `qec-code/src/cli.rs`
  - Add the `code css <code-id> <hx|hz>` branch.
  - Define the `CssMatrixKind` selector with Clap value parsing.
  - Implement `run_css(...)` as the only new command handler.
- `qec-code/tests/cli.rs`
  - Add fixture-reading helpers.
  - Add the three issue #56 binary regression tests.

No fixture, serializer, registry, `rstim`, or `rsinter` files should change.

### Task 1: Add failing CLI regression tests

**Files:**
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Extend the CLI test imports and helpers**

Replace the file's existing import and helper header:

```rust
use std::process::Command;

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}
```

with:

```rust
use std::path::PathBuf;
use std::process::{Command, Output};

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_fixture(rel_path: &str) -> String {
    std::fs::read_to_string(workspace_root().join(rel_path))
        .unwrap_or_else(|err| panic!("failed to read fixture {rel_path}: {err}"))
}

fn run_qec_code(args: &[&str]) -> Output {
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("qec-code binary should run")
}
```

- [ ] **Step 2: Add the issue #56 CLI tests**

Append these tests to `qec-code/tests/cli.rs` after the existing Steane CLI tests:

```rust
#[test]
fn code_css_steane_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hx"]);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_steane_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hz"]);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_unknown_id_fails() {
    let output = run_qec_code(&["code", "css", "unknown", "hx"]);

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout was: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");

    assert!(
        stderr.contains("unknown built-in CSS code: unknown"),
        "stderr was: {stderr}"
    );
}
```

- [ ] **Step 3: Run the focused tests and verify they fail for the missing command**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected: the test target builds, then the new tests fail because `qec-code code css ...` is not implemented yet. The success-path tests should report a non-success status and Clap stderr mentioning an unrecognized `css` subcommand.

- [ ] **Step 4: Commit the failing tests**

Run:

```bash
git add qec-code/tests/cli.rs
git commit -m "test: add qec-code css cli export coverage"
```

Expected: one commit containing only `qec-code/tests/cli.rs`.

### Task 2: Implement the CSS export command

**Files:**
- Modify: `qec-code/src/cli.rs`

- [ ] **Step 1: Replace `qec-code/src/cli.rs` with the CSS-aware command handler**

Update `qec-code/src/cli.rs` so its complete contents are:

```rust
use clap::{Parser, Subcommand, ValueEnum};

use crate::QecError;
use crate::codes::built_in_css::built_in_css_checks;
use crate::codes::steane::Steane;
use crate::css::SparseRowsMatrix;
use crate::distance::compute_distance;

#[derive(Debug, Parser)]
#[command(name = "qec-code")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Code {
        #[command(subcommand)]
        command: CodeCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodeCommands {
    Steane {
        #[command(subcommand)]
        command: SteaneCommands,
    },
    Css {
        code_id: String,
        matrix: CssMatrixKind,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CssMatrixKind {
    Hx,
    Hz,
}

#[derive(Debug, Subcommand)]
pub enum SteaneCommands {
    Summary,
    Stabilizers,
    Logicals,
    Distance,
}

pub fn run(cli: Cli) -> Result<String, QecError> {
    match cli.command {
        Commands::Code { command } => run_code(command),
    }
}

fn run_code(command: CodeCommands) -> Result<String, QecError> {
    match command {
        CodeCommands::Steane { command } => run_steane(command),
        CodeCommands::Css { code_id, matrix } => run_css(&code_id, matrix),
    }
}

fn run_css(code_id: &str, matrix: CssMatrixKind) -> Result<String, QecError> {
    let checks = built_in_css_checks(code_id)?;
    let num_cols = checks.num_cols;
    let rows = match matrix {
        CssMatrixKind::Hx => checks.hx,
        CssMatrixKind::Hz => checks.hz,
    };

    let matrix = SparseRowsMatrix::new(num_cols, rows)?;
    Ok(matrix.to_json_string())
}

fn run_steane(command: SteaneCommands) -> Result<String, QecError> {
    let steane = Steane::new()?;
    let code = steane.code();

    match command {
        SteaneCommands::Summary => Ok(format!(
            "name: steane\nn: {}\nstabilizer_rank: {}\nk: {}",
            code.n(),
            code.stabilizer_rank(),
            code.num_logical_qubits()
        )),
        SteaneCommands::Stabilizers => {
            let lines = code
                .stabilizers()
                .iter()
                .enumerate()
                .map(|(index, stabilizer)| format!("g{}: {}", index + 1, format_pauli(stabilizer)))
                .collect::<Vec<_>>();
            Ok(lines.join("\n"))
        }
        SteaneCommands::Logicals => {
            let basis = code.logical_basis()?;
            Ok(format!(
                "k: {}\nlogical_x:\n{}\nlogical_z:\n{}",
                basis.k,
                format_pauli_list(&basis.logical_x),
                format_pauli_list(&basis.logical_z)
            ))
        }
        SteaneCommands::Distance => {
            let distance = compute_distance(code)?;
            Ok(format!(
                "distance: {}\nlogical_class: {:?}\nwitness: {}",
                distance.distance,
                distance.logical_class,
                format_pauli(&distance.witness)
            ))
        }
    }
}

fn format_pauli_list(paulis: &[crate::Pauli]) -> String {
    paulis
        .iter()
        .enumerate()
        .map(|(index, pauli)| format!("  {}: {}", index + 1, format_pauli(pauli)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_pauli(pauli: &crate::Pauli) -> String {
    format!(
        "x={:?} z={:?} weight={}",
        pauli.x_bits(),
        pauli.z_bits(),
        pauli.weight()
    )
}
```

- [ ] **Step 2: Run the focused CSS CLI tests**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected: all three `code_css_...` tests pass.

- [ ] **Step 3: Run the full CLI test target**

Run:

```bash
cargo test -p qec-code --test cli
```

Expected: all existing Steane CLI tests and all new CSS CLI tests pass.

- [ ] **Step 4: Commit the implementation**

Run:

```bash
git add qec-code/src/cli.rs
git commit -m "feat: add qec-code css matrix export CLI"
```

Expected: one commit containing only `qec-code/src/cli.rs`.

### Task 3: Verify the final issue #56 scope

**Files:**
- Test: `qec-code/tests/cli.rs`
- Test: `qec-code/tests/css_export.rs`
- Test: `qec-code/tests/code.rs`

- [ ] **Step 1: Run the dependency serializer regression tests**

Run:

```bash
cargo test -p qec-code --test css_export
```

Expected: all sparse-row export tests pass, proving the CLI still sits on the issue #55 JSON contract.

- [ ] **Step 2: Run the full `qec-code` crate suite**

Run:

```bash
cargo test -p qec-code
```

Expected: all `qec-code` unit, integration, and binary tests pass.

- [ ] **Step 3: Inspect the final diff and worktree**

Run:

```bash
git diff --stat HEAD~2..HEAD
git status --short
```

Expected: the final implementation commits touch only:

```text
qec-code/src/cli.rs
qec-code/tests/cli.rs
```

The unrelated untracked file may still appear:

```text
?? docs/superpowers/plans/2026-06-16-rsinter-memory-z-parity.md
```

Do not stage or modify that file as part of issue #56.

- [ ] **Step 4: Manually confirm the two success commands**

Run:

```bash
cargo run -p qec-code -- code css steane hx
cargo run -p qec-code -- code css steane hz
```

Expected: each command prints one compact `sparse_rows` JSON document with one trailing newline. The `hx` and `hz` output should match the Steane fixture files exactly.
