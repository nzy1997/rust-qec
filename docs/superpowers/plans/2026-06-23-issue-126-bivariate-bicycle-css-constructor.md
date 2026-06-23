# Issue 126 Bivariate-Bicycle CSS Constructor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a typed Rust bivariate-bicycle CSS constructor and focused validation tests for issue #126.

**Architecture:** Keep the matrix generation in `qec-code/src/codes/built_in_css.rs`, where fixed and built-in CSS constructors already live. Add a public typed parameter struct plus a public constructor that validates inputs, reuses the existing private bivariate-bicycle row generator, and routes the fixed `bb72` alias through the new constructor while preserving its visible `code_id`.

**Tech Stack:** Rust 2024, `qec-code`, existing `QecError`, existing `CssCode` and sparse-row test helpers.

## Global Constraints

- Modify only `docs/superpowers/specs/2026-06-23-bivariate-bicycle-css-family-design.md`, `docs/superpowers/plans/2026-06-23-issue-126-bivariate-bicycle-css-constructor.md`, `qec-code/src/codes/built_in_css.rs`, and `qec-code/tests/code.rs`.
- Do not add CLI parsing, catalog text, benchmark integration, or logical observable generation.
- Constructor inputs are `lx`, `ly`, `a_terms`, and `b_terms`, where term lists are non-empty `Vec<(usize, usize)>`.
- Constructor output uses `code_id = "bb"`, `num_cols = 2 * lx * ly`, `hx.len() = lx * ly`, and `hz.len() = lx * ly`.
- Reject `lx = 0`, `ly = 0`, empty `a_terms`, empty `b_terms`, and duplicate normalized shifts within either term list.
- Normalize duplicate detection with `(dx % lx, dy % ly)`.
- Keep row supports sorted, duplicate-free, and in range.
- Preserve the existing fixed `bb72` public behavior with `code_id = "bb72"`.

---

## File Structure

- Modify `qec-code/tests/code.rs`: add focused constructor tests under the existing built-in CSS tests.
- Modify `qec-code/src/codes/built_in_css.rs`: add `BivariateBicycleParams`, `bivariate_bicycle_css_checks(...)`, validation helpers, and route `bb72` through the public constructor.

### Task 1: Typed Bivariate-Bicycle Constructor

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

**Interfaces:**
- Consumes: existing `BuiltInCssChecks`, `QecError`, `CssCode::from_hx_hz(...)`, and private `bivariate_bicycle_checks(...)`.
- Produces:
  - `pub struct BivariateBicycleParams { pub lx: usize, pub ly: usize, pub a_terms: Vec<(usize, usize)>, pub b_terms: Vec<(usize, usize)> }`
  - `pub fn bivariate_bicycle_css_checks(params: BivariateBicycleParams) -> Result<BuiltInCssChecks>`

- [ ] **Step 1: Write the failing tests**

Update the import in `qec-code/tests/code.rs` to include the new API:

```rust
use qec_code::codes::built_in_css::{
    BivariateBicycleParams, BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams,
    bivariate_bicycle_css_checks, built_in_css_catalog, built_in_css_checks,
    parse_built_in_css_code_spec,
};
```

Add these tests after `bb72_has_expected_shape_and_css_orthogonality`:

```rust
fn bb72_bivariate_bicycle_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 6,
        ly: 6,
        a_terms: vec![(3, 0), (0, 1), (0, 2)],
        b_terms: vec![(0, 3), (1, 0), (2, 0)],
    }
}

fn bb144_bivariate_bicycle_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 12,
        ly: 6,
        a_terms: vec![(3, 0), (0, 1), (0, 2)],
        b_terms: vec![(0, 3), (1, 0), (2, 0)],
    }
}

#[test]
fn bivariate_bicycle_css_checks_bb72_matches_fixed_alias() {
    let fixed = built_in_css_checks("bb72").unwrap();
    let generic = bivariate_bicycle_css_checks(bb72_bivariate_bicycle_params()).unwrap();

    assert_eq!(generic.code_id, "bb");
    assert_eq!(generic.num_cols, fixed.num_cols);
    assert_eq!(generic.hx, fixed.hx);
    assert_eq!(generic.hz, fixed.hz);
}

#[test]
fn bivariate_bicycle_css_checks_bb144_shape_orthogonality_and_canonical_rows() {
    let checks = bivariate_bicycle_css_checks(bb144_bivariate_bicycle_params()).unwrap();

    assert_eq!(checks.code_id, "bb");
    assert_eq!(checks.num_cols, 144);
    assert_eq!(checks.hx.len(), 72);
    assert_eq!(checks.hz.len(), 72);

    for row in checks.hx.iter().chain(checks.hz.iter()) {
        assert_eq!(row.len(), 6, "row has wrong weight: {row:?}");
    }

    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    CssCode::from_hx_hz(
        dense_rows(&checks.hx, checks.num_cols),
        dense_rows(&checks.hz, checks.num_cols),
    )
    .unwrap();
}

#[test]
fn bivariate_bicycle_css_checks_rejects_zero_lattice_dimension() {
    let mut params = bb144_bivariate_bicycle_params();
    params.lx = 0;

    assert_eq!(
        bivariate_bicycle_css_checks(params),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
            value: 0,
        })
    );
}

#[test]
fn bivariate_bicycle_css_checks_rejects_modulo_duplicate_terms() {
    let mut params = bb72_bivariate_bicycle_params();
    params.a_terms = vec![(0, 0), (6, 0)];

    assert!(bivariate_bicycle_css_checks(params).is_err());
}
```

- [ ] **Step 2: Run the filtered test to verify RED**

Run:

```bash
cargo test -p qec-code --test code bivariate_bicycle
```

Expected: FAIL because `BivariateBicycleParams` and `bivariate_bicycle_css_checks` do not exist.

- [ ] **Step 3: Add the public type and constructor**

In `qec-code/src/codes/built_in_css.rs`, add this type after `BuiltInCssChecks`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BivariateBicycleParams {
    pub lx: usize,
    pub ly: usize,
    pub a_terms: Vec<(usize, usize)>,
    pub b_terms: Vec<(usize, usize)>,
}
```

Add `use std::collections::HashSet;` at the top of the file.

Replace the private `bb72_checks()` helper with a version that returns `Result<BuiltInCssChecks>` through the new public constructor:

```rust
fn bb72_checks() -> Result<BuiltInCssChecks> {
    let mut checks = bivariate_bicycle_css_checks(BivariateBicycleParams {
        lx: BB72_LX,
        ly: BB72_LY,
        a_terms: BB72_A_TERMS.to_vec(),
        b_terms: BB72_B_TERMS.to_vec(),
    })?;
    checks.code_id = "bb72";
    Ok(checks)
}
```

Add the public constructor and validation helpers before the private `bivariate_bicycle_checks(...)` function:

```rust
pub fn bivariate_bicycle_css_checks(params: BivariateBicycleParams) -> Result<BuiltInCssChecks> {
    validate_bivariate_bicycle_params(&params)?;

    let (hx, hz) =
        bivariate_bicycle_checks(params.lx, params.ly, &params.a_terms, &params.b_terms);

    Ok(BuiltInCssChecks {
        code_id: "bb",
        num_cols: 2 * params.lx * params.ly,
        hx,
        hz,
    })
}

fn validate_bivariate_bicycle_params(params: &BivariateBicycleParams) -> Result<()> {
    validate_positive_bivariate_bicycle_dimension("lx", params.lx)?;
    validate_positive_bivariate_bicycle_dimension("ly", params.ly)?;
    validate_bivariate_bicycle_terms("a_terms", &params.a_terms, params.lx, params.ly)?;
    validate_bivariate_bicycle_terms("b_terms", &params.b_terms, params.lx, params.ly)?;
    Ok(())
}

fn validate_positive_bivariate_bicycle_dimension(parameter: &'static str, value: usize) -> Result<()> {
    if value == 0 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: parameter.to_owned(),
            value,
        });
    }

    Ok(())
}

fn validate_bivariate_bicycle_terms(
    parameter: &'static str,
    terms: &[(usize, usize)],
    lx: usize,
    ly: usize,
) -> Result<()> {
    if terms.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let mut seen = HashSet::new();
    for &(dx, dy) in terms {
        if !seen.insert((dx % lx, dy % ly)) {
            return Err(QecError::DuplicateBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: parameter.to_owned(),
            });
        }
    }

    Ok(())
}
```

Update the fixed `bb72` branch in `fixed_built_in_css_checks(...)`:

```rust
"bb72" => bb72_checks(),
```

- [ ] **Step 4: Run the filtered test to verify GREEN**

Run:

```bash
cargo test -p qec-code --test code bivariate_bicycle
```

Expected: PASS with the four `bivariate_bicycle` tests.

- [ ] **Step 5: Run broader qec-code tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-06-23-bivariate-bicycle-css-family-design.md docs/superpowers/plans/2026-06-23-issue-126-bivariate-bicycle-css-constructor.md qec-code/src/codes/built_in_css.rs qec-code/tests/code.rs
git commit -m "feat: add bivariate-bicycle css constructor"
```
