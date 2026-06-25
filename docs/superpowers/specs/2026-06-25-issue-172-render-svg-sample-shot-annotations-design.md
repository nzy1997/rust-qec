# Issue 172 Render SVG Sample-Shot Annotations Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #172, seeded sample-shot annotations through `rstim render_svg`

## Summary

Issue #172 extends `rstim render_svg` so the SVG CLI can render the same seeded
sample-shot annotation layer that `rstim export_json --sample_shot --seed <n>`
already exports into QP101. The CLI should accept:

```sh
rstim render_svg --sample_shot --seed 7 --in <circuit.stim> --out <sample.svg>
```

and produce a normal built-in SVG circuit diagram plus visible, text-inspectable
annotation markers for fired noise branches, measurement outcomes,
loss-caused measurement information, and detector flips when those annotations
exist in the QP101 document.

The change should reuse the existing sample export path:

```rust
crate::qp101::export_qp101_with_sample_trace(&instrs, &trace)
```

The renderer must consume QP101 annotations. It must not recompute sampled
semantics from measurement records or raw simulator output.

## Current State

The current branch starts at `master` after the dependency PRs for issues
#167, #168, #169, #170, and #171 were merged.

Relevant existing pieces:

- `rstim/src/cli.rs` defines `export_json` with `--sample_shot`,
  `--highlight_dem_error`, and `--seed` compatibility checks.
- `run_export_json` parses the circuit before validating sample option
  compatibility, then uses `Executor::run_with_trace` and
  `export_qp101_with_sample_trace` for sample-shot QP101 output.
- `rstim render_svg` currently accepts only `--in` and `--out`, builds plain
  QP101 through `build_plain_qp101_document`, renders into memory, and opens
  `--out` only after rendering succeeds.
- `rstim/src/qp101_svg.rs` already renders QP101 annotations generically as
  deterministic text containing `kind`, `label`, and `text`, but all
  annotation text uses one fill color and does not expose style presets in the
  SVG.
- `rstim/tests/cli_export_json.rs` already has a deterministic seeded sample
  fixture:

```stim
DEPOLARIZE1(1) 0
LOSS(1) 1
LOSS(1) 2
M 1
MRL 2
DETECTOR rec[-3]
```

with seed `7`, producing sample annotation labels `X`, `L`, `1[L]`,
`L=1 | M=1[L]`, and flipped detector label `D0`.

The repository instructions in `.AGENTS/AGENTS.md` require focused integration
tests for behavior changes, rustfmt-clean Rust 2024 code, and PR descriptions
with exact verification commands. The narrower QP101 format-sync rules do not
apply because this design does not change the QP101 schema or exporter format.

## Goals

- Add `--sample_shot` and `--seed` flags to `rstim render_svg`.
- Keep `--seed` valid only when `--sample_shot` is present.
- Keep `--sample_shot` incompatible with `--highlight_dem_error` for option
  parity with `export_json`. Since `render_svg` does not expose
  `--highlight_dem_error` in this issue, this is preserved by not adding that
  flag and by keeping shared validation ready for future options.
- Reuse `Executor::run_with_trace` and `export_qp101_with_sample_trace` for
  sampled QP101 construction.
- Preserve safe output behavior: invalid sample options or sample-export errors
  must not create, truncate, or replace the requested SVG output before the
  command fails.
- Ensure the SVG renderer emits deterministic annotation text containing
  annotation `label` and `text`.
- Ensure the renderer exposes annotation style presets in a deterministic,
  text-inspectable way, for example as SVG classes and data attributes.
- Add a seeded CLI acceptance test named
  `render_svg_sample_shot_draws_seeded_annotations`.
- Cover the negative control where `rstim render_svg --seed 7 --out <existing>`
  fails without `--sample_shot` and leaves the existing output unchanged.

## Non-Goals

- Do not add batch shot visualization.
- Do not infer sample semantics in `qp101_svg`.
- Do not add interactive shot exploration.
- Do not add `render_svg --highlight_dem_error`; issue #173 owns DEM-origin SVG
  rendering.
- Do not change the QP101 JSON schema.
- Do not update Typst fixtures, because this is a built-in SVG CLI path.

## Approaches Considered

### 1. Share a QP101 document builder between `export_json` and `render_svg`

Refactor `cli.rs` around a private `build_qp101_document` helper that accepts
the parsed `StimInstr` list plus visualization options. `export_json` and
`render_svg` both call this helper. For `sample_shot`, the helper runs
`Executor::run_with_trace` and `export_qp101_with_sample_trace`; for plain mode,
it delegates to `build_plain_qp101_document`.

Benefits:

- matches issue #172's recommendation to reuse the sample export path
- avoids renderer-side sampling logic
- keeps option compatibility in one place for `export_json` and `render_svg`
- preserves `render_svg` safe output by rendering to a string before opening
  `--out`
- keeps the change localized to the CLI and renderer annotation formatting

Costs:

- introduces one small CLI option struct, but it reduces duplication as
  visualization options grow

This is the chosen approach.

### 2. Duplicate the sample-shot branch inside `run_render_svg_to_string`

Add `sample_shot` and `seed` parameters to `run_render_svg_to_string`, then
copy the sample branch from `run_export_json`.

Benefits:

- fewer edits in the short term

Costs:

- duplicates compatibility checks and sample-export error rewriting
- increases drift risk between `export_json` and `render_svg`
- makes issue #173 harder because another visualization option would need more
  duplication

This is rejected.

### 3. Sample inside `qp101_svg`

Pass raw Stim text or simulator traces into the renderer and let
`qp101_svg::render_svg` compute sample markers.

Benefits:

- could keep CLI helper signatures small

Costs:

- directly conflicts with issue #172's requirement to consume QP101
  annotations instead of recomputing sample semantics
- entangles the renderer with simulator semantics
- makes annotation-only QP101 documents impossible to render consistently

This is rejected.

## CLI Design

Add sample flags to the `RenderSvg` command:

```rust
#[command(name = "render_svg")]
RenderSvg {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long = "sample_shot")]
    sample_shot: bool,
    #[arg(long)]
    seed: Option<u64>,
}
```

Introduce a private options struct in `cli.rs`:

```rust
#[derive(Clone, Copy)]
struct Qp101BuildOptions {
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
}
```

Then add a helper:

```rust
fn build_qp101_document(
    instrs: &[crate::ir::StimInstr],
    options: Qp101BuildOptions,
) -> Result<crate::qp101::Qp101Document, String>
```

The helper owns the compatibility rules:

- if `options.seed.is_some() && !options.sample_shot`, return
  `--seed is only supported with --sample_shot`
- if `options.sample_shot && options.highlight_dem_error.is_some()`, return
  `--sample_shot cannot be combined with --highlight_dem_error`

The helper should keep the existing export error message rewrites:

- unsupported highlight tracking instruction errors become the existing
  `--highlight_dem_error currently supports a subset of noise instructions:
  ...` message
- out-of-range highlight indices become the existing
  `DEM error index out of range: ...` message
- unsupported sample visualization instruction errors become the existing
  `--sample_shot currently supports a subset of sample visualization
  instructions: ...` message
- other export errors pass through unchanged

`run_export_json` should parse once, call `build_qp101_document`, then serialize.
`run_render_svg_to_string` should parse once, call `build_qp101_document`, then
call `crate::qp101_svg::render_svg`.

The `render_svg` dispatcher must preserve the existing safe-output ordering:

```rust
let text = read_input(r#in.as_deref())?;
let svg = run_render_svg_to_string(&text, options)?;
let mut w = open_output(out.as_deref())?;
w.write_all(svg.as_bytes())
    .map_err(|e| format!("write error: {e}"))
```

This means invalid `--seed` usage and sample-export failures happen before the
output file is opened.

## Renderer Annotation Design

The renderer already emits annotation text by joining `kind`, `label`, and
`text`. Keep that visible text stable so existing tests and users can inspect
markers such as:

- `marker: X`
- `marker: L`
- `marker: 1[L]`
- `marker: L=1 | M=1[L]`
- `marker: D0`

Extend the annotation text element to include deterministic style metadata when
`annotation.style` is present:

- `class="annotation annotation-preset-danger"` for preset `danger`
- `class="annotation annotation-preset-info"` for preset `info`
- `data-style-preset="danger"` or `data-style-preset="info"`
- `data-style-highlight="true"` or `false` when `highlight` is present

Use the style color if present for SVG `fill`, otherwise keep the existing
annotation color. Escape class/data attribute values with the existing XML text
escaping helper. Do not parse or branch on annotation `context`; the renderer
should remain a QP101 annotation consumer, not a sample semantics engine.

## Testing

Add `render_svg_sample_shot_draws_seeded_annotations` to
`rstim/tests/cli_render_svg.rs`.

The test should:

- run `rstim render_svg --sample_shot --seed 7` on the small fixture from the
  existing `export_json` seeded sample test
- use stdin for one run and `--in`/`--out` for a second run
- assert output starts with `<svg`
- assert base circuit labels remain visible, including `q0`, `D1`, `LOSS`,
  `M`, `MRL`, and `DETECTOR`
- assert sample-specific semantic markers are visible, including `marker: X`,
  `marker: L`, `marker: 1[L]`, `marker: L=1 | M=1[L]`, and `marker: D0`
- assert style metadata is visible for at least danger and info annotation
  presets
- assert the repeated file output is byte-identical for the same seed and input

The same test should cover the negative control:

- create an existing output file containing `existing svg should remain`
- run `rstim render_svg --seed 7 --out <same-output>` without `--sample_shot`
  using a valid circuit on stdin
- assert the command exits nonzero
- assert stderr contains `--seed is only supported with --sample_shot`
- assert the output file still contains `existing svg should remain`

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_sample_shot_draws_seeded_annotations -q
cargo test -p rstim --test cli_render_svg -q
cargo test
rustfmt --check rstim/src/cli.rs rstim/src/qp101_svg.rs rstim/tests/cli_render_svg.rs
git diff --check
```
