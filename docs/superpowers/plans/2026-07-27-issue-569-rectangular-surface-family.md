# Rectangular Surface Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generalize the requested `surface` CSS construction to rotated and ordinary planar rectangular patches while preserving all legacy rotated-square output.

**Architecture:** Extend the #553 common construction contract in `qec-code/src/family_contract.rs` with `SurfaceLayout` and `SurfaceSpec`, then generate generalized surface sparse supports through that typed route. Keep `surface_rotated:d=<distance>` and legacy JSON `distance` as `SurfaceFamilySpec { distance }` compatibility adapters backed by the built-in constructor.

**Tech Stack:** Rust 2024, serde/serde_json, clap in-process CLI tests, existing `qec-code` sparse-row and rank helpers.

## Global Constraints

- Legacy compact `surface_rotated:d=<distance>` remains valid for every `d >= 2`, including even distances.
- Existing `surface_rotated:d=3` matrix JSON remains byte-for-byte identical to checked-in fixtures.
- Rotated `3 x 5` exact supports are `H_X = [[0,5], [1,2,6,7], [3,4,8,9], [5,6,10,11], [7,8,12,13], [9,14]]` and `H_Z = [[1,2], [3,4], [0,1,5,6], [2,3,7,8], [6,7,11,12], [8,9,13,14], [10,11], [12,13]]`.
- Rotated `3 x 5` reports `n=15`, `m_x=6`, `m_z=8`, `rank_x=6`, `rank_z=8`, `k=1`, `d_x=5`, and `d_z=3`.
- Unrotated distance `3` exact supports are `H_X = [[0,3,5], [1,3,4,6], [2,4,7], [5,8,10], [6,8,9,11], [7,9,12]]` and `H_Z = [[0,1,3], [1,2,4], [3,5,6,8], [4,6,7,9], [8,10,11], [9,11,12]]`.
- Unrotated distance `3` reports `n=13`, `m_x=m_z=6`, `rank_x=rank_z=6`, `k=1`, and distance `3`.
- Exact checks and orthogonality match both fixtures.
- Rust API and CLI expose layout and dimensions through the common contract.
- Reject row or column dimensions below `2`, conflicting legacy and new parameters, an unknown layout, and size overflow.

---

## File Structure

- Modify `qec-code/src/family_contract.rs`: public `SurfaceLayout`, public `SurfaceSpec`, legacy `SurfaceFamilySpec` adapter, JSON parsing, normalized parameters, known distance stats, and surface support generation.
- Add `qec-code/tests/surface_family.rs`: issue-named TDD regression tests for exact fixtures, compatibility, CLI structured JSON export, invalid inputs, and overflow.
- Add `qec-code/tests/fixtures/css/surface_rotated_d4_hx.json`: pre-change even-distance legacy fixture.
- Add `qec-code/tests/fixtures/css/surface_rotated_d4_hz.json`: pre-change even-distance legacy fixture.
- Modify `qec-code/tests/family_contract.rs`: retain #553 coverage for `SurfaceFamilySpec { distance: 3 }` and add generalized construction coverage separately.
- Modify `docs/showcases/qec-code-css-construction.md`: document the structured rectangular surface JSON shape while keeping legacy compact examples.

### Task 1: Red Tests And Legacy Fixtures

**Files:**
- Create: `qec-code/tests/surface_family.rs`
- Create: `qec-code/tests/fixtures/css/surface_rotated_d4_hx.json`
- Create: `qec-code/tests/fixtures/css/surface_rotated_d4_hz.json`

**Interfaces:**
- Consumes: current public `construct_css`, `parse_css_construction_json`, `CssFamilySpec`, `CssConstructionSpec`, `verify_css_orthogonality`, `SparseRowsMatrix`, and `qec_code::cli::run`.
- Produces: failing tests that define `SurfaceLayout`, `SurfaceSpec`, `d_x`, `d_z`, rectangular JSON parsing, unrotated layout, and invalid input behavior.

- [ ] **Step 1: Add d=4 legacy fixtures**

Create `qec-code/tests/fixtures/css/surface_rotated_d4_hx.json`:

```json
{"format":"sparse_rows","num_cols":16,"rows":[[0,4],[1,2,5,6],[3,7],[4,5,8,9],[6,7,10,11],[8,12],[9,10,13,14],[11,15]]}
```

Create `qec-code/tests/fixtures/css/surface_rotated_d4_hz.json`:

```json
{"format":"sparse_rows","num_cols":16,"rows":[[1,2],[0,1,4,5],[2,3,6,7],[5,6,9,10],[8,9,12,13],[10,11,14,15],[13,14]]}
```

- [ ] **Step 2: Add the failing surface-family integration test**

Create `qec-code/tests/surface_family.rs` with the full test content:

```rust
use std::path::{Path, PathBuf};

use qec_code::QecError;
use qec_code::cli::{CodeCommands, Commands, CssArgs, CssMatrixKind, run};
use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    CssConstructionSpec, CssFamilySpec, SurfaceFamilySpec, SurfaceLayout, SurfaceSpec,
    construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(workspace_root().join("tests/fixtures/css").join(name))
        .expect("fixture should be readable")
        .trim_end_matches('\n')
        .to_owned()
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
}

fn write_spec(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("spec should be writable");
    path
}

fn cli_export_from_spec(spec: PathBuf, matrix: CssMatrixKind) -> String {
    run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs {
                command: Some(qec_code::cli::CssCommands::Construct { spec, matrix }),
                code_id: None,
                matrix: None,
            }),
        },
    })
    .unwrap()
}

#[test]
fn rectangular_rotated_surface_3x5_matches_fixture() {
    let expected_hx = vec![
        vec![0, 5],
        vec![1, 2, 6, 7],
        vec![3, 4, 8, 9],
        vec![5, 6, 10, 11],
        vec![7, 8, 12, 13],
        vec![9, 14],
    ];
    let expected_hz = vec![
        vec![1, 2],
        vec![3, 4],
        vec![0, 1, 5, 6],
        vec![2, 3, 7, 8],
        vec![6, 7, 11, 12],
        vec![8, 9, 13, 14],
        vec![10, 11],
        vec![12, 13],
    ];

    let spec = SurfaceSpec {
        layout: SurfaceLayout::Rotated,
        row_distance: 3,
        column_distance: 5,
    };
    let result = construct_css(CssFamilySpec::Surface(spec.clone()).into()).unwrap();

    assert_eq!(result.construction_id, "surface_rotated");
    assert_eq!(result.normalized_parameters["layout"], serde_json::json!("rotated"));
    assert_eq!(result.normalized_parameters["row_distance"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["column_distance"], serde_json::json!(5));
    assert_eq!(result.stats.n, 15);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 8);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 8);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(5));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let json = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":3,"column_distance":5}"#,
    )
    .unwrap();
    assert_eq!(json, CssFamilySpec::Surface(spec).into());
}

#[test]
fn ordinary_surface_d3_matches_fixture() {
    let expected_hx = vec![
        vec![0, 3, 5],
        vec![1, 3, 4, 6],
        vec![2, 4, 7],
        vec![5, 8, 10],
        vec![6, 8, 9, 11],
        vec![7, 9, 12],
    ];
    let expected_hz = vec![
        vec![0, 1, 3],
        vec![1, 2, 4],
        vec![3, 5, 6, 8],
        vec![4, 6, 7, 9],
        vec![8, 10, 11],
        vec![9, 11, 12],
    ];
    let spec = SurfaceSpec {
        layout: SurfaceLayout::Unrotated,
        row_distance: 3,
        column_distance: 3,
    };

    let result = construct_css(CssFamilySpec::Surface(spec).into()).unwrap();

    assert_eq!(result.construction_id, "surface_unrotated");
    assert_eq!(result.stats.n, 13);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "surface-unrotated-d3.json",
        r#"{"schema_version":1,"construction":"surface","layout":"unrotated","row_distance":3,"column_distance":3}"#,
    );
    let cli_hx = cli_export_from_spec(spec_path, CssMatrixKind::Hx);
    assert_eq!(
        cli_hx,
        SparseRowsMatrix::new(result.stats.n, expected_hx)
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn legacy_rotated_surface_outputs_are_unchanged() {
    for distance in 2..=6 {
        let inline = CssConstructionSpec::from_inline(&format!("surface_rotated:d={distance}"))
            .unwrap();
        let typed = CssFamilySpec::Surface(SurfaceFamilySpec { distance }).into();
        assert_eq!(inline, typed);

        let legacy = construct_css(inline).unwrap();
        let oracle = built_in_css_checks(&format!("surface_rotated:d={distance}")).unwrap();
        assert_eq!(
            SparseRowsMatrix::new(legacy.stats.n, legacy.checks.h_x.clone())
                .unwrap()
                .to_json_string(),
            SparseRowsMatrix::new(oracle.num_cols, oracle.hx)
                .unwrap()
                .to_json_string()
        );
        assert_eq!(
            SparseRowsMatrix::new(legacy.stats.n, legacy.checks.h_z.clone())
                .unwrap()
                .to_json_string(),
            SparseRowsMatrix::new(oracle.num_cols, oracle.hz)
                .unwrap()
                .to_json_string()
        );
        assert_eq!(legacy.stats.d_x, Some(distance));
        assert_eq!(legacy.stats.d_z, Some(distance));
    }

    let d3_hx = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let d3_hz = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();
    let d4_hx = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=4".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let d4_hz = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=4".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();

    assert_eq!(d3_hx, fixture_text("surface_rotated_d3_hx.json"));
    assert_eq!(d3_hz, fixture_text("surface_rotated_d3_hz.json"));
    assert_eq!(d4_hx, fixture_text("surface_rotated_d4_hx.json"));
    assert_eq!(d4_hz, fixture_text("surface_rotated_d4_hz.json"));
}

#[test]
fn surface_family_rejects_invalid_dimensions() {
    assert!(construct_css(SurfaceSpec {
        layout: SurfaceLayout::Rotated,
        row_distance: 1,
        column_distance: 3,
    }
    .into())
    .is_err());

    assert!(construct_css(SurfaceSpec {
        layout: SurfaceLayout::Unrotated,
        row_distance: 3,
        column_distance: 1,
    }
    .into())
    .is_err());

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","layout":"diagonal","row_distance":3,"column_distance":5}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("unknown surface layout")
    ));

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","distance":3,"layout":"rotated","row_distance":3,"column_distance":3}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("conflicting")
    ));

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":18446744073709551616,"column_distance":3}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("row_distance")
    ));

    assert!(matches!(
        construct_css(SurfaceSpec {
            layout: SurfaceLayout::Unrotated,
            row_distance: usize::MAX,
            column_distance: 2,
        }
        .into()),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("overflow")
    ));
}
```

- [ ] **Step 3: Run exact tests and verify RED**

Run:

```bash
cargo test -p qec-code --test surface_family rectangular_rotated_surface_3x5_matches_fixture -- --exact
cargo test -p qec-code --test surface_family ordinary_surface_d3_matches_fixture -- --exact
cargo test -p qec-code --test surface_family legacy_rotated_surface_outputs_are_unchanged -- --exact
cargo test -p qec-code --test surface_family surface_family_rejects_invalid_dimensions -- --exact
```

Expected: FAIL to compile because `SurfaceLayout`, `SurfaceSpec`, `stats.d_x`,
and `stats.d_z` do not exist yet.

### Task 2: SurfaceSpec Contract And Generators

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`

**Interfaces:**
- Consumes: failing `qec-code/tests/surface_family.rs` tests.
- Produces: `SurfaceLayout`, `SurfaceSpec`, legacy `SurfaceFamilySpec`
  adapter, JSON lowering, known distance stats, rotated and ordinary support
  generation.

- [ ] **Step 1: Add the layout-aware spec alongside the legacy adapter**

In `qec-code/src/family_contract.rs`, add `SurfaceSpec` beside the legacy
`SurfaceFamilySpec` adapter:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceLayout {
    Rotated,
    Unrotated,
}

impl SurfaceLayout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rotated => "rotated",
            Self::Unrotated => "unrotated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSpec {
    pub layout: SurfaceLayout,
    pub row_distance: usize,
    pub column_distance: usize,
}

impl SurfaceSpec {
    pub const fn rotated_square(distance: usize) -> Self {
        Self {
            layout: SurfaceLayout::Rotated,
            row_distance: distance,
            column_distance: distance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceFamilySpec {
    pub distance: usize,
}
```

- [ ] **Step 2: Add known distances to stats**

Change `CssCodeStats` to:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssCodeStats {
    pub n: usize,
    pub m_x: usize,
    pub m_z: usize,
    pub rank_x: usize,
    pub rank_z: usize,
    pub k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d_x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d_z: Option<usize>,
}
```

Add a `known_distances: Option<(usize, usize)>` argument to
`construction_result` and set:

```rust
let (d_x, d_z) = known_distances
    .map(|(d_x, d_z)| (Some(d_x), Some(d_z)))
    .unwrap_or((None, None));
```

Then include `d_x` and `d_z` in `CssCodeStats`.

- [ ] **Step 3: Implement validated surface generation**

Add helper functions in `qec-code/src/family_contract.rs`:

```rust
fn construct_surface(spec: SurfaceSpec) -> Result<CssConstructionResult> {
    validate_surface_spec(&spec)?;
    let (n, h_x, h_z, construction_id) = match spec.layout {
        SurfaceLayout::Rotated => {
            let n = spec
                .row_distance
                .checked_mul(spec.column_distance)
                .ok_or_else(|| surface_overflow("data qubit count"))?;
            let (h_x, h_z) = rotated_surface_supports(spec.row_distance, spec.column_distance);
            (n, h_x, h_z, "surface_rotated")
        }
        SurfaceLayout::Unrotated => {
            let n = unrotated_surface_num_data_qubits(spec.row_distance, spec.column_distance)?;
            let (h_x, h_z) = unrotated_surface_supports(spec.row_distance, spec.column_distance)?;
            (n, h_x, h_z, "surface_unrotated")
        }
    };
    let mut parameters = BTreeMap::new();
    parameters.insert("layout".to_owned(), Value::from(spec.layout.as_str()));
    parameters.insert("row_distance".to_owned(), Value::from(spec.row_distance));
    parameters.insert("column_distance".to_owned(), Value::from(spec.column_distance));
    construction_result(
        construction_id,
        Some(RequestedFamilyId::Surface),
        parameters,
        n,
        h_x,
        h_z,
        "surface",
        Some((spec.column_distance, spec.row_distance)),
    )
}

fn validate_surface_spec(spec: &SurfaceSpec) -> Result<()> {
    validate_surface_distance("row_distance", spec.row_distance)?;
    validate_surface_distance("column_distance", spec.column_distance)
}

fn validate_surface_distance(parameter: &'static str, value: usize) -> Result<()> {
    if value < 2 {
        return Err(QecError::InvalidCssConstruction {
            construction: "surface".to_owned(),
            reason: format!("{parameter} must be at least 2, got {value}"),
        });
    }
    Ok(())
}

fn surface_overflow(operation: &'static str) -> QecError {
    QecError::InvalidCssConstruction {
        construction: "surface".to_owned(),
        reason: format!("surface dimension overflow during {operation}"),
    }
}
```

Add the rotated helpers by copying the existing square logic with separate row
and column bounds:

```rust
fn rotated_surface_supports(
    row_distance: usize,
    column_distance: usize,
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut h_x = Vec::new();
    let mut h_z = Vec::new();

    for ax in 0..=row_distance {
        for ay in 0..=column_distance {
            let on_row_boundary = ax == 0 || ax == row_distance;
            let on_column_boundary = ay == 0 || ay == column_distance;
            let parity = (ax % 2) != (ay % 2);
            if on_row_boundary && parity {
                continue;
            }
            if on_column_boundary && !parity {
                continue;
            }

            let support =
                rotated_surface_measure_support(row_distance, column_distance, ax, ay);
            if support.is_empty() {
                continue;
            }

            if parity {
                h_x.push(support);
            } else {
                h_z.push(support);
            }
        }
    }

    (h_x, h_z)
}
```

Add ordinary planar helpers using the parity grid described in the design:

```rust
fn unrotated_surface_supports(
    row_distance: usize,
    column_distance: usize,
) -> Result<(Vec<Vec<usize>>, Vec<Vec<usize>>)> {
    let grid_rows = checked_surface_grid_extent(row_distance, "row grid extent")?;
    let grid_columns = checked_surface_grid_extent(column_distance, "column grid extent")?;
    let data_indices = unrotated_surface_data_indices(grid_rows, grid_columns)?;
    let mut h_x = Vec::with_capacity(
        row_distance
            .checked_sub(1)
            .and_then(|rows| rows.checked_mul(column_distance))
            .ok_or_else(|| surface_overflow("X-check count"))?,
    );
    let mut h_z = Vec::with_capacity(
        column_distance
            .checked_sub(1)
            .and_then(|columns| row_distance.checked_mul(columns))
            .ok_or_else(|| surface_overflow("Z-check count"))?,
    );

    for row in (1..grid_rows).step_by(2) {
        for column in (0..grid_columns).step_by(2) {
            h_x.push(unrotated_surface_check_support(
                grid_rows,
                grid_columns,
                &data_indices,
                row,
                column,
            ));
        }
    }
    for row in (0..grid_rows).step_by(2) {
        for column in (1..grid_columns).step_by(2) {
            h_z.push(unrotated_surface_check_support(
                grid_rows,
                grid_columns,
                &data_indices,
                row,
                column,
            ));
        }
    }

    Ok((h_x, h_z))
}
```

- [ ] **Step 4: Wire construction and JSON parsing**

Replace the surface match arm in `construct_css` with:

```rust
CssConstructionSpec::Family(CssFamilySpec::Surface(spec)) => construct_legacy_surface(spec),
CssConstructionSpec::Surface(spec) => construct_surface(spec),
```

Change inline lowering to:

```rust
return Ok(CssFamilySpec::Surface(SurfaceFamilySpec { distance }).into());
```

Change JSON parsing for `"surface"` to call a `surface_construction_from_json(object,
construction)` helper that accepts either legacy `distance` or the new
`layout`/`row_distance`/`column_distance` fields and rejects conflicts.

- [ ] **Step 5: Update existing #553 tests**

In `qec-code/tests/family_contract.rs`, update imports and struct literals:

```rust
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality,
    CssClassicalCheckSpec, CssConstructionSpec, CssFamilySpec, HypergraphProductSpec,
    RequestedFamilyId, SurfaceFamilySpec, SurfaceSpec, CLASSICAL_IDENTITY_2,
};
```

Keep `SurfaceFamilySpec { distance: 3 }` for the legacy route. Use
`SurfaceSpec::rotated_square(3).into()` for generalized surface construction.

- [ ] **Step 6: Run exact tests and verify GREEN**

Run the four issue exact commands. Expected: all four pass.

### Task 3: Documentation, Formatting, And Contract Regression

**Files:**
- Modify: `docs/showcases/qec-code-css-construction.md`
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`
- Modify: `qec-code/tests/surface_family.rs`

**Interfaces:**
- Consumes: passing task 2 implementation.
- Produces: updated docs and clean formatting across touched Rust files.

- [ ] **Step 1: Document structured rectangular surface JSON**

In `docs/showcases/qec-code-css-construction.md`, keep the existing
`surface_rotated:d=3` compact example and add this structured example near the
current surface JSON description:

```json
{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":3,"column_distance":5}
```

Also state that `{"schema_version":1,"construction":"surface","distance":3}`
remains the legacy square rotated adapter.

- [ ] **Step 2: Format touched Rust files**

Run:

```bash
cargo fmt --check -p qec-code
cargo fmt -p qec-code
cargo fmt --check -p qec-code
```

Expected: first command may fail if formatting is needed; final command passes.

- [ ] **Step 3: Run focused regression**

Run:

```bash
cargo test -p qec-code --test family_contract
cargo test -p qec-code --test surface_family
cargo test -p qec-code --test cli run_code_css_construct_json_surface_rotated_d3_matches_inline_fixture -- --exact
```

Expected: all pass.

- [ ] **Step 4: Commit implementation**

Run:

```bash
git add qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/surface_family.rs qec-code/tests/fixtures/css/surface_rotated_d4_hx.json qec-code/tests/fixtures/css/surface_rotated_d4_hz.json docs/showcases/qec-code-css-construction.md docs/superpowers/plans/2026-07-27-issue-569-rectangular-surface-family.md
git commit -m "feat: add rectangular surface family"
```

- [ ] **Step 5: Final verification before finishing**

Run:

```bash
cargo test -p qec-code --test surface_family rectangular_rotated_surface_3x5_matches_fixture -- --exact
cargo test -p qec-code --test surface_family ordinary_surface_d3_matches_fixture -- --exact
cargo test -p qec-code --test surface_family legacy_rotated_surface_outputs_are_unchanged -- --exact
cargo test -p qec-code --test surface_family surface_family_rejects_invalid_dimensions -- --exact
cargo test
```

Expected: exact issue commands pass. `cargo test` should pass; if the command is
terminated by the environment after long-running suites, record the exact exit
status and last passing output as a verification risk.
