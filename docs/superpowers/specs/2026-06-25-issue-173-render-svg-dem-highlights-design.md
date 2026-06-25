# Issue 173 `rstim render_svg` DEM Highlight Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #173, DEM-origin highlight rendering through `rstim render_svg`

## Summary

Issue #173 wires the existing DEM-origin QP101 annotation export path into the
built-in SVG CLI renderer. Users should be able to run:

```sh
rstim render_svg --highlight_dem_error 0 --in circuit.stim --out highlight.svg
```

The resulting SVG should contain the base circuit plus visible annotation
markers for the selected detector error model error's source operations and
symptom detectors or observables. The implementation should reuse
`ErrorAnalyzer::circuit_to_tracked_dem` and
`export_qp101_with_highlighted_dem_error`; the SVG renderer consumes the
resulting QP101 annotations instead of recomputing provenance.

## Current State

The repository is already in an Agent Desk linked worktree on branch
`agent/issue-173-render-dem-origin-highlights-through-render_svg-run-1`.

Relevant existing pieces:

- `rstim/src/cli.rs` has `export_json --highlight_dem_error`, including
  tracked-DEM construction, unsupported-instruction message rewriting,
  out-of-range message rewriting, and `--sample_shot` compatibility checks.
- `rstim/src/cli.rs` has `render_svg --in/--out`, but it only builds a plain
  QP101 document through `build_plain_qp101_document`.
- `rstim/src/qp101.rs` exposes
  `export_qp101_with_highlighted_dem_error`, which annotates source operations
  with `marker` annotations tagged `dem-origin` and symptoms with `marker`
  annotations tagged `dem-symptom`.
- `rstim/src/qp101_svg.rs` already renders generic annotation text below the
  operation it annotates, but it uses one hard-coded annotation fill color and
  does not expose annotation style presets in the SVG text.
- `rstim/tests/cli_render_svg.rs` covers plain render output and safe
  file-output behavior for invalid input.

## Goals

- Add `--highlight_dem_error <index>` to `rstim render_svg`.
- Reuse the same tracked-DEM and highlighted QP101 export path as
  `export_json --highlight_dem_error`.
- Keep highlighted file output safe: do not open or truncate `--out` until
  parse, tracked-DEM construction, highlighted QP101 export, and SVG rendering
  have all succeeded.
- Keep error messages aligned with `export_json`, including a clear
  `DEM error index out of range` message for invalid indexes.
- Render QP101 annotation labels and text visibly in SVG.
- Render annotation style presets deterministically in raw SVG text through
  stable attributes and visual color selection.
- Add the issue's required CLI integration test, including a negative control
  that proves invalid highlight queries preserve an existing output file.

## Non-Goals

- Do not add new DEM provenance semantics.
- Do not implement seeded sample-shot rendering from issue #172.
- Do not add interactive DEM error selection.
- Do not change QP101 JSON shape or annotation schema.
- Do not update Typst fixtures because this change consumes existing QP101
  annotations and changes only the built-in SVG path.

## Approaches Considered

### 1. Shared highlighted QP101 builder plus annotation-aware SVG attributes

Extract the document-selection logic used by `export_json` into a helper that
returns a `Qp101Document` from parsed instructions and visualization options.
`export_json` will keep its serialization behavior, and `render_svg` will call
the same helper before rendering to SVG. Extend SVG annotation rendering to add
stable annotation classes/data attributes for style presets while preserving
the visible text format.

Benefits:

- directly follows the issue's recommendation to reuse tracked DEM export
- avoids renderer-side provenance logic
- keeps safe file-output behavior from issue #167
- keeps `export_json` and `render_svg` option validation aligned
- gives tests stable SVG text to inspect without changing the QP101 schema

Costs:

- touches both CLI document construction and SVG annotation rendering

This is the chosen approach.

### 2. Build DEM provenance inside `qp101_svg`

Pass raw Stim instructions or tracked-DEM state into the renderer and have it
decide which operations to highlight.

Benefits:

- avoids adding option plumbing to the CLI document builder

Costs:

- duplicates provenance semantics already implemented for QP101 export
- changes the renderer's public responsibility from QP101 rendering to circuit
  analysis
- conflicts with the issue's technical recommendation

This is rejected.

### 3. Run `export_json --highlight_dem_error` internally and deserialize JSON

Reuse the existing CLI behavior by serializing highlighted QP101 JSON into
memory, deserializing it back to `Qp101Document`, then rendering SVG.

Benefits:

- minimizes direct refactoring of `run_export_json`

Costs:

- adds unnecessary serialization round trips
- couples SVG rendering to JSON output formatting instead of typed QP101 data
- makes error paths harder to reason about

This is rejected.

## CLI Behavior

Extend the `RenderSvg` Clap variant:

```rust
RenderSvg {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long = "highlight_dem_error")]
    highlight_dem_error: Option<usize>,
}
```

The dispatcher should read input, render the SVG to a `String`, and only then
open the output destination:

```rust
let text = read_input(r#in.as_deref())?;
let svg = run_render_svg_to_string(&text, highlight_dem_error)?;
let mut w = open_output(out.as_deref())?;
w.write_all(svg.as_bytes())
    .map_err(|e| format!("write error: {e}"))
```

The safe-output boundary remains the call to `open_output`. Highlight-specific
failures must happen before that call.

## Document Construction

Add a private helper in `rstim/src/cli.rs`:

```rust
fn build_qp101_document_for_visualization(
    instrs: &[crate::ir::StimInstr],
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
) -> Result<crate::qp101::Qp101Document, String>
```

This helper should own option compatibility shared by `export_json` and
`render_svg`:

- `seed.is_some() && !sample_shot` returns
  `--seed is only supported with --sample_shot`
- `sample_shot && highlight_dem_error.is_some()` returns
  `--sample_shot cannot be combined with --highlight_dem_error`
- `Some(index)` builds a tracked DEM with
  `ErrorAnalyzer::circuit_to_tracked_dem`, rewrites unsupported-instruction
  messages the same way `export_json` does today, and calls
  `export_qp101_with_highlighted_dem_error`
- out-of-range highlighted QP101 export errors are rewritten to start with
  `DEM error index out of range`
- `None if sample_shot` keeps the existing sample-trace QP101 path for
  `export_json`
- `None` returns `build_plain_qp101_document(instrs)`

`run_export_json` should call this helper with its existing options.
`run_render_svg_to_string` should call it with `sample_shot = false` and
`seed = None` because issue #173 does not add sample-shot SVG rendering.

This design keeps the compatibility logic ready for issue #172 without adding
the `render_svg --sample_shot` interface in this branch.

## SVG Annotation Rendering

Keep annotation text content stable:

```text
<kind>[: <label>][: <text>]
```

For annotations with a style preset, render deterministic raw SVG attributes:

- `class="annotation annotation-preset-danger"` for `preset = "danger"`
- `data-style-preset="danger"`
- `data-style-highlight="true"` when `style.highlight` is present
- `fill` selected from the style color or preset

For annotations with tags, render a deterministic `data-annotation-tags`
attribute using the tag order supplied by QP101, for example
`data-annotation-tags="dem-origin query-result"`.

Use existing XML escaping for text and attributes. This keeps existing
annotation text assertions working while making style presets inspectable from
the SVG string.

## Error Handling

Invalid Stim syntax, unsupported tracked-DEM instructions, out-of-range DEM
error indexes, and renderer errors should propagate through the existing CLI
error path as nonzero exits with stderr text. Existing output files passed to
`--out` must remain unchanged for all failures that occur before final SVG
bytes are ready.

## Testing

Add `render_svg_highlight_dem_error_draws_query_markers` to
`rstim/tests/cli_render_svg.rs`.

The test should use:

```stim
X_ERROR(0.1) 0
M 0
DETECTOR rec[-1]
```

It should verify:

- plain `rstim render_svg` succeeds and does not contain DEM highlight marker
  attributes such as `dem-origin` or `data-style-preset="danger"`
- `rstim render_svg --highlight_dem_error 0 --in <input> --out <output>`
  succeeds, writes no stdout, and produces an SVG starting with `<svg`
- the highlighted SVG contains base labels such as `q0`, `XE`, `M`, and
  `DETECTOR`
- the highlighted SVG contains visible marker text for the source operation and
  detector symptom, including `marker: X` and `marker: D0`
- the highlighted SVG contains deterministic annotation metadata such as
  `annotation-preset-danger`, `data-style-preset="danger"`, `dem-origin`, and
  `dem-symptom`
- an out-of-range `--highlight_dem_error 99 --out <existing-output>` exits
  nonzero, stderr contains `DEM error index out of range`, and the existing
  output file remains unchanged

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_highlight_dem_error_draws_query_markers -q
cargo test -p rstim --test cli_render_svg -q
cargo test -p rstim --test qp101_svg -q
cargo test
git diff --check
```

If the sandbox blocks online Cargo registry access, use equivalent focused
`--offline` reruns and record any exact online command that failed.
