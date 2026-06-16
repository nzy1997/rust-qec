# Issue 69 Surface Rotated CSS CLI Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add qec-code CLI fixture and error regression coverage for `qec-code code css surface_rotated:d=3 hx|hz`.

**Architecture:** Reuse the existing built-in CSS CLI export path and fixture manifest. Add issue-specific binary CLI tests in `qec-code/tests/cli.rs`, pin the two `surface_rotated:d=3` sparse-row JSON fixtures under `qec-code/tests/fixtures/css/`, and avoid production changes unless the new tests reveal a real mismatch.

**Tech Stack:** Rust 2024, Cargo integration tests, clap CLI parsing, qec-code `SparseRowsMatrix` JSON fixtures.

---

## File Structure

- Modify `qec-code/tests/cli.rs`
  - Extend `BUILT_IN_CSS_FIXTURE_CASES` with `surface_rotated:d=3` `hx` and `hz`.
  - Add two issue-named success tests that compare binary stdout byte-for-byte against qec-code-owned fixtures.
  - Add one grouped failure test for missing distance, non-integer distance, out-of-range distance, and invalid matrix selector.
- Create `qec-code/tests/fixtures/css/surface_rotated_d3_hx.json`
  - Owns the pinned CLI stdout for `qec-code code css surface_rotated:d=3 hx`.
- Create `qec-code/tests/fixtures/css/surface_rotated_d3_hz.json`
  - Owns the pinned CLI stdout for `qec-code code css surface_rotated:d=3 hz`.

No production files should change during the normal path for this issue.

## Task 1: Add Failing CLI Fixture Tests And Manifest Entries

**Files:**
- Modify: `qec-code/tests/cli.rs`
- Later create: `qec-code/tests/fixtures/css/surface_rotated_d3_hx.json`
- Later create: `qec-code/tests/fixtures/css/surface_rotated_d3_hz.json`

- [ ] **Step 1: Extend the built-in CSS fixture manifest**

In `qec-code/tests/cli.rs`, update `BUILT_IN_CSS_FIXTURE_CASES` so the final entries are:

```rust
const BUILT_IN_CSS_FIXTURE_CASES: &[BuiltInCssFixtureCase] = &[
    BuiltInCssFixtureCase {
        code_id: "steane",
        matrix: "hx",
        fixture: "steane_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "steane",
        matrix: "hz",
        fixture: "steane_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "repetition_x:d=5",
        matrix: "hx",
        fixture: "repetition_x_d5_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "repetition_z:d=5",
        matrix: "hz",
        fixture: "repetition_z_d5_hz.json",
    },
    BuiltInCssFixtureCase {
        code_id: "bb72",
        matrix: "hx",
        fixture: "bb72_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: "bb72",
        matrix: "hz",
        fixture: "bb72_hz.json",
    },
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
];
```

- [ ] **Step 2: Add the issue-named success tests**

In `qec-code/tests/cli.rs`, insert these tests after `code_css_bb72_hx_prints_sparse_rows_json`:

```rust
#[test]
fn code_css_surface_rotated_d3_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "surface_rotated:d=3", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_surface_rotated_d3_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "surface_rotated:d=3", "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/surface_rotated_d3_hz.json");

    assert_eq!(stdout, expected);
}
```

- [ ] **Step 3: Run the focused success tests and verify they fail for missing fixtures**

Run:

```bash
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hz_prints_workspace_fixture
```

Expected:

- Both commands exit non-zero.
- Each failure reports `fixture should be readable`.
- This proves the tests are reaching the existing CLI path and now need pinned fixtures.

## Task 2: Add The Surface Rotated d=3 Fixtures

**Files:**
- Create: `qec-code/tests/fixtures/css/surface_rotated_d3_hx.json`
- Create: `qec-code/tests/fixtures/css/surface_rotated_d3_hz.json`
- Verify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Create the `hx` fixture**

Create `qec-code/tests/fixtures/css/surface_rotated_d3_hx.json` with exactly this content, including the final newline:

```json
{"format":"sparse_rows","num_cols":9,"rows":[[0,3],[1,2,4,5],[3,4,6,7],[5,8]]}
```

- [ ] **Step 2: Create the `hz` fixture**

Create `qec-code/tests/fixtures/css/surface_rotated_d3_hz.json` with exactly this content, including the final newline:

```json
{"format":"sparse_rows","num_cols":9,"rows":[[1,2],[0,1,3,4],[4,5,7,8],[6,7]]}
```

- [ ] **Step 3: Run the focused success tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hz_prints_workspace_fixture
```

Expected:

- Both commands exit zero.
- Each test result includes `ok`.

- [ ] **Step 4: Run the shared manifest test and verify the new entries pass**

Run:

```bash
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
```

Expected:

- The command exits zero.
- `built_in_css_fixture_manifest_exports_match_pinned_json` passes with the two new `surface_rotated:d=3` manifest entries.

- [ ] **Step 5: Commit the success fixtures and tests**

Run:

```bash
git add qec-code/tests/cli.rs qec-code/tests/fixtures/css/surface_rotated_d3_hx.json qec-code/tests/fixtures/css/surface_rotated_d3_hz.json
git commit -m "test: pin surface rotated css cli fixtures"
```

## Task 3: Add The Rotated Surface Failure Cases

**Files:**
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add the grouped failure test**

In `qec-code/tests/cli.rs`, insert this test after `code_css_surface_rotated_d3_hz_prints_workspace_fixture`:

```rust
#[test]
fn code_css_surface_rotated_missing_or_bad_distance_fails() {
    #[derive(Debug)]
    struct FailureCase {
        args: &'static [&'static str],
        stderr_fragment: &'static str,
    }

    const CASES: &[FailureCase] = &[
        FailureCase {
            args: &["code", "css", "surface_rotated", "hx"],
            stderr_fragment: "missing built-in CSS parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=nope", "hx"],
            stderr_fragment: "invalid built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=1", "hx"],
            stderr_fragment: "out-of-range built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "surface_rotated:d=3", "foo"],
            stderr_fragment: "invalid value 'foo'",
        },
    ];

    for case in CASES {
        let output = run_qec_code(case.args);

        assert!(
            !output.status.success(),
            "case {case:?} unexpectedly succeeded"
        );
        assert_eq!(
            output.stdout, b"",
            "case {case:?} should not print stdout"
        );

        let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
        assert!(
            stderr.contains(case.stderr_fragment),
            "case {case:?} stderr was: {stderr}"
        );
    }
}
```

- [ ] **Step 2: Run the failure test**

Run:

```bash
cargo test -p qec-code --test cli code_css_surface_rotated_missing_or_bad_distance_fails
```

Expected:

- The command exits zero.
- The test passes because the registry parser and clap selector validation already reject these malformed inputs.
- If this fails, fix only the specific production mismatch exposed by the failing assertion, then rerun this command.

- [ ] **Step 3: Commit the failure regression test**

Run:

```bash
git add qec-code/tests/cli.rs
git commit -m "test: cover invalid surface rotated css cli specs"
```

## Task 4: Final Verification

**Files:**
- Verify: `qec-code/tests/cli.rs`
- Verify: `qec-code/tests/fixtures/css/surface_rotated_d3_hx.json`
- Verify: `qec-code/tests/fixtures/css/surface_rotated_d3_hz.json`

- [ ] **Step 1: Run the issue-requested focused tests**

Run:

```bash
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_d3_hz_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_surface_rotated_missing_or_bad_distance_fails
```

Expected:

- All three commands exit zero.
- The three issue #69 test names pass.

- [ ] **Step 2: Run nearby CSS CLI regressions**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected:

- The command exits zero.
- Existing fixed-code exports, repetition exports, explicit export, list tests, the shared manifest test, and the new surface-rotated tests all pass.

- [ ] **Step 3: Run the qec-code package tests**

Run:

```bash
cargo test -p qec-code
```

Expected:

- The command exits zero.
- No `qec-code` unit or integration tests fail.

- [ ] **Step 4: Run the qec-code format check**

Run:

```bash
cargo fmt --check --package qec-code
```

Expected:

- The command exits zero.
- Rustfmt reports no diff.

- [ ] **Step 5: Inspect final git state**

Run:

```bash
git status --short --branch
```

Expected:

- The issue #69 implementation commits are present on the current branch.
- No unrelated file is staged.
- The unrelated untracked `docs/superpowers/specs/2026-06-17-toric-css-family-design.md`, if still present, remains untouched and unstaged.
