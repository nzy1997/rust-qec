# Issue 129 Bivariate-Bicycle CSS Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make parsed `bb:lx=...,ly=...,a=...,b=...` built-in CSS specs export sparse-row JSON through the existing `qec-code code css <spec> hx|hz` path.

**Architecture:** Keep parsing and matrix generation in `qec-code/src/codes/built_in_css.rs`. Replace the parser-only rejection for `BuiltInCssFamily::BivariateBicycle` with a call to `bivariate_bicycle_css_checks(...)`, then add CLI tests that compare parameterized BB72 output against existing BB72 fixtures and verify the catalog line.

**Tech Stack:** Rust 2024, `qec-code`, existing `QecError`, existing `BivariateBicycleParams`, existing `SparseRowsMatrix` JSON export.

## Global Constraints

- Keep the CLI grammar under the existing positional `qec-code code css <code-id-or-family-spec> <hx|hz>` path.
- Add the catalog entry `bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...`.
- Keep the list output human-readable and one-line per entry.
- Parameterized BB72 `hx` output must equal `qec-code/tests/fixtures/css/bb72_hx.json`.
- Parameterized BB72 `hz` output must equal `qec-code/tests/fixtures/css/bb72_hz.json`.
- The negative control `bb:lx=0,ly=6,a=3:0,b=0:3` must exit non-zero, write empty stdout, and include a built-in CSS parameter error on stderr.
- Do not generate new fixtures, add benchmarks, or add circuit integration.

---

## File Structure

- Modify `qec-code/src/codes/built_in_css.rs`: add the parameterized BB catalog entry and route parsed `BuiltInCssParams::BivariateBicycle(params)` to `bivariate_bicycle_css_checks(params)`.
- Modify `qec-code/tests/cli.rs`: add parameterized BB72 fixture cases for hx/hz, add the catalog assertion, add the negative CLI control, and update the exact list-output expectation.
- Modify `qec-code/tests/code.rs`: update existing code-level catalog and built-in dispatch assertions from the old parser-only BB behavior to the issue #129 generation behavior.

### Task 1: Wire Bivariate-Bicycle Specs Through CSS Export

**Files:**
- Modify: `qec-code/src/codes/built_in_css.rs`
- Modify: `qec-code/tests/cli.rs`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `parse_built_in_css_code_spec(...)`, `BuiltInCssFamily::BivariateBicycle`, `BuiltInCssParams::BivariateBicycle(BivariateBicycleParams)`, and `bivariate_bicycle_css_checks(...)`.
- Produces: `built_in_css_checks("bb:lx=6,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0") -> Ok(BuiltInCssChecks { code_id: "bb", ... })` and a catalog row for the `bb:lx=...` family.

- [x] **Step 1: Write failing CLI tests**

In `qec-code/tests/cli.rs`, add this constant after `run_qec_code_in_process_os(...)`:

```rust
const BB72_PARAMETERIZED_SPEC: &str = "bb:lx=6,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";
```

Add these fixture cases after the existing `bb72` cases in `BUILT_IN_CSS_FIXTURE_CASES`:

```rust
    BuiltInCssFixtureCase {
        code_id: BB72_PARAMETERIZED_SPEC,
        matrix: "hx",
        fixture: "bb72_hx.json",
    },
    BuiltInCssFixtureCase {
        code_id: BB72_PARAMETERIZED_SPEC,
        matrix: "hz",
        fixture: "bb72_hz.json",
    },
```

Add these tests after `code_css_bb72_hx_prints_sparse_rows_json`:

```rust
#[test]
fn code_css_bb_parameterized_hx_matches_bb72_fixture() {
    let output = run_qec_code(&["code", "css", BB72_PARAMETERIZED_SPEC, "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/bb72_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_bb_parameterized_hz_matches_bb72_fixture() {
    let output = run_qec_code(&["code", "css", BB72_PARAMETERIZED_SPEC, "hz"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("qec-code/tests/fixtures/css/bb72_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_bb_parameterized_invalid_lattice_dimension_fails_without_json() {
    let output = run_qec_code(&["code", "css", "bb:lx=0,ly=6,a=3:0,b=0:3", "hx"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("out-of-range built-in CSS integer parameter lx for family bb: 0"),
        "stderr was: {stderr}"
    );
}
```

Add this assertion to `code_css_list_includes_supported_built_ins` after the `bb72` assertion:

```rust
    assert!(
        stdout.contains("bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|..."),
        "stdout was: {stdout}"
    );
```

Replace the `expected` string in `run_code_css_list_returns_catalog_without_newline` with:

```rust
    let expected = "Built-in CSS codes:\n  steane                                                                 fixed [[7,1,3]] CSS code\n  bb72                                                                   fixed [[72,12,6]] bivariate-bicycle CSS code\n  bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...  bivariate-bicycle CSS family over periodic lattice\n  repetition_x:d=<distance>                                              X-check chain, distance >= 2\n  repetition_z:d=<distance>                                              Z-check chain, distance >= 2\n  surface_rotated:d=<distance>                                           rotated surface CSS code, distance >= 2\n  toric:d=<distance>                                                     periodic square-lattice toric CSS code, distance >= 2";
```

- [x] **Step 2: Run the filtered CLI tests to verify RED**

Run:

```bash
cargo test -p qec-code --test cli bb
```

Expected: FAIL. The parameterized BB72 hx/hz tests should fail because `built_in_css_checks(...)` rejects parsed bivariate-bicycle family specs as parser-only, and the catalog tests should fail because the `bb:lx=...` catalog entry is missing.

- [x] **Step 3: Add catalog entry and bivariate-bicycle dispatch**

In `qec-code/src/codes/built_in_css.rs`, add this catalog entry after the fixed `bb72` entry:

```rust
    BuiltInCssCatalogEntry {
        spec: "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...",
        description: "bivariate-bicycle CSS family over periodic lattice",
    },
```

Replace the bivariate-bicycle parser-only rejection in `built_in_css_checks(...)`:

```rust
        BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            ..
        } => Err(QecError::UnknownBuiltInCssCode {
            code_id: code_id.to_owned(),
        }),
```

with:

```rust
        BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            params: BuiltInCssParams::BivariateBicycle(params),
        } => bivariate_bicycle_css_checks(params),
```

Leave the `BuiltInCssFamily::BivariateBicycle` fallback inside `family_css_checks(...)` as an unreachable guard for incompatible internal dispatch.

- [x] **Step 4: Update existing code-level tests for the new behavior**

In `qec-code/tests/code.rs`, add the BB family spec to the expected catalog list in `built_in_css_catalog_lists_supported_specs`:

```rust
            "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...",
```

Rename `built_in_css_checks_rejects_parser_only_bivariate_bicycle_specs` to:

```rust
fn built_in_css_checks_accepts_bivariate_bicycle_specs()
```

and replace the old `UnknownBuiltInCssCode` assertion with:

```rust
    let expected = bivariate_bicycle_css_checks(bb144_bivariate_bicycle_params()).unwrap();

    assert_eq!(built_in_css_checks(spec), Ok(expected));
```

- [x] **Step 5: Run the filtered CLI tests to verify GREEN**

Run:

```bash
cargo test -p qec-code --test cli bb
```

Expected: PASS. The output should include the parameterized BB72 hx/hz fixture checks, the catalog entry assertion, and the invalid-lattice negative control.

- [x] **Step 6: Run required broader verification**

Run:

```bash
cargo test -p qec-code
cargo test
git diff --check
```

Expected: all commands exit 0. If workspace tests print pre-existing warnings, record them in the final report without treating warnings as failures.

- [x] **Step 7: Publish changes**

Because this Agent Desk sandbox may not be able to write the external worktree git metadata, publish by the available safe path:

```bash
git status -sb
```

If local git index writes are allowed, commit:

```bash
git add docs/superpowers/specs/2026-06-23-issue-129-bivariate-bicycle-css-export-design.md docs/superpowers/plans/2026-06-23-issue-129-bivariate-bicycle-css-export.md qec-code/src/codes/built_in_css.rs qec-code/tests/cli.rs qec-code/tests/code.rs
git commit -m "feat: wire bivariate-bicycle css specs"
git push -u origin agent/issue-129-wire-bivariate-bicycle-specs-through-css-export--run-1
```

If local git index writes are blocked, create the same commit on the worker branch using the GitHub app commit APIs, then open a draft PR against `master`.
