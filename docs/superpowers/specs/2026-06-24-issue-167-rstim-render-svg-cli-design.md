# Issue 167 `rstim render_svg` CLI Design

Date: 2026-06-24
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #167, plain SVG rendering CLI entry point for `rstim`

## Summary

Issue #167 adds the first command-line entry point for the QP101 SVG renderer
introduced by issue #166. The command should let users run:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

It should also support stdin to stdout when no input or output path is supplied.
The command is intentionally limited to plain circuit rendering. Sample-shot
overlays, DEM-origin highlights, site integration, and `export_json` removal
remain out of scope.

## Current State

`rstim` already has these pieces:

- `rstim/src/cli.rs` defines the Clap subcommands, I/O helpers, and the
  `export_json` implementation.
- `export_json` reads Stim text from `--in` or stdin, builds a
  `Qp101Document`, serializes it, and writes to `--out` or stdout.
- `rstim/src/qp101.rs` exposes `export_qp101` and the typed QP101 document
  model.
- `rstim/src/qp101_svg.rs` exposes
  `render_svg(doc: &Qp101Document) -> Result<String, String>`.
- `rstim/tests/cli_export_json.rs` shows the current CLI integration-test
  style with `CARGO_BIN_EXE_rstim`, stdin piping, temporary files, and stderr
  assertions.

The workspace is already in an Agent Desk linked worktree on branch
`agent/issue-167-add-the-rstim-render_svg-cli-command-run-1`.

## Goals

- Add a `render_svg` subcommand to `rstim`.
- Accept `--in <circuit.stim>` or read Stim text from stdin.
- Accept `--out <circuit.svg>` or write SVG text to stdout.
- Parse Stim text, export through QP101, then call
  `rstim::qp101_svg::render_svg`.
- Share the plain QP101 document-building logic with `export_json` instead of
  duplicating the parser/export behavior.
- Preserve output safety for `--out`: do not create, truncate, or replace the
  destination until parsing, QP101 export, and SVG rendering have succeeded.
- Return a nonzero exit with clear stderr on invalid input.
- Add a focused CLI integration test file named `rstim/tests/cli_render_svg.rs`.

## Non-Goals

- Do not add `render_svg` flags for sample-shot overlays.
- Do not add `render_svg` flags for DEM-origin highlighting.
- Do not change the QP101 JSON schema.
- Do not change the SVG renderer layout.
- Do not update CLI user documentation in this issue.
- Do not remove or rewrite `export_json`.

## Approaches Considered

### 1. Shared QP101 builder plus in-memory SVG output

Extract the plain Stim-to-QP101 path from `run_export_json` into a small helper,
for example `build_plain_qp101_document(text: &str) -> Result<Qp101Document,
String>`. Add `run_render_svg_to_string(text: &str) -> Result<String, String>`
that calls the shared builder and `qp101_svg::render_svg`. In the CLI match arm,
read input, render SVG into a `String`, then open the output destination and
write the final bytes.

Benefits:

- matches the issue's recommendation to share document-building logic
- keeps the first CLI issue focused on plain rendering
- naturally guarantees safe file-output behavior for `render_svg`
- keeps the renderer dependency direction as Stim -> QP101 -> SVG
- needs only localized changes in `rstim/src/cli.rs`

Costs:

- sample-shot and DEM-highlight document construction remain inside
  `run_export_json` until later issues add those renderer flags

This is the chosen approach.

### 2. Call `run_export_json` and parse JSON back into QP101

Reuse the command's JSON serialization path by writing QP101 JSON into memory,
deserializing it, then rendering SVG.

Benefits:

- avoids extracting a helper from `run_export_json`

Costs:

- adds an unnecessary serialize/deserialize round trip
- couples SVG rendering to JSON formatting instead of the typed QP101 model
- makes error paths more indirect

This is rejected.

### 3. Duplicate parser and QP101 export logic in `render_svg`

Implement `render_svg` with its own `parse_lines(text)` and
`export_qp101(&instrs)` sequence.

Benefits:

- fastest local edit

Costs:

- duplicates the behavior `export_json` already owns
- increases drift risk as QP101 export options grow
- conflicts with the issue's recommendation

This is rejected.

## Design

Add a new Clap variant:

```rust
#[command(name = "render_svg")]
RenderSvg {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
}
```

The `run` dispatcher should handle it by reading input first, rendering into
memory second, and opening output last:

```rust
let text = read_input(r#in.as_deref())?;
let svg = run_render_svg_to_string(&text)?;
let mut w = open_output(out.as_deref())?;
w.write_all(svg.as_bytes())
    .map_err(|e| format!("write error: {e}"))
```

That ordering is the safe-output boundary. If input parsing, QP101 export, or
SVG rendering fails, `open_output` is never called, so an existing output file
remains untouched.

Extract one plain document-building helper:

```rust
fn build_plain_qp101_document(text: &str) -> Result<crate::qp101::Qp101Document, String> {
    let instrs = parse_lines(text)?;
    crate::qp101::export_qp101(&instrs)
}
```

`run_export_json` should reuse this helper only for the plain path. Its
sample-shot and DEM-highlight branches keep their existing specialized logic
and validation because they are out of scope for `render_svg`.

Add:

```rust
fn run_render_svg_to_string(text: &str) -> Result<String, String> {
    let doc = build_plain_qp101_document(text)?;
    crate::qp101_svg::render_svg(&doc)
}
```

The helper can remain private to `cli.rs` because the current public interface
is the CLI command and the renderer module's public `render_svg` function.

## Error Handling

Invalid Stim syntax should fail through `parse_lines` and be returned through
the existing `main` error path as `Error: <message>` on stderr. Renderer errors
should propagate the same way. The CLI should not catch and rewrite these
errors unless a future issue needs command-specific messages.

For `--out`, the destination file should be opened only after all fallible work
that determines the SVG contents has completed. Write failures can still leave a
partial new output after opening, which matches normal filesystem behavior; the
issue's safety requirement is about not truncating or replacing an existing file
before validation and rendering succeed.

## Testing

Create `rstim/tests/cli_render_svg.rs` with one required test:

- `render_svg_writes_svg_from_stdin_and_file`

The test should cover three paths:

- Pipe `H 0\nCX 0 1\nTICK\nM 0\n` into `rstim render_svg`, assert success,
  empty stderr, stdout starts with `<svg`, and stdout contains `q0`, `H`, and
  `M`.
- Write the same circuit to a temp file, run
  `rstim render_svg --in <input> --out <output>`, assert success, empty stdout,
  output starts with `<svg`, and output contains `q0`, `H`, and `M`.
- Create an existing output file containing `existing output should remain`,
  run `rstim render_svg --in <bad-circuit> --out <same-output>` with invalid
  Stim syntax, assert failure, stderr names a parse error, and the output file
  still contains the original text.

Run the issue verification command:

```sh
cargo test -p rstim --test cli_render_svg render_svg_writes_svg_from_stdin_and_file -q
```

Also run broad verification:

```sh
cargo test
git diff --check
```

If online Cargo registry access is blocked, rerun Rust verification with
`--offline` where accepted by Cargo and record the online failure.
