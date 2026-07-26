# Issue 567 Toric 3D CSS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a parameterized periodic 3D toric CSS family with qubits on edges, X checks on vertices, Z checks on plaquettes, common-family API/CLI exposure, exact 3x3x3 fixtures, and required negative controls.

**Architecture:** Implement a focused `qec_code::codes::toric_3d` module that validates `Toric3dSpec`, builds `boundary_1` and `boundary_2` with `BinaryBoundaryMap`, obtains checks from `BinaryChainComplex::css_view(1)`, and exposes analytic distances. Integrate that constructor into `CssFamilySpec`, `parse_css_construction_json`, `CssConstructionSpec::from_inline`, `built_in_css_checks`, and existing CLI routes.

**Tech Stack:** Rust 2024, existing `SparseGf2Matrix`, `BinaryBoundaryMap`, `BinaryChainComplex`, `SparseRowsMatrix`, `QecError`, `serde_json`, and Cargo integration tests.

## Global Constraints

- Issue #567 requires `Toric3dSpec { lx, ly, lz }`.
- Every period must be at least 3.
- For volume `V`, report `n=3V`, `m_x=V`, `m_z=3V`, `rank_x=V-1`, `rank_z=2V-2`, and `k=3`.
- For `lx=ly=lz=3`, report `n=81`, `m_x=27`, `m_z=81`, `rank_x=26`, `rank_z=52`, and `k=3`.
- For `lx=ly=lz=3`, exact leading rows are `H_X[0] = [0,18,27,33,54,56]`, `H_Z_xy[0] = [0,3,27,36]`, `H_Z_xz[0] = [0,1,54,63]`, and `H_Z_yz[0] = [27,28,54,57]`.
- Every X row has weight 6 and every Z row has weight 4.
- The 3x3x3 fixture has analytic `d_z=3`, `d_x=9`, and overall distance 3.
- Exact full checks are fixture-tested and orthogonal.
- Rust API and CLI accept arbitrary valid rectangular periods.
- Reject a period below 3, coordinate or dimension overflow, and a deliberately corrupted boundary composition.
- Keep the sparse-row JSON schema unchanged.
- Use the shared boundary-map layer for construction validation.

---

## File Structure

- Create `qec-code/src/codes/toric_3d.rs`: period validation, checked indexing, boundary construction, CSS check export, analytic distances, and a unit negative-control test that corrupts one plaquette boundary.
- Modify `qec-code/src/codes/mod.rs`: expose `pub mod toric_3d`.
- Modify `qec-code/src/family_contract.rs`: add `Toric3dSpec` to `CssFamilySpec`, JSON parsing, normalized parameters, construction result routing, and callable family list.
- Modify `qec-code/src/codes/built_in_css.rs`: add `toric_3d:lx=...,ly=...,lz=...` parser/catalog/check construction.
- Modify `qec-code/src/cli.rs` only if the existing route needs imports adjusted; keep command shape unchanged.
- Create `qec-code/tests/toric_3d.rs`: integration tests for exact fixture, rectangular specs, period rejection, overflow rejection, and CLI/API routes.
- Create `qec-code/tests/fixtures/css/toric_3d_3x3x3_hx.json` and `qec-code/tests/fixtures/css/toric_3d_3x3x3_hz.json`: exact sparse-row fixtures.
- Modify `qec-code/tests/family_contract.rs`: common-family contract tests for `Toric3d`.
- Modify `qec-code/tests/cli.rs`: catalog and export coverage for `toric_3d`.

---

### Task 1: Add Failing Toric 3D Fixtures And Integration Tests

**Files:**
- Create: `qec-code/tests/toric_3d.rs`
- Create: `qec-code/tests/fixtures/css/toric_3d_3x3x3_hx.json`
- Create: `qec-code/tests/fixtures/css/toric_3d_3x3x3_hz.json`

**Interfaces:**
- Consumes: planned `qec_code::codes::toric_3d::{toric_3d_css_checks, Toric3dDistances, Toric3dSpec}`
- Consumes: planned `qec_code::family_contract::{construct_css, parse_css_construction_json, CssFamilySpec}`
- Produces: failing tests that define the constructor, fixture, stats, distance, rectangular, and rejection behavior

- [ ] **Step 1: Write the failing constructor/fixture tests**

Create `qec-code/tests/toric_3d.rs` with helpers that read sparse-row fixtures, verify row weights, and check orthogonality via the family contract. The main fixture test must use the checked-in fixture files for the full row comparison and direct leading-row assertions:

```rust
use std::path::PathBuf;

use qec_code::codes::toric_3d::{toric_3d_css_checks, Toric3dDistances, Toric3dSpec};
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality, CssFamilySpec,
    RequestedFamilyId,
};
use qec_code::QecError;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_rows(name: &str) -> Vec<Vec<usize>> {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture should be readable"))
            .expect("fixture should be valid JSON");
    serde_json::from_value(value["rows"].clone()).expect("fixture rows should be arrays")
}

fn fixture_text(name: &str) -> String {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    std::fs::read_to_string(path)
        .expect("fixture should be readable")
        .trim_end_matches('\n')
        .to_owned()
}

fn assert_all_row_weights(rows: &[Vec<usize>], weight: usize) {
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), weight, "row {index} had support {row:?}");
    }
}

#[test]
fn toric_3d_3x3x3_matches_fixture() {
    let spec = Toric3dSpec { lx: 3, ly: 3, lz: 3 };
    let checks = toric_3d_css_checks(spec).unwrap();
    assert_eq!(checks.num_cols, 81);
    assert_eq!(
        checks.distances,
        Toric3dDistances { d_x: 9, d_z: 3, distance: 3 }
    );
    assert_eq!(checks.hx[0], vec![0, 18, 27, 33, 54, 56]);
    assert_eq!(checks.hz[0], vec![0, 3, 27, 36]);
    assert_eq!(checks.hz[27], vec![0, 1, 54, 63]);
    assert_eq!(checks.hz[54], vec![27, 28, 54, 57]);
    assert_eq!(checks.hx, fixture_rows("toric_3d_3x3x3_hx.json"));
    assert_eq!(checks.hz, fixture_rows("toric_3d_3x3x3_hz.json"));
    assert_all_row_weights(&checks.hx, 6);
    assert_all_row_weights(&checks.hz, 4);
    verify_css_orthogonality(checks.num_cols, &checks.hx, &checks.hz).unwrap();

    let result = construct_css(CssFamilySpec::Toric3d(spec).into()).unwrap();
    assert_eq!(result.construction_id, "toric_3d");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::Toric3d));
    assert_eq!(result.normalized_parameters["lx"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["ly"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["lz"], serde_json::json!(3));
    assert_eq!(result.stats.n, 81);
    assert_eq!(result.stats.m_x, 27);
    assert_eq!(result.stats.m_z, 81);
    assert_eq!(result.stats.rank_x, 26);
    assert_eq!(result.stats.rank_z, 52);
    assert_eq!(result.stats.k, 3);
    assert_eq!(result.checks.h_x, checks.hx);
    assert_eq!(result.checks.h_z, checks.hz);

    let hx_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_x).unwrap().to_json_string();
    let hz_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_z).unwrap().to_json_string();
    assert_eq!(hx_json, fixture_text("toric_3d_3x3x3_hx.json"));
    assert_eq!(hz_json, fixture_text("toric_3d_3x3x3_hz.json"));
}
```

- [ ] **Step 2: Write API, JSON, rectangular, and rejection tests**

Append these tests to the same file:

```rust
#[test]
fn toric_3d_accepts_rectangular_periods() {
    let spec = Toric3dSpec { lx: 3, ly: 4, lz: 5 };
    let checks = toric_3d_css_checks(spec).unwrap();
    assert_eq!(checks.num_cols, 180);
    assert_eq!(checks.hx.len(), 60);
    assert_eq!(checks.hz.len(), 180);
    assert_eq!(
        checks.distances,
        Toric3dDistances { d_x: 12, d_z: 3, distance: 3 }
    );
    assert_all_row_weights(&checks.hx, 6);
    assert_all_row_weights(&checks.hz, 4);
    verify_css_orthogonality(checks.num_cols, &checks.hx, &checks.hz).unwrap();

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"toric_3d","lx":3,"ly":4,"lz":5}"#,
    )
    .unwrap();
    assert_eq!(parsed, CssFamilySpec::Toric3d(spec).into());
    let result = construct_css(parsed).unwrap();
    assert_eq!(result.stats.n, 180);
    assert_eq!(result.stats.m_x, 60);
    assert_eq!(result.stats.m_z, 180);
    assert_eq!(result.stats.rank_x, 59);
    assert_eq!(result.stats.rank_z, 118);
    assert_eq!(result.stats.k, 3);
}

#[test]
fn toric_3d_rejects_degenerate_periods() {
    for (spec, parameter) in [
        (Toric3dSpec { lx: 2, ly: 3, lz: 3 }, "lx"),
        (Toric3dSpec { lx: 3, ly: 2, lz: 3 }, "ly"),
        (Toric3dSpec { lx: 3, ly: 3, lz: 2 }, "lz"),
    ] {
        assert_eq!(
            toric_3d_css_checks(spec),
            Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
                family: "toric_3d".to_owned(),
                parameter: parameter.to_owned(),
                value: 2,
            })
        );
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"toric_3d","lx":2,"ly":3,"lz":3}"#
        ),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter { family, parameter, value })
            if family == "toric_3d" && parameter == "lx" && value == 2
    ));
}

#[test]
fn toric_3d_rejects_overflowing_dimensions() {
    assert!(matches!(
        toric_3d_css_checks(Toric3dSpec {
            lx: usize::MAX,
            ly: 3,
            lz: 3,
        }),
        Err(QecError::SparseGf2DimensionOverflow { operation: "toric_3d" })
    ));
}
```

- [ ] **Step 3: Add exact sparse-row fixture files**

Create the two fixture JSON files with `format: "sparse_rows"`, `num_cols: 81`,
27 full X rows, and 81 full Z rows. The fixture data must follow the indexing in
the design: x, y, z edge blocks; xy, xz, yz plaquette blocks. The first rows in
the files must be the exact required leading rows from the issue.

- [ ] **Step 4: Run the focused test and verify it fails for the missing API**

Run:

```bash
cargo test -p qec-code --test toric_3d toric_3d_3x3x3_matches_fixture -- --exact
```

Expected: FAIL at compile time because `qec_code::codes::toric_3d` and
`CssFamilySpec::Toric3d` do not exist yet.

- [ ] **Step 5: Commit the failing tests**

```bash
git add qec-code/tests/toric_3d.rs qec-code/tests/fixtures/css/toric_3d_3x3x3_hx.json qec-code/tests/fixtures/css/toric_3d_3x3x3_hz.json
git commit -m "test(qec-code): add toric 3d css fixtures"
```

---

### Task 2: Implement The Toric 3D Chain-Complex Constructor

**Files:**
- Create: `qec-code/src/codes/toric_3d.rs`
- Modify: `qec-code/src/codes/mod.rs`

**Interfaces:**
- Consumes: `BinaryBoundaryMap::new`, `BinaryChainComplex::new`, and `BinaryChainComplex::css_view(1)`
- Produces: `Toric3dSpec`, `Toric3dCssChecks`, `Toric3dDistances`, `toric_3d_css_checks`, and `toric_3d_chain_complex`

- [ ] **Step 1: Create the module and public data types**

Implement the top of `qec-code/src/codes/toric_3d.rs`:

```rust
use crate::binary_chain_complex::{BinaryBoundaryMap, BinaryChainComplex};
use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Toric3dSpec {
    pub lx: usize,
    pub ly: usize,
    pub lz: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toric3dCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub distances: Toric3dDistances,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toric3dDistances {
    pub d_x: usize,
    pub d_z: usize,
    pub distance: usize,
}
```

- [ ] **Step 2: Implement checked dimensions and coordinate indexing**

Add a private dimensions helper whose `new` method validates the period floor
and all products/additions. Use `QecError::OutOfRangeBuiltInCssIntegerParameter`
for period floor violations and `QecError::SparseGf2DimensionOverflow {
operation: "toric_3d" }` for checked arithmetic failures:

```rust
#[derive(Debug, Clone, Copy)]
struct Toric3dDimensions {
    spec: Toric3dSpec,
    volume: usize,
    num_edges: usize,
    num_plaquettes: usize,
}

impl Toric3dDimensions {
    fn new(spec: Toric3dSpec) -> Result<Self> {
        validate_period("lx", spec.lx)?;
        validate_period("ly", spec.ly)?;
        validate_period("lz", spec.lz)?;
        let xy = checked_mul(spec.lx, spec.ly)?;
        let volume = checked_mul(xy, spec.lz)?;
        let num_edges = checked_mul(3, volume)?;
        let num_plaquettes = checked_mul(3, volume)?;
        Ok(Self { spec, volume, num_edges, num_plaquettes })
    }

    fn cell(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        let xy = checked_add(checked_mul(x, self.spec.ly)?, y)?;
        checked_add(checked_mul(xy, self.spec.lz)?, z)
    }

    fn x_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        self.cell(x, y, z)
    }

    fn y_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        checked_add(self.volume, self.cell(x, y, z)?)
    }

    fn z_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        checked_add(checked_mul(2, self.volume)?, self.cell(x, y, z)?)
    }
}
```

- [ ] **Step 3: Implement boundary rows and CSS export**

Build `boundary_1` as vertex rows over edge columns and `boundary_2` as edge
rows over plaquette columns. For each plaquette, push its plaquette index into
the four incident edge rows. Then construct `BinaryChainComplex` and use
`css_view(1)`:

```rust
pub fn toric_3d_chain_complex(spec: Toric3dSpec) -> Result<BinaryChainComplex> {
    let dims = Toric3dDimensions::new(spec)?;
    let boundary_1 = BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(
        dims.volume,
        dims.num_edges,
        vertex_edge_rows(&dims)?,
    )?)?;
    let boundary_2 = BinaryBoundaryMap::new(2, 1, SparseGf2Matrix::new(
        dims.num_edges,
        dims.num_plaquettes,
        edge_plaquette_rows(&dims)?,
    )?)?;
    BinaryChainComplex::new(vec![boundary_1, boundary_2])
}

pub fn toric_3d_css_checks(spec: Toric3dSpec) -> Result<Toric3dCssChecks> {
    let dims = Toric3dDimensions::new(spec)?;
    let complex = toric_3d_chain_complex(spec)?;
    let css = complex.css_view(1)?;
    Ok(Toric3dCssChecks {
        num_cols: css.num_qubits(),
        hx: css.hx().rows().to_vec(),
        hz: css.hz().rows().to_vec(),
        distances: analytic_distances(&dims)?,
    })
}
```

`vertex_edge_rows` must add outgoing and incoming x, y, and z edges. `edge_plaquette_rows` must append xy, xz, and yz plaquettes in that order and use the row supports:

```text
xy(x,y,z): x_edge(x,y,z), x_edge(x,next_y,z), y_edge(x,y,z), y_edge(next_x,y,z)
xz(x,y,z): x_edge(x,y,z), x_edge(x,y,next_z), z_edge(x,y,z), z_edge(next_x,y,z)
yz(x,y,z): y_edge(x,y,z), y_edge(x,y,next_z), z_edge(x,y,z), z_edge(x,next_y,z)
```

- [ ] **Step 4: Add the corrupt-boundary unit test**

Inside `#[cfg(test)] mod tests`, construct a valid 3x3x3 `edge_plaquette_rows`,
remove one support from the first edge row, build the two boundary maps, and
assert:

```rust
assert!(matches!(
    BinaryChainComplex::new(vec![boundary_1, boundary_2]),
    Err(QecError::NonzeroBoundaryComposition {
        lower_dimension: 1,
        upper_dimension: 2,
        ..
    })
));
```

- [ ] **Step 5: Export the module**

Modify `qec-code/src/codes/mod.rs`:

```rust
pub mod toric_3d;
```

- [ ] **Step 6: Run tests for this task**

Run:

```bash
cargo test -p qec-code --test toric_3d toric_3d_3x3x3_matches_fixture -- --exact
cargo test -p qec-code toric_3d::tests::corrupt_boundary_composition_is_rejected -- --exact
```

Expected: the module unit test passes; the integration fixture test still fails
only because the family contract variant is not integrated yet.

- [ ] **Step 7: Commit the constructor**

```bash
git add qec-code/src/codes/toric_3d.rs qec-code/src/codes/mod.rs
git commit -m "feat(qec-code): construct toric 3d chain complex"
```

---

### Task 3: Integrate Toric 3D Into The Family Contract And Built-In CLI

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`
- Modify: `qec-code/src/cli.rs` if imports require adjustment
- Modify: `qec-code/tests/family_contract.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: `toric_3d_css_checks(Toric3dSpec) -> Result<Toric3dCssChecks>`
- Produces: JSON construction, inline construction, built-in code ID, CLI export, and CSS-distance `--code-id` support

- [ ] **Step 1: Add the family contract variant**

In `family_contract.rs`, import `Toric3dSpec` and `toric_3d_css_checks`, add
`CssFamilySpec::Toric3d(Toric3dSpec)`, include `RequestedFamilyId::Toric3d` in
`callable_requested_family_ids()`, and add a `construct_css` branch:

```rust
CssConstructionSpec::Family(CssFamilySpec::Toric3d(spec)) => {
    let checks = toric_3d_css_checks(spec)?;
    let mut parameters = BTreeMap::new();
    parameters.insert("lx".to_owned(), Value::from(spec.lx));
    parameters.insert("ly".to_owned(), Value::from(spec.ly));
    parameters.insert("lz".to_owned(), Value::from(spec.lz));
    construction_result(
        "toric_3d",
        Some(RequestedFamilyId::Toric3d),
        parameters,
        checks.num_cols,
        checks.hx,
        checks.hz,
        "toric_3d_chain_complex",
        "CssFamilySpec::Toric3d",
    )
}
```

- [ ] **Step 2: Add JSON and inline parsing**

In `parse_css_construction_json`, add:

```rust
"toric_3d" => Ok(CssFamilySpec::Toric3d(Toric3dSpec {
    lx: required_usize(object, "lx", construction)?,
    ly: required_usize(object, "ly", construction)?,
    lz: required_usize(object, "lz", construction)?,
})
.into()),
```

In `CssConstructionSpec::from_inline`, map parsed built-in `toric_3d` specs to
the family variant the same way `surface_rotated` currently maps to `Surface`.

- [ ] **Step 3: Add built-in CSS parsing and catalog support**

In `built_in_css.rs`, add `BuiltInCssFamily::Toric3d` and
`BuiltInCssParams::Toric3d(Toric3dSpec)`. Parse `toric_3d:lx=3,ly=4,lz=5`,
reject missing/duplicate/unexpected parameters with existing built-in parameter
errors, validate each period by calling `toric_3d_css_checks`, and return:

```rust
Ok(BuiltInCssChecks {
    code_id: "toric_3d",
    num_cols: checks.num_cols,
    hx: checks.hx,
    hz: checks.hz,
})
```

Add catalog entry:

```rust
BuiltInCssCatalogEntry {
    spec: "toric_3d:lx=<period-x>,ly=<period-y>,lz=<period-z>",
    description: "periodic cubic 3D toric CSS code, periods >= 3",
},
```

- [ ] **Step 4: Add family contract tests**

Update `qec-code/tests/family_contract.rs` imports and tests:

```rust
use qec_code::codes::toric_3d::Toric3dSpec;
```

Change callable IDs to:

```rust
assert_eq!(
    CssFamilySpec::callable_requested_family_ids(),
    &[
        RequestedFamilyId::Surface,
        RequestedFamilyId::QuantumTanner,
        RequestedFamilyId::Toric3d,
    ]
);
```

Add a route equivalence test:

```rust
#[test]
fn inline_json_and_rust_routes_lower_to_same_toric_3d_spec() {
    let inline = CssConstructionSpec::from_inline("toric_3d:lx=3,ly=4,lz=5").unwrap();
    let json = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"toric_3d","lx":3,"ly":4,"lz":5}"#,
    )
    .unwrap();
    let rust_api = CssFamilySpec::Toric3d(Toric3dSpec { lx: 3, ly: 4, lz: 5 }).into();
    assert_eq!(inline, json);
    assert_eq!(json, rust_api);
}
```

- [ ] **Step 5: Add CLI tests**

In `qec-code/tests/cli.rs`, add the toric 3D catalog spec constant, assert the
list output contains it, update expected list width/output, and add direct
export tests:

```rust
const TORIC_3D_PARAMETERIZED_SPEC: &str = "toric_3d:lx=3,ly=3,lz=3";
const TORIC_3D_FAMILY_CATALOG_SPEC: &str =
    "toric_3d:lx=<period-x>,ly=<period-y>,lz=<period-z>";
```

Add fixture cases for `TORIC_3D_PARAMETERIZED_SPEC` `hx` and `hz`.

Add a negative CLI test case for `toric_3d:lx=2,ly=3,lz=3`.

- [ ] **Step 6: Run focused integration tests**

Run:

```bash
cargo test -p qec-code --test toric_3d toric_3d_3x3x3_matches_fixture -- --exact
cargo test -p qec-code --test toric_3d toric_3d_rejects_degenerate_periods -- --exact
cargo test -p qec-code --test family_contract inline_json_and_rust_routes_lower_to_same_toric_3d_spec -- --exact
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json -- --exact
```

Expected: all focused tests pass.

- [ ] **Step 7: Commit integration**

```bash
git add qec-code/src/family_contract.rs qec-code/src/codes/built_in_css.rs qec-code/src/cli.rs qec-code/tests/family_contract.rs qec-code/tests/cli.rs
git commit -m "feat(qec-code): expose toric 3d css family"
```

---

### Task 4: Verify Full Suite And Prepare PR

**Files:**
- Modify: any file touched by formatting only if `cargo fmt` changes it

**Interfaces:**
- Consumes: all prior tasks
- Produces: verified branch ready to push and open as a PR

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: exit 0.

- [ ] **Step 2: Run issue verification**

Run:

```bash
cargo test -p qec-code --test toric_3d toric_3d_3x3x3_matches_fixture -- --exact
cargo test -p qec-code --test toric_3d toric_3d_rejects_degenerate_periods -- --exact
```

Expected: both exit 0.

- [ ] **Step 3: Run package verification**

Run:

```bash
cargo test -p qec-code
```

Expected: exit 0.

- [ ] **Step 4: Run repository verification**

Run:

```bash
cargo test
```

Expected: exit 0.

- [ ] **Step 5: Commit final formatting or missed edits**

If `cargo fmt` or verification fixes changed files, commit them:

```bash
git add qec-code docs/superpowers/plans/2026-07-27-issue-567-toric-3d-css.md
git commit -m "chore: finalize toric 3d css implementation"
```

If no files changed, skip this commit.

- [ ] **Step 6: Finish the branch**

Use `superpowers:verification-before-completion` and
`superpowers:finishing-a-development-branch`. At the finishing menu, choose
`Push and create a Pull Request` because the Agent Desk standing instruction
requires a PR and explicitly says not to merge.

## Self-Review

- Spec coverage: tasks cover `Toric3dSpec`, period validation, overflow
  rejection, boundary-map construction, fixture rows, stats, distances,
  orthogonality, Rust API, JSON parsing, CLI/built-in route, and PR creation.
- Placeholder scan: no placeholder markers or vague "handle later" steps remain.
- Type consistency: `Toric3dSpec`, `Toric3dCssChecks`, `Toric3dDistances`,
  `toric_3d_css_checks`, and `toric_3d_chain_complex` names match across tasks.
