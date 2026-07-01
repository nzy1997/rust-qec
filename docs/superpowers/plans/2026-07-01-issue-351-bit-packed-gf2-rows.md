# Issue 351 Bit-Packed GF(2) Rows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add crate-private bit-packed GF(2) row primitives with dense round-trip behavior, width-aware tail-bit handling, and focused tests.

**Architecture:** Implement `BitPackedRow` inside `qec-code/src/gf2.rs` near the existing dense row helpers. Keep public and existing crate-private dense APIs unchanged, use `u64` words plus explicit logical width, and validate all dense inputs with the existing `QecError` style.

**Tech Stack:** Rust 2024, existing `qec-code` GF(2) helpers, no new dependencies, Cargo tests.

## Global Constraints

- Keep the public `try_random_window_kernel_basis_with_width` and existing dense GF(2) APIs unchanged.
- Bit-packed helpers must remain `pub(crate)`.
- For every valid dense binary row and width, packing then unpacking returns the original logical row exactly.
- Operations must ignore storage padding beyond the logical width.
- Non-binary dense input, dense row-width mismatch, and incompatible bit-packed operation widths must return clear `QecError` values.
- Test widths must include `0`, `1`, `63`, `64`, `65`, and at least one width greater than `128`.
- Do not replace random-window kernel generation or CSS component span filtering in this issue.
- Do not add SIMD, unsafe code, external GF(2) libraries, M4RI, `dist-m4ri`, `QDistRnd`, or `codeDistancePYPI` dependencies.
- Run the issue verification commands plus `cargo test -p qec-code gf2 -q` and `cargo test`.

---

## File Structure

- Modify `qec-code/src/gf2.rs`: add `BitPackedRow`, helpers for word count and final-word masking, and unit tests.
- Create no runtime dependencies and no new public API.

### Task 1: Bit-Packed Row Tests And Implementation

**Files:**
- Modify: `qec-code/src/gf2.rs`
- Test: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: `BinaryRow`, `validate_target`, `QecError`, `Result`.
- Produces: `pub(crate) struct BitPackedRow` with `try_from_dense`, `zeros`, `width`, `to_dense`, `xor_assign`, `dot_parity`, `weight`, `eq_logical`, and `is_zero`.

- [ ] **Step 1: Write failing tests**

Add the following behavioral tests in the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn gf2_bitpacked_rows_match_dense_binary_rows() {
    for width in [0, 1, 63, 64, 65, 144] {
        let dense = patterned_row(width, 7);
        let packed = BitPackedRow::try_from_dense(&dense, width).unwrap();

        assert_eq!(packed.width(), width);
        assert_eq!(packed.to_dense(), dense);
    }
}

#[test]
fn gf2_bitpacked_row_ops_match_dense_ops() {
    let lhs_dense = patterned_row(144, 3);
    let rhs_dense = patterned_row(144, 5);
    let mut expected_xor = lhs_dense.clone();
    for (left, right) in expected_xor.iter_mut().zip(&rhs_dense) {
        *left ^= *right;
    }

    let mut lhs = BitPackedRow::try_from_dense(&lhs_dense, 144).unwrap();
    let rhs = BitPackedRow::try_from_dense(&rhs_dense, 144).unwrap();

    assert_eq!(lhs.dot_parity(&rhs).unwrap(), dense_dot_parity(&lhs_dense, &rhs_dense));
    assert_eq!(lhs.weight(), dense_weight(&lhs_dense));
    assert!(!lhs.eq_logical(&rhs).unwrap());
    assert!(!lhs.is_zero());

    lhs.xor_assign(&rhs).unwrap();
    assert_eq!(lhs.to_dense(), expected_xor);
    assert_eq!(lhs.weight(), dense_weight(&expected_xor));
    assert_eq!(lhs.dot_parity(&rhs).unwrap(), dense_dot_parity(&expected_xor, &rhs_dense));
    assert!(BitPackedRow::zeros(144).is_zero());
    assert!(BitPackedRow::zeros(144)
        .eq_logical(&BitPackedRow::try_from_dense(&vec![0; 144], 144).unwrap())
        .unwrap());
}

#[test]
fn gf2_bitpacked_row_ops_handle_tail_bits() {
    for width in [1, 63, 65, 144] {
        let dense = patterned_row(width, 11);
        let mut clean = BitPackedRow::try_from_dense(&dense, width).unwrap();
        let mut dirty = BitPackedRow::try_from_dense(&dense, width).unwrap();
        dirty.set_storage_padding_for_test();

        assert_eq!(dirty.to_dense(), dense);
        assert_eq!(dirty.weight(), dense_weight(&dense));
        assert_eq!(dirty.dot_parity(&clean).unwrap(), dense_dot_parity(&dense, &dense));
        assert!(dirty.eq_logical(&clean).unwrap());
        assert_eq!(dirty.is_zero(), dense.iter().all(|bit| *bit == 0));

        clean.xor_assign(&dirty).unwrap();
        assert!(clean.is_zero());
        assert_eq!(clean.to_dense(), vec![0; width]);
    }
}

#[test]
fn gf2_bitpacked_rows_reject_invalid_binary_inputs() {
    assert_eq!(
        BitPackedRow::try_from_dense(&[1, 2, 0], 3),
        Err(QecError::InvalidBinaryEntry {
            row: 0,
            col: 1,
            value: 2,
        })
    );
    assert_eq!(
        BitPackedRow::try_from_dense(&[1, 0], 3),
        Err(QecError::RowWidthMismatch {
            expected: 3,
            actual: 2,
        })
    );

    let width_three = BitPackedRow::try_from_dense(&[1, 0, 1], 3).unwrap();
    let width_four = BitPackedRow::try_from_dense(&[1, 0, 1, 0], 4).unwrap();
    assert_eq!(
        width_three.dot_parity(&width_four),
        Err(QecError::RowWidthMismatch {
            expected: 3,
            actual: 4,
        })
    );
    assert_eq!(
        width_three.eq_logical(&width_four),
        Err(QecError::RowWidthMismatch {
            expected: 3,
            actual: 4,
        })
    );
    let mut width_three_copy = width_three.clone();
    assert_eq!(
        width_three_copy.xor_assign(&width_four),
        Err(QecError::RowWidthMismatch {
            expected: 3,
            actual: 4,
        })
    );
}
```

Also add helper functions in the test module:

```rust
fn patterned_row(width: usize, salt: usize) -> Vec<u8> {
    (0..width)
        .map(|index| u8::from(((index * salt + index / 3 + salt) % 5) < 2))
        .collect()
}

fn dense_weight(row: &[u8]) -> usize {
    row.iter().map(|bit| usize::from(*bit)).sum()
}

fn dense_dot_parity(lhs: &[u8], rhs: &[u8]) -> u8 {
    lhs.iter()
        .zip(rhs)
        .fold(0, |parity, (left, right)| parity ^ (*left & *right))
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code gf2_bitpacked_rows_match_dense_binary_rows -q
cargo test -p qec-code gf2_bitpacked_row_ops_match_dense_ops -q
cargo test -p qec-code gf2_bitpacked_row_ops_handle_tail_bits -q
cargo test -p qec-code gf2_bitpacked_rows_reject_invalid_binary_inputs -q
```

Expected: the tests fail to compile because `BitPackedRow` does not exist yet.

- [ ] **Step 3: Implement `BitPackedRow`**

Add `BitPackedRow` near `BinaryRow` and `ReducedRows`. Use `Vec<u64>` storage,
little-endian bit positions, `word_count(width)`, `tail_mask(width)`, and a
private `clear_padding_bits` method. `try_from_dense` must validate binary input
with `validate_target(row)` and then check `row.len() == width`.

Required method behavior:

```rust
pub(crate) fn try_from_dense(row: &[u8], width: usize) -> Result<Self>;
pub(crate) fn zeros(width: usize) -> Self;
pub(crate) fn width(&self) -> usize;
pub(crate) fn to_dense(&self) -> Vec<u8>;
pub(crate) fn xor_assign(&mut self, rhs: &Self) -> Result<()>;
pub(crate) fn dot_parity(&self, rhs: &Self) -> Result<u8>;
pub(crate) fn weight(&self) -> usize;
pub(crate) fn eq_logical(&self, rhs: &Self) -> Result<bool>;
pub(crate) fn is_zero(&self) -> bool;
```

- [ ] **Step 4: Run focused GREEN tests**

Run the four issue-specific tests from Step 2. Expected: all pass.

- [ ] **Step 5: Run broader GF(2) and workspace verification**

Run:

```bash
cargo test -p qec-code gf2 -q
cargo test
```

Expected: all tests pass.
