# P96 APM Sparse Fixture Design

Scope: GitHub issue #133, pinned qec-code sparse-row fixtures for the P=96 APM CSS instance from arXiv:2604.16209 Table A1.

## Context

Issue #132 added `qec-code/tests/fixtures/apm/table_a1_manifest.json` with the Table A1 coefficients and dimensions for `apm_kasai:p=96` and `apm_kasai:p=192`. Issue #133 consumes that manifest and pins the reference P=96 matrices so later native Rust code can compare against known-answer `Hx` and `Hz` supports.

The paper defines the Kasai CSS construction in Appendix A. Equations (A1) and (A2) give the three retained block rows of `Hx` and `Hz`: `Hx` uses circulant rows of `F0..F5` followed by `G0..G5`, while `Hz` uses transposed reverse-circulant `G` blocks followed by transposed reverse-circulant `F` blocks. Table A1 gives the P=96 affine maps and the expected `[[1152,580,<=12]]` shape.

The local `drafts/` reference clone named in the issue is not present in this worktree. The generator therefore treats the checked-in manifest plus the paper's explicit block equations as the reproducible reference contract. It does not add a built-in APM code id or production generator API.

## Alternatives Considered

1. Add a production APM constructor in Rust and export fixtures from it.

   This would be convenient for future work but violates the issue's out-of-scope boundary. It would expose design choices before the native APM implementation issue exists.

2. Add only committed JSON fixtures with no regeneration helper.

   This would satisfy the output paths but would leave reviewers without a precise way to reproduce the supports from the manifest.

3. Add a fixture-local generator plus Rust verifier.

   This is selected. A small Python script under `qec-code/tests/fixtures/apm/` reads the P=96 manifest entry, applies the Appendix A block pattern, and writes compact `sparse_rows` JSON. A Rust test loads the committed fixtures and checks dimensions, degrees, orthogonality, and rank.

## Fixture Contract

Create these committed files:

- `qec-code/tests/fixtures/apm/p96_hx.json`
- `qec-code/tests/fixtures/apm/p96_hz.json`

Both files use the existing qec-code sparse-row JSON shape:

```json
{"format":"sparse_rows","num_cols":1152,"rows":[...]}
```

The P=96 fixture has `P = 96`, `J = 3`, `L = 12`, and `L2 = 6`. Each matrix has `J * P = 288` rows and `L * P = 1152` columns. Each row has weight 12 and each column has weight 3.

For an affine map `m(x) = ax + b mod P`, the generator represents a block `M` by putting the row-local one at column `m(x)`. For a transposed block `M^T`, the generator uses the inverse affine map. This convention gives the Appendix A `Hx` and `Hz` matrices with `Hx * Hz^T == 0 mod 2` and rank sum 572.

## Generator

Create `qec-code/tests/fixtures/apm/generate_p96_fixtures.py` with only Python standard-library dependencies.

The default regeneration command is:

```bash
python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py
```

The script reads `table_a1_manifest.json`, selects the `apm_kasai:p=96` entry, verifies the expected dimensions, builds the Appendix A block matrices, and writes the two fixture files. It also supports:

```bash
python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py --check
```

`--check` regenerates in memory and exits non-zero if either committed fixture differs.

## Test Contract

Add a qec-code integration test named `apm_p96_fixture_matches_reference_stats`. It loads `p96_hx.json` and `p96_hz.json`, then verifies:

- `num_cols == 1152`
- both matrices have 288 rows
- every row has weight 12
- every column has weight 3
- `Hx * Hz^T == 0 mod 2`
- `rank_x + rank_z == 572`, so `k = 1152 - 572 = 580`

Add a negative control in the same test module that mutates one support in memory, then asserts the same verifier rejects the mutated fixture through the degree, orthogonality, or rank checks.

The focused verification command is:

```bash
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
```

The broader Agent Desk gate is:

```bash
cargo test
```

## Out Of Scope

- No built-in `apm_kasai` code id.
- No public APM construction API.
- No P=192 matrix fixtures.
- No decoder benchmark or simulation work.

## Non-Interactive Approval

This Agent Desk run is non-interactive. The design approval gate is resolved by the Standing Answer Policy: choose the safest conservative option that satisfies the issue while avoiding public API compatibility risk and unrelated edits.
