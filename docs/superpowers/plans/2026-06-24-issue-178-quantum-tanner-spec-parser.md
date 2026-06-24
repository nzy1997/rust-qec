# Issue 178 Quantum Tanner Spec Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a serde-backed `QuantumTannerSpec` JSON parser that reads the shared quantum Tanner fixtures into typed Rust data and rejects malformed multiplication-table shape.

**Architecture:** Put the parser in `qec-code/src/codes/quantum_tanner.rs`, exported from `qec-code/src/codes/mod.rs`, so it lives next to future construction code. Use serde only for JSON decoding, then run explicit shape checks for group-table and local-code matrix structure while leaving group axioms, generator symmetry, face enumeration, and CSS generation to later issues.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, existing `qec-code::QecError`, existing quantum Tanner contract and fixtures under `qec-code/doc/quantum_tanner.md` and `qec-code/tests/fixtures/quantum_tanner/`.

## Global Constraints

- Implement the parser near future construction code at `qec-code/src/codes/quantum_tanner.rs`.
- Export the module from `qec-code/src/codes/mod.rs`.
- Accept the #177/#179 contract/catalog JSON field names: `construction_mode`, `base_group`, `a_generator_indices`, `b_generator_indices`, and `local_codes`.
- Supported construction mode is exactly `lr_cayley_no_cover_v1`.
- The parser must consume the committed positive fixture `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`.
- The parser must reject the committed malformed-table fixture `qec-code/tests/fixtures/quantum_tanner/invalid_bad_table.json`.
- Do not validate group axioms, check generator symmetry, enumerate Cayley faces, compute local tensor/dual codes, generate `Hx`/`Hz`, or add CLI support.
- Keep references in comments/docstrings to `drafts/qLDPC/src/qldpc/codes/quantum.py` for `QTCode` input concepts and `drafts/qLDPC/src/qldpc/objects.py` for `CayleyComplex` input concepts.
- Verification command is `cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q`.
- Broader repository verification must include `cargo test`.

---

## File Structure

- Modify `qec-code/src/error.rs`: add typed parser error variants.
- Create `qec-code/src/codes/quantum_tanner.rs`: typed parser structs, mode enum, JSON DTOs, and parse-time shape validation.
- Modify `qec-code/src/codes/mod.rs`: export `quantum_tanner`.
- Modify `qec-code/tests/code.rs`: add the required fixture-backed parser test.
- Keep this plan in `docs/superpowers/plans/2026-06-24-issue-178-quantum-tanner-spec-parser.md`.

### Task 1: QuantumTannerSpec Parser API And Fixture Test

**Files:**
- Modify: `qec-code/src/error.rs`
- Create: `qec-code/src/codes/quantum_tanner.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/tests/code.rs`
- Modify: `docs/superpowers/plans/2026-06-24-issue-178-quantum-tanner-spec-parser.md`

**Interfaces:**
- Consumes: `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`, `qec-code/tests/fixtures/quantum_tanner/invalid_bad_table.json`, and existing `QecError`.
- Produces: public parser function `qec_code::codes::quantum_tanner::quantum_tanner_spec_from_json_str(input: &str) -> qec_code::error::Result<QuantumTannerSpec>`.
- Produces: public enum `QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1`.
- Produces: public typed value `QuantumTannerSpec` with public fields `construction_mode`, `base_group`, `a_generator_indices`, `b_generator_indices`, and `local_codes`.

- [x] **Step 1: Write the failing parser test**

Modify the `qec-code/tests/code.rs` imports to include the parser API:

```rust
use qec_code::codes::quantum_tanner::{
    QuantumTannerConstructionMode, quantum_tanner_spec_from_json_str,
};
```

Add this test near the existing quantum Tanner catalog and contract tests:

```rust
#[test]
fn quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    assert_eq!(
        spec.construction_mode,
        QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1
    );
    assert_eq!(spec.base_group.order, 16);
    assert_eq!(spec.base_group.identity, 0);
    assert_eq!(spec.base_group.multiplication_table.len(), 16);
    assert!(
        spec.base_group
            .multiplication_table
            .iter()
            .all(|row| row.len() == 16)
    );
    assert_eq!(spec.a_generator_indices, vec![4, 12]);
    assert_eq!(spec.b_generator_indices, vec![1, 3]);
    assert_eq!(spec.local_codes.matrix_role.as_str(), "parity_check");
    assert_eq!(spec.local_codes.field.as_str(), "GF(2)");
    assert_eq!(spec.local_codes.h_a, vec![vec![1, 1]]);
    assert_eq!(spec.local_codes.h_b, vec![vec![1, 1]]);

    let error = quantum_tanner_spec_from_json_str(include_str!(
        "fixtures/quantum_tanner/invalid_bad_table.json"
    ))
    .unwrap_err();

    assert!(
        matches!(error, QecError::InvalidQuantumTannerGroupTable { .. }),
        "expected malformed table to fail before construction, got {error:?}"
    );
    assert!(
        error.to_string().contains("row 0"),
        "malformed table error should identify the bad row: {error}"
    );
}
```

- [x] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q
```

Expected: FAIL to compile because `qec_code::codes::quantum_tanner` does not exist yet. That proves the test is wired to the missing parser API.

- [x] **Step 3: Add typed parser errors**

Add these variants to `QecError` in `qec-code/src/error.rs` near the other input-parsing errors:

```rust
#[error("invalid quantum Tanner spec JSON: {0}")]
InvalidQuantumTannerSpecJson(String),
#[error("invalid quantum Tanner group table: {reason}")]
InvalidQuantumTannerGroupTable { reason: String },
#[error("unsupported quantum Tanner construction mode: {mode}")]
UnsupportedQuantumTannerConstructionMode { mode: String },
#[error("invalid quantum Tanner local code matrix {matrix}: {reason}")]
InvalidQuantumTannerLocalCodeMatrix {
    matrix: &'static str,
    reason: String,
},
```

- [x] **Step 4: Export the parser module**

Modify `qec-code/src/codes/mod.rs`:

```rust
pub(crate) mod apm;
pub mod built_in_css;
pub mod quantum_tanner;
pub mod steane;
```

- [x] **Step 5: Implement the parser module**

Create `qec-code/src/codes/quantum_tanner.rs` with this complete implementation:

```rust
use serde::Deserialize;

use crate::error::{QecError, Result};

pub const LR_CAYLEY_NO_COVER_V1: &str = "lr_cayley_no_cover_v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerSpec {
    pub construction_mode: QuantumTannerConstructionMode,
    pub base_group: ExplicitFiniteGroup,
    pub a_generator_indices: Vec<usize>,
    pub b_generator_indices: Vec<usize>,
    pub local_codes: QuantumTannerLocalCodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantumTannerConstructionMode {
    LeftRightCayleyNoCoverV1,
}

impl QuantumTannerConstructionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeftRightCayleyNoCoverV1 => LR_CAYLEY_NO_COVER_V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitFiniteGroup {
    pub name: Option<String>,
    pub element_order: Option<String>,
    pub order: usize,
    pub identity: usize,
    pub multiplication_table: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalCodes {
    pub matrix_role: String,
    pub field: String,
    pub h_a: Vec<Vec<u8>>,
    pub h_b: Vec<Vec<u8>>,
}

#[derive(Debug, Deserialize)]
struct QuantumTannerSpecJson {
    construction_mode: String,
    base_group: ExplicitFiniteGroupJson,
    a_generator_indices: Vec<usize>,
    b_generator_indices: Vec<usize>,
    local_codes: QuantumTannerLocalCodesJson,
}

#[derive(Debug, Deserialize)]
struct ExplicitFiniteGroupJson {
    name: Option<String>,
    element_order: Option<String>,
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
struct QuantumTannerLocalCodesJson {
    matrix_role: String,
    field: String,
    h_a: Vec<Vec<u8>>,
    h_b: Vec<Vec<u8>>,
}

/// Parse explicit quantum Tanner input JSON into typed Rust data.
///
/// Input concepts follow the qLDPC `QTCode` and `CayleyComplex` vocabulary in
/// `drafts/qLDPC/src/qldpc/codes/quantum.py` and
/// `drafts/qLDPC/src/qldpc/objects.py`. This parser intentionally stops before
/// semantic group validation, generator symmetry checks, face enumeration, or
/// CSS matrix generation.
pub fn quantum_tanner_spec_from_json_str(input: &str) -> Result<QuantumTannerSpec> {
    let parsed: QuantumTannerSpecJson = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidQuantumTannerSpecJson(error.to_string()))?;

    let construction_mode = parse_construction_mode(&parsed.construction_mode)?;
    validate_group_table(
        parsed.base_group.order,
        parsed.base_group.identity,
        &parsed.base_group.multiplication_table,
    )?;
    let local_codes = parse_local_codes(
        parsed.local_codes,
        parsed.a_generator_indices.len(),
        parsed.b_generator_indices.len(),
    )?;

    Ok(QuantumTannerSpec {
        construction_mode,
        base_group: ExplicitFiniteGroup {
            name: parsed.base_group.name,
            element_order: parsed.base_group.element_order,
            order: parsed.base_group.order,
            identity: parsed.base_group.identity,
            multiplication_table: parsed.base_group.multiplication_table,
        },
        a_generator_indices: parsed.a_generator_indices,
        b_generator_indices: parsed.b_generator_indices,
        local_codes,
    })
}

fn parse_construction_mode(input: &str) -> Result<QuantumTannerConstructionMode> {
    match input {
        LR_CAYLEY_NO_COVER_V1 => Ok(QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1),
        mode => Err(QecError::UnsupportedQuantumTannerConstructionMode {
            mode: mode.to_owned(),
        }),
    }
}

fn validate_group_table(order: usize, identity: usize, table: &[Vec<usize>]) -> Result<()> {
    if order == 0 {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: "order must be positive".to_owned(),
        });
    }
    if identity >= order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("identity {identity} is out of range for order {order}"),
        });
    }
    if table.len() != order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("expected {order} rows, got {}", table.len()),
        });
    }

    for (row_index, row) in table.iter().enumerate() {
        if row.len() != order {
            return Err(QecError::InvalidQuantumTannerGroupTable {
                reason: format!("row {row_index} has width {}, expected {order}", row.len()),
            });
        }
        for (col_index, &entry) in row.iter().enumerate() {
            if entry >= order {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "entry at row {row_index}, column {col_index} is {entry}, expected < {order}"
                    ),
                });
            }
        }
    }

    Ok(())
}

fn parse_local_codes(
    local_codes: QuantumTannerLocalCodesJson,
    a_width: usize,
    b_width: usize,
) -> Result<QuantumTannerLocalCodes> {
    if local_codes.matrix_role != "parity_check" {
        return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
            matrix: "local_codes",
            reason: format!(
                "matrix_role must be parity_check, got {}",
                local_codes.matrix_role
            ),
        });
    }
    if local_codes.field != "GF(2)" {
        return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
            matrix: "local_codes",
            reason: format!("field must be GF(2), got {}", local_codes.field),
        });
    }

    validate_binary_matrix_width("h_a", &local_codes.h_a, a_width)?;
    validate_binary_matrix_width("h_b", &local_codes.h_b, b_width)?;

    Ok(QuantumTannerLocalCodes {
        matrix_role: local_codes.matrix_role,
        field: local_codes.field,
        h_a: local_codes.h_a,
        h_b: local_codes.h_b,
    })
}

fn validate_binary_matrix_width(
    matrix: &'static str,
    rows: &[Vec<u8>],
    expected_width: usize,
) -> Result<()> {
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != expected_width {
            return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
                matrix,
                reason: format!(
                    "row {row_index} has width {}, expected {expected_width}",
                    row.len()
                ),
            });
        }
        for (col_index, &entry) in row.iter().enumerate() {
            if entry > 1 {
                return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
                    matrix,
                    reason: format!(
                        "entry at row {row_index}, column {col_index} is {entry}, expected 0 or 1"
                    ),
                });
            }
        }
    }

    Ok(())
}
```

- [x] **Step 6: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q
```

Expected: PASS with the new parser test executed.

- [x] **Step 7: Run formatting for touched Rust files**

Run:

```bash
rustfmt qec-code/src/error.rs qec-code/src/codes/mod.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs
```

Expected: command exits 0 and only formats the touched Rust files.

- [x] **Step 8: Run focused verification after formatting**

Run:

```bash
cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q
```

Expected: PASS.

- [x] **Step 9: Run broader verification**

Run:

```bash
cargo test
```

Expected: PASS for the workspace test suite.

- [x] **Step 10: Check whitespace and scope**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

Review the diff and confirm it only includes the parser, parser test, and required Superpowers spec/plan files. There must be no constructor, CLI, face enumeration, CSS generation, or unrelated refactor.

- [x] **Step 11: Commit implementation**

Run:

```bash
git add qec-code/src/error.rs qec-code/src/codes/mod.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs docs/superpowers/plans/2026-06-24-issue-178-quantum-tanner-spec-parser.md
git commit -m "feat: add quantum tanner spec parser"
```

Expected: one implementation commit after the earlier design commit.
