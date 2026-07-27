# Generalized-Bicycle CSS Constructor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the parameterized cyclic generalized-bicycle CSS constructor requested by GitHub issue #558.

**Architecture:** Add a dedicated `qec_code::codes::generalized_bicycle` module that validates and normalizes `GeneralizedBicycleSpec`, builds cyclic circulants as `SparseGf2Matrix` values, and emits `H_X = [A | B]` and `H_Z = [B^T | A^T]`. Extend the shared `family_contract` routing so Rust API and versioned JSON CLI requests lower to `CssFamilySpec::GeneralizedBicycle`.

**Tech Stack:** Rust 2024, `qec-code`, `serde`, Cargo integration tests, existing sparse GF(2) primitives.

## Global Constraints

- Use `.AGENTS/AGENTS.md` repository rules.
- Do not route generalized-bicycle construction through `qec-code/src/finite_group.rs` or any finite-group/lift table API.
- Use `qec_code::sparse_gf2::SparseGf2Matrix` plus transpose and horizontal concatenation for CSS check construction.
- Accept any nonzero cyclic order and nonempty exponent supports with values `< order`.
- Normalize exponent metadata to sorted arrays and reject duplicate exponents.
- The order-5 fixture must report `n=10`, `m_x=5`, `m_z=5`, `rank_x=4`, `rank_z=4`, `k=2`, `d_x=3`, and `d_z=3`.
- The order-5 fixture rows must match the issue text exactly.
- Versioned JSON construction uses `construction = "generalized_bicycle"`.
- Required verification commands:
  - `cargo test -p qec-code --test generalized_bicycle generalized_bicycle_order5_matches_fixture -- --exact`
  - `cargo test -p qec-code --test generalized_bicycle generalized_bicycle_rejects_invalid_exponents -- --exact`
  - `cargo test`

---

### Task 1: Add Generalized-Bicycle Contract Tests

**Files:**
- Create: `qec-code/tests/generalized_bicycle.rs`

**Interfaces:**
- Consumes: planned `GeneralizedBicycleSpec`, `CssFamilySpec::GeneralizedBicycle`, `parse_css_construction_json`, `construct_css`, and CLI `code css construct --spec`.
- Produces: failing positive and negative integration tests named exactly as the issue verification commands require.

- [ ] **Step 1: Write the failing integration test file**

Create `qec-code/tests/generalized_bicycle.rs`:

```rust
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::cli::{run, Cli, CodeCommands, Commands, CssArgs, CssMatrixKind};
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, CssFamilySpec, GeneralizedBicycleSpec,
    RequestedFamilyId, verify_css_orthogonality,
};
use qec_code::QecError;
use tempfile::tempdir;

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn fixture_spec() -> GeneralizedBicycleSpec {
    GeneralizedBicycleSpec {
        order: 5,
        a_exponents: vec![0, 1],
        b_exponents: vec![0, 2],
    }
}

fn fixture_hx() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 5, 7],
        vec![1, 2, 6, 8],
        vec![2, 3, 7, 9],
        vec![3, 4, 5, 8],
        vec![0, 4, 6, 9],
    ]
}

fn fixture_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 3, 5, 9],
        vec![1, 4, 5, 6],
        vec![0, 2, 6, 7],
        vec![1, 3, 7, 8],
        vec![2, 4, 8, 9],
    ]
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must be sorted and duplicate-free: {row:?}"
        );
    }
}

fn write_spec(path: &Path, contents: &str) -> PathBuf {
    let spec = path.join("generalized-bicycle.json");
    fs::write(&spec, contents).expect("spec fixture should be writable");
    spec
}

fn run_qec_code_in_process_os(args: Vec<OsString>) -> Result<String, QecError> {
    let mut argv = vec![OsString::from("qec-code")];
    argv.extend(args);
    run(Cli::parse_from(argv))
}

#[test]
fn generalized_bicycle_order5_matches_fixture() {
    let result = construct_css(CssFamilySpec::GeneralizedBicycle(fixture_spec()).into()).unwrap();

    assert_eq!(result.construction_id, "generalized_bicycle");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::GeneralizedBicycle)
    );
    assert_eq!(result.normalized_parameters["order"], serde_json::json!(5));
    assert_eq!(
        result.normalized_parameters["a_exponents"],
        serde_json::json!([0, 1])
    );
    assert_eq!(
        result.normalized_parameters["b_exponents"],
        serde_json::json!([0, 2])
    );
    assert_eq!(result.provenance.adapter, "generalized_bicycle");
    assert_eq!(result.provenance.source, "CssFamilySpec::GeneralizedBicycle");
    assert!(result
        .provenance
        .normalized_input_digest
        .starts_with("sha256:"));

    assert_eq!(result.stats.n, 10);
    assert_eq!(result.stats.m_x, 5);
    assert_eq!(result.stats.m_z, 5);
    assert_eq!(result.stats.rank_x, 4);
    assert_eq!(result.stats.rank_z, 4);
    assert_eq!(result.stats.k, 2);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, fixture_hx());
    assert_eq!(result.checks.h_z, fixture_hz());
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0,1],"b_exponents":[0,2]}"#,
    )
    .unwrap();
    assert_eq!(parsed, CssFamilySpec::GeneralizedBicycle(fixture_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&parsed_result).unwrap()
    );

    let unsorted = construct_css(
        CssFamilySpec::GeneralizedBicycle(GeneralizedBicycleSpec {
            order: 5,
            a_exponents: vec![1, 0],
            b_exponents: vec![2, 0],
        })
        .into(),
    )
    .unwrap();
    assert_eq!(
        unsorted.normalized_parameters["a_exponents"],
        serde_json::json!([0, 1])
    );
    assert_eq!(unsorted.checks.h_x, fixture_hx());

    let dir = tempdir().unwrap();
    let spec = write_spec(
        dir.path(),
        r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0,1],"b_exponents":[0,2]}"#,
    );
    let output = std::process::Command::new(qec_code_bin())
        .args(["code", "css", "construct", "--spec"])
        .arg(&spec)
        .arg("hx")
        .output()
        .expect("qec-code binary should run");
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(
        stdout,
        SparseRowsMatrix::new(10, fixture_hx()).unwrap().to_json_string()
    );

    let in_process = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css"),
        OsString::from("construct"),
        OsString::from("--spec"),
        spec.into_os_string(),
        OsString::from("hz"),
    ])
    .unwrap();
    assert_eq!(
        in_process,
        SparseRowsMatrix::new(10, fixture_hz()).unwrap().to_json_string()
    );
}

#[test]
fn generalized_bicycle_rejects_invalid_exponents() {
    for (case, expected) in [
        (
            GeneralizedBicycleSpec {
                order: 0,
                a_exponents: vec![0],
                b_exponents: vec![0],
            },
            "order must be nonzero",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0, 5],
                b_exponents: vec![0],
            },
            "a_exponents exponent 5 is out of range for order 5",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0, 1, 1],
                b_exponents: vec![0],
            },
            "a_exponents contains duplicate exponent 1",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![],
                b_exponents: vec![0],
            },
            "a_exponents must not be empty",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0],
                b_exponents: vec![],
            },
            "b_exponents must not be empty",
        ),
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::GeneralizedBicycle(case).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "generalized_bicycle" && reason == expected
        ));
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0],"b_exponents":[0,"x"]}"#
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "generalized_bicycle"
                && reason == "b_exponents[1] must be a nonnegative integer"
    ));
}
```

- [ ] **Step 2: Run the positive test and confirm it fails**

Run:

```bash
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_order5_matches_fixture -- --exact
```

Expected: FAIL because `GeneralizedBicycleSpec` and the family variant do not exist yet.

- [ ] **Step 3: Run the negative test and confirm it fails**

Run:

```bash
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_rejects_invalid_exponents -- --exact
```

Expected: FAIL because `GeneralizedBicycleSpec` and the family variant do not exist yet.

- [ ] **Step 4: Commit the failing tests**

Run:

```bash
git add qec-code/tests/generalized_bicycle.rs
git commit -m "test: add generalized bicycle constructor coverage"
```

### Task 2: Implement the Cyclic Constructor and Shared Contract Routing

**Files:**
- Create: `qec-code/src/codes/generalized_bicycle.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`

**Interfaces:**
- Consumes: tests from Task 1 and existing `SparseGf2Matrix`.
- Produces: public `GeneralizedBicycleSpec`, `generalized_bicycle_sparse_checks`, callable `CssFamilySpec::GeneralizedBicycle`, JSON parsing for `construction = "generalized_bicycle"`, metadata, stats, and fixture distance annotations.

- [ ] **Step 1: Add the generalized-bicycle module**

Create `qec-code/src/codes/generalized_bicycle.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

pub const GENERALIZED_BICYCLE_CONSTRUCTION_ID: &str = "generalized_bicycle";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralizedBicycleSpec {
    pub order: usize,
    pub a_exponents: Vec<usize>,
    pub b_exponents: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedBicycleSparseChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub normalized_spec: GeneralizedBicycleSpec,
}

pub fn generalized_bicycle_sparse_checks(
    spec: &GeneralizedBicycleSpec,
) -> Result<GeneralizedBicycleSparseChecks> {
    let normalized_spec = normalize_spec(spec)?;
    let a = cyclic_circulant(normalized_spec.order, &normalized_spec.a_exponents)?;
    let b = cyclic_circulant(normalized_spec.order, &normalized_spec.b_exponents)?;
    let h_x = a.hconcat(&b)?;
    let h_z = b.transpose()?.hconcat(&a.transpose()?)?;

    Ok(GeneralizedBicycleSparseChecks {
        num_cols: h_x.num_cols(),
        h_x: h_x.rows().to_vec(),
        h_z: h_z.rows().to_vec(),
        normalized_spec,
    })
}

pub fn generalized_bicycle_known_distances(
    spec: &GeneralizedBicycleSpec,
) -> Option<(usize, usize)> {
    (spec.order == 5 && spec.a_exponents == [0, 1] && spec.b_exponents == [0, 2])
        .then_some((3, 3))
}

fn normalize_spec(spec: &GeneralizedBicycleSpec) -> Result<GeneralizedBicycleSpec> {
    if spec.order == 0 {
        return Err(invalid("order must be nonzero"));
    }
    Ok(GeneralizedBicycleSpec {
        order: spec.order,
        a_exponents: normalize_exponents("a_exponents", spec.order, &spec.a_exponents)?,
        b_exponents: normalize_exponents("b_exponents", spec.order, &spec.b_exponents)?,
    })
}

fn normalize_exponents(
    parameter: &'static str,
    order: usize,
    exponents: &[usize],
) -> Result<Vec<usize>> {
    if exponents.is_empty() {
        return Err(invalid(format!("{parameter} must not be empty")));
    }

    let mut normalized = Vec::with_capacity(exponents.len());
    for &exponent in exponents {
        if exponent >= order {
            return Err(invalid(format!(
                "{parameter} exponent {exponent} is out of range for order {order}"
            )));
        }
        normalized.push(exponent);
    }
    normalized.sort_unstable();

    for window in normalized.windows(2) {
        if window[0] == window[1] {
            return Err(invalid(format!(
                "{parameter} contains duplicate exponent {}",
                window[0]
            )));
        }
    }

    Ok(normalized)
}

fn cyclic_circulant(order: usize, exponents: &[usize]) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::with_capacity(order);
    for row in 0..order {
        rows.push(
            exponents
                .iter()
                .map(|&exponent| (row + exponent) % order)
                .collect(),
        );
    }
    SparseGf2Matrix::new(order, order, rows)
}

fn invalid(reason: impl Into<String>) -> QecError {
    QecError::InvalidCssConstruction {
        construction: GENERALIZED_BICYCLE_CONSTRUCTION_ID.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order5_rows_match_issue_fixture() {
        let checks = generalized_bicycle_sparse_checks(&GeneralizedBicycleSpec {
            order: 5,
            a_exponents: vec![0, 1],
            b_exponents: vec![0, 2],
        })
        .unwrap();

        assert_eq!(checks.num_cols, 10);
        assert_eq!(
            checks.h_x,
            vec![
                vec![0, 1, 5, 7],
                vec![1, 2, 6, 8],
                vec![2, 3, 7, 9],
                vec![3, 4, 5, 8],
                vec![0, 4, 6, 9],
            ]
        );
        assert_eq!(
            checks.h_z,
            vec![
                vec![0, 3, 5, 9],
                vec![1, 4, 5, 6],
                vec![0, 2, 6, 7],
                vec![1, 3, 7, 8],
                vec![2, 4, 8, 9],
            ]
        );
    }
}
```

- [ ] **Step 2: Register the module**

In `qec-code/src/codes/mod.rs`, add:

```rust
pub mod generalized_bicycle;
```

- [ ] **Step 3: Extend imports and public types in `family_contract.rs`**

Add imports:

```rust
use crate::codes::generalized_bicycle::{
    GENERALIZED_BICYCLE_CONSTRUCTION_ID, GeneralizedBicycleSpec,
    generalized_bicycle_known_distances, generalized_bicycle_sparse_checks,
};
pub use crate::codes::generalized_bicycle::GeneralizedBicycleSpec;
```

Extend `CssFamilySpec`:

```rust
pub enum CssFamilySpec {
    Surface(SurfaceFamilySpec),
    QuantumTanner(QuantumTannerSpec),
    GeneralizedBicycle(GeneralizedBicycleSpec),
    Color666(Color666FamilySpec),
}
```

Update `callable_requested_family_ids()` to include
`RequestedFamilyId::GeneralizedBicycle`.

- [ ] **Step 4: Route construction and metadata**

In `construct_css`, add a `CssFamilySpec::GeneralizedBicycle` arm before
`Color666`:

```rust
CssConstructionSpec::Family(CssFamilySpec::GeneralizedBicycle(spec)) => {
    let checks = generalized_bicycle_sparse_checks(&spec)?;
    let normalized = checks.normalized_spec;
    let mut parameters = BTreeMap::new();
    parameters.insert("order".to_owned(), Value::from(normalized.order));
    parameters.insert(
        "a_exponents".to_owned(),
        serde_json::to_value(&normalized.a_exponents).expect("serializable exponents"),
    );
    parameters.insert(
        "b_exponents".to_owned(),
        serde_json::to_value(&normalized.b_exponents).expect("serializable exponents"),
    );
    let known_distances = generalized_bicycle_known_distances(&normalized);
    construction_result(
        GENERALIZED_BICYCLE_CONSTRUCTION_ID,
        Some(RequestedFamilyId::GeneralizedBicycle),
        parameters,
        checks.num_cols,
        checks.h_x,
        checks.h_z,
        GENERALIZED_BICYCLE_CONSTRUCTION_ID,
        "CssFamilySpec::GeneralizedBicycle",
        known_distances,
    )
}
```

- [ ] **Step 5: Parse versioned JSON specs**

In `parse_css_construction_json`, add:

```rust
"generalized_bicycle" => Ok(CssFamilySpec::GeneralizedBicycle(GeneralizedBicycleSpec {
    order: required_usize(object, "order", construction)?,
    a_exponents: required_usize_array(object, "a_exponents", construction)?,
    b_exponents: required_usize_array(object, "b_exponents", construction)?,
})
.into()),
```

Add this helper near `required_usize`:

```rust
fn required_usize_array(
    object: &Map<String, Value>,
    field: &'static str,
    construction: &str,
) -> Result<Vec<usize>> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| QecError::InvalidCssConstruction {
            construction: construction.to_owned(),
            reason: format!("missing or invalid {field}"),
        })?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value.as_u64().ok_or_else(|| QecError::InvalidCssConstruction {
                construction: construction.to_owned(),
                reason: format!("{field}[{index}] must be a nonnegative integer"),
            })?;
            usize::try_from(value).map_err(|_| QecError::InvalidCssConstruction {
                construction: construction.to_owned(),
                reason: format!("{field}[{index}] is outside usize range"),
            })
        })
        .collect()
}
```

- [ ] **Step 6: Update the callable-family contract test**

In `qec-code/tests/family_contract.rs`, update
`planned_families_have_no_callable_stub` to expect:

```rust
&[
    RequestedFamilyId::Surface,
    RequestedFamilyId::QuantumTanner,
    RequestedFamilyId::GeneralizedBicycle,
    RequestedFamilyId::Color666,
]
```

- [ ] **Step 7: Run the exact tests**

Run:

```bash
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_order5_matches_fixture -- --exact
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_rejects_invalid_exponents -- --exact
```

Expected: PASS.

- [ ] **Step 8: Commit the implementation**

Run:

```bash
git add qec-code/src/codes/generalized_bicycle.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs
git commit -m "feat: add generalized bicycle css constructor"
```

### Task 3: Verify and Polish the Branch

**Files:**
- Modify only if verification exposes formatting, documentation, or integration issues.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: rustfmt-clean, fully verified branch ready for PR.

- [ ] **Step 1: Format the touched Rust files**

Run:

```bash
cargo fmt -- qec-code/src/codes/generalized_bicycle.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/generalized_bicycle.rs
```

Expected: command exits `0`.

- [ ] **Step 2: Run issue-required exact tests**

Run:

```bash
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_order5_matches_fixture -- --exact
cargo test -p qec-code --test generalized_bicycle generalized_bicycle_rejects_invalid_exponents -- --exact
```

Expected: both commands pass.

- [ ] **Step 3: Run contract regression tests**

Run:

```bash
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
cargo test -p qec-code --test family_contract inline_json_and_rust_routes_lower_to_same_spec -- --exact
```

Expected: both commands pass.

- [ ] **Step 4: Run full required verification**

Run:

```bash
cargo test
```

Expected: full workspace test suite passes.

- [ ] **Step 5: Commit any verification fixes**

If formatting or verification changed files, run:

```bash
git add qec-code/src/codes/generalized_bicycle.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/generalized_bicycle.rs
git commit -m "fix: polish generalized bicycle constructor"
```

If no files changed, do not create an empty commit.

## Plan Self-Review

- Spec coverage: Task 1 covers exact rows, stats, orthogonality, metadata, CLI, and negative controls; Task 2 implements the API and routing; Task 3 verifies.
- Placeholder scan: no `TBD`, `TODO`, or unspecified test command remains.
- Type consistency: `GeneralizedBicycleSpec`, `CssFamilySpec::GeneralizedBicycle`, and `generalized_bicycle_sparse_checks` names match across tasks.
