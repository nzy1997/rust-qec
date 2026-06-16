# Built-In CSS Fixture Manifest Design

Date: 2026-06-16
Status: Design approved in-session, written for review
Scope: GitHub issue #61, shared CLI fixture manifest sweep for built-in CSS exports in `qec-code`

## Summary

Issue #61 asks for one shared regression sweep that enumerates representative
built-in CSS exports and compares `qec-code` CLI stdout against pinned
`sparse_rows` JSON fixtures.

The goal is not to add new built-in families or new export behavior. The goal
is to protect the export surface that already exists after issues #54 through
#59:

- #54 added the built-in CSS registry.
- #55 added the `sparse_rows` JSON contract.
- #56 added `qec-code code css <code-id> <hx|hz>`.
- #57 added the code-spec parser.
- #58 added `repetition_x:d=<distance>` and `repetition_z:d=<distance>`.
- #59 added fixed `bb72` support.

This design adds a test-side manifest, qec-code-owned pinned fixtures, and one
binary CLI regression test. It deliberately remains a regression yardstick, not
a second registry.

## Goals

- Add one manifest-driven CLI test named
  `built_in_css_fixture_manifest_exports_match_pinned_json`.
- Keep the manifest small, explicit, and test-local.
- Compare CLI stdout byte-for-byte against pinned `sparse_rows` fixtures.
- Cover fixed ids, parameterized families, small matrices, empty-side matrices,
  and the larger `bb72` matrices.
- Make future issues such as #70 and #73 extend the same manifest by adding
  explicit entries and fixtures.
- Keep existing `rsinter` fixtures unchanged.

## Non-Goals

- Do not dynamically generate expected fixtures at test time.
- Do not add or change built-in CSS families.
- Do not add a registry listing command or JSON catalog.
- Do not add benchmark-spec generation.
- Do not move or rewrite existing `rsinter` fixtures.
- Do not replace existing family-specific tests. They still cover constructor
  details; this sweep covers end-to-end CLI drift.

## Current State

The current `qec-code` CLI path is:

```text
qec-code code css <code-id> <hx|hz>
```

It dispatches through `built_in_css_checks(...)`, selects one matrix, validates
it with `SparseRowsMatrix`, and writes compact `sparse_rows` JSON with one
trailing newline from the CLI writer.

Existing coverage is useful but scattered:

- `qec-code/tests/cli.rs` checks Steane `hx` and `hz` against the existing
  `rsinter/tests/fixtures/css` files.
- `qec-code/tests/cli.rs` has a `bb72` CLI smoke test that checks JSON shape,
  but not byte-for-byte pinned content.
- `qec-code/tests/code.rs` checks repetition-family library shapes, but not CLI
  stdout against fixtures.

Issue #61 should add the missing shared end-to-end fixture sweep.

## Fixture Ownership

The new #61 fixtures should live under:

```text
qec-code/tests/fixtures/css/
```

This keeps the producer crate responsible for its own CLI regression fixtures.
Existing `rsinter/tests/fixtures/css` files remain as workspace fixtures for
the earlier sparse-row contract and any `rsinter` consumers.

This does duplicate the small Steane `hx` and `hz` fixture contents. That is an
acceptable tradeoff because the #61 sweep belongs to `qec-code`, and future
manifest extensions should not make `rsinter` a catch-all golden-output
directory for another crate's CLI.

## Alternatives Considered

### 1. qec-code-owned fixtures

Add pinned fixtures under `qec-code/tests/fixtures/css/` and make the new
manifest sweep read only from that directory.

Benefits:

- keeps fixture ownership aligned with the CLI producer
- makes future manifest extensions local to `qec-code`
- gives every manifest entry the same fixture-path rule
- leaves existing `rsinter` fixture consumers untouched

Costs:

- duplicates the current Steane `hx` and `hz` JSON text

This is the recommended approach.

### 2. Continue using rsinter fixtures

Put all new pinned fixtures in `rsinter/tests/fixtures/css/`.

Benefits:

- reuses the existing Steane fixture location
- preserves the original workspace-contract fixture source from #55

Costs:

- makes `qec-code` CLI regression coverage depend on another crate's test data
- pushes future non-`rsinter` built-in fixtures into the wrong ownership area
- makes later manifest extension issues noisier across crates

This is not recommended for issue #61.

### 3. Mix fixture directories

Read Steane fixtures from `rsinter`, while placing repetition and `bb72`
fixtures under `qec-code`.

Benefits:

- avoids copying two small Steane files

Costs:

- makes the manifest path rule conditional per case
- obscures fixture ownership
- increases maintenance cost for future entries

This is not recommended.

## Manifest

The manifest should be a small array in `qec-code/tests/cli.rs`, using a
test-local struct:

```rust
struct BuiltInCssFixtureCase {
    code_id: &'static str,
    matrix: &'static str,
    fixture: &'static str,
}
```

The initial entries should be:

```text
steane / hx
steane / hz
repetition_x:d=5 / hx
repetition_z:d=5 / hz
bb72 / hx
bb72 / hz
```

These entries cover:

- fixed small code exports through `steane`
- parameterized-family dispatch through repetition specs
- one-sided repetition matrices with empty opposite sides
- large fixed-code exports through `bb72`
- both `hx` and `hz` matrix selection for fixed ids

Do not include every possible matrix side for every family in this issue. The
manifest is a representative regression yardstick, not a parameter sweep. The
issue-specified entries are enough for the first version.

## Fixture Files

Create these pinned JSON files:

```text
qec-code/tests/fixtures/css/steane_hx.json
qec-code/tests/fixtures/css/steane_hz.json
qec-code/tests/fixtures/css/repetition_x_d5_hx.json
qec-code/tests/fixtures/css/repetition_z_d5_hz.json
qec-code/tests/fixtures/css/bb72_hx.json
qec-code/tests/fixtures/css/bb72_hz.json
```

Each file should contain exactly the CLI stdout expected from:

```text
qec-code code css <code-id> <hx|hz>
```

That means compact JSON in the existing format plus one trailing newline:

```json
{"format":"sparse_rows","num_cols":N,"rows":[...]}
```

The implementation should produce these fixtures from the already-implemented
CLI command, then pin them in git. Once pinned, the sweep compares byte-for-byte
against the files and must not regenerate them during the test.

## Test Flow

Add one binary-driven test in `qec-code/tests/cli.rs`:

```rust
#[test]
fn built_in_css_fixture_manifest_exports_match_pinned_json() {
    for case in BUILT_IN_CSS_FIXTURE_CASES {
        let output = run_qec_code(&["code", "css", case.code_id, case.matrix]);
        assert!(output.status.success(), ...);
        assert_eq!(output.stderr, b"");

        let stdout = String::from_utf8(output.stdout).expect(...);
        let expected = read_qec_code_fixture(case.fixture);
        assert_eq!(stdout, expected, ...);
    }
}
```

The helper should read from `qec-code/tests/fixtures/css/`, not from
`rsinter/tests/fixtures/css/`. Existing `read_fixture(...)` helpers can remain
for old Steane tests, or the implementation can add a second helper with an
explicit qec-code fixture root to avoid changing old tests unnecessarily.

When an entry fails:

- CLI failure should report the code id, matrix, status, and stderr.
- fixture read failure should identify the missing path.
- stdout mismatch should use `assert_eq!` so Rust's test output shows the
  byte-for-byte difference.

## Error Handling

No production error handling changes are needed.

The test should surface failures directly:

- malformed manifest entry: CLI test fails
- missing fixture: fixture read panic includes the path
- stdout drift: byte-for-byte assertion fails
- unexpected stderr: stderr assertion fails

This keeps issue #61 as a test-only regression layer over existing behavior.

## Verification

Primary issue verification:

```text
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
```

Regression checks to run after the manifest passes:

```text
cargo test -p qec-code --test cli code_css_
cargo test -p qec-code --test css_export
cargo test -p qec-code
```

Manual negative check:

1. Temporarily change one pinned fixture row order or one support entry.
2. Re-run:

```text
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
```

3. Confirm the test fails with a byte-for-byte mismatch.
4. Revert the temporary fixture change before committing implementation work.

This proves the manifest sweep has teeth and will catch export drift.

## Follow-Up Compatibility

Issues #70 and #73 are expected to extend the same sweep after rotated-surface
and toric CLI exports exist. They should add only:

- new manifest entries
- new pinned fixtures under `qec-code/tests/fixtures/css/`
- focused family-specific CLI tests if their issue requires them

They should not need to redesign the sweep, move fixture directories, or add a
second registry-like catalog.

## Acceptance Criteria

- `qec-code/tests/fixtures/css/` contains pinned fixtures for the six initial
  representative exports.
- `qec-code/tests/cli.rs` contains
  `built_in_css_fixture_manifest_exports_match_pinned_json`.
- The test loops over an explicit small manifest.
- Each manifest entry runs the real `qec-code` binary CLI path.
- Stdout is compared byte-for-byte against pinned fixtures.
- The issue-specific test command passes.
- A deliberate temporary fixture mutation makes the same test fail before the
  mutation is reverted.
