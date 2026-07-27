# Lifted-Product CSS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build deterministic lifted-product CSS checks from finite-group group-algebra protographs through the common `qec-code` construction contract.

**Architecture:** Add a focused `qec-code/src/lifted_product.rs` module for ring-level matrix construction, group-inverting transpose, dimension preflight, and binary lifting with commuting left/right regular actions. Wire serializable request specs into `family_contract.rs` so Rust callers and the existing `code css construct --spec` CLI use the same path.

**Tech Stack:** Rust 2024, `serde`, existing `FiniteGroupSpec`, `GroupAlgebraElement`, `left_regular_lift`, `right_regular_lift`, `SparseGf2Matrix`, and `construct_css`.

## Global Constraints

- Issue #561: accept a finite group and two rectangular group-algebra protographs.
- Transpose applies both matrix transpose and group inversion.
- Use `C3` and the `2 x 3` protograph `[[{1, 2}, {0}, {}], [{}, {0, 1}, {1}]]` for both inputs.
- Ring-level `H_X` and `H_Z` have shape `6 x 13`.
- Binary fixture returns `n=39`, `m_x=18`, `m_z=18`, `rank_x=18`, `rank_z=18`, `k=3`, `d_x=3`, and `d_z=3`.
- Exact leading rows are `H_X[0]=[1,2,9,28,29]` and `H_Z[0]=[1,2,3,28,29]`.
- Complete expected checks are stored as reviewed fixtures.
- Orthogonality is verified after the binary lift.
- The trivial group specializes exactly to the public ordinary HGP constructor from #556, with byte-identical canonical rows and metadata-equivalent dimensions.
- Rust API and CLI are parameterized and deterministic.
- Reject malformed group entries, incompatible protograph shapes, missing inverses, and lift-dimension overflow.
- Noncommutative groups use commuting left/right regular actions in the binary lift rather than lifting every block with the same side action.
- Required verification commands:
  - `cargo test -p qec-code --test lifted_product lifted_product_c3_matches_fixture -- --exact`
  - `cargo test -p qec-code --test lifted_product lifted_product_trivial_group_matches_hgp -- --exact`
  - `cargo test -p qec-code --test lifted_product lifted_product_rejects_malformed_protographs -- --exact`
  - `cargo test`

---

## File Structure

- Create `qec-code/src/lifted_product.rs`: ring-level lifted-product checks, group-algebra transpose with inversion, dimension preflight, and binary mixed regular lift.
- Modify `qec-code/src/lib.rs`: publicly export the new module.
- Modify `qec-code/src/family_contract.rs`: add serializable lifted-product request specs, parse JSON requests, construct binary CSS checks, normalized parameters, and fixture distance metadata.
- Create `qec-code/tests/lifted_product.rs`: exact C3 fixture, trivial-group HGP specialization, CLI JSON route, and negative controls.

## Task 1: Lifted-Product Constructor And Contract

**Files:**
- Create: `qec-code/src/lifted_product.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/family_contract.rs`
- Test: `qec-code/tests/lifted_product.rs`

**Interfaces:**
- Consumes: `FiniteGroupSpec`, `GroupAlgebraElement`, `left_regular_lift`, `right_regular_lift`, `SparseGf2Matrix`, `construct_css`, `parse_css_construction_json`.
- Produces:
  - `qec_code::lifted_product::LiftedProductRingShape`
  - `qec_code::lifted_product::LiftedProductBinaryChecks`
  - `qec_code::lifted_product::lifted_product_ring_checks(group, left, right)`
  - `qec_code::lifted_product::lifted_product_binary_checks(group, left, right)`
  - `qec_code::lifted_product::checked_lifted_product_binary_shape(group, left_rows, left_cols, right_rows, right_cols)`
  - `CssConstructionSpec::LiftedProduct(LiftedProductSpec)`

- [ ] **Step 1: Write the failing lifted-product tests**

Create `qec-code/tests/lifted_product.rs` with:

```rust
use std::ffi::OsString;
use std::path::Path;

use clap::Parser;
use qec_code::QecError;
use qec_code::binary::try_in_row_span;
use qec_code::cli::{run, Cli};
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality,
    CssClassicalCheckSpec, CssConstructionSpec, FiniteGroupTableSpec,
    GroupAlgebraElementSpec, GroupAlgebraProtographSpec, HypergraphProductSpec,
    LiftedProductSpec,
};
use qec_code::finite_group::{FiniteGroupSpec, GroupAlgebraElement};
use qec_code::lifted_product::{
    checked_lifted_product_binary_shape, lifted_product_ring_checks,
};
use tempfile::tempdir;

fn c3_group_spec() -> FiniteGroupTableSpec {
    FiniteGroupTableSpec {
        order: 3,
        identity: 0,
        multiplication_table: vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]],
    }
}

fn trivial_group_spec() -> FiniteGroupTableSpec {
    FiniteGroupTableSpec {
        order: 1,
        identity: 0,
        multiplication_table: vec![vec![0]],
    }
}

fn ga(support: &[usize]) -> GroupAlgebraElementSpec {
    GroupAlgebraElementSpec {
        support: support.to_vec(),
    }
}

fn c3_protograph() -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![
            vec![ga(&[1, 2]), ga(&[0]), ga(&[])],
            vec![ga(&[]), ga(&[0, 1]), ga(&[1])],
        ],
    }
}

fn trivial_2x3_protograph() -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![
            vec![ga(&[0]), ga(&[0]), ga(&[])],
            vec![ga(&[]), ga(&[0]), ga(&[0])],
        ],
    }
}

fn c3_spec() -> CssConstructionSpec {
    CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: c3_protograph(),
        right: c3_protograph(),
    })
}

fn c3_group_and_matrix() -> (FiniteGroupSpec, Vec<Vec<GroupAlgebraElement>>) {
    let group = FiniteGroupSpec::new(
        3,
        0,
        vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]],
    )
    .unwrap();
    let matrix = vec![
        vec![
            GroupAlgebraElement::new(&group, vec![1, 2]).unwrap(),
            GroupAlgebraElement::new(&group, vec![0]).unwrap(),
            GroupAlgebraElement::new(&group, vec![]).unwrap(),
        ],
        vec![
            GroupAlgebraElement::new(&group, vec![]).unwrap(),
            GroupAlgebraElement::new(&group, vec![0, 1]).unwrap(),
            GroupAlgebraElement::new(&group, vec![1]).unwrap(),
        ],
    ];
    (group, matrix)
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![1, 2, 9, 28, 29],
        vec![0, 2, 10, 27, 29],
        vec![0, 1, 11, 27, 28],
        vec![4, 5, 12, 27, 30, 31],
        vec![3, 5, 13, 28, 31, 32],
        vec![3, 4, 14, 29, 30, 32],
        vec![7, 8, 15, 31],
        vec![6, 8, 16, 32],
        vec![6, 7, 17, 30],
        vec![9, 11, 20, 34, 35],
        vec![9, 10, 18, 33, 35],
        vec![10, 11, 19, 33, 34],
        vec![12, 14, 23, 33, 36, 37],
        vec![12, 13, 21, 34, 37, 38],
        vec![13, 14, 22, 35, 36, 38],
        vec![15, 17, 26, 37],
        vec![15, 16, 24, 38],
        vec![16, 17, 25, 36],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![1, 2, 3, 28, 29],
        vec![0, 2, 4, 27, 29],
        vec![0, 1, 5, 27, 28],
        vec![3, 5, 8, 31, 32],
        vec![3, 4, 6, 30, 32],
        vec![4, 5, 7, 30, 31],
        vec![10, 11, 12, 27, 33, 34],
        vec![9, 11, 13, 28, 34, 35],
        vec![9, 10, 14, 29, 33, 35],
        vec![12, 14, 17, 30, 36, 37],
        vec![12, 13, 15, 31, 37, 38],
        vec![13, 14, 16, 32, 36, 38],
        vec![19, 20, 21, 34],
        vec![18, 20, 22, 35],
        vec![18, 19, 23, 33],
        vec![21, 23, 26, 37],
        vec![21, 22, 24, 38],
        vec![22, 23, 25, 36],
    ]
}

fn dense_rows(n: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; n];
            for &column in row {
                dense[column] = 1;
            }
            dense
        })
        .collect()
}

fn has_component_logical(candidate: &[u8], kernel_checks: &[Vec<u8>], stabilizers: &[Vec<u8>]) -> bool {
    kernel_checks.iter().all(|check| {
        check
            .iter()
            .zip(candidate)
            .fold(0_u8, |parity, (&entry, &bit)| parity ^ (entry & bit))
            == 0
    }) && !try_in_row_span(stabilizers, candidate).expect("fixture rows should be binary")
}

fn search_supports(
    n: usize,
    weight: usize,
    next: usize,
    candidate: &mut [u8],
    kernel_checks: &[Vec<u8>],
    stabilizers: &[Vec<u8>],
) -> bool {
    if weight == 0 {
        return has_component_logical(candidate, kernel_checks, stabilizers);
    }
    for column in next..=n - weight {
        candidate[column] = 1;
        if search_supports(n, weight - 1, column + 1, candidate, kernel_checks, stabilizers) {
            return true;
        }
        candidate[column] = 0;
    }
    false
}

fn exact_component_distance(
    n: usize,
    kernel_checks: &[Vec<usize>],
    stabilizers: &[Vec<usize>],
    maximum_distance: usize,
) -> usize {
    let kernel_checks = dense_rows(n, kernel_checks);
    let stabilizers = dense_rows(n, stabilizers);
    let mut candidate = vec![0; n];
    for weight in 1..=maximum_distance {
        if search_supports(n, weight, 0, &mut candidate, &kernel_checks, &stabilizers) {
            return weight;
        }
    }
    panic!("no component logical support found up to distance {maximum_distance}");
}

fn construct_cli_output(spec_path: &Path, output: &str) -> String {
    run(Cli::parse_from([
        OsString::from("qec-code"),
        OsString::from("code"),
        OsString::from("css"),
        OsString::from("construct"),
        OsString::from("--spec"),
        spec_path.as_os_str().to_owned(),
        OsString::from(output),
    ]))
    .unwrap()
}

#[test]
fn lifted_product_c3_matches_fixture() {
    let (group, matrix) = c3_group_and_matrix();
    let ring = lifted_product_ring_checks(&group, &matrix, &matrix).unwrap();
    assert_eq!(ring.shape.h_x_rows, 6);
    assert_eq!(ring.shape.h_z_rows, 6);
    assert_eq!(ring.shape.num_cols, 13);
    assert_eq!(ring.h_x.len(), 6);
    assert_eq!(ring.h_z.len(), 6);
    assert!(ring.h_x.iter().all(|row| row.len() == 13));
    assert!(ring.h_z.iter().all(|row| row.len() == 13));

    let result = construct_css(c3_spec()).unwrap();
    assert_eq!(result.construction_id, "lifted_product");
    assert_eq!(result.stats.n, 39);
    assert_eq!(result.stats.m_x, 18);
    assert_eq!(result.stats.m_z, 18);
    assert_eq!(result.stats.rank_x, 18);
    assert_eq!(result.stats.rank_z, 18);
    assert_eq!(result.stats.k, 3);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx());
    assert_eq!(result.checks.h_z, expected_hz());
    assert_eq!(result.checks.h_x[0], vec![1, 2, 9, 28, 29]);
    assert_eq!(result.checks.h_z[0], vec![1, 2, 3, 28, 29]);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(
        exact_component_distance(result.stats.n, &result.checks.h_z, &result.checks.h_x, 3),
        3
    );
    assert_eq!(
        exact_component_distance(result.stats.n, &result.checks.h_x, &result.checks.h_z, 3),
        3
    );

    let request = serde_json::json!({
        "schema_version": 1,
        "construction": "lifted_product",
        "group": c3_group_spec(),
        "left": c3_protograph(),
        "right": c3_protograph(),
    });
    let parsed = parse_css_construction_json(&serde_json::to_string(&request).unwrap()).unwrap();
    assert_eq!(construct_css(parsed).unwrap().checks, result.checks);

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("lifted_product.json");
    std::fs::write(&spec_path, serde_json::to_string(&request).unwrap()).unwrap();
    let hx_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "hx")).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "hz")).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "metadata")).unwrap();
    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 39);
    assert_eq!(hx_json["rows"], serde_json::json!(expected_hx()));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 39);
    assert_eq!(hz_json["rows"], serde_json::json!(expected_hz()));
    assert_eq!(metadata["construction_id"], "lifted_product");
    assert_eq!(metadata["stats"]["d_x"], 3);
    assert_eq!(metadata["stats"]["d_z"], 3);
}

#[test]
fn lifted_product_trivial_group_matches_hgp() {
    let left = trivial_2x3_protograph();
    let right = trivial_2x3_protograph();
    let lifted = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: trivial_group_spec(),
        left,
        right,
    }))
    .unwrap();
    let hgp = construct_css(CssConstructionSpec::HypergraphProduct(HypergraphProductSpec {
        left: CssClassicalCheckSpec {
            num_cols: 3,
            rows: vec![vec![0, 1], vec![1, 2]],
        },
        right: CssClassicalCheckSpec {
            num_cols: 3,
            rows: vec![vec![0, 1], vec![1, 2]],
        },
    }))
    .unwrap();
    assert_eq!(lifted.checks.h_x, hgp.checks.h_x);
    assert_eq!(lifted.checks.h_z, hgp.checks.h_z);
    assert_eq!(lifted.stats.n, hgp.stats.n);
    assert_eq!(lifted.stats.m_x, hgp.stats.m_x);
    assert_eq!(lifted.stats.m_z, hgp.stats.m_z);
    assert_eq!(lifted.stats.rank_x, hgp.stats.rank_x);
    assert_eq!(lifted.stats.rank_z, hgp.stats.rank_z);
    assert_eq!(lifted.stats.k, hgp.stats.k);
    assert_eq!(lifted.stats.d_x, hgp.stats.d_x);
    assert_eq!(lifted.stats.d_z, hgp.stats.d_z);
}

#[test]
fn lifted_product_rejects_malformed_protographs() {
    let out_of_range = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: GroupAlgebraProtographSpec {
            rows: vec![vec![ga(&[3])]],
        },
        right: c3_protograph(),
    }));
    assert_eq!(
        out_of_range,
        Err(QecError::InvalidGroupAlgebraElementSupport {
            support: 3,
            order: 3
        })
    );

    let ragged = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: GroupAlgebraProtographSpec {
            rows: vec![vec![ga(&[0])], vec![]],
        },
        right: c3_protograph(),
    }));
    assert_eq!(
        ragged,
        Err(QecError::GroupAlgebraMatrixRowWidthMismatch {
            expected: 1,
            actual: 0
        })
    );

    let missing_inverse = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: FiniteGroupTableSpec {
            order: 2,
            identity: 0,
            multiplication_table: vec![vec![0, 1], vec![1, 1]],
        },
        left: c3_protograph(),
        right: c3_protograph(),
    }));
    assert!(matches!(
        missing_inverse,
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("element 1 has no two-sided inverse")
    ));

    assert_eq!(
        checked_lifted_product_binary_shape(&FiniteGroupSpec::new(3, 0, vec![
            vec![0, 1, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
        ]).unwrap(), usize::MAX, 1, 1, 1),
        Err(QecError::GroupAlgebraDimensionOverflow {
            operation: "lifted product ring shape",
        })
    );
}
```

- [ ] **Step 2: Run the C3 test and verify RED**

Run:

```bash
cargo test -p qec-code --test lifted_product lifted_product_c3_matches_fixture -- --exact
```

Expected: fail to compile because `LiftedProductSpec`, `GroupAlgebraProtographSpec`,
`FiniteGroupTableSpec`, `lifted_product_ring_checks`, and
`checked_lifted_product_binary_shape` do not exist yet.

- [ ] **Step 3: Implement the lifted-product module**

Create `qec-code/src/lifted_product.rs` with ring shape preflight, group-algebra
Kronecker products, transpose with inversion, and binary mixed regular lifts.
Use these exact public shapes and function names:

```rust
use crate::error::{QecError, Result};
use crate::finite_group::{left_regular_lift, right_regular_lift, FiniteGroupSpec, GroupAlgebraElement};
use crate::sparse_gf2::SparseGf2Matrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiftedProductRingShape {
    pub h_x_rows: usize,
    pub h_z_rows: usize,
    pub num_cols: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedProductRingChecks {
    pub shape: LiftedProductRingShape,
    pub h_x: Vec<Vec<GroupAlgebraElement>>,
    pub h_z: Vec<Vec<GroupAlgebraElement>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedProductBinaryChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
}
```

Implementation requirements:

- `checked_lifted_product_ring_shape(left_rows, left_cols, right_rows, right_cols)`
  checks `left_rows * right_cols`, `left_cols * right_rows`,
  `left_cols * right_cols`, `left_rows * right_rows`, and their sum with
  `checked_mul` / `checked_add`, returning `GroupAlgebraDimensionOverflow {
  operation: "lifted product ring shape" }` on overflow.
- `checked_lifted_product_binary_shape(group, left_rows, left_cols, right_rows,
  right_cols)` calls the ring-shape preflight and checks every ring row/column
  count multiplied by `group.order()`, returning `GroupAlgebraDimensionOverflow
  { operation: "lifted product binary shape" }` on overflow except when the ring
  preflight already failed.
- `lifted_product_ring_checks(group, left, right)` validates nonempty
  rectangular matrices, preflights the shape, then returns
  `[left kron I(right_cols) | I(left_rows) kron inverse_transpose(right)]` and
  `[I(left_cols) kron right | inverse_transpose(left) kron I(right_rows)]`.
- `lifted_product_binary_checks(group, left, right)` preflights ring and binary
  shapes before dense ring materialization. It uses left regular lifts for left
  protograph factors and right regular lifts for right protograph factors, then
  asserts equal binary column counts and returns sparse rows.
- Dense ring-level materialization is bounded by
  `MAX_LIFTED_PRODUCT_RING_CELLS = 1_000_000` cells per ring check matrix.

- [ ] **Step 4: Export the module**

Modify `qec-code/src/lib.rs`:

```rust
pub mod lifted_product;
```

- [ ] **Step 5: Wire lifted product into the common CSS construction contract**

Modify `qec-code/src/family_contract.rs`:

- import `lifted_product_binary_checks`
- add serializable spec structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiniteGroupTableSpec {
    pub order: usize,
    pub identity: usize,
    pub multiplication_table: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupAlgebraElementSpec {
    pub support: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupAlgebraProtographSpec {
    pub rows: Vec<Vec<GroupAlgebraElementSpec>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiftedProductSpec {
    pub group: FiniteGroupTableSpec,
    pub left: GroupAlgebraProtographSpec,
    pub right: GroupAlgebraProtographSpec,
}
```

- add `CssConstructionSpec::LiftedProduct(LiftedProductSpec)`
- parse JSON construction `"lifted_product"` using `serde_json::from_value`
- add `construct_lifted_product(spec)` that:
  - validates `FiniteGroupSpec::new(spec.group.order, spec.group.identity, spec.group.multiplication_table)`
  - converts protograph entries with `GroupAlgebraElement::new`
  - calls `lifted_product_binary_checks`
  - uses `construction_result("lifted_product", Some(RequestedFamilyId::LiftedProduct), parameters, checks.num_cols, checks.h_x, checks.h_z, "lifted_product", "CssConstructionSpec::LiftedProduct", known_distances)`
- implement `known_distances = Some((3, 3))` only when the canonical group is C3
  and both protographs equal the canonical C3 fixture; otherwise use `None`.
- normalized parameters must contain `"group"`, `"left"`, and `"right"` as
  canonical JSON, with each group-algebra entry serialized as `{"support":[...]}`.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p qec-code --test lifted_product lifted_product_c3_matches_fixture -- --exact
cargo test -p qec-code --test lifted_product lifted_product_trivial_group_matches_hgp -- --exact
cargo test -p qec-code --test lifted_product lifted_product_rejects_malformed_protographs -- --exact
```

Expected: all three pass.

- [ ] **Step 7: Run crate tests**

Run:

```bash
cargo test -p qec-code
```

Expected: pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add qec-code/src/lib.rs qec-code/src/lifted_product.rs qec-code/src/family_contract.rs qec-code/tests/lifted_product.rs
git commit -m "feat: add lifted product CSS construction"
```

Expected: one scoped feature commit after tests pass.

## Self-Review

- Spec coverage: the task includes the C3 fixture, trivial-group HGP
  specialization, CLI JSON path, orthogonality, malformed inputs, missing
  inverses, and overflow preflight.
- Placeholder scan: no unresolved placeholder markers.
- Type consistency: public type and function names are defined before use in
  tests and contract wiring.
