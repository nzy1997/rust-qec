# Visualization Update Flow

## Scope

Follow this process whenever `rstim export_json` changes the QP101-ZY JSON shape or semantics.

## 1. Update Rust output first

- Core exporter: `rstim/src/qp101.rs`
- CLI export path: `rstim/src/cli.rs`
- Provenance and highlight metadata when relevant: `rstim/src/dem_provenance.rs`

Stabilize the emitted JSON before touching docs, fixtures, or Typst rendering.

## 2. Update the JSON format documentation

- The format reference lives in `rstim/doc/QP101-ZY.md`.
- Update it for any added, removed, renamed, or reinterpreted field.
- Do not update tests and examples without updating the format document.

## 3. Update Rust tests and fixtures

- Structure tests: `rstim/tests/qp101_export.rs`
- CLI export tests: `rstim/tests/cli_export_json.rs`
- Fixture parity tests: `rstim/tests/qp101_fixtures.rs`
- Stored fixtures: `rstim/tests/fixtures/qp101/`

If the JSON changes, tests and fixtures must move together.

## 4. Update the Typst visualization package

- Renderer entrypoint: `qp101-viz/lib.typ`
- Package docs: `qp101-viz/README.md`
- Committed examples: `qp101-viz/examples/`
- Render checks: `qp101-viz/checks/`

Any change to `type`, `annotations`, `raw_targets`, `target_slots`, `context`, `detector`, or `observable_include` requires a review of the corresponding field reads in `qp101-viz/lib.typ`.

## 5. Minimum verification

Run:

```sh
cargo test -p rstim --test qp101_export --test qp101_fixtures --test cli_export_json
```

Then compile at least one affected Typst example:

```sh
typst compile --root qp101-viz qp101-viz/examples/<example>.typ /tmp/out.pdf
```

If renderer behavior changed, also run:

```sh
make -C qp101-viz
```
