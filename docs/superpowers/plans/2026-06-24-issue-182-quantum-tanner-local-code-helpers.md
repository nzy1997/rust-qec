# Issue 182 Quantum Tanner Local Code Helpers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add local GF(2) binary-code dual and tensor helpers for the quantum Tanner parser.

**Architecture:** Keep the code in `qec-code/src/codes/quantum_tanner.rs`, next to the parser added by #178. Reuse private `qec-code/src/gf2.rs` helpers for binary validation, rank, nullspace, and independent row bases; expose only quantum Tanner-specific structs and one helper function for later sparse CSS construction.

**Tech Stack:** Rust 2024, existing `serde`/`serde_json`, existing `QecError`, existing `qec-code::gf2` utilities, fixture-backed integration tests in `qec-code/tests/code.rs`.

## Global Constraints

- Keep this code local to `qec-code`.
- Reuse existing GF(2) helpers from `qec-code/src/gf2.rs` where possible.
- Do not add public abstractions broader than quantum Tanner needs.
- If local code input contains only check matrices, derive generator and dual information from those rows.
- If optional generator rows are supplied, verify check/generator orthogonality over GF(2) and verify the supplied generator rank is `width - rank(check_rows)`.
- Include validation for non-binary values and inconsistent matrix widths.
- Positive verification must match the hand-computed repetition-code tensor example used by `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`.
- Negative verification must reject a non-binary local-code entry and a supplied check/generator pair that is not orthogonal over GF(2).
- Do not enumerate Cayley-complex faces, generate global `Hx`/`Hz`, add CLI support, compute code distance, or implement a general classical-code library.
- Focused verification command is `cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q`.
- Broader Agent Desk verification must include `cargo test`.

---

## File Structure

- Modify `qec-code/src/codes/quantum_tanner.rs`: parse optional `g_a`/`g_b`, validate optional generators, add local-code structs, derive local generator/dual bases, and form tensor rows.
- Modify `qec-code/tests/code.rs`: import the new helper/types and add the required positive and negative integration test.
- Keep this plan in `docs/superpowers/plans/2026-06-24-issue-182-quantum-tanner-local-code-helpers.md`.

### Task 1: Local Binary Code Dual And Tensor Helpers

**Files:**
- Modify: `qec-code/src/codes/quantum_tanner.rs`
- Modify: `qec-code/tests/code.rs`
- Modify: `docs/superpowers/plans/2026-06-24-issue-182-quantum-tanner-local-code-helpers.md`

**Interfaces:**
- Consumes: `QuantumTannerSpec` from `qec-code/src/codes/quantum_tanner.rs`.
- Consumes: parsed `local_codes.h_a`, `local_codes.h_b`, and optional parsed `local_codes.g_a`, `local_codes.g_b`.
- Produces: `pub struct QuantumTannerLocalBinaryCode { pub width: usize, pub generator_rows: Vec<Vec<u8>>, pub dual_rows: Vec<Vec<u8>> }`.
- Produces: `pub struct QuantumTannerLocalCodeTensorDual { pub code_a: QuantumTannerLocalBinaryCode, pub code_b: QuantumTannerLocalBinaryCode, pub x_sector_rows: Vec<Vec<u8>>, pub z_sector_rows: Vec<Vec<u8>> }`.
- Produces: `pub fn quantum_tanner_local_code_tensor_dual(spec: &QuantumTannerSpec) -> Result<QuantumTannerLocalCodeTensorDual>`.
- Extends: `QuantumTannerLocalCodes` with `pub g_a: Option<Vec<Vec<u8>>>` and `pub g_b: Option<Vec<Vec<u8>>>`.

- [ ] **Step 1: Write the failing integration test**

Modify the quantum Tanner import in `qec-code/tests/code.rs`:

```rust
use qec_code::codes::quantum_tanner::{
    quantum_tanner_local_code_tensor_dual, quantum_tanner_spec_from_json_str,
    QuantumTannerConstructionMode,
};
```

Add this helper near `toric_d4_json_with`:

```rust
fn expect_quantum_tanner_local_code_matrix_error(
    input: &str,
    expected_matrix: &'static str,
    expected_reason_part: &str,
) {
    let error = quantum_tanner_spec_from_json_str(input)
        .and_then(|spec| quantum_tanner_local_code_tensor_dual(&spec))
        .unwrap_err();
    let QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason } = error else {
        panic!("expected InvalidQuantumTannerLocalCodeMatrix, got {error:?}");
    };
    assert_eq!(matrix, expected_matrix);
    assert!(
        reason.contains(expected_reason_part),
        "expected reason to contain {expected_reason_part:?}, got {reason:?}"
    );
}
```

Add this test near the existing quantum Tanner parser tests:

```rust
#[test]
fn quantum_tanner_local_code_tensor_dual_repetition_example_rejects_bad_inputs() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();
    let local = quantum_tanner_local_code_tensor_dual(&spec).unwrap();

    assert_eq!(local.code_a.width, 2);
    assert_eq!(local.code_a.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_a.dual_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_b.width, 2);
    assert_eq!(local.code_b.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_b.dual_rows, vec![vec![1, 1]]);
    assert_eq!(local.x_sector_rows, vec![vec![1, 1, 1, 1]]);
    assert_eq!(local.z_sector_rows, vec![vec![1, 1, 1, 1]]);

    let nonbinary_h_a = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["h_a"][0][0] = Value::from(2);
    });
    expect_quantum_tanner_local_code_matrix_error(&nonbinary_h_a, "h_a", "expected 0 or 1");

    let nonorthogonal_g_a = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["g_a"] = serde_json::json!([[1, 0]]);
    });
    expect_quantum_tanner_local_code_matrix_error(
        &nonorthogonal_g_a,
        "code_a",
        "not orthogonal",
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q --offline
```

Expected: FAIL to compile because `quantum_tanner_local_code_tensor_dual` does not exist yet. If the environment can access crates.io without the local proxy, also run the exact non-offline command:

```bash
cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q
```

Expected for the exact command in the Agent Desk sandbox may be a crates.io proxy/network failure before compilation; record that separately if it happens.

- [ ] **Step 3: Extend the parser local-code structs**

In `qec-code/src/codes/quantum_tanner.rs`, add `use crate::gf2;` after the existing crate imports:

```rust
use crate::error::{QecError, Result};
use crate::gf2;
```

Extend `QuantumTannerLocalCodes`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalCodes {
    pub matrix_role: String,
    pub field: String,
    pub h_a: Vec<Vec<u8>>,
    pub h_b: Vec<Vec<u8>>,
    pub g_a: Option<Vec<Vec<u8>>>,
    pub g_b: Option<Vec<Vec<u8>>>,
}
```

Extend `QuantumTannerLocalCodesJson`:

```rust
#[derive(Debug, Deserialize)]
struct QuantumTannerLocalCodesJson {
    matrix_role: String,
    field: String,
    h_a: Vec<Vec<u8>>,
    h_b: Vec<Vec<u8>>,
    #[serde(default)]
    g_a: Option<Vec<Vec<u8>>>,
    #[serde(default)]
    g_b: Option<Vec<Vec<u8>>>,
}
```

Modify the end of `parse_local_codes` so optional generator rows are validated and preserved:

```rust
    validate_binary_matrix_width("h_a", &local_codes.h_a, a_width)?;
    validate_binary_matrix_width("h_b", &local_codes.h_b, b_width)?;
    validate_optional_generator_rows("code_a", "g_a", &local_codes.h_a, local_codes.g_a.as_deref(), a_width)?;
    validate_optional_generator_rows("code_b", "g_b", &local_codes.h_b, local_codes.g_b.as_deref(), b_width)?;

    Ok(QuantumTannerLocalCodes {
        matrix_role: local_codes.matrix_role,
        field: local_codes.field,
        h_a: local_codes.h_a,
        h_b: local_codes.h_b,
        g_a: local_codes.g_a,
        g_b: local_codes.g_b,
    })
```

- [ ] **Step 4: Add the local-code helper structs and functions**

Add this code below `QuantumTannerLocalCodes` in `qec-code/src/codes/quantum_tanner.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalBinaryCode {
    pub width: usize,
    pub generator_rows: Vec<Vec<u8>>,
    pub dual_rows: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalCodeTensorDual {
    pub code_a: QuantumTannerLocalBinaryCode,
    pub code_b: QuantumTannerLocalBinaryCode,
    pub x_sector_rows: Vec<Vec<u8>>,
    pub z_sector_rows: Vec<Vec<u8>>,
}
```

Add this public helper below `quantum_tanner_spec_from_json_str`:

```rust
pub fn quantum_tanner_local_code_tensor_dual(
    spec: &QuantumTannerSpec,
) -> Result<QuantumTannerLocalCodeTensorDual> {
    let code_a = build_local_binary_code(
        "code_a",
        &spec.local_codes.h_a,
        spec.local_codes.g_a.as_deref(),
        spec.a_generator_indices.len(),
    )?;
    let code_b = build_local_binary_code(
        "code_b",
        &spec.local_codes.h_b,
        spec.local_codes.g_b.as_deref(),
        spec.b_generator_indices.len(),
    )?;
    let x_sector_rows = tensor_product_rows(&code_a.generator_rows, &code_b.generator_rows);
    let z_sector_rows = tensor_product_rows(&code_a.dual_rows, &code_b.dual_rows);

    Ok(QuantumTannerLocalCodeTensorDual {
        code_a,
        code_b,
        x_sector_rows,
        z_sector_rows,
    })
}
```

Add these private helpers near the existing local-code validation helpers:

```rust
fn build_local_binary_code(
    code: &'static str,
    check_rows: &[Vec<u8>],
    supplied_generator_rows: Option<&[Vec<u8>]>,
    width: usize,
) -> Result<QuantumTannerLocalBinaryCode> {
    validate_binary_matrix_width(code, check_rows, width)?;
    let dual_rows = gf2::try_select_independent_rows(check_rows)
        .map_err(|error| local_code_error(code, error.to_string()))?;
    let generator_rows = match supplied_generator_rows {
        Some(rows) => validate_generator_rows(code, "generator", check_rows, rows, width)?,
        None => gf2::try_nullspace_basis_with_width(check_rows, width)
            .map_err(|error| local_code_error(code, error.to_string()))?,
    };

    Ok(QuantumTannerLocalBinaryCode {
        width,
        generator_rows,
        dual_rows,
    })
}

fn validate_optional_generator_rows(
    code: &'static str,
    matrix: &'static str,
    check_rows: &[Vec<u8>],
    generator_rows: Option<&[Vec<u8>]>,
    width: usize,
) -> Result<()> {
    let Some(generator_rows) = generator_rows else {
        return Ok(());
    };
    validate_generator_rows(code, matrix, check_rows, generator_rows, width)?;
    Ok(())
}

fn validate_generator_rows(
    code: &'static str,
    matrix: &'static str,
    check_rows: &[Vec<u8>],
    generator_rows: &[Vec<u8>],
    width: usize,
) -> Result<Vec<Vec<u8>>> {
    validate_binary_matrix_width(matrix, generator_rows, width)?;
    for (check_index, check_row) in check_rows.iter().enumerate() {
        for (generator_index, generator_row) in generator_rows.iter().enumerate() {
            if dot_mod2(check_row, generator_row) != 0 {
                return Err(local_code_error(
                    code,
                    format!(
                        "{matrix} row {generator_index} is not orthogonal to check row {check_index}"
                    ),
                ));
            }
        }
    }

    let check_rank =
        gf2::try_rank(check_rows).map_err(|error| local_code_error(code, error.to_string()))?;
    let expected_generator_rank = width - check_rank;
    let generator_basis = gf2::try_select_independent_rows(generator_rows)
        .map_err(|error| local_code_error(code, error.to_string()))?;
    if generator_basis.len() != expected_generator_rank {
        return Err(local_code_error(
            code,
            format!(
                "{matrix} rank is {}, expected {expected_generator_rank}",
                generator_basis.len()
            ),
        ));
    }

    Ok(generator_basis)
}

fn dot_mod2(lhs: &[u8], rhs: &[u8]) -> u8 {
    lhs.iter()
        .zip(rhs)
        .fold(0, |parity, (&left, &right)| parity ^ (left & right))
}

fn tensor_product_rows(lhs: &[Vec<u8>], rhs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    lhs.iter()
        .flat_map(|left| {
            rhs.iter().map(move |right| {
                left.iter()
                    .flat_map(|&left_bit| right.iter().map(move |&right_bit| left_bit & right_bit))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

fn local_code_error(matrix: &'static str, reason: String) -> QecError {
    QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason }
}
```

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q --offline
```

Expected: PASS with `quantum_tanner_local_code_tensor_dual_repetition_example_rejects_bad_inputs` executed.

- [ ] **Step 6: Format touched Rust files**

Run:

```bash
rustfmt qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs
```

Expected: command exits 0 and only formats touched Rust files.

- [ ] **Step 7: Run focused verification after formatting**

Run:

```bash
cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q --offline
```

Expected: PASS.

- [ ] **Step 8: Run broader verification**

Run:

```bash
cargo test --offline
```

Expected: PASS for the workspace test suite. Then run the Agent Desk required exact command:

```bash
cargo test
```

Expected in a network-enabled environment: PASS. If the sandbox blocks crates.io access through the configured proxy, record the network failure and keep the offline `cargo test --offline` result as the executable verification evidence.

- [ ] **Step 9: Check whitespace and review scope**

Run:

```bash
git diff --check
git status --short
git diff --stat HEAD
```

Expected: no whitespace errors. Scope should be limited to the design doc commit, this plan, `qec-code/src/codes/quantum_tanner.rs`, and `qec-code/tests/code.rs`.

- [ ] **Step 10: Commit implementation**

Run:

```bash
git add qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs docs/superpowers/plans/2026-06-24-issue-182-quantum-tanner-local-code-helpers.md
git commit -m "feat: add quantum tanner local code helpers"
```

Expected: one implementation commit after the design commit.
