# QP101-ZY Maintenance Rule

## Purpose

`rstim/doc/QP101-ZY.md` is the repository’s format contract for QP101-ZY JSON output. Exported JSON, tests, fixtures, and visualization code must stay aligned with it.

## When `rstim/doc/QP101-ZY.md` must be updated

- A top-level field is added, removed, or renamed
- An `operations` `type` or field name changes
- `annotations`, `style`, or `context` changes shape
- The JSON structure for `detector`, `observable_include`, `noise`, or `repeat` changes
- Field semantics change even if the field names stay the same

## Required update order

1. Change `rstim/src/qp101.rs`
2. Update `rstim/doc/QP101-ZY.md`
3. Update Rust tests and fixtures
4. Update `qp101-viz`

Do not skip step 2. The Typst package and committed examples should derive from the documented format, not from guesswork.

## Matching Typst package update points

When QP101-ZY output changes, review these files first:

- `qp101-viz/lib.typ`
- `qp101-viz/README.md`
- `qp101-viz/examples/*.qp101.json`
- `qp101-viz/checks/*.qp101.json`

`lib.typ` is the primary renderer sync point. Examples and checks prove that the new format still renders correctly.

## Pre-commit verification

```sh
cargo test -p rstim --test qp101_export --test qp101_fixtures --test cli_export_json
typst compile --root qp101-viz qp101-viz/examples/<example>.typ /tmp/out.pdf
```

If the renderer output changed, include an updated PNG or PDF in the PR or review notes.
