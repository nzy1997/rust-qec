# Packed Inverse Tableau Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone packed inverse-tableau storage primitive with exact identity layout, packed signs, row copy/XOR operations, and acceptance tests for issue #455.

**Architecture:** Implement `PackedInverseTableau` as one focused simulator storage type with separate contiguous X and Z `Vec<u64>` planes and one packed sign `Vec<u64>` plane. Keep it independent from `StabilizerState` and production routing so the PR only adds storage primitives and tests.

**Tech Stack:** Rust 2024, `rstim` crate, integration tests under `rstim/tests/`, Cargo test runner.

## Global Constraints

- Add `rstim/src/sim/packed_inverse_tableau.rs` with `PackedInverseTableau`.
- For `w = ceil(n / 64)`, X and Z are separate contiguous `Vec<u64>` planes, each of length `2*n*w`.
- `(row, qubit)` maps to word `row*w + qubit/64`, bit `qubit%64`.
- Signs use one packed `Vec<u64>` of length `ceil(2*n/64)`.
- Sign bit `1` represents canonical phase `2`; sign bit `0` represents canonical phase `0`.
- Identity has `x(i,i) = true`, `z(n+i,i) = true`, and all other bits and signs zero.
- `n=0` is valid: `w=0`, no rows, and every plane is empty.
- Accessors expose logical X/Z bits, sign bits, and canonical phase values.
- Invalid indexes panic consistently.
- `copy_row(src,dst)` copies X, Z, and sign.
- `xor_pauli_planes(src,dst)` performs `dst.x ^= src.x` and `dst.z ^= src.z`, leaves the sign unchanged, and masks padding.
- Document that `xor_pauli_planes` is a storage primitive, not phase-aware Pauli multiplication.
- Bits at qubit indexes `n..w*64` must remain zero after every mutation.
- Do not implement Clifford evolution, phase-aware row multiplication, measurement/reset, or production routing.
- The focused acceptance command is `cargo test -p rstim --test packed_inverse_tableau_storage -- --nocapture`.
- The final verification command required by Agent Desk is `cargo test`.

---

### Task 1: Packed Inverse Tableau Storage Contract

**Files:**
- Create: `rstim/tests/packed_inverse_tableau_storage.rs`
- Create: `rstim/src/sim/packed_inverse_tableau.rs`
- Modify: `rstim/src/sim/mod.rs`

**Interfaces:**
- Consumes: no new project-local interfaces.
- Produces:
  - `pub struct PackedInverseTableau`
  - `PackedInverseTableau::identity(num_qubits: usize) -> Self`
  - `num_qubits(&self) -> usize`
  - `num_rows(&self) -> usize`
  - `words_per_row(&self) -> usize`
  - `x_plane_words(&self) -> &[u64]`
  - `z_plane_words(&self) -> &[u64]`
  - `sign_words(&self) -> &[u64]`
  - `x(&self, row: usize, qubit: usize) -> bool`
  - `z(&self, row: usize, qubit: usize) -> bool`
  - `sign_bit(&self, row: usize) -> bool`
  - `canonical_phase(&self, row: usize) -> u8`
  - `set_sign_bit(&mut self, row: usize, negative: bool)`
  - `set_canonical_phase(&mut self, row: usize, phase: u8)`
  - `copy_row(&mut self, src: usize, dst: usize)`
  - `xor_pauli_planes(&mut self, src: usize, dst: usize)`

- [ ] **Step 1: Write the failing acceptance test**

Create `rstim/tests/packed_inverse_tableau_storage.rs` with:

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};

use rstim::sim::packed_inverse_tableau::PackedInverseTableau;

fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(64)
}

fn expected_plane_len(num_qubits: usize) -> usize {
    2 * num_qubits * words_for_bits(num_qubits)
}

fn assert_identity(num_qubits: usize) {
    let tableau = PackedInverseTableau::identity(num_qubits);
    let words_per_row = words_for_bits(num_qubits);
    let num_rows = 2 * num_qubits;

    assert_eq!(tableau.num_qubits(), num_qubits);
    assert_eq!(tableau.num_rows(), num_rows);
    assert_eq!(tableau.words_per_row(), words_per_row);
    assert_eq!(tableau.x_plane_words().len(), expected_plane_len(num_qubits));
    assert_eq!(tableau.z_plane_words().len(), expected_plane_len(num_qubits));
    assert_eq!(tableau.sign_words().len(), words_for_bits(num_rows));

    if num_qubits == 0 {
        assert!(tableau.x_plane_words().is_empty());
        assert!(tableau.z_plane_words().is_empty());
        assert!(tableau.sign_words().is_empty());
    }

    for row in 0..num_rows {
        assert!(!tableau.sign_bit(row), "identity sign bit row {row}");
        assert_eq!(tableau.canonical_phase(row), 0, "identity phase row {row}");

        for qubit in 0..num_qubits {
            let expected_x = row < num_qubits && row == qubit;
            let expected_z = row >= num_qubits && row - num_qubits == qubit;
            assert_eq!(tableau.x(row, qubit), expected_x, "x({row}, {qubit})");
            assert_eq!(tableau.z(row, qubit), expected_z, "z({row}, {qubit})");
        }
    }
}

fn row_words(plane: &[u64], row: usize, words_per_row: usize) -> &[u64] {
    let start = row * words_per_row;
    &plane[start..start + words_per_row]
}

fn assert_padding_zero(tableau: &PackedInverseTableau) {
    assert_eq!(tableau.num_qubits(), 130);
    let words_per_row = tableau.words_per_row();
    let valid_mask = (1u64 << (130 % 64)) - 1;
    let padding_mask = !valid_mask;

    for row in 0..tableau.num_rows() {
        let last_word = row * words_per_row + words_per_row - 1;
        assert_eq!(
            tableau.x_plane_words()[last_word] & padding_mask,
            0,
            "x padding row {row}",
        );
        assert_eq!(
            tableau.z_plane_words()[last_word] & padding_mask,
            0,
            "z padding row {row}",
        );
    }
}

#[test]
fn identity_and_lengths_are_exact_for_0_1_64_65_130() {
    for num_qubits in [0, 1, 64, 65, 130] {
        assert_identity(num_qubits);
    }
}

#[test]
fn boundary_bits_63_64_129_map_to_expected_words() {
    let tableau = PackedInverseTableau::identity(130);
    let w = tableau.words_per_row();

    assert_eq!(w, 3);
    assert_eq!(row_words(tableau.x_plane_words(), 63, w), &[1u64 << 63, 0, 0]);
    assert_eq!(row_words(tableau.x_plane_words(), 64, w), &[0, 1, 0]);
    assert_eq!(row_words(tableau.x_plane_words(), 129, w), &[0, 0, 1u64 << 1]);

    assert_eq!(
        row_words(tableau.z_plane_words(), 130 + 63, w),
        &[1u64 << 63, 0, 0],
    );
    assert_eq!(row_words(tableau.z_plane_words(), 130 + 64, w), &[0, 1, 0]);
    assert_eq!(
        row_words(tableau.z_plane_words(), 130 + 129, w),
        &[0, 0, 1u64 << 1],
    );

    assert!(!tableau.x(129, 130 - 2));
    assert!(tableau.x(129, 129));
    assert!(tableau.z(259, 129));

    assert!(catch_unwind(|| tableau.x(0, 130)).is_err());
    assert!(catch_unwind(|| tableau.z(0, 130)).is_err());
    assert!(catch_unwind(|| tableau.sign_bit(260)).is_err());
}

#[test]
fn packed_signs_round_trip_positive_and_negative() {
    let mut tableau = PackedInverseTableau::identity(130);

    assert_eq!(tableau.sign_words().len(), 5);
    assert!(tableau.sign_words().iter().all(|word| *word == 0));

    tableau.set_sign_bit(0, true);
    assert!(tableau.sign_bit(0));
    assert_eq!(tableau.canonical_phase(0), 2);

    tableau.set_canonical_phase(0, 0);
    assert!(!tableau.sign_bit(0));
    assert_eq!(tableau.canonical_phase(0), 0);

    tableau.set_canonical_phase(64, 2);
    tableau.set_sign_bit(129, true);

    assert!(catch_unwind(AssertUnwindSafe(|| tableau.set_sign_bit(260, false))).is_err());

    assert_eq!(tableau.canonical_phase(64), 2);
    assert_eq!(tableau.canonical_phase(129), 2);
    assert_eq!(tableau.sign_words()[1] & 1, 1);
    assert_eq!((tableau.sign_words()[2] >> 1) & 1, 1);
    assert_eq!(tableau.sign_words()[0] & 1, 0);
}

#[test]
fn row_copy_and_plane_xor_obey_contract() {
    let mut tableau = PackedInverseTableau::identity(130);

    tableau.set_canonical_phase(1, 2);
    tableau.copy_row(1, 131);
    assert!(tableau.x(131, 1));
    assert!(!tableau.z(131, 1));
    assert_eq!(tableau.canonical_phase(131), 2);

    tableau.set_canonical_phase(0, 2);
    tableau.set_canonical_phase(64, 0);
    tableau.xor_pauli_planes(0, 64);
    assert!(tableau.x(64, 0));
    assert!(tableau.x(64, 64));
    assert!(!tableau.z(64, 0));
    assert!(!tableau.sign_bit(64));
    assert_eq!(tableau.canonical_phase(64), 0);

    tableau.set_canonical_phase(194, 2);
    tableau.xor_pauli_planes(130, 194);
    assert!(tableau.z(194, 0));
    assert!(tableau.z(194, 64));
    assert!(!tableau.x(194, 0));
    assert_eq!(tableau.canonical_phase(194), 2);

    assert!(catch_unwind(AssertUnwindSafe(|| tableau.copy_row(260, 0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| tableau.xor_pauli_planes(0, 260))).is_err());
}

#[test]
fn unused_padding_bits_stay_zero() {
    let mut tableau = PackedInverseTableau::identity(130);

    assert_padding_zero(&tableau);
    tableau.copy_row(129, 0);
    assert_padding_zero(&tableau);
    tableau.xor_pauli_planes(128, 0);
    assert_padding_zero(&tableau);
    tableau.copy_row(259, 1);
    assert_padding_zero(&tableau);
    tableau.xor_pauli_planes(258, 1);
    assert_padding_zero(&tableau);

    println!("PASS packed inverse-tableau storage");
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_storage -- --nocapture
```

Expected: FAIL because `rstim::sim::packed_inverse_tableau` does not exist yet.

- [ ] **Step 3: Add the packed storage module**

Create `rstim/src/sim/packed_inverse_tableau.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedInverseTableau {
    num_qubits: usize,
    words_per_row: usize,
    x_plane: Vec<u64>,
    z_plane: Vec<u64>,
    signs: Vec<u64>,
}

impl PackedInverseTableau {
    pub fn identity(num_qubits: usize) -> Self {
        let words_per_row = words_for_bits(num_qubits);
        let num_rows = num_qubits
            .checked_mul(2)
            .expect("packed inverse tableau row count overflow");
        let plane_len = num_rows
            .checked_mul(words_per_row)
            .expect("packed inverse tableau plane length overflow");

        let mut tableau = Self {
            num_qubits,
            words_per_row,
            x_plane: vec![0; plane_len],
            z_plane: vec![0; plane_len],
            signs: vec![0; words_for_bits(num_rows)],
        };

        for qubit in 0..num_qubits {
            tableau.set_x_storage_bit(qubit, qubit);
            tableau.set_z_storage_bit(num_qubits + qubit, qubit);
        }

        tableau
    }

    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    pub fn num_rows(&self) -> usize {
        2 * self.num_qubits
    }

    pub fn words_per_row(&self) -> usize {
        self.words_per_row
    }

    pub fn x_plane_words(&self) -> &[u64] {
        &self.x_plane
    }

    pub fn z_plane_words(&self) -> &[u64] {
        &self.z_plane
    }

    pub fn sign_words(&self) -> &[u64] {
        &self.signs
    }

    pub fn x(&self, row: usize, qubit: usize) -> bool {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        bit_is_set(self.x_plane[word], qubit % 64)
    }

    pub fn z(&self, row: usize, qubit: usize) -> bool {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        bit_is_set(self.z_plane[word], qubit % 64)
    }

    pub fn sign_bit(&self, row: usize) -> bool {
        self.check_row(row);
        bit_is_set(self.signs[row / 64], row % 64)
    }

    pub fn canonical_phase(&self, row: usize) -> u8 {
        if self.sign_bit(row) {
            2
        } else {
            0
        }
    }

    pub fn set_sign_bit(&mut self, row: usize, negative: bool) {
        self.check_row(row);
        let word = row / 64;
        let mask = 1u64 << (row % 64);
        if negative {
            self.signs[word] |= mask;
        } else {
            self.signs[word] &= !mask;
        }
    }

    pub fn set_canonical_phase(&mut self, row: usize, phase: u8) {
        match phase {
            0 => self.set_sign_bit(row, false),
            2 => self.set_sign_bit(row, true),
            _ => panic!("canonical phase must be 0 or 2, got {phase}"),
        }
    }

    pub fn copy_row(&mut self, src: usize, dst: usize) {
        self.check_row(src);
        self.check_row(dst);

        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        for offset in 0..self.words_per_row {
            self.x_plane[dst_start + offset] = self.x_plane[src_start + offset];
            self.z_plane[dst_start + offset] = self.z_plane[src_start + offset];
        }

        self.set_sign_bit(dst, self.sign_bit(src));
        self.mask_row_padding(dst);
    }

    /// XORs only the packed X/Z storage planes from `src` into `dst`.
    ///
    /// This is a storage primitive, not phase-aware Pauli multiplication:
    /// the destination sign is intentionally left unchanged.
    pub fn xor_pauli_planes(&mut self, src: usize, dst: usize) {
        self.check_row(src);
        self.check_row(dst);

        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        for offset in 0..self.words_per_row {
            self.x_plane[dst_start + offset] ^= self.x_plane[src_start + offset];
            self.z_plane[dst_start + offset] ^= self.z_plane[src_start + offset];
        }

        self.mask_row_padding(dst);
    }

    fn check_row(&self, row: usize) {
        assert!(
            row < self.num_rows(),
            "row index {row} out of range for {} rows",
            self.num_rows()
        );
    }

    fn check_qubit(&self, qubit: usize) {
        assert!(
            qubit < self.num_qubits,
            "qubit index {qubit} out of range for {} qubits",
            self.num_qubits
        );
    }

    fn row_start(&self, row: usize) -> usize {
        row * self.words_per_row
    }

    fn plane_word_index(&self, row: usize, qubit: usize) -> usize {
        self.row_start(row) + qubit / 64
    }

    fn set_x_storage_bit(&mut self, row: usize, qubit: usize) {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        self.x_plane[word] |= 1u64 << (qubit % 64);
    }

    fn set_z_storage_bit(&mut self, row: usize, qubit: usize) {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        self.z_plane[word] |= 1u64 << (qubit % 64);
    }

    fn mask_row_padding(&mut self, row: usize) {
        if self.words_per_row == 0 {
            return;
        }

        let mask = self.final_word_mask();
        let last_word = self.row_start(row) + self.words_per_row - 1;
        self.x_plane[last_word] &= mask;
        self.z_plane[last_word] &= mask;
    }

    fn final_word_mask(&self) -> u64 {
        let tail_bits = self.num_qubits % 64;
        if tail_bits == 0 {
            u64::MAX
        } else {
            (1u64 << tail_bits) - 1
        }
    }
}

fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(64)
}

fn bit_is_set(word: u64, bit: usize) -> bool {
    ((word >> bit) & 1) == 1
}
```

- [ ] **Step 4: Export the module**

Modify `rstim/src/sim/mod.rs` to include:

```rust
pub mod bit_table;
pub mod frame;
pub mod measure_record_batch;
pub mod packed_inverse_tableau;
pub mod tableau;
```

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p rstim --test packed_inverse_tableau_storage -- --nocapture
```

Expected: PASS with output containing `PASS packed inverse-tableau storage`.

- [ ] **Step 6: Run the full required verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add rstim/src/sim/mod.rs rstim/src/sim/packed_inverse_tableau.rs rstim/tests/packed_inverse_tableau_storage.rs docs/superpowers/plans/2026-07-10-packed-inverse-tableau.md
git commit -m "feat: add packed inverse tableau storage"
```

## Self-Review

- Spec coverage: Task 1 covers the requested module, exact packed X/Z/sign lengths, identity construction, logical accessors, sign/phase accessors, row copy, plane XOR, padding masks, required tests, focused verification, and full `cargo test`.
- Placeholder scan: no TBD/TODO/fill-in steps; commands, files, and code are concrete.
- Type consistency: the test imports and all method names match the produced `PackedInverseTableau` interface.
