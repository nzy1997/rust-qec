# Issue 559 La-Cross CSS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the open and periodic parameterized La-cross CSS family requested by GitHub issue #559.

**Architecture:** Add a dedicated `qec_code::codes::la_cross` module that validates `LaCrossSpec`, generates the deterministic classical sparse GF(2) check matrix for open or periodic boundary mode, and exposes known fixture distance metadata. Extend the shared `family_contract` routing so Rust API and versioned JSON CLI requests lower to `CssFamilySpec::LaCross`, then build the CSS checks by passing the generated classical matrix through the existing hypergraph-product constructor.

**Tech Stack:** Rust 2024, `qec-code`, `serde`, Cargo integration tests, existing sparse GF(2) primitives, existing hypergraph-product constructor, existing CSS exact-distance helper.

## Global Constraints

- Use `.AGENTS/AGENTS.md` repository rules.
- Use `CssFamilySpec::LaCross(LaCrossSpec)` as the Rust API.
- Use explicit enum variants `LaCrossBoundary::Open` and `LaCrossBoundary::Periodic`; JSON strings are exactly `open` and `periodic`.
- Generate the classical matrix from `h(x)=1+x+x^z` and pass identical left/right inputs through the general HGP constructor from issue #556.
- For open `seed_length=5`, `reach=2`, generated classical rows are exactly `[[0,1,2], [1,2,3], [2,3,4]]`.
- Open `seed_length=5`, `reach=2` must return `n=34`, `m_x=15`, `m_z=15`, `rank_x=15`, `rank_z=15`, `k=4`, `d_x=Some(3)`, `d_z=Some(3)`, and exact CSS distance 3.
- Periodic `seed_length=5`, `reach=2` must return `n=50`, `m_x=25`, `m_z=25`, and orthogonal CSS checks.
- Normalized parameters must include deterministic `seed_length`, `reach`, `boundary`, and canonical generated `classical_check`.
- Provenance must be deterministic with `adapter = "la_cross"` and `source = "CssFamilySpec::LaCross"`.
- Reject reach zero, reach outside the seed length, an invalid boundary string, and dimensions that overflow the HGP result.
- Do not add dependencies.
- Required verification commands:
  - `cargo test -p qec-code --test la_cross la_cross_open_5_2_matches_fixture -- --exact`
  - `cargo test -p qec-code --test la_cross la_cross_periodic_5_2_is_orthogonal -- --exact`
  - `cargo test -p qec-code --test la_cross la_cross_rejects_invalid_reach -- --exact`
  - `cargo test`

---

## File Structure

- Create `qec-code/tests/la_cross.rs`: issue fixture tests, Rust API assertions, JSON parser assertions, CLI `hx`/`hz`/`metadata` assertions, exact distance verification, and negative controls.
- Create `qec-code/src/codes/la_cross.rs`: typed spec, boundary enum, validation, deterministic classical row generation, HGP overflow preflight, known fixture distances, and module unit coverage for generated classical rows.
- Modify `qec-code/src/codes/mod.rs`: register the La-cross module.
- Modify `qec-code/src/family_contract.rs`: re-export La-cross types, add the `CssFamilySpec::LaCross` variant, include La-cross in callable family IDs, route construction, parse versioned JSON, and wrap HGP checks with La-cross metadata/provenance.
- Modify `qec-code/tests/family_contract.rs`: update the callable-family contract list to include `RequestedFamilyId::LaCross`.

## Task 1: Add La-Cross Contract Tests

**Files:**
- Create: `qec-code/tests/la_cross.rs`

**Interfaces:**
- Consumes: planned `LaCrossBoundary`, `LaCrossSpec`, `CssFamilySpec::LaCross`, `parse_css_construction_json`, `construct_css`, `verify_css_orthogonality`, CLI `code css construct --spec`, `CssCode`, and `compute_distance`.
- Produces: failing integration tests named exactly as the issue verification commands require.

- [ ] **Step 1: Write the failing integration test file**

Create `qec-code/tests/la_cross.rs`:

```rust
use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, run};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    CssFamilySpec, LaCrossBoundary, LaCrossSpec, RequestedFamilyId, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};
use tempfile::tempdir;

fn open_spec() -> LaCrossSpec {
    LaCrossSpec {
        seed_length: 5,
        reach: 2,
        boundary: LaCrossBoundary::Open,
    }
}

fn periodic_spec() -> LaCrossSpec {
    LaCrossSpec {
        seed_length: 5,
        reach: 2,
        boundary: LaCrossBoundary::Periodic,
    }
}

fn open_json() -> &'static str {
    r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"open"}"#
}

fn periodic_json() -> &'static str {
    r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"periodic"}"#
}

fn expected_open_classical_rows() -> Vec<Vec<usize>> {
    vec![vec![0, 1, 2], vec![1, 2, 3], vec![2, 3, 4]]
}

fn expected_periodic_classical_rows() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2],
        vec![1, 2, 3],
        vec![2, 3, 4],
        vec![0, 3, 4],
        vec![0, 1, 4],
    ]
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
}

fn css_code_from_result(result: &qec_code::family_contract::CssConstructionResult) -> CssCode {
    let hx = SparseRowsMatrix::new(result.stats.n, result.checks.h_x.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(result.stats.n, result.checks.h_z.clone())
        .unwrap()
        .to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

fn write_spec(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("spec should be writable");
    path
}

fn cli_construct_output(spec: &Path, output: &str) -> String {
    run(Cli::parse_from([
        "qec-code",
        "code",
        "css",
        "construct",
        "--spec",
        spec.to_str().expect("spec path should be UTF-8"),
        output,
    ]))
    .unwrap()
}

#[test]
fn la_cross_open_5_2_matches_fixture() {
    let result = construct_css(CssFamilySpec::LaCross(open_spec()).into()).unwrap();

    assert_eq!(result.schema_version, 1);
    assert_eq!(result.construction_id, "la_cross");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::LaCross));
    assert_eq!(result.normalized_parameters["seed_length"], serde_json::json!(5));
    assert_eq!(result.normalized_parameters["reach"], serde_json::json!(2));
    assert_eq!(result.normalized_parameters["boundary"], serde_json::json!("open"));
    assert_eq!(
        result.normalized_parameters["classical_check"],
        serde_json::json!({"num_cols": 5, "rows": expected_open_classical_rows()})
    );
    assert_eq!(result.provenance.adapter, "la_cross");
    assert_eq!(result.provenance.source, "CssFamilySpec::LaCross");
    assert!(result.provenance.normalized_input_digest.starts_with("sha256:"));

    assert_eq!(result.stats.n, 34);
    assert_eq!(result.stats.m_x, 15);
    assert_eq!(result.stats.m_z, 15);
    assert_eq!(result.stats.rank_x, 15);
    assert_eq!(result.stats.rank_z, 15);
    assert_eq!(result.stats.k, 4);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(
        compute_distance(css_code_from_result(&result).code())
            .unwrap()
            .distance,
        3
    );

    let parsed = parse_css_construction_json(open_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::LaCross(open_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&parsed_result).unwrap()
    );

    let repeated = construct_css(CssFamilySpec::LaCross(open_spec()).into()).unwrap();
    assert_eq!(
        result.provenance.normalized_input_digest,
        repeated.provenance.normalized_input_digest
    );
    assert_eq!(
        serde_json::to_string(&result.normalized_parameters).unwrap(),
        serde_json::to_string(&repeated.normalized_parameters).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = write_spec(dir.path(), "la-cross-open.json", open_json());
    let hx_json: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "hx")).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "hz")).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "metadata")).unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 34);
    assert_eq!(hx_json["rows"], serde_json::json!(result.checks.h_x));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 34);
    assert_eq!(hz_json["rows"], serde_json::json!(result.checks.h_z));
    assert_eq!(metadata["construction_id"], "la_cross");
    assert_eq!(metadata["requested_family_id"], "la_cross");
    assert_eq!(
        metadata["normalized_parameters"]["classical_check"]["rows"],
        serde_json::json!(expected_open_classical_rows())
    );
    assert_eq!(metadata["provenance"]["adapter"], "la_cross");
}

#[test]
fn la_cross_periodic_5_2_is_orthogonal() {
    let result = construct_css(CssFamilySpec::LaCross(periodic_spec()).into()).unwrap();

    assert_eq!(result.construction_id, "la_cross");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::LaCross));
    assert_eq!(result.normalized_parameters["seed_length"], serde_json::json!(5));
    assert_eq!(result.normalized_parameters["reach"], serde_json::json!(2));
    assert_eq!(
        result.normalized_parameters["boundary"],
        serde_json::json!("periodic")
    );
    assert_eq!(
        result.normalized_parameters["classical_check"],
        serde_json::json!({"num_cols": 5, "rows": expected_periodic_classical_rows()})
    );
    assert_eq!(result.stats.n, 50);
    assert_eq!(result.stats.m_x, 25);
    assert_eq!(result.stats.m_z, 25);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let parsed = parse_css_construction_json(periodic_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::LaCross(periodic_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(
        serde_json::to_string(&parsed_result.normalized_parameters).unwrap(),
        serde_json::to_string(&result.normalized_parameters).unwrap()
    );
}

#[test]
fn la_cross_rejects_invalid_reach() {
    for (spec, expected) in [
        (
            LaCrossSpec {
                seed_length: 5,
                reach: 0,
                boundary: LaCrossBoundary::Open,
            },
            "reach must be nonzero",
        ),
        (
            LaCrossSpec {
                seed_length: 5,
                reach: 5,
                boundary: LaCrossBoundary::Open,
            },
            "reach must be less than seed_length",
        ),
        (
            LaCrossSpec {
                seed_length: 1,
                reach: 1,
                boundary: LaCrossBoundary::Periodic,
            },
            "seed_length must be at least 2",
        ),
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::LaCross(spec).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "la_cross" && reason.contains(expected)
        ));
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"closed"}"#
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "la_cross" && reason.contains("unknown la_cross boundary")
    ));

    assert!(matches!(
        construct_css(
            CssFamilySpec::LaCross(LaCrossSpec {
                seed_length: usize::MAX,
                reach: 1,
                boundary: LaCrossBoundary::Periodic,
            })
            .into()
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "la_cross" && reason.contains("overflow")
    ));
}
```

- [ ] **Step 2: Run the required tests and confirm RED**

Run:

```bash
cargo test -p qec-code --test la_cross la_cross_open_5_2_matches_fixture -- --exact
cargo test -p qec-code --test la_cross la_cross_periodic_5_2_is_orthogonal -- --exact
cargo test -p qec-code --test la_cross la_cross_rejects_invalid_reach -- --exact
```

Expected: FAIL because `LaCrossSpec`, `LaCrossBoundary`, and `CssFamilySpec::LaCross` do not exist.

- [ ] **Step 3: Commit the failing tests**

Run:

```bash
git add qec-code/tests/la_cross.rs
git commit -m "test: add la-cross constructor coverage"
```

## Task 2: Implement La-Cross Module and Contract Routing

**Files:**
- Create: `qec-code/src/codes/la_cross.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`

**Interfaces:**
- Consumes: Task 1 tests, `CssClassicalCheckSpec`, `HypergraphProductSpec`, `SparseGf2Matrix`, `construct_css`, and `construction_result`.
- Produces: public `LaCrossSpec`, public `LaCrossBoundary`, callable `CssFamilySpec::LaCross`, versioned JSON parsing for `construction = "la_cross"`, deterministic normalized metadata, and HGP-backed CSS checks.

- [ ] **Step 1: Add the La-cross module**

Create `qec-code/src/codes/la_cross.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};
use crate::family_contract::CssClassicalCheckSpec;
use crate::sparse_gf2::SparseGf2Matrix;

pub const LA_CROSS_CONSTRUCTION_ID: &str = "la_cross";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaCrossBoundary {
    Open,
    Periodic,
}

impl LaCrossBoundary {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Periodic => "periodic",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "periodic" => Ok(Self::Periodic),
            _ => Err(invalid(format!("unknown la_cross boundary {value}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaCrossSpec {
    pub seed_length: usize,
    pub reach: usize,
    pub boundary: LaCrossBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaCrossClassicalCheck {
    pub spec: LaCrossSpec,
    pub check: CssClassicalCheckSpec,
}

pub(crate) fn la_cross_classical_check(spec: &LaCrossSpec) -> Result<LaCrossClassicalCheck> {
    validate_la_cross_spec(spec)?;
    let rows = match spec.boundary {
        LaCrossBoundary::Open => open_rows(spec.seed_length, spec.reach)?,
        LaCrossBoundary::Periodic => periodic_rows(spec.seed_length, spec.reach)?,
    };
    let matrix = SparseGf2Matrix::new(rows.len(), spec.seed_length, rows)?;
    Ok(LaCrossClassicalCheck {
        spec: spec.clone(),
        check: CssClassicalCheckSpec {
            num_cols: matrix.num_cols(),
            rows: matrix.rows().to_vec(),
        },
    })
}

pub(crate) fn la_cross_known_distances(spec: &LaCrossSpec) -> Option<(usize, usize)> {
    (spec.seed_length == 5 && spec.reach == 2 && spec.boundary == LaCrossBoundary::Open)
        .then_some((3, 3))
}

fn validate_la_cross_spec(spec: &LaCrossSpec) -> Result<()> {
    if spec.seed_length < 2 {
        return Err(invalid(format!(
            "seed_length must be at least 2, got {}",
            spec.seed_length
        )));
    }
    if spec.reach == 0 {
        return Err(invalid("reach must be nonzero"));
    }
    if spec.reach >= spec.seed_length {
        return Err(invalid(format!(
            "reach must be less than seed_length, got reach {} and seed_length {}",
            spec.reach, spec.seed_length
        )));
    }
    preflight_hgp_dimensions(spec)
}

fn preflight_hgp_dimensions(spec: &LaCrossSpec) -> Result<()> {
    let row_count = classical_row_count(spec);
    spec.seed_length
        .checked_mul(spec.seed_length)
        .and_then(|left| row_count.checked_mul(row_count).and_then(|right| left.checked_add(right)))
        .ok_or_else(|| overflow("HGP data qubit count"))?;
    row_count
        .checked_mul(spec.seed_length)
        .ok_or_else(|| overflow("HGP check count"))?;
    Ok(())
}

fn classical_row_count(spec: &LaCrossSpec) -> usize {
    match spec.boundary {
        LaCrossBoundary::Open => spec.seed_length - spec.reach,
        LaCrossBoundary::Periodic => spec.seed_length,
    }
}

fn open_rows(seed_length: usize, reach: usize) -> Result<Vec<Vec<usize>>> {
    let row_count = seed_length - reach;
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count)
        .map_err(|_| overflow("classical row allocation"))?;
    for row in 0..row_count {
        rows.push(vec![row, row + 1, row + reach]);
    }
    Ok(rows)
}

fn periodic_rows(seed_length: usize, reach: usize) -> Result<Vec<Vec<usize>>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(seed_length)
        .map_err(|_| overflow("classical row allocation"))?;
    for row in 0..seed_length {
        rows.push(vec![
            row,
            periodic_add(row, 1, seed_length),
            periodic_add(row, reach, seed_length),
        ]);
    }
    Ok(rows)
}

fn periodic_add(value: usize, shift: usize, period: usize) -> usize {
    let shift = shift % period;
    if shift == 0 {
        value
    } else if value >= period - shift {
        value - (period - shift)
    } else {
        value + shift
    }
}

fn invalid(reason: impl Into<String>) -> QecError {
    QecError::InvalidCssConstruction {
        construction: LA_CROSS_CONSTRUCTION_ID.to_owned(),
        reason: reason.into(),
    }
}

fn overflow(operation: &'static str) -> QecError {
    invalid(format!("la_cross dimension overflow during {operation}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_5_2_classical_rows_match_issue_fixture() {
        let check = la_cross_classical_check(&LaCrossSpec {
            seed_length: 5,
            reach: 2,
            boundary: LaCrossBoundary::Open,
        })
        .unwrap();

        assert_eq!(check.check.num_cols, 5);
        assert_eq!(
            check.check.rows,
            vec![vec![0, 1, 2], vec![1, 2, 3], vec![2, 3, 4]]
        );
    }

    #[test]
    fn periodic_5_2_rows_wrap_deterministically() {
        let check = la_cross_classical_check(&LaCrossSpec {
            seed_length: 5,
            reach: 2,
            boundary: LaCrossBoundary::Periodic,
        })
        .unwrap();

        assert_eq!(
            check.check.rows,
            vec![
                vec![0, 1, 2],
                vec![1, 2, 3],
                vec![2, 3, 4],
                vec![0, 3, 4],
                vec![0, 1, 4],
            ]
        );
    }
}
```

- [ ] **Step 2: Register the module**

In `qec-code/src/codes/mod.rs`, add:

```rust
pub mod la_cross;
```

- [ ] **Step 3: Extend imports and public types in `family_contract.rs`**

In `qec-code/src/family_contract.rs`, add:

```rust
pub use crate::codes::la_cross::{LaCrossBoundary, LaCrossSpec};
use crate::codes::la_cross::{
    LA_CROSS_CONSTRUCTION_ID, la_cross_classical_check, la_cross_known_distances,
};
```

Extend `CssFamilySpec`:

```rust
pub enum CssFamilySpec {
    Surface(SurfaceFamilySpec),
    QuantumTanner(QuantumTannerSpec),
    GeneralizedBicycle(GeneralizedBicycleSpec),
    LaCross(LaCrossSpec),
    Toric3d(Toric3dSpec),
    RandomTwoBlock(RandomTwoBlockSpec),
    Color666(Color666FamilySpec),
    ShorLike(ShorLikeSpec),
    Directional(DirectionalCssSpec),
}
```

Update `CssFamilySpec::callable_requested_family_ids()` to include
`RequestedFamilyId::LaCross` immediately after
`RequestedFamilyId::GeneralizedBicycle`.

- [ ] **Step 4: Route La-cross construction**

In `construct_css`, add a match arm after generalized bicycle:

```rust
CssConstructionSpec::Family(CssFamilySpec::LaCross(spec)) => construct_la_cross(spec),
```

Add this helper near the other family helpers:

```rust
fn construct_la_cross(spec: LaCrossSpec) -> Result<CssConstructionResult> {
    let generated = la_cross_classical_check(&spec)?;
    let hgp = construct_hypergraph_product(HypergraphProductSpec {
        left: generated.check.clone(),
        right: generated.check.clone(),
    })?;
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "seed_length".to_owned(),
        Value::from(generated.spec.seed_length),
    );
    parameters.insert("reach".to_owned(), Value::from(generated.spec.reach));
    parameters.insert(
        "boundary".to_owned(),
        Value::from(generated.spec.boundary.as_str()),
    );
    parameters.insert(
        "classical_check".to_owned(),
        serde_json::to_value(&generated.check).expect("serializable classical check"),
    );
    let known_distances = la_cross_known_distances(&generated.spec);

    construction_result(
        LA_CROSS_CONSTRUCTION_ID,
        Some(RequestedFamilyId::LaCross),
        parameters,
        hgp.stats.n,
        hgp.checks.h_x,
        hgp.checks.h_z,
        LA_CROSS_CONSTRUCTION_ID,
        "CssFamilySpec::LaCross",
        known_distances,
    )
}
```

- [ ] **Step 5: Parse versioned JSON specs**

In `parse_css_construction_json`, add before `"random_two_block"`:

```rust
"la_cross" => {
    let spec = LaCrossSpec {
        seed_length: required_usize(object, "seed_length", construction)?,
        reach: required_usize(object, "reach", construction)?,
        boundary: LaCrossBoundary::parse(required_string(object, "boundary")?)?,
    };
    la_cross_classical_check(&spec)?;
    Ok(CssFamilySpec::LaCross(spec).into())
}
```

- [ ] **Step 6: Update family contract tests**

In `qec-code/tests/family_contract.rs`, add `RequestedFamilyId::LaCross` to
the expected callable list immediately after `RequestedFamilyId::GeneralizedBicycle`.

- [ ] **Step 7: Run exact tests to verify GREEN**

Run:

```bash
cargo test -p qec-code --test la_cross la_cross_open_5_2_matches_fixture -- --exact
cargo test -p qec-code --test la_cross la_cross_periodic_5_2_is_orthogonal -- --exact
cargo test -p qec-code --test la_cross la_cross_rejects_invalid_reach -- --exact
```

Expected: PASS.

- [ ] **Step 8: Commit the implementation**

Run:

```bash
git add qec-code/src/codes/la_cross.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs
git commit -m "feat: add la-cross css constructor"
```

## Task 3: Verify and Polish the Branch

**Files:**
- Modify only if formatting or verification exposes issues in touched files.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: rustfmt-clean branch with issue-required tests and full workspace verification.

- [ ] **Step 1: Format touched Rust files**

Run:

```bash
rustfmt --edition 2024 qec-code/src/codes/la_cross.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/la_cross.rs
```

Expected: command exits `0`.

- [ ] **Step 2: Run issue-required exact tests**

Run:

```bash
cargo test -p qec-code --test la_cross la_cross_open_5_2_matches_fixture -- --exact
cargo test -p qec-code --test la_cross la_cross_periodic_5_2_is_orthogonal -- --exact
cargo test -p qec-code --test la_cross la_cross_rejects_invalid_reach -- --exact
```

Expected: all commands pass.

- [ ] **Step 3: Run contract regression tests**

Run:

```bash
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
cargo test -p qec-code --test family_contract unified_family_contract_preserves_requested_family_ids -- --exact
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
git add qec-code/src/codes/la_cross.rs qec-code/src/codes/mod.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/la_cross.rs
git commit -m "fix: polish la-cross constructor"
```

If no files changed, do not create an empty commit.

## Plan Self-Review

- Spec coverage: Task 1 covers exact open rows, open stats and distance, periodic stats and orthogonality, deterministic normalized parameters/provenance, Rust API, JSON parser, CLI output, and negative controls. Task 2 implements the module and shared routing. Task 3 verifies.
- Placeholder scan: no `TBD`, `TODO`, `FIXME`, or unspecified code step remains.
- Type consistency: `LaCrossSpec`, `LaCrossBoundary`, `CssFamilySpec::LaCross`, and `la_cross_classical_check` names match across tests, module, and contract routing.
