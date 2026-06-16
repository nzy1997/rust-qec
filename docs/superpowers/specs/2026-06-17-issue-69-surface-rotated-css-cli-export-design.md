# Issue 69 Surface Rotated CSS CLI Export Design

Date: 2026-06-17
Status: Design approved in-session, written for review
Scope: GitHub issue #69, `qec-code code css surface_rotated:d=3 hx|hz`

## Summary

Issue #69 asks the existing built-in CSS export CLI to support the
parameterized rotated-surface family through the same command shape used by
fixed built-ins:

```text
qec-code code css surface_rotated:d=3 hx
qec-code code css surface_rotated:d=3 hz
```

The production path is already present after the dependency chain landed:

- #56 added `qec-code code css <code-id> <hx|hz>`.
- #68 added the `surface_rotated:d=<distance>` registry family.
- #61 added qec-code-owned pinned CLI fixtures and a manifest sweep.
- #60 added `code css list` and an explicit `code css export` subcommand while
  preserving the positional export form.

The remaining issue #69 work should therefore be a CLI regression and fixture
completion pass. It should pin the `surface_rotated:d=3` `hx` and `hz` stdout
as workspace fixtures and add the issue-requested CLI tests. Production code
should change only if those tests expose a real mismatch with the interface.

## Goals

- Add qec-code-owned pinned fixtures for `surface_rotated:d=3` `hx` and `hz`.
- Extend the existing built-in CSS CLI fixture manifest with the two
  rotated-surface entries.
- Add focused CLI tests named:
  - `code_css_surface_rotated_d3_hx_prints_workspace_fixture`
  - `code_css_surface_rotated_d3_hz_prints_workspace_fixture`
  - `code_css_surface_rotated_missing_or_bad_distance_fails`
- Verify malformed rotated-surface CLI inputs fail clearly instead of printing
  a fake matrix.
- Preserve the existing command surface and output contract.

## Non-Goals

- Do not add a new top-level CLI command.
- Do not add JSON output that combines `hx` and `hz`.
- Do not change the rotated-surface registry geometry.
- Do not update README or user-facing docs.
- Do not refactor `CssArgs`, `BuiltInCssChecks`, or `SparseRowsMatrix` unless a
  test exposes a necessary bug fix.
- Do not move existing fixtures into `rsinter`.

## Current State

`qec-code/src/cli.rs` routes built-in CSS exports through:

```text
CssArgs
  -> run_css_args(...)
  -> run_css(code_id, matrix)
  -> built_in_css_checks(code_id)
  -> SparseRowsMatrix::to_json_string()
```

`qec-code/src/codes/built_in_css.rs` already recognizes
`surface_rotated:d=<distance>`, rejects `distance < 2`, and returns canonical
`BuiltInCssChecks` rows.

`qec-code/tests/cli.rs` already has:

- direct Steane fixture tests using the older `rsinter` fixture location
- a qec-code-owned `BUILT_IN_CSS_FIXTURE_CASES` manifest
- `built_in_css_fixture_manifest_exports_match_pinned_json`
- list/export regression tests from #60

The missing issue #69 pieces are:

- no qec-code-owned `surface_rotated:d=3` fixtures
- no manifest entries for `surface_rotated:d=3`
- no issue-named CLI tests for `surface_rotated:d=3`
- no grouped CLI test for missing, non-integer, out-of-range, and bad selector
  rotated-surface inputs

## Alternatives Considered

### 1. Fixture and CLI-test completion

Add two fixture files, extend the existing manifest, and add the issue-named
CLI tests.

Benefits:

- matches the current codebase shape
- reuses the #61 fixture-manifest pattern
- keeps the change small and easy to review
- verifies the issue at the binary CLI boundary

Costs:

- duplicates the registry-level `d=3` expected rows in fixture form, but this is
  useful because issue #69 is about CLI stdout drift.

This is the recommended approach.

### 2. Refactor the CLI while adding tests

Use issue #69 as a chance to reorganize `CssArgs`, matrix selector handling, or
error rendering.

Benefits:

- could tidy some recently changed CLI code

Costs:

- broadens the issue beyond its requested behavior
- risks touching the #60 command-shape work unnecessarily
- makes the fixture-only acceptance harder to review

This is not recommended.

### 3. Add only the three issue-named tests

Write the specific tests from the issue but leave the shared manifest alone.

Benefits:

- smallest test diff

Costs:

- bypasses the manifest sweep added specifically for built-in CSS CLI fixture
  coverage
- makes future drift protection less uniform

This is not recommended.

## Decision

Use the fixture and CLI-test completion approach.

Keep all production code unchanged unless the new tests fail for a real product
reason. Add issue #69 coverage under `qec-code/tests/cli.rs` and
`qec-code/tests/fixtures/css/`.

## Fixture Files

Create:

```text
qec-code/tests/fixtures/css/surface_rotated_d3_hx.json
qec-code/tests/fixtures/css/surface_rotated_d3_hz.json
```

The expected file contents are compact `sparse_rows` JSON plus one trailing
newline, matching the binary CLI writer:

```json
{"format":"sparse_rows","num_cols":9,"rows":[[0,3],[1,2,4,5],[3,4,6,7],[5,8]]}
```

```json
{"format":"sparse_rows","num_cols":9,"rows":[[1,2],[0,1,3,4],[4,5,7,8],[6,7]]}
```

The test should read these as pinned files. It should not regenerate them at
test time.

## CLI Test Design

Extend `BUILT_IN_CSS_FIXTURE_CASES` with:

```rust
BuiltInCssFixtureCase {
    code_id: "surface_rotated:d=3",
    matrix: "hx",
    fixture: "surface_rotated_d3_hx.json",
},
BuiltInCssFixtureCase {
    code_id: "surface_rotated:d=3",
    matrix: "hz",
    fixture: "surface_rotated_d3_hz.json",
},
```

Add explicit issue-named tests:

```rust
#[test]
fn code_css_surface_rotated_d3_hx_prints_workspace_fixture() { ... }

#[test]
fn code_css_surface_rotated_d3_hz_prints_workspace_fixture() { ... }
```

Each test should:

1. Run the binary command with `run_qec_code(...)`.
2. Assert success.
3. Assert empty stderr.
4. Compare stdout byte-for-byte against the qec-code-owned fixture.

Add:

```rust
#[test]
fn code_css_surface_rotated_missing_or_bad_distance_fails() { ... }
```

This test should cover representative failure cases:

- `code css surface_rotated hx`
- `code css surface_rotated:d=nope hx`
- `code css surface_rotated:d=1 hx`
- `code css surface_rotated:d=3 foo`

Each case should assert:

- non-zero exit status
- empty stdout
- stderr contains a focused fragment such as `missing built-in CSS parameter d`,
  `invalid built-in CSS integer parameter d`, `out-of-range built-in CSS
  integer parameter d`, or `invalid value 'foo'`

The selector case is clap-owned, so the assertion should allow normal clap
usage text as long as the invalid selector is reported and no stdout matrix is
printed.

## Data Flow

Successful CLI export:

```text
qec-code code css surface_rotated:d=3 hx|hz
  -> clap parses positional CSS export
  -> run_css(...)
  -> built_in_css_checks("surface_rotated:d=3")
  -> BuiltInCssChecks { num_cols: 9, hx, hz }
  -> select requested matrix
  -> SparseRowsMatrix validates rows
  -> compact sparse_rows JSON stdout
  -> test compares stdout to pinned fixture
```

Failure cases:

```text
surface_rotated hx
  -> registry parser reports missing d

surface_rotated:d=nope hx
  -> registry parser reports invalid integer

surface_rotated:d=1 hx
  -> family generator reports out-of-range distance

surface_rotated:d=3 foo
  -> clap rejects matrix selector before run_css(...)
```

## Error Handling

No new `QecError` variants are needed.

The tests should check stable, user-relevant fragments rather than entire stderr
messages, because clap may include usage text around selector errors. For
registry errors, the current `main.rs` error rendering is concise enough to
check exact identifying fragments.

## Verification

Run the issue-requested focused tests:

```bash
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hz_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_missing_or_bad_distance_fails
```

Run nearby CLI coverage:

```bash
cargo test -p qec-code --test cli code_css_
```

Run package and formatting checks:

```bash
cargo test -p qec-code
cargo fmt --check --package qec-code
```

## Acceptance Criteria

- `qec-code code css surface_rotated:d=3 hx` prints the pinned `hx`
  `sparse_rows` fixture and exits `0`.
- `qec-code code css surface_rotated:d=3 hz` prints the pinned `hz`
  `sparse_rows` fixture and exits `0`.
- The shared built-in CSS fixture manifest includes both rotated-surface
  `d=3` entries.
- Missing `d`, non-integer `d`, and `d < 2` fail with clear stderr and no
  stdout matrix.
- Unknown matrix selectors beyond `hx` and `hz` are rejected by clap with no
  stdout matrix.
- Existing built-in CSS CLI exports continue to pass.
