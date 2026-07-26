# Issue 564 Shor-Like CSS Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add rectangular generalized Shor-like CSS codes to the common qec-code family contract.

**Architecture:** Extend `qec-code/src/family_contract.rs` with a typed `ShorLikeSpec`, JSON parsing for `construction = "shor_like"`, and a direct sparse-row constructor. Reuse the existing common `construction_result` path so canonical rows, orthogonality, rank/stat calculation, deterministic metadata, Rust API access, and `code css construct --spec` CLI export all stay consistent with the #553 contract.

**Tech Stack:** Rust 2024, serde/serde_json, existing `QecError`, `SparseRowsMatrix`, `CssCode`, exact CSS distance utilities, Cargo integration tests.

## Global Constraints

- `outer_blocks >= 2`.
- `inner_block >= 2`.
- `n = outer_blocks * inner_block`.
- `k = 1`.
- Code distance is `min(outer_blocks, inner_block)`.
- For `outer_blocks=3` and `inner_block=3`, `H_X = [[0,1,2,3,4,5], [3,4,5,6,7,8]]`.
- For `outer_blocks=3` and `inner_block=3`, `H_Z = [[0,1], [1,2], [3,4], [4,5], [6,7], [7,8]]`.
- Successful constructions use `construction_id = "shor_like"`.
- Successful constructions use `requested_family_id = Some(RequestedFamilyId::ShorLike)`.
- Normalized parameters serialize deterministically as `inner_block` and `outer_blocks` keys in a `BTreeMap`.
- Reject dimensions below 2, missing dimensions, zero dimensions, and multiplication overflow with typed errors.
- Rust API and CLI use the common family contract.
- Do not add compact inline syntax in this issue.

---

### Task 1: Shor-Like Family Contract

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Create: `qec-code/tests/shor_like.rs`
- Modify: `docs/superpowers/plans/2026-07-27-issue-564-shor-like-css.md`

**Interfaces:**
- Consumes: `construct_css`, `parse_css_construction_json`, `verify_css_orthogonality`, `CssConstructionSpec`, `CssFamilySpec`, `RequestedFamilyId`, `QecError`, `SparseRowsMatrix`, `CssCode`, and `compute_distance`.
- Produces: public `ShorLikeSpec { outer_blocks: usize, inner_block: usize }` and `CssFamilySpec::ShorLike(ShorLikeSpec)`.

- [ ] **Step 1: Write the failing integration tests**

Create `qec-code/tests/shor_like.rs`:

```rust
use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, CssMatrixKind, run};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, ShorLikeSpec, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use tempfile::tempdir;

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

fn cli_export_from_spec(spec: PathBuf, matrix: CssMatrixKind) -> String {
    let matrix = match matrix {
        CssMatrixKind::Hx => "hx",
        CssMatrixKind::Hz => "hz",
    };
    run(Cli::parse_from([
        "qec-code",
        "code",
        "css",
        "construct",
        "--spec",
        spec.to_str().expect("spec path should be UTF-8"),
        matrix,
    ]))
    .unwrap()
}

#[test]
fn shor_like_3x3_matches_fixture() {
    let expected_hx = vec![vec![0, 1, 2, 3, 4, 5], vec![3, 4, 5, 6, 7, 8]];
    let expected_hz = vec![
        vec![0, 1],
        vec![1, 2],
        vec![3, 4],
        vec![4, 5],
        vec![6, 7],
        vec![7, 8],
    ];
    let result = construct_css(
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 3,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.schema_version, 1);
    assert_eq!(result.construction_id, "shor_like");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::ShorLike));
    assert_eq!(result.normalized_parameters["outer_blocks"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["inner_block"], serde_json::json!(3));
    assert_eq!(result.stats.n, 9);
    assert_eq!(result.stats.m_x, 2);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 2);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.stats.d_x.unwrap().min(result.stats.d_z.unwrap()), 3);
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(compute_distance(css_code_from_result(&result).code()).unwrap().distance, 3);

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":3}"#,
    )
    .unwrap();
    assert_eq!(
        parsed,
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 3,
        })
        .into()
    );
    let repeated = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "shor-like-3x3.json",
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":3}"#,
    );
    let cli_hx = cli_export_from_spec(spec_path, CssMatrixKind::Hx);
    assert_eq!(
        cli_hx,
        SparseRowsMatrix::new(result.stats.n, result.checks.h_x.clone())
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn shor_like_rectangular_3x4_has_expected_parameters() {
    let result = construct_css(
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 4,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.construction_id, "shor_like");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::ShorLike));
    assert_eq!(result.stats.n, 12);
    assert_eq!(result.stats.m_x, 2);
    assert_eq!(result.stats.m_z, 9);
    assert_eq!(result.stats.rank_x, 2);
    assert_eq!(result.stats.rank_z, 9);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(4));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.stats.d_x.unwrap().min(result.stats.d_z.unwrap()), 3);
    assert_eq!(
        result.checks.h_x,
        vec![vec![0, 1, 2, 3, 4, 5, 6, 7], vec![4, 5, 6, 7, 8, 9, 10, 11]]
    );
    assert_eq!(
        result.checks.h_z,
        vec![
            vec![0, 1],
            vec![1, 2],
            vec![2, 3],
            vec![4, 5],
            vec![5, 6],
            vec![6, 7],
            vec![8, 9],
            vec![9, 10],
            vec![10, 11],
        ]
    );
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(compute_distance(css_code_from_result(&result).code()).unwrap().distance, 3);

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "shor-like-3x4.json",
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":4}"#,
    );
    let cli_hz = cli_export_from_spec(spec_path, CssMatrixKind::Hz);
    assert_eq!(
        cli_hz,
        SparseRowsMatrix::new(result.stats.n, result.checks.h_z.clone())
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn shor_like_rejects_invalid_dimensions() {
    for spec in [
        ShorLikeSpec {
            outer_blocks: 1,
            inner_block: 3,
        },
        ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 1,
        },
        ShorLikeSpec {
            outer_blocks: 0,
            inner_block: 3,
        },
        ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 0,
        },
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::ShorLike(spec).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "shor_like" && reason.contains("at least 2")
        ));
    }

    for body in [
        r#"{"schema_version":1,"construction":"shor_like","inner_block":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":0,"inner_block":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":0}"#,
    ] {
        assert!(matches!(
            parse_css_construction_json(body),
            Err(QecError::InvalidCssConstruction { construction, .. })
                if construction == "shor_like"
        ));
    }

    assert!(matches!(
        construct_css(
            CssFamilySpec::ShorLike(ShorLikeSpec {
                outer_blocks: usize::MAX,
                inner_block: 2,
            })
            .into(),
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "shor_like" && reason.contains("overflow")
    ));

    let json_overflow = format!(
        r#"{{"schema_version":1,"construction":"shor_like","outer_blocks":{},"inner_block":2}}"#,
        usize::MAX
    );
    let parsed = parse_css_construction_json(&json_overflow).unwrap();
    assert!(matches!(
        construct_css(parsed),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "shor_like" && reason.contains("overflow")
    ));
}
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cargo test -p qec-code --test shor_like shor_like_3x3_matches_fixture -- --exact
```

Expected: FAIL with unresolved `ShorLikeSpec` or missing `CssFamilySpec::ShorLike`.

- [ ] **Step 3: Implement the typed spec and route**

In `qec-code/src/family_contract.rs`, add a serializable `ShorLikeSpec`, add `ShorLike(ShorLikeSpec)` to `CssFamilySpec`, append `RequestedFamilyId::ShorLike` to `CssFamilySpec::callable_requested_family_ids()`, route it in `construct_css`, add JSON parsing for `construction = "shor_like"`, and add validation/construction helpers with this behavior:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShorLikeSpec {
    pub outer_blocks: usize,
    pub inner_block: usize,
}

fn construct_shor_like(spec: ShorLikeSpec) -> Result<CssConstructionResult> {
    validate_shor_like_spec(&spec)?;
    let n = spec.outer_blocks.checked_mul(spec.inner_block).ok_or_else(|| {
        QecError::InvalidCssConstruction {
            construction: "shor_like".to_owned(),
            reason: "shor_like dimension overflow during data qubit count".to_owned(),
        }
    })?;
    let (h_x, h_z) = shor_like_supports(spec.outer_blocks, spec.inner_block)?;
    let mut parameters = BTreeMap::new();
    parameters.insert("inner_block".to_owned(), Value::from(spec.inner_block));
    parameters.insert("outer_blocks".to_owned(), Value::from(spec.outer_blocks));
    construction_result(
        "shor_like",
        Some(RequestedFamilyId::ShorLike),
        parameters,
        n,
        h_x,
        h_z,
        "shor_like",
        "CssFamilySpec::ShorLike",
        Some((spec.inner_block, spec.outer_blocks)),
    )
}
```

`shor_like_supports` must push X rows for each adjacent outer-block pair and Z rows for each adjacent inner-block pair. Use checked arithmetic for every index and return `InvalidCssConstruction { construction: "shor_like", reason: "... overflow ..." }` if arithmetic overflows.

`parse_css_construction_json` must map:

```rust
"shor_like" => Ok(CssFamilySpec::ShorLike(ShorLikeSpec {
    outer_blocks: required_usize(object, "outer_blocks", construction)?,
    inner_block: required_usize(object, "inner_block", construction)?,
}).into())
```

- [ ] **Step 4: Run green verification for issue tests**

Run:

```bash
cargo test -p qec-code --test shor_like shor_like_3x3_matches_fixture -- --exact
cargo test -p qec-code --test shor_like shor_like_rectangular_3x4_has_expected_parameters -- --exact
cargo test -p qec-code --test shor_like shor_like_rejects_invalid_dimensions -- --exact
```

Expected: PASS for all three commands.

- [ ] **Step 5: Run focused regression tests**

Run:

```bash
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
cargo test -p qec-code --test cli run_code_css_construct_json_surface_rotated_d3_matches_inline_fixture -- --exact
cargo test -p qec-code --test surface_family legacy_rotated_surface_outputs_are_unchanged -- --exact
```

Expected: PASS for all three commands. Update the `planned_families_have_no_callable_stub` expectation to include `RequestedFamilyId::ShorLike` because it is no longer a planned unimplemented family.

- [ ] **Step 6: Format and run full verification**

Run:

```bash
cargo fmt
cargo test
```

Expected: `cargo fmt` exits 0 and `cargo test` exits 0.

- [ ] **Step 7: Commit the implementation**

Run:

```bash
git add qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/shor_like.rs docs/superpowers/plans/2026-07-27-issue-564-shor-like-css.md
git commit -m "feat: add shor-like css family"
```

Expected: commit succeeds with only the Shor-like implementation and plan status changes staged.

## Self-Review

- Spec coverage: Task 1 covers the typed spec, direct sparse construction, common contract API, JSON CLI route, fixtures, rank/stat checks, distance checks, deterministic metadata, orthogonality, and negative controls from issue #564.
- Completion-marker scan: all executable work is represented as checkbox steps, with concrete commands and expected outcomes.
- Type consistency: `ShorLikeSpec`, `CssFamilySpec::ShorLike`, `construction = "shor_like"`, `outer_blocks`, and `inner_block` are used consistently.
