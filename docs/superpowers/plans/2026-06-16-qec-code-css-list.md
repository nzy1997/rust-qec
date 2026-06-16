# QEC Code CSS List Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `qec-code code css list` so users can discover supported built-in CSS fixed ids and family shapes without reading Rust source.

**Architecture:** Add a small catalog API beside the existing built-in CSS registry in `qec-code/src/codes/built_in_css.rs`. Refactor the `code css` CLI branch into an `Args` wrapper that accepts either a `list`/`export` subcommand or the existing positional export form, then render the catalog as stable human-readable text.

**Tech Stack:** Rust 2024, clap derive, qec-code crate tests, Cargo integration tests.

---

## File Structure

- Modify `qec-code/src/codes/built_in_css.rs`
  - Owns built-in CSS registry data and parser.
  - Add `BuiltInCssCatalogEntry` and `built_in_css_catalog()` here so listing metadata stays beside dispatch metadata.
- Modify `qec-code/src/cli.rs`
  - Owns command parsing and command execution.
  - Refactor `CodeCommands::Css` into `CssArgs`, add `CssCommands::{List, Export}`, and add `run_css_list()`.
- Modify `qec-code/tests/cli.rs`
  - Owns binary-facing CLI regressions and direct `run(...)` tests.
  - Add list-command tests, export-subcommand regression, and update direct enum construction for the new `CssArgs` shape.
- Modify `qec-code/tests/code.rs`
  - Owns built-in CSS registry tests.
  - Add a focused catalog metadata test to prevent drift and duplicate specs.

## Task 1: Add Built-In CSS Catalog Metadata

**Files:**
- Modify: `qec-code/src/codes/built_in_css.rs`
- Modify: `qec-code/tests/code.rs`

- [ ] **Step 1: Write the catalog metadata test**

At the top of `qec-code/tests/code.rs`, add:

```rust
use std::collections::HashSet;
```

Update the existing built-in CSS import to include `built_in_css_catalog`:

```rust
use qec_code::codes::built_in_css::{
    BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams, built_in_css_catalog,
    built_in_css_checks, parse_built_in_css_code_spec,
};
```

Insert this test after `built_in_css_registry_exposes_steane_checks`:

```rust
#[test]
fn built_in_css_catalog_lists_supported_specs() {
    let catalog = built_in_css_catalog();
    let specs = catalog.iter().map(|entry| entry.spec).collect::<Vec<_>>();
    let unique_specs = specs.iter().copied().collect::<HashSet<_>>();

    assert_eq!(
        specs,
        vec![
            "steane",
            "bb72",
            "repetition_x:d=<distance>",
            "repetition_z:d=<distance>",
        ]
    );
    assert_eq!(unique_specs.len(), specs.len());
    assert!(
        catalog.iter().all(|entry| !entry.description.is_empty()),
        "all catalog entries need descriptions: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_x:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_x entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_z:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_z entry should describe the distance constraint: {catalog:?}"
    );
}
```

- [ ] **Step 2: Run the new catalog test and verify it fails to compile**

Run:

```bash
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
```

Expected:

- The command exits non-zero.
- The compiler reports that `built_in_css_catalog` does not exist.

- [ ] **Step 3: Add the catalog API**

In `qec-code/src/codes/built_in_css.rs`, add this type immediately after `BuiltInCssChecks`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltInCssCatalogEntry {
    pub spec: &'static str,
    pub description: &'static str,
}
```

Add this catalog immediately after `BuiltInCssParams`:

```rust
const BUILT_IN_CSS_CATALOG: &[BuiltInCssCatalogEntry] = &[
    BuiltInCssCatalogEntry {
        spec: "steane",
        description: "fixed [[7,1,3]] CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "bb72",
        description: "fixed [[72,12,6]] bivariate-bicycle CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_x:d=<distance>",
        description: "X-check chain, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_z:d=<distance>",
        description: "Z-check chain, distance >= 2",
    },
];

pub fn built_in_css_catalog() -> &'static [BuiltInCssCatalogEntry] {
    BUILT_IN_CSS_CATALOG
}
```

- [ ] **Step 4: Run the catalog test and verify it passes**

Run:

```bash
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
```

Expected:

- The command exits zero.
- The test result includes `built_in_css_catalog_lists_supported_specs ... ok`.

- [ ] **Step 5: Commit the catalog metadata**

Run:

```bash
git add qec-code/src/codes/built_in_css.rs qec-code/tests/code.rs
git commit -m "feat: add built-in css catalog metadata"
```

## Task 2: Add CSS List CLI Dispatch

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add binary list tests**

In `qec-code/tests/cli.rs`, insert these tests after `code_css_bb72_hx_prints_sparse_rows_json`:

```rust
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
        stdout.contains("repetition_x:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("repetition_z:d=<distance>"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("distance >= 2"), "stdout was: {stdout}");
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
```

- [ ] **Step 2: Run the list tests and verify the success-path test fails**

Run:

```bash
cargo test -p qec-code --test cli code_css_list_
```

Expected:

- The command exits non-zero.
- `code_css_list_includes_supported_built_ins` fails because `qec-code code css list` is still parsed as an incomplete matrix export.
- `code_css_list_rejects_unexpected_extra_arguments` may already pass through clap validation.

- [ ] **Step 3: Refactor the CSS command type**

In `qec-code/src/cli.rs`, change the clap import and built-in CSS import:

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::QecError;
use crate::codes::built_in_css::{built_in_css_catalog, built_in_css_checks};
```

Replace the current `CodeCommands::Css { code_id, matrix }` variant with:

```rust
    Css(CssArgs),
```

Then add these types immediately after `CodeCommands`:

```rust
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_negates_reqs = true)]
#[command(arg_required_else_help = true)]
pub struct CssArgs {
    #[command(subcommand)]
    pub command: Option<CssCommands>,
    #[arg(value_name = "CODE_ID", required = true)]
    pub code_id: Option<String>,
    #[arg(value_name = "MATRIX", required = true)]
    pub matrix: Option<CssMatrixKind>,
}

#[derive(Debug, Subcommand)]
pub enum CssCommands {
    List,
    Export {
        code_id: String,
        matrix: CssMatrixKind,
    },
}
```

Replace the CSS arm in `run_code`:

```rust
        CodeCommands::Css(args) => run_css_args(args),
```

Add this helper immediately before `run_css`:

```rust
fn run_css_args(args: CssArgs) -> Result<String, QecError> {
    match args.command {
        Some(CssCommands::List) => Ok(run_css_list()),
        Some(CssCommands::Export { code_id, matrix }) => run_css(&code_id, matrix),
        None => {
            let code_id = args
                .code_id
                .expect("clap requires CODE_ID when no css subcommand is used");
            let matrix = args
                .matrix
                .expect("clap requires MATRIX when no css subcommand is used");

            run_css(&code_id, matrix)
        }
    }
}

fn run_css_list() -> String {
    let catalog = built_in_css_catalog();
    let width = catalog
        .iter()
        .map(|entry| entry.spec.len())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::with_capacity(catalog.len() + 1);

    lines.push("Built-in CSS codes:".to_owned());
    lines.extend(catalog.iter().map(|entry| {
        format!(
            "  {:width$}  {}",
            entry.spec,
            entry.description,
            width = width
        )
    }));

    lines.join("\n")
}
```

- [ ] **Step 4: Update direct `run(...)` tests for the new enum shape**

In `qec-code/tests/cli.rs`, update the import:

```rust
use qec_code::cli::{Cli, CodeCommands, Commands, CssArgs, CssCommands, CssMatrixKind, run};
```

In `run_code_css_steane_matrices_return_fixture_json_without_newline`, replace each direct CSS command construction.

For `hx`:

```rust
command: CodeCommands::Css(CssArgs {
    command: None,
    code_id: Some("steane".to_owned()),
    matrix: Some(CssMatrixKind::Hx),
}),
```

For `hz`:

```rust
command: CodeCommands::Css(CssArgs {
    command: None,
    code_id: Some("steane".to_owned()),
    matrix: Some(CssMatrixKind::Hz),
}),
```

In `run_code_css_unknown_id_returns_registry_error`, replace the direct CSS command construction with:

```rust
command: CodeCommands::Css(CssArgs {
    command: None,
    code_id: Some("unknown".to_owned()),
    matrix: Some(CssMatrixKind::Hx),
}),
```

- [ ] **Step 5: Add direct list and explicit export tests**

In `qec-code/tests/cli.rs`, insert this binary export regression after `code_css_steane_hx_prints_workspace_fixture`:

```rust
#[test]
fn code_css_export_subcommand_steane_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "export", "steane", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");

    assert_eq!(stdout, expected);
}
```

Insert this direct `run(...)` test after `run_code_css_steane_matrices_return_fixture_json_without_newline`:

```rust
#[test]
fn run_code_css_list_returns_catalog_without_newline() {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs {
                command: Some(CssCommands::List),
                code_id: None,
                matrix: None,
            }),
        },
    })
    .unwrap();

    assert!(output.starts_with("Built-in CSS codes:\n"));
    assert!(!output.ends_with('\n'));
    assert!(output.contains("steane"));
    assert!(output.contains("bb72"));
    assert!(output.contains("repetition_x:d=<distance>"));
    assert!(output.contains("repetition_z:d=<distance>"));
}
```

- [ ] **Step 6: Run the issue list tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test cli code_css_list_
```

Expected:

- The command exits zero.
- Both `code_css_list_includes_supported_built_ins` and `code_css_list_rejects_unexpected_extra_arguments` pass.

- [ ] **Step 7: Run the nearby CSS CLI tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected:

- The command exits zero.
- Existing positional exports still pass.
- `code_css_export_subcommand_steane_hx_prints_workspace_fixture` passes.
- Unknown-code behavior still returns `unknown built-in CSS code: unknown`.

- [ ] **Step 8: Commit the CLI list command**

Run:

```bash
git add qec-code/src/cli.rs qec-code/tests/cli.rs
git commit -m "feat: add qec-code css list command"
```

## Task 3: Final Verification

**Files:**
- Verify: `qec-code/src/codes/built_in_css.rs`
- Verify: `qec-code/src/cli.rs`
- Verify: `qec-code/tests/cli.rs`
- Verify: `qec-code/tests/code.rs`

- [ ] **Step 1: Run the issue verification filter**

Run:

```bash
cargo test -p qec-code --test cli code_css_list_
```

Expected:

- The command exits zero.
- Both issue-requested list tests pass.

- [ ] **Step 2: Run the CSS CLI regression filter**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected:

- The command exits zero.
- Positional exports, explicit export, list, and unknown-code regressions pass.

- [ ] **Step 3: Run the catalog metadata test**

Run:

```bash
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
```

Expected:

- The command exits zero.
- The catalog exposes exactly `steane`, `bb72`, `repetition_x:d=<distance>`, and `repetition_z:d=<distance>`.

- [ ] **Step 4: Run the full qec-code package tests**

Run:

```bash
cargo test -p qec-code
```

Expected:

- The command exits zero.
- No `qec-code` tests fail.

- [ ] **Step 5: Run formatting check**

Run:

```bash
cargo fmt --check --package qec-code
```

Expected:

- The command exits zero.
- Rustfmt reports no formatting diff.

- [ ] **Step 6: Inspect final git state**

Run:

```bash
git status --short --branch
```

Expected:

- Only intentional issue #60 files are modified or committed.
- The pre-existing untracked `docs/superpowers/plans/2026-06-16-randomized-css-distance-upper-bound.md` in the original checkout remains unrelated and unstaged.
