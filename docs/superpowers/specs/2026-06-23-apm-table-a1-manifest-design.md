# APM Table A1 Manifest Design

Scope: GitHub issue #132, qec-code test fixture manifest for arXiv:2604.16209 Table A1 APM CSS instances.

## Context

The repository already keeps source-grounded qec-code fixtures under `qec-code/tests/fixtures/` and validates built-in CSS code behavior from `qec-code/tests/code.rs`. Issue #132 asks for a checked-in JSON manifest for the two finite-size APM CSS instances from Table A1 of arXiv:2604.16209: `P = 96` and `P = 192`.

The issue is intentionally fixture-only. It must not implement Delta/Gamma generation, APM matrix generation, or decoder benchmarks. The local `drafts/` reference clone named in the issue is not present in this worktree, so the source-grounded data comes from the arXiv paper text and the checked-in qec-code fixture/test patterns.

## Chosen Approach

Use a simple JSON manifest at `qec-code/tests/fixtures/apm/table_a1_manifest.json`, with one entry per code id:

- `apm_kasai:p=96`
- `apm_kasai:p=192`

Each entry stores direct Table A1 fields as first-class keys: `P`, `J`, `L`, `L2`, affine `f` and `g` maps, expected code dimensions, girth status, noncommutativity pairs, and references. Derived expectations such as `mx`, `mz`, row weights, and column weights are nested under explicit expectation objects so future generator tests can distinguish paper data from values derived from the Kasai block template.

## Alternatives Considered

1. Add production Rust types for an APM manifest parser.

   This would create public or semi-public API before native APM generation exists. It is unnecessary for a pinned fixture and increases compatibility risk.

2. Generate the APM matrices now and validate the manifest against real `Hx` and `Hz`.

   This is out of scope for issue #132 and belongs to the later generator work. It would also blur whether this PR is pinning reference parameters or implementing construction logic.

3. Add only JSON plus test-local validation.

   This is the selected approach. It keeps the manifest easy to deserialize with existing `serde_json`, avoids new dependencies, and gives later issues a stable fixture contract.

## Manifest Contract

Each entry includes:

- `code_id`: stable fixture id.
- `P`, `J`, `L`, `L2`: APM block and template dimensions.
- `f`: six affine maps with keys `i`, `a`, and `b`, representing `f_i(x) = a_i x + b_i mod P`.
- `g`: six affine maps with keys `i`, `c`, and `d`, representing `g_i(x) = c_i x + d_i mod P`.
- `expected_code_shape`: `n`, `mx`, `mz`, `k`, `rate`, and distance upper-bound status.
- `expected_weights`: row and column weights implied by the `J = 3`, `L = 12` truncated Kasai template.
- `girth`: lower-bound status, because the paper states all Table A1 codes have girth at least 6.
- `required_commuting_pairs`: explicit source-backed column-component pairs that should commute modulo `P / 3` for the P=96 and P=192 structural movement constraints.
- `required_noncommuting_pairs`: the Table A1 caption pairs `(0, 3)` and `(1, 2)`.
- `structural_expectations`: stable hints later generator tests can consume, including the active row count, block-column count, row/column weight expectations, and Appendix D column-component group status.
- `provenance`: paper/table references and separate `source_grounded_fields` and `derived_fields` lists.
- `references`: URLs and local reference paths when available.

Distance is encoded as an upper bound, not as an exact theorem statement, because Appendix A states that `Jn, k, d <= dubK` reports a best-found upper bound rather than an exact distance determination.

## Validation

Add test-local helper code in `qec-code/tests/code.rs` that:

- Loads the JSON with `include_str!`.
- Requires exactly the two expected code ids.
- Checks the exact Table A1 coefficients for both `f` and `g`.
- Checks `P`, `J`, `L`, `L2`, `n`, `mx`, `mz`, `k`, row weights, column weights, girth status, required pair fields, structural expectation fields, provenance, and references.
- Verifies affine multipliers are invertible modulo `P`.
- Mutates one in-memory affine coefficient and asserts validation fails with an error message that mentions the code id and changed coefficient.

The focused verification command is:

```bash
cargo test -p qec-code apm_table_a1_manifest -q
```

The broader repository gate requested by Agent Desk is:

```bash
cargo test
```

## Out Of Scope

- Delta/Gamma generation.
- Native APM matrix generation.
- Decoder benchmark fixtures.
- Public Rust manifest parser API.
