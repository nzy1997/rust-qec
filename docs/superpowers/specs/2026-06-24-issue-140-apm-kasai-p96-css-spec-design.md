# Issue 140 APM Kasai P=96 CSS Spec Design

Issue: #140 Register built-in CSS spec for APM Kasai P=96

## Context

`qec-code` already has a native APM CSS matrix builder in `qec-code/src/codes/apm.rs` and pinned Table A1 manifest/fixture coverage for `apm_kasai:p=96`. The missing piece is the built-in CSS registry entrypoint used by `built_in_css_checks`, `qec-code code css`, and `qec-code code css list`.

No project `AGENTS.md` or `CONVENTIONS.md` is present in this checkout. Issue #140 is open, has no comments, and no existing matching PR was found.

## Approach

Register a single fixed APM family id, `apm_kasai:p=96`, in the existing built-in CSS registry. The registry should parse `apm_kasai:p=<P>` but only build P=96 for this issue. It should call the existing APM builder with pinned Table A1 P=96 constants rather than reading test fixtures or duplicating sparse-row JSON.

Unsupported P values stay explicit. `apm_kasai:p=128` and `apm_kasai:p=192` should both fail with an error that names the requested P value, the currently supported value P=96, and notes that P=192 is tracked by #143.

## Chosen Design

Add an `ApmKasai` built-in CSS family with `BuiltInCssParams::ApmKasai { p }`. The parser accepts only the `p` parameter for this family and keeps the existing parser behavior for missing, duplicate, invalid, and unexpected parameters.

Add `apm_kasai:p=96` to `built_in_css_catalog()` as a concrete supported spec. Do not list a generic `apm_kasai:p=<P>` family because only P=96 is available in this issue.

Build P=96 by constructing an `ApmCssManifestEntry` from the Table A1 constants already pinned in the test manifest:

- `P = 96`
- `J = 3`
- `L = 12`
- `f = [(5,41), (85,77), (73,66), (1,0), (1,72), (37,9)]`
- `g = [(61,15), (1,24), (89,62), (25,22), (85,93), (25,78)]`

## Error Handling

Add a focused `QecError` variant for unsupported built-in CSS integer parameter values. This keeps the CLI error stable and avoids mislabeling an unsupported but syntactically valid P value as an unknown family or a generic range error.

Internal APM builder failures should be wrapped as a built-in CSS build failure with the code id and underlying reason. These failures are not expected for the pinned constants, but the conversion keeps the API total without panics.

## Testing

Add the issue's required CLI regression test named `apm_kasai_css_export` in `qec-code/tests/cli.rs`. It should assert:

- `qec-code code css list` includes `apm_kasai:p=96`
- `qec-code code css list` excludes `apm_kasai:p=192`
- `qec-code code css apm_kasai:p=96 hx` emits sparse-row JSON with `num_cols = 1152`
- `qec-code code css apm_kasai:p=96 hz` emits sparse-row JSON with `num_cols = 1152`
- `qec-code code css apm_kasai:p=128 hx` fails and names unsupported P=128 plus supported P=96
- `qec-code code css apm_kasai:p=192 hx` remains unsupported and mentions #143

Update existing catalog/parser unit tests whose expected supported spec list changes.

## Out of Scope

- Do not register `apm_kasai:p=192`
- Do not add rsinter benchmark fixtures
- Do not add decoding smoke tests
- Do not change the APM matrix builder's construction logic
