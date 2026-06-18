# Issue 72 And 73 Toric CSS CLI Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pinned `qec-code` CLI fixture coverage for `toric:d=3` `hx` and `hz`, completing GitHub issues #72 and #73 together.

**Architecture:** Keep the implementation as a test-only CLI regression pass. Extend the existing `qec-code/tests/cli.rs` fixture manifest and issue-named binary CLI tests, then add two compact `sparse_rows` fixture files under `qec-code/tests/fixtures/css/`. The existing production path from `run_css` through `built_in_css_checks` to `SparseRowsMatrix::to_json_string` remains unchanged unless the tests reveal a real mismatch.

**Tech Stack:** Rust 2024, Cargo integration tests, clap CLI parsing, `qec-code` `SparseRowsMatrix` JSON fixtures.

## Global Constraints

- Do not modify production code under `qec-code/src/` for the normal path.
- Do not add a new top-level CLI command.
- Do not add combined `hx`/`hz` JSON output.
- Do not change toric registry geometry or indexing.
- Do not add `toric:d=4` fixture-manifest entries.
- Do not update README or user-facing docs.
- Do not move existing fixtures into `rsinter`.
- Stage only the issue #72/#73 files listed in this plan; if `docs/superpowers/plans/2026-06-18-issue-91-rbposd-lsd-runner-params.md` is still untracked, leave it unstaged and untouched.

---

## File Structure

- Modify `qec-code/tests/cli.rs`
  - Extend `BUILT_IN_CSS_FIXTURE_CASES` with `toric:d=3` `hx` and `hz`.
  - Add two issue #72 success tests that compare binary stdout byte-for-byte against qec-code-owned fixtures.
  - Add one issue #72 grouped failure test for missing distance, non-integer distance, out-of-range distance, and invalid matrix selector.
- Create `qec-code/tests/fixtures/css/toric_d3_hx.json`
  - Owns the pinned CLI stdout for `qec-code code css toric:d=3 hx`.
- Create `qec-code/tests/fixtures/css/toric_d3_hz.json`
  - Owns the pinned CLI stdout for `qec-code code css toric:d=3 hz`.

No production files should change during the normal implementation path.

### Task 1: Add Toric CLI Tests, Manifest Entries, And Fixtures

**Files:**
- Modify: `qec-code/tests/cli.rs`
- Create: `qec-code/tests/fixtures/css/toric_d3_hx.json`
- Create: `qec-code/tests/fixtures/css/toric_d3_hz.json`

**Interfaces:**
- Consumes: existing helper `run_qec_code(args: &[&str]) -> std::process::Output`.
- Consumes: existing helper `read_fixture(rel_path: &str) -> String`.
- Consumes: existing `BuiltInCssFixtureCase { code_id: &'static str, matrix: &'static str, fixture: &'static str }`.
- Produces: `code_css_toric_d3_hx_prints_workspace_fixture()`.
- Produces: `code_css_toric_d3_hz_prints_workspace_fixture()`.
- Produces: `code_css_toric_missing_or_bad_distance_fails()`.
- Produces: manifest entries that make `built_in_css_fixture_manifest_exports_match_pinned_json()` cover `toric:d=3`.

- [ ] **Step 1: Extend the built-in CSS fixture manifest**

In `qec-code/tests/cli.rs`, add these entries to `BUILT_IN_CSS_FIXTURE_CASES` immediately after the existing `surface_rotated:d=3` entries:

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

After this edit, the tail of `BUILT_IN_CSS_FIXTURE_CASES` should read:

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
];
```

- [ ] **Step 2: Add the issue #72 success tests**

In `qec-code/tests/cli.rs`, insert these tests immediately after `code_css_surface_rotated_d3_hz_prints_workspace_fixture`:

```rust
#[test]
fn code_css_toric_d3_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "toric:d=3", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/toric_d3_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_toric_d3_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "toric:d=3", "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/toric_d3_hz.json");

    assert_eq!(stdout, expected);
}
```

- [ ] **Step 3: Add the issue #72 malformed-input test**

In `qec-code/tests/cli.rs`, insert this test immediately after `code_css_toric_d3_hz_prints_workspace_fixture`:

```rust
#[test]
fn code_css_toric_missing_or_bad_distance_fails() {
    #[derive(Debug)]
    struct FailureCase {
        args: &'static [&'static str],
        stderr_fragment: &'static str,
    }

    const CASES: &[FailureCase] = &[
        FailureCase {
            args: &["code", "css", "toric", "hx"],
            stderr_fragment: "missing built-in CSS parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=nope", "hx"],
            stderr_fragment: "invalid built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=1", "hx"],
            stderr_fragment: "out-of-range built-in CSS integer parameter d",
        },
        FailureCase {
            args: &["code", "css", "toric:d=3", "foo"],
            stderr_fragment: "invalid value 'foo'",
        },
    ];

    for case in CASES {
        let output = run_qec_code(case.args);

        assert!(
            !output.status.success(),
            "case {case:?} unexpectedly succeeded"
        );
        assert_eq!(output.stdout, b"", "case {case:?} should not print stdout");

        let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
        assert!(
            stderr.contains(case.stderr_fragment),
            "case {case:?} stderr was: {stderr}"
        );
    }
}
```

- [ ] **Step 4: Run the new success tests and verify they fail because fixtures are missing**

Run:

```bash
cargo test -p qec-code --test cli code_css_toric_d3_hx_prints_workspace_fixture
cargo test -p qec-code --test cli code_css_toric_d3_hz_prints_workspace_fixture
```

Expected: both commands exit non-zero, and each failure contains:

```text
fixture should be readable
```

This proves the tests reach the real binary CLI path and now need pinned fixtures.

- [ ] **Step 5: Run the malformed-input test and verify it already passes**

Run:

```bash
cargo test -p qec-code --test cli code_css_toric_missing_or_bad_distance_fails
```

Expected: PASS. The registry parser and clap selector validation already reject these malformed inputs.

- [ ] **Step 6: Create the `hx` fixture**

Create `qec-code/tests/fixtures/css/toric_d3_hx.json` with exactly this content, including the final newline:

```json
{"format":"sparse_rows","num_cols":18,"rows":[[0,2,9,15],[0,1,10,16],[1,2,11,17],[3,5,9,12],[3,4,10,13],[4,5,11,14],[6,8,12,15],[6,7,13,16],[7,8,14,17]]}
```

- [ ] **Step 7: Create the `hz` fixture**

Create `qec-code/tests/fixtures/css/toric_d3_hz.json` with exactly this content, including the final newline:

```json
{"format":"sparse_rows","num_cols":18,"rows":[[0,3,9,10],[1,4,10,11],[2,5,9,11],[3,6,12,13],[4,7,13,14],[5,8,12,14],[0,6,15,16],[1,7,16,17],[2,8,15,17]]}
```

- [ ] **Step 8: Format the qec-code package**

Run:

```bash
cargo fmt --package qec-code
```

Expected: PASS with no output.

- [ ] **Step 9: Run the issue #72 focused tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test cli code_css_toric
```

Expected: PASS. The output includes these passing tests:

```text
code_css_toric_d3_hx_prints_workspace_fixture
code_css_toric_d3_hz_prints_workspace_fixture
code_css_toric_missing_or_bad_distance_fails
```

- [ ] **Step 10: Run the issue #73 manifest sweep and verify it passes**

Run:

```bash
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json
```

Expected: PASS. The manifest sweep now includes the two `toric:d=3` entries and compares them byte-for-byte against the new fixture files.

- [ ] **Step 11: Commit the toric CLI fixture coverage**

Run:

```bash
git add qec-code/tests/cli.rs qec-code/tests/fixtures/css/toric_d3_hx.json qec-code/tests/fixtures/css/toric_d3_hz.json
git commit -m "test: pin toric css cli fixtures"
```

Expected: commit succeeds with only the issue #72/#73 test and fixture files staged.

### Task 2: Final Verification

**Files:**
- Verify: `qec-code/tests/cli.rs`
- Verify: `qec-code/tests/fixtures/css/toric_d3_hx.json`
- Verify: `qec-code/tests/fixtures/css/toric_d3_hz.json`

**Interfaces:**
- Consumes: `code_css_toric_d3_hx_prints_workspace_fixture()`.
- Consumes: `code_css_toric_d3_hz_prints_workspace_fixture()`.
- Consumes: `code_css_toric_missing_or_bad_distance_fails()`.
- Consumes: `built_in_css_fixture_manifest_exports_match_pinned_json()`.
- Produces: verified issue #72 and #73 implementation.

- [ ] **Step 1: Run the nearby CSS CLI regression filter**

Run:

```bash
cargo test -p qec-code --test cli code_css_
```

Expected: PASS. Existing Steane, BB72, repetition, surface-rotated, list/export, and new toric CSS CLI tests all pass.

- [ ] **Step 2: Run the full qec-code package tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS. No `qec-code` unit, integration, or binary test fails.

- [ ] **Step 3: Run the qec-code format check**

Run:

```bash
cargo fmt --check --package qec-code
```

Expected: PASS. Rustfmt reports no diff.

- [ ] **Step 4: Inspect final git state**

Run:

```bash
git status --short --branch
```

Expected: the branch is ahead of `origin/master`, no issue #72/#73 files are unstaged, and any unrelated untracked file such as `docs/superpowers/plans/2026-06-18-issue-91-rbposd-lsd-runner-params.md` remains untouched and unstaged.

- [ ] **Step 5: Inspect the issue #72/#73 implementation commit**

Run:

```bash
git show --stat --oneline --no-renames HEAD
```

Expected: the latest implementation commit is `test: pin toric css cli fixtures` and touches only:

```text
qec-code/tests/cli.rs
qec-code/tests/fixtures/css/toric_d3_hx.json
qec-code/tests/fixtures/css/toric_d3_hz.json
```

- [ ] **Step 6: Record completion notes for the final response**

The final response should mention:

```text
Implemented issue #72 and issue #73 together.
Added toric:d=3 hx/hz pinned sparse_rows fixtures.
Extended the built-in CSS fixture manifest.
Added toric CLI success and malformed-input regression tests.
Verified with cargo test -p qec-code --test cli code_css_, cargo test -p qec-code, and cargo fmt --check --package qec-code.
```
