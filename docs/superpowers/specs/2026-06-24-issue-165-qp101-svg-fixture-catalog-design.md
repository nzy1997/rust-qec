# Issue 165 QP101 SVG Fixture Catalog Design

Date: 2026-06-24
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #165, acceptance fixture catalog for the future built-in QP101 SVG renderer in `rstim`

## Summary

Issue #165 needs a shared, checked-in catalog of QP101 SVG renderer acceptance
fixtures. The renderer is out of scope. The deliverable is a manifest, small
input fixtures or pointers to existing fixtures, and a focused integration test
that proves the catalog is usable and rejects broken entries.

The catalog should live under:

```text
rstim/tests/fixtures/qp101_svg/
```

The test file should be:

```text
rstim/tests/qp101_svg_fixtures.rs
```

The primary test should be named:

```text
qp101_svg_fixture_manifest_is_valid
```

## Current State

`rstim` already has:

- `rstim/src/parser.rs` for Stim text parsing into `StimInstr`.
- `rstim/src/qp101.rs` for exporting and deserializing `Qp101Document`.
- QP101 JSON fixtures under `rstim/tests/fixtures/qp101/`.
- QP101 Typst renderer examples and checks under `qp101-viz/examples/` and
  `qp101-viz/checks/`.

The repository rules require documentation and Typst sync when QP101 JSON shape
or semantics change. This issue does not change `rstim/src/qp101.rs`, the
QP101 format, or `qp101-viz` behavior.

## Goals

- Add a manifest-driven fixture catalog with at least six cases.
- Include explicit `source_kind` values instead of inferring from file
  extensions.
- Support `stim` and `qp101_json` input kinds.
- Require every case to provide a stable `id`, non-empty `provenance`,
  `source_kind`, `input_path`, and non-empty semantic SVG marker list.
- Verify every input path exists and can be parsed by the parser selected by
  `source_kind`.
- Include negative controls for missing input paths, empty marker lists, and
  unsupported source kinds, with errors that name the bad case id.
- Keep expected output semantic rather than pixel-perfect.

## Non-Goals

- Do not implement the SVG renderer.
- Do not require SVG snapshots.
- Do not change QP101 JSON serialization.
- Do not replace or alter the Typst renderer.
- Do not generate large new circuit fixtures.

## Approaches Considered

### 1. JSON manifest with test-local validator

Create `rstim/tests/fixtures/qp101_svg/manifest.json` and parse it from a
focused integration test using `serde_json`. The validator lives in the test
file because no production renderer API needs this catalog yet.

Benefits:

- uses dependencies already present in `rstim`
- keeps issue #165 scoped to fixtures and validation
- lets future renderer tests reuse the same manifest path
- avoids committing a production API before the renderer exists

Costs:

- future renderer tests may later move shared helpers out of the test file

This is the recommended approach.

### 2. Rust-only inline manifest in the test

Represent the fixture catalog as a Rust array in
`rstim/tests/qp101_svg_fixtures.rs`.

Benefits:

- least file I/O
- compile-time field names

Costs:

- not a checked-in manifest that later issues can cite independent of Rust code
- harder for renderer documentation and tooling to consume

This is not recommended because the issue explicitly asks for a manifest.

### 3. YAML or TOML manifest

Use a human-friendly manifest format and add or reuse a parser.

Benefits:

- compact authoring syntax

Costs:

- `rstim` does not currently depend on YAML or TOML parsing
- adding a dependency for test fixture metadata is unnecessary

This is not recommended.

## Manifest Shape

The manifest should be:

```json
{
  "version": 1,
  "cases": [
    {
      "id": "basic_wires_gates_tick",
      "provenance": "Short human-readable source note.",
      "source_kind": "stim",
      "input_path": "basic_wires_gates_tick.stim",
      "expected_semantic_markers": [
        { "kind": "qubit_label", "value": "q0" },
        { "kind": "operation_label", "value": "H" }
      ]
    }
  ]
}
```

`input_path` is relative to the manifest directory. Relative paths may point to
existing fixtures outside `qp101_svg` when that avoids duplicating committed
QP101 JSON examples.

`expected_semantic_markers` are intentionally renderer-facing metadata. The
validator only requires the list and each marker's `kind` and `value` to be
non-empty. Future SVG renderer tests can decide how each marker kind maps to
SVG text, IDs, anchors, classes, or data attributes.

## Initial Catalog Cases

The initial manifest should cover:

- `basic_wires_gates_tick`: small Stim fixture with qubit coordinates, H/CX,
  tick, and measurements.
- `measurement_detector_source`: small Stim fixture with measurement followed
  by `DETECTOR rec[-1]`.
- `observable_include_source`: small Stim fixture with measurement followed by
  `OBSERVABLE_INCLUDE(0) rec[-1]`.
- `repeat_repeated_measurements`: small Stim fixture with a `REPEAT` block that
  measures inside the body.
- `noise_operation_rendering`: existing `qp101-viz/checks/noise-render.qp101.json`
  fixture so the catalog reuses the Typst renderer's noise check.
- `sample_shot_overlay`: existing
  `rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json`
  fixture so the catalog points to the seeded sample overlay already kept in
  sync by `qp101_fixtures`.

This gives both supported source kinds coverage and keeps new fixture text
small.

## Validation

`rstim/tests/qp101_svg_fixtures.rs` should define test-local deserialization
types for the manifest, case, and marker structures.

Validation rules:

- manifest version must be `1`
- at least six cases must be present
- ids must be non-empty, stable ASCII slugs and unique
- provenance must be non-empty
- source kind must be exactly `stim` or `qp101_json`
- input path must be non-empty, relative, and must exist when joined to the
  manifest directory
- `stim` paths must parse with `rstim::parser::parse_lines`
- `qp101_json` paths must deserialize as `rstim::qp101::Qp101Document`
- expected semantic markers must be non-empty
- every marker must have non-empty `kind` and `value`

Negative controls should mutate valid manifest entries in memory and assert the
validator rejects:

- missing input path
- empty expected marker list
- unsupported source kind

Each rejected error must include the bad case id.

## Testing

Run the issue verification:

```sh
cargo test -p rstim --test qp101_svg_fixtures qp101_svg_fixture_manifest_is_valid -q
```

Run finish gates:

```sh
cargo test -p rstim --test qp101_svg_fixtures -q
cargo test
git diff --check
```

If the full workspace `cargo test` cannot run because the sandbox blocks
crates.io index access, record the exact failure and rely on the focused
`rstim` verification that compiles from cached dependencies.
