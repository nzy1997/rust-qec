# Issue 72 And 73 Toric CSS CLI Fixtures Design

Date: 2026-06-18
Status: Design approved in-session, written for review
Scope: GitHub issues #72 and #73, `qec-code code css toric:d=3 hx|hz`

## Summary

Issue #72 asks the existing built-in CSS export CLI to support the toric family
through the same command shape used by fixed built-ins and other parameterized
families:

```text
qec-code code css toric:d=3 hx
qec-code code css toric:d=3 hz
```

Issue #73 asks the shared built-in CSS fixture manifest sweep to include
representative toric exports so future registry or CLI changes cannot silently
alter emitted matrices.

The production path is already present:

- #56 added `qec-code code css <code-id> <hx|hz>`.
- #61 added qec-code-owned pinned CLI fixtures and a manifest sweep.
- #71 added the `toric:d=<distance>` registry family and exact `d=3` generator
  coverage.
- `qec-code code css toric:d=3 hx|hz` already emits compact `sparse_rows` JSON
  through the existing CLI path.

The remaining work should therefore be a test-only CLI regression and fixture
completion pass. It should pin the `toric:d=3` `hx` and `hz` stdout as
workspace fixtures, add issue #72 focused CLI tests, and add the same exports
to the issue #73 manifest sweep. Production code should change only if these
tests expose a real mismatch with the documented interface.

## Goals

- Add qec-code-owned pinned fixtures for `toric:d=3` `hx` and `hz`.
- Extend the existing `BUILT_IN_CSS_FIXTURE_CASES` manifest with both toric
  entries.
- Add focused binary CLI tests named:
  - `code_css_toric_d3_hx_prints_workspace_fixture`
  - `code_css_toric_d3_hz_prints_workspace_fixture`
  - `code_css_toric_missing_or_bad_distance_fails`
- Verify malformed toric CLI inputs fail clearly instead of printing a fake
  matrix.
- Preserve the existing command surface and output contract.
- Complete issue #73 as part of issue #72 because both issues use the same
  toric `d=3` pinned fixture data.

## Non-Goals

- Do not add a new top-level CLI command.
- Do not add JSON output that combines `hx` and `hz`.
- Do not change toric registry geometry or indexing.
- Do not add `toric:d=4` fixture-manifest entries.
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
`toric:d=<distance>`, rejects `distance < 2`, and returns canonical
`BuiltInCssChecks` rows. The `qec-code/tests/code.rs` toric tests already pin
the exact `d=3` generator rows and `d=4` shape and weight constraints.

`qec-code/tests/cli.rs` already has:

- direct Steane fixture tests using the older `rsinter` fixture location
- a qec-code-owned `BUILT_IN_CSS_FIXTURE_CASES` manifest
- `built_in_css_fixture_manifest_exports_match_pinned_json`
- `surface_rotated:d=3` fixture and malformed-input coverage from the issue #69
  pattern
- list/export regression tests from #60

The missing issue #72 and #73 pieces are:

- no qec-code-owned `toric:d=3` fixtures
- no manifest entries for `toric:d=3`
- no issue-named CLI tests for `toric:d=3`
- no grouped CLI test for missing, non-integer, out-of-range, and bad selector
  toric inputs

## Alternatives Considered

### 1. Complete #72 and #73 together

Add two fixture files, extend the existing manifest, and add the issue #72
focused CLI tests.

Benefits:

- matches the current #69 surface-rotated pattern
- avoids creating the same toric fixtures twice
- satisfies #73 with no extra production surface
- keeps review focused on one small CLI regression layer
- verifies the issue at the binary CLI boundary

Costs:

- slightly broadens issue #72 to close the adjacent manifest issue, but the
  implementation surface is identical.

This is the recommended approach.

### 2. Complete only #72

Add issue-named tests and fixtures but leave `BUILT_IN_CSS_FIXTURE_CASES`
unchanged.

Benefits:

- narrowest interpretation of the issue #72 verification commands

Costs:

- leaves #73 as a near-duplicate follow-up
- bypasses the manifest sweep that #61 created for this exact kind of export
  drift protection

This is not recommended.

### 3. Complete only #73

Add manifest entries and fixtures, relying on the current CLI path for #72.

Benefits:

- smallest diff if one treats #72 as already functionally satisfied by #71

Costs:

- does not add the issue #72 named success tests
- does not add the malformed-input CLI regression requested by #72

This is not recommended.

## Decision

Complete #72 and #73 together.

Keep all production code unchanged unless the new tests fail for a real product
reason. Add toric CLI fixture coverage under `qec-code/tests/cli.rs` and
`qec-code/tests/fixtures/css/`.

## Fixture Files

Create:

```text
qec-code/tests/fixtures/css/toric_d3_hx.json
qec-code/tests/fixtures/css/toric_d3_hz.json
```

The expected file contents are compact `sparse_rows` JSON plus one trailing
newline, matching the binary CLI writer:

```json
{"format":"sparse_rows","num_cols":18,"rows":[[0,2,9,15],[0,1,10,16],[1,2,11,17],[3,5,9,12],[3,4,10,13],[4,5,11,14],[6,8,12,15],[6,7,13,16],[7,8,14,17]]}
```

```json
{"format":"sparse_rows","num_cols":18,"rows":[[0,3,9,10],[1,4,10,11],[2,5,9,11],[3,6,12,13],[4,7,13,14],[5,8,12,14],[0,6,15,16],[1,7,16,17],[2,8,15,17]]}
```

The tests should read these as pinned files. They should not regenerate them at
test time.

## CLI Test Design

Extend `BUILT_IN_CSS_FIXTURE_CASES` with:

```rust
BuiltInCssFixtureCase {
    code_id: "toric:d=3",
    matrix: "hx",
    fixture: "toric_d3_hx.json",
},
BuiltInCssFixtureCase {
    code_id: "toric:d=3",
    matrix: "hz",
    fixture: "toric_d3_hz.json",
},
```

Add explicit issue #72 named tests:

```rust
#[test]
fn code_css_toric_d3_hx_prints_workspace_fixture() { ... }

#[test]
fn code_css_toric_d3_hz_prints_workspace_fixture() { ... }
```

Each test should:

1. Run the binary command with `run_qec_code(...)`.
2. Assert success.
3. Assert empty stderr.
4. Compare stdout byte-for-byte against the qec-code-owned fixture.

Add:

```rust
#[test]
fn code_css_toric_missing_or_bad_distance_fails() { ... }
```

This test should cover representative failure cases:

- `code css toric hx`
- `code css toric:d=nope hx`
- `code css toric:d=1 hx`
- `code css toric:d=3 foo`

Each case should assert:

- non-zero exit status
- empty stdout
- stderr contains a focused fragment such as `missing built-in CSS parameter d`,
  `invalid built-in CSS integer parameter d`, `out-of-range built-in CSS
  integer parameter d`, or `invalid value 'foo'`

The selector case is clap-owned, so the assertion should allow normal clap
usage text as long as the invalid selector is reported and no stdout matrix is
printed.

No new direct library tests are needed. Issue #71 already covers toric registry
generation and parser behavior. This work protects the binary CLI boundary and
the shared manifest sweep.

## Data Flow

Successful CLI export:

```text
qec-code code css toric:d=3 hx|hz
  -> clap parses positional CSS export
  -> run_css(...)
  -> built_in_css_checks("toric:d=3")
  -> BuiltInCssChecks { num_cols: 18, hx, hz }
  -> select requested matrix
  -> SparseRowsMatrix validates rows
  -> compact sparse_rows JSON stdout
  -> test compares stdout to pinned fixture
```

Manifest sweep:

```text
BUILT_IN_CSS_FIXTURE_CASES
  -> toric:d=3 / hx
  -> toric:d=3 / hz
  -> built_in_css_fixture_manifest_exports_match_pinned_json
  -> real qec-code binary invocation for each case
  -> byte-for-byte fixture comparison
```

Failure cases:

```text
toric hx
  -> registry parser reports missing d

toric:d=nope hx
  -> registry parser reports invalid integer

toric:d=1 hx
  -> family generator reports out-of-range distance

toric:d=3 foo
  -> clap rejects matrix selector before run_css(...)
```

## Error Handling

No new `QecError` variants are needed.

Tests should check stable, user-relevant stderr fragments rather than entire
stderr messages, because clap may include usage text around selector errors.
For registry errors, the current `main.rs` error rendering is concise enough to
check exact identifying fragments.

## Verification

Run the issue #72 focused tests:

```bash
cargo test -p qec-code --test cli code_css_toric_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_toric_d3_hz_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_toric_missing_or_bad_distance_fails
```

Run the issue #73 manifest sweep:

```bash
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
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

Optional manual negative check:

1. Temporarily change one toric pinned fixture row order or one exported toric
   support row.
2. Re-run:

```bash
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
```

3. Confirm the test fails on a byte-for-byte mismatch.
4. Revert the temporary mutation before committing implementation work.

## Acceptance Criteria

- `qec-code code css toric:d=3 hx` prints the pinned `hx` `sparse_rows`
  fixture and exits `0`.
- `qec-code code css toric:d=3 hz` prints the pinned `hz` `sparse_rows`
  fixture and exits `0`.
- The shared built-in CSS fixture manifest includes both toric `d=3` entries.
- Missing `d`, non-integer `d`, and `d < 2` fail with clear stderr and no
  stdout matrix.
- Unknown matrix selectors beyond `hx` and `hz` are rejected by clap with no
  stdout matrix.
- Existing built-in CSS CLI exports continue to pass.
- No files outside `qec-code/tests/cli.rs`,
  `qec-code/tests/fixtures/css/toric_d3_hx.json`,
  `qec-code/tests/fixtures/css/toric_d3_hz.json`, and the superpowers
  plan/spec docs are needed for implementation.
