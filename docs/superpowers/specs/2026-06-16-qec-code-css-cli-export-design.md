Date: 2026-06-16
Status: Draft accepted in-session, written for review
Scope: GitHub issue #56, `qec-code` CLI export for built-in CSS parity-check matrices

## Summary

Issue #56 asks `qec-code` to expose the built-in CSS parity-check matrices from
the command line:

```text
qec-code code css <code-id> <hx|hz>
```

The command should print exactly one `sparse_rows` JSON document on stdout and
return a non-zero exit code with a clear stderr message for unknown built-in CSS
ids.

This builds directly on issue #54, which added
`built_in_css_checks("steane")`, and issue #55, which added
`SparseRowsMatrix::to_json_string()` for the existing workspace JSON contract.

## Goals

- Add a generic CSS CLI subtree under `qec-code code css`.
- Export one selected matrix at a time: either `hx` or `hz`.
- Reuse the built-in CSS registry as the source of row supports.
- Reuse `SparseRowsMatrix` as the owner of sparse-row validation and JSON
  serialization.
- Preserve all existing `qec-code code steane ...` inspection commands.
- Add the three issue-requested CLI regression tests.

## Non-Goals

- Do not add one-shot export of both matrices together.
- Do not add a new top-level `{hx, hz}` JSON document shape.
- Do not add logical basis, distance, or observable output.
- Do not implement parameterized code specs from issue #57.
- Do not add repetition or `bb72` families from issues #58 and #59.
- Do not add a listing command from issue #60.
- Do not change `rstim` or `rsinter` behavior.

## Current State

The current CLI has one `code` subtree with a Steane-specific branch:

```text
qec-code code steane summary
qec-code code steane stabilizers
qec-code code steane logicals
qec-code code steane distance
```

Those commands are human-readable inspection commands. They should remain
unchanged.

The library side now has the two pieces needed for machine-readable export:

1. `qec_code::codes::built_in_css::built_in_css_checks(code_id)` returns
   canonical `num_cols`, `hx`, and `hz` row supports for fixed built-ins such as
   `steane`.
2. `qec_code::css::SparseRowsMatrix::new(...).to_json_string()` validates one
   sparse matrix and emits compact JSON in the workspace format:

```json
{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}
```

The CLI should be a thin adapter between these two existing library APIs.

## Alternatives Considered

### 1. Add `code css <code-id> <hx|hz>` as a generic subtree

This is the recommended approach.

Benefits:

- matches issue #56 exactly
- keeps CSS export separate from Steane-only inspection
- makes issue #57's future parameterized code specs a matter of widening the
  `code-id` parser, not redesigning the command shape
- reuses existing main stdout/stderr behavior

Costs:

- adds a new enum branch and a small matrix selector type

### 2. Add `code steane hx` and `code steane hz`

Benefits:

- smallest short-term command addition for the only current built-in

Costs:

- hard-codes Steane into a machine-readable export path
- conflicts with the issue's requested generic CSS command
- would need another command shape once parameterized families land

This is not recommended.

### 3. Add flags to the existing Steane commands

For example:

```text
qec-code code steane --matrix hx --format sparse_rows
```

Benefits:

- keeps all current Steane behavior under one branch

Costs:

- mixes human-readable inspection and machine-readable export
- creates unnecessary format options before the project needs them
- makes the future family surface less obvious

This is not recommended.

## Decision

Add a `Css` branch to `CodeCommands` in `qec-code/src/cli.rs`:

```rust
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
```

Add a small matrix selector:

```rust
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CssMatrixKind {
    Hx,
    Hz,
}
```

The command path becomes:

```text
qec-code code css steane hx
qec-code code css steane hz
```

Clap handles malformed selector values such as `foo` as CLI usage errors.
Unknown code ids such as `unknown` reach the registry and return
`QecError::UnknownBuiltInCssCode`, which `main.rs` already renders on stderr
with exit code `1`.

## Data Flow

The success path should be:

1. Clap parses `code css steane hx` into `CodeCommands::Css`.
2. `run_code(...)` dispatches to a new helper such as `run_css(...)`.
3. `run_css(...)` calls `built_in_css_checks(&code_id)`.
4. The selected matrix is cloned from `checks.hx` or `checks.hz`.
5. `SparseRowsMatrix::new(checks.num_cols, selected_rows)?` validates the
   selected matrix.
6. `to_json_string()` returns compact JSON without a trailing newline.
7. Existing `main.rs::write_success(...)` writes the string plus one trailing
   newline to stdout.

That keeps newline ownership in the CLI writer, matching the existing output
pattern and the fixture files that end with a newline.

The unknown-id path should be:

1. Clap parses `code css unknown hx`.
2. `built_in_css_checks("unknown")` returns `UnknownBuiltInCssCode`.
3. Existing `main.rs::write_error(...)` writes a human-readable error to
   stderr and exits non-zero.

## Error Handling

No new error enum variants are required.

Expected behavior:

- Unknown built-in CSS ids return the existing
  `QecError::UnknownBuiltInCssCode { code_id }`.
- Invalid `hx|hz` selector values are handled by Clap before entering
  `qec_code::cli::run(...)`.
- Sparse-row validation failures from `SparseRowsMatrix::new(...)` propagate as
  existing `QecError` variants. Built-in Steane should not trigger them, but
  propagation keeps the boundary honest.

## Testing And Verification

Add three tests to `qec-code/tests/cli.rs`, using the existing binary-driven
test style.

### `code_css_steane_hx_prints_workspace_fixture`

Run:

```text
qec-code code css steane hx
```

Assert:

- exit status succeeds
- stderr is empty
- stdout equals `rsinter/tests/fixtures/css/steane_hx.json`

### `code_css_steane_hz_prints_workspace_fixture`

Run:

```text
qec-code code css steane hz
```

Assert:

- exit status succeeds
- stderr is empty
- stdout equals `rsinter/tests/fixtures/css/steane_hz.json`

### `code_css_unknown_id_fails`

Run:

```text
qec-code code css unknown hx
```

Assert:

- exit status fails
- stdout is empty
- stderr contains `unknown built-in CSS code: unknown`

The acceptance command from issue #56 is:

```text
cargo test -p qec-code --test cli code_css_steane_hx_prints_workspace_fixture code_css_steane_hz_prints_workspace_fixture code_css_unknown_id_fails
```

The implementation should also run:

```text
cargo test -p qec-code
```

## Scope Check

This is a narrow CLI adapter over APIs already introduced by issues #54 and
#55. It should fit in one implementation pass touching:

- `qec-code/src/cli.rs`
- `qec-code/tests/cli.rs`

No other crates or fixtures should need changes.
