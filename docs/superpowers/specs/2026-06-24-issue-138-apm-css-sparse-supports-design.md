# Issue 138 APM CSS Sparse Supports Design

Scope: GitHub issue #138, native crate-private APM-CSS sparse support
construction for `qec-code`.

## Context

Issue #132 added `qec-code/tests/fixtures/apm/table_a1_manifest.json` with
the Table A1 APM entries. Issue #133 added exact P=96 sparse-row fixtures.
Issues #135 through #137 added crate-private affine permutation arithmetic,
commutation checks, and Delta/Gamma active-row construction in
`qec-code/src/codes/apm.rs`.

This issue builds the next layer: use one validated APM manifest entry
(`P`, `J`, `L`, `f_i`, and `g_i`) to assemble `Hx` and `Hz` sparse row
supports. The output remains internal and must not register a CLI or catalog
spec for `apm_kasai:p=96`.

The local `drafts/` reference clone named by the issue is not present in this
worktree, so the implementation source of truth is the committed fixture
generator `qec-code/tests/fixtures/apm/generate_p96_fixtures.py`, the APM
contract doc, and the exact fixture files from #133.

## Approaches Considered

1. Add APM construction directly in `built_in_css.rs`.

   This would sit near `bivariate_bicycle_checks`, but it would mix APM
   algebra with the built-in registry and parsing code before a public
   `apm_kasai` code id exists.

2. Add a crate-private builder in `qec-code/src/codes/apm.rs`.

   This is the selected approach. The file already owns affine maps,
   commutation validation, and active-row sets. Keeping construction there
   avoids public API churn while letting later built-in registration call one
   focused helper.

3. Read and return the pinned fixture JSON as the implementation.

   This would pass exact equality but would not implement the native algorithm
   the issue asks for. It also would not exercise the affine helpers from the
   dependency issues.

## Chosen Design

Add a crate-private manifest-entry data shape and builder in
`qec-code/src/codes/apm.rs`:

- `ApmCssManifestEntry` stores `code_id`, `p`, `j`, `l`, and the validated
  `f`/`g` affine families.
- `ApmCssBuildError` reports invalid active-row dimensions, family length
  mismatches, modulus mismatches, and unsupported values that cannot fit in
  Rust `usize`.
- `build_apm_css_checks(&ApmCssManifestEntry) -> Result<BuiltInCssChecks, ApmCssBuildError>`
  returns `num_cols = L * P`, `J * P` X rows, and `J * P` Z rows.

The row construction matches the pinned fixture generator:

- `Hx`: for each block row `r in 0..J`, local row `x in 0..P`, and block
  column `c in 0..L`, use `f[(c mod L2 - r) mod L2]` for the first half of
  block columns and `g[(c mod L2 - r) mod L2]` for the second half.
- `Hz`: use inverse `g` blocks in the first half and inverse `f` blocks in
  the second half, with family index `(r - c mod L2) mod L2`.
- Each row is sorted and deduplicated before returning.

The builder validates `L` through `build_apm_active_row_sets`, requires
`L = 2 * L2`, requires both affine families to have exactly `L2` maps, and
requires every map modulus to equal `P`. It returns
`BuiltInCssChecks { code_id, num_cols, hx, hz }` but does not register that
code id in the public built-in CSS parser or CLI.

## Test Contract

Add unit tests in `qec-code/src/codes/apm.rs` because the builder is
crate-private:

- `apm_p96_builds_expected_hx_hz` loads the P=96 manifest entry, builds the
  native supports, and asserts exact equality with
  `qec-code/tests/fixtures/apm/p96_hx.json` and `p96_hz.json`.
- The same test verifies `num_cols == 1152`,
  `hx.len() == hz.len() == 288`, and canonical sorted/deduplicated supports.
- The negative control builds an in-memory wrong-Hz variant that uses
  forward blocks instead of inverse/transpose blocks, then asserts it differs
  from the fixture and fails `Hx * Hz^T == 0 mod 2`.

Focused verification:

```sh
cargo test -p qec-code apm_p96_builds_expected_hx_hz -q
```

Full verification:

```sh
cargo test
```

## Out Of Scope

- No CLI or catalog registration for `apm_kasai:p=96`.
- No P=192 fixture generation.
- No public manifest parser API.
- No decoder benchmark integration.
