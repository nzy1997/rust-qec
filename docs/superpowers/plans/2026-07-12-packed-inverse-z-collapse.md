# Packed Inverse Z Collapse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a direct packed-inverse Z collapse subsystem that returns deterministic signs without canonical rows and uses one transposed working view per random-collapse batch.

**Architecture:** Add a hidden direct `PackedInverseTableau::collapse_z_many_biased` entrypoint. It scans inverse Z rows for deterministic signs, creates a packed transposed working view only for random targets, mutates that view with append-gate formulas, then writes it back once.

**Tech Stack:** Rust 2024, existing `rstim` packed inverse tableau storage, Cargo integration tests.

## Global Constraints

- Do not route production reference sampling to this subsystem yet.
- Do not implement X/Y adapters.
- Do not fold repeats.
- Do not call `canonical_rows` or `replace_from_canonical_rows` from the new direct collapse subsystem.
- Preserve packed phase/sign semantics across 64- and 128-qubit word boundaries.

---

### Task 1: Direct Collapse Integration Tests

**Files:**
- Create: `rstim/tests/packed_inverse_direct_collapse.rs`

**Interfaces:**
- Consumes: existing `PackedInverseTableau::{identity,h,x_gate,z_gate,cx,canonical_snapshot}` and `StabilizerState::{new,h,x_gate,z_gate,cx,measure_z_biased,canonical_snapshot}`.
- Produces: failing tests for `PackedInverseTableau::collapse_z_many_biased(&[(usize, bool)], &mut ReferenceBuildPhaseCounters) -> Vec<bool>`.

- [ ] **Step 1: Write the failing test file**

```rust
use rstim::data_path::ReferenceBuildPhaseCounters;
use rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau};
use rstim::sim::tableau::StabilizerState;

fn direct_collapse(
    tableau: &mut PackedInverseTableau,
    targets: &[(usize, bool)],
) -> (Vec<bool>, ReferenceBuildPhaseCounters) {
    let mut counters = ReferenceBuildPhaseCounters::default();
    let bits = tableau.collapse_z_many_biased(targets, &mut counters);
    (bits, counters)
}

fn legacy_measure_z_many(
    state: &mut StabilizerState,
    targets: &[(usize, bool)],
) -> Vec<bool> {
    targets
        .iter()
        .map(|&(q, inverted)| (state.measure_z_biased(q) == 1) ^ inverted)
        .collect()
}

fn assert_snapshot_matches_legacy(
    packed: &PackedInverseTableau,
    legacy: &StabilizerState,
    label: &str,
) {
    let packed_snapshot: CanonicalTableauSnapshot = packed.canonical_snapshot();
    let legacy_snapshot = legacy.canonical_snapshot();
    assert_eq!(packed_snapshot, legacy_snapshot, "{label}");
}

#[test]
fn deterministic_z_batch_avoids_transpose() {
    let mut tableau = PackedInverseTableau::identity(3);
    tableau.x_gate(1);
    tableau.z_gate(2);

    let (bits, counters) = direct_collapse(&mut tableau, &[(0, false), (1, false), (2, true)]);

    assert_eq!(bits, vec![false, true, true]);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 0);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.collapse_pivots, 0);
}

#[test]
fn random_z_collapse_matches_legacy_tableau() {
    let mut packed = PackedInverseTableau::identity(4);
    let mut legacy = StabilizerState::new(4);
    for q in [0, 2] {
        packed.h(q);
        legacy.h(q);
    }
    packed.cx(0, 1);
    legacy.cx(0, 1);
    packed.z_gate(2);
    legacy.z_gate(2);

    let targets = [(0, false), (1, false), (2, false), (3, true)];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert!(counters.collapse_pivots >= 1);
    assert_snapshot_matches_legacy(&packed, &legacy, "random collapse snapshot");
}

#[test]
fn mixed_z_batch_reuses_one_transposed_view() {
    let mut tableau = PackedInverseTableau::identity(5);
    tableau.x_gate(0);
    tableau.h(1);
    tableau.h(3);

    let (bits, counters) =
        direct_collapse(&mut tableau, &[(0, false), (1, false), (2, false), (3, true)]);

    assert_eq!(bits, vec![true, false, false, true]);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.collapse_pivots, 2);
}

#[test]
fn direct_collapse_preserves_deterministic_one() {
    let mut tableau = PackedInverseTableau::identity(1);
    tableau.x_gate(0);

    let (bits, counters) = direct_collapse(&mut tableau, &[(0, false)]);

    assert_eq!(bits, vec![true]);
    assert_eq!(counters.transposed_collapse_batches, 0);
    assert_eq!(counters.collapse_pivots, 0);
}

#[test]
fn direct_collapse_crosses_64_and_128_qubit_boundaries() {
    let num_qubits = 130;
    let mut packed = PackedInverseTableau::identity(num_qubits);
    let mut legacy = StabilizerState::new(num_qubits);

    for q in [0, 63, 64, 65, 127, 128, 129] {
        packed.h(q);
        legacy.h(q);
    }
    for (control, target) in [(63, 64), (64, 65), (127, 128), (128, 129)] {
        packed.cx(control, target);
        legacy.cx(control, target);
    }
    packed.x_gate(129);
    legacy.x_gate(129);

    let targets = [
        (0, false),
        (63, false),
        (64, true),
        (65, false),
        (127, false),
        (128, true),
        (129, false),
    ];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert!(counters.collapse_pivots >= 3);
    assert_snapshot_matches_legacy(&packed, &legacy, "boundary collapse snapshot");
}
```

- [ ] **Step 2: Run the new focused test and confirm it fails before implementation**

Run: `cargo test -p rstim --test packed_inverse_direct_collapse -- --nocapture`

Expected: compile failure containing `no method named collapse_z_many_biased`.

- [ ] **Step 3: Commit the failing test**

```bash
git add rstim/tests/packed_inverse_direct_collapse.rs
git commit -m "test: specify direct packed inverse z collapse"
```

### Task 2: Packed Transposed Z Collapse Subsystem

**Files:**
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`
- Test: `rstim/tests/packed_inverse_direct_collapse.rs`

**Interfaces:**
- Consumes: `ReferenceBuildPhaseCounters`, `PackedInverseTableau` storage helpers, `bit_from_words`, `set_bit`, `words_for_bits`.
- Produces: `#[doc(hidden)] pub fn collapse_z_many_biased(&mut self, targets: &[(usize, bool)], counters: &mut ReferenceBuildPhaseCounters) -> Vec<bool>`.
- Produces: private `PackedTransposedInverseTableau` with `from_tableau`, `write_back`, `collapse_z`, `append_zcx`, `append_h_xz`, `append_h_yz`, and `append_x`.

- [ ] **Step 1: Add direct entrypoint and deterministic scan helpers**

Add this method inside `impl PackedInverseTableau` near the existing Z measurement methods:

```rust
    #[doc(hidden)]
    pub fn collapse_z_many_biased(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        counters.direct_inverse_batches += 1;

        let mut bits = Vec::with_capacity(targets.len());
        let mut random_targets = Vec::new();
        for (index, &(q, inverted)) in targets.iter().enumerate() {
            self.check_qubit(q);
            let z_row = self.num_qubits + q;
            if self.row_has_x_support(z_row) {
                bits.push(inverted);
                random_targets.push((index, q, inverted));
            } else {
                bits.push(self.sign_bit(z_row) ^ inverted);
            }
        }

        if random_targets.is_empty() {
            return bits;
        }

        counters.transposed_collapse_batches += 1;
        let mut transposed = PackedTransposedInverseTableau::from_tableau(self);
        for (index, q, inverted) in random_targets {
            if transposed.collapse_z(q) {
                counters.collapse_pivots += 1;
            }
            bits[index] = transposed.z_sign(q) ^ inverted;
        }
        transposed.write_back(self);
        bits
    }

    fn row_has_x_support(&self, row: usize) -> bool {
        self.check_row(row);
        let start = self.row_start(row);
        self.x_plane[start..start + self.words_per_row]
            .iter()
            .any(|word| *word != 0)
    }
```

- [ ] **Step 2: Add transposed working view**

Add this private struct before `impl PackedInverseTableau`:

```rust
#[derive(Debug, Clone)]
struct PackedTransposedInverseTableau {
    num_qubits: usize,
    row_words: usize,
    x_columns: Vec<u64>,
    z_columns: Vec<u64>,
    signs: Vec<u64>,
}
```

Add its implementation after `impl PackedCanonicalRows`:

```rust
impl PackedTransposedInverseTableau {
    fn from_tableau(tableau: &PackedInverseTableau) -> Self {
        let num_qubits = tableau.num_qubits;
        let row_words = words_for_bits(tableau.num_rows());
        let mut view = Self {
            num_qubits,
            row_words,
            x_columns: vec![0; num_qubits * row_words],
            z_columns: vec![0; num_qubits * row_words],
            signs: tableau.signs.clone(),
        };

        for row in 0..tableau.num_rows() {
            let row_start = tableau.row_start(row);
            for qubit in 0..num_qubits {
                if bit_is_set(tableau.x_plane[row_start + qubit / 64], qubit % 64) {
                    set_bit(view.x_column_mut(qubit), row);
                }
                if bit_is_set(tableau.z_plane[row_start + qubit / 64], qubit % 64) {
                    set_bit(view.z_column_mut(qubit), row);
                }
            }
        }
        view
    }

    fn write_back(self, tableau: &mut PackedInverseTableau) {
        assert_eq!(self.num_qubits, tableau.num_qubits);
        tableau.x_plane.fill(0);
        tableau.z_plane.fill(0);
        tableau.signs.clone_from_slice(&self.signs);

        for row in 0..tableau.num_rows() {
            let row_start = tableau.row_start(row);
            for qubit in 0..self.num_qubits {
                if bit_from_words(self.x_column(qubit), row) {
                    tableau.x_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
                if bit_from_words(self.z_column(qubit), row) {
                    tableau.z_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
            }
            tableau.mask_row_padding(row);
        }
    }

    fn collapse_z(&mut self, target: usize) -> bool {
        let Some(pivot) = self.find_zx_pivot(target) else {
            return false;
        };

        for qubit in pivot + 1..self.num_qubits {
            if self.z_x(target, qubit) {
                self.append_zcx(pivot, qubit);
            }
        }

        if self.z_z(target, pivot) {
            self.append_h_yz(pivot);
        } else {
            self.append_h_xz(pivot);
        }
        if self.z_sign(target) {
            self.append_x(pivot);
        }
        true
    }

    fn find_zx_pivot(&self, target: usize) -> Option<usize> {
        (0..self.num_qubits).find(|&qubit| self.z_x(target, qubit))
    }

    fn z_x(&self, z_row_qubit: usize, x_qubit: usize) -> bool {
        bit_from_words(self.x_column(x_qubit), self.num_qubits + z_row_qubit)
    }

    fn z_z(&self, z_row_qubit: usize, z_qubit: usize) -> bool {
        bit_from_words(self.z_column(z_qubit), self.num_qubits + z_row_qubit)
    }

    fn z_sign(&self, target: usize) -> bool {
        bit_from_words(&self.signs, self.num_qubits + target)
    }

    fn append_zcx(&mut self, control: usize, target: usize) {
        for word in 0..self.row_words {
            let cx = self.x_column(control)[word];
            let cz = self.z_column(control)[word];
            let tx = self.x_column(target)[word];
            let tz = self.z_column(target)[word];
            self.signs[word] ^= (cx & tz) & !(cz ^ tx);
            self.z_column_mut(control)[word] ^= tz;
            self.x_column_mut(target)[word] ^= cx;
        }
        self.mask_padding();
    }

    fn append_h_xz(&mut self, q: usize) {
        for word in 0..self.row_words {
            let x = self.x_column(q)[word];
            let z = self.z_column(q)[word];
            self.signs[word] ^= x & z;
            self.x_column_mut(q)[word] = z;
            self.z_column_mut(q)[word] = x;
        }
        self.mask_padding();
    }

    fn append_h_yz(&mut self, q: usize) {
        for word in 0..self.row_words {
            let x = self.x_column(q)[word];
            let z = self.z_column(q)[word];
            self.signs[word] ^= x & !z;
            self.x_column_mut(q)[word] = x ^ z;
        }
        self.mask_padding();
    }

    fn append_x(&mut self, q: usize) {
        for word in 0..self.row_words {
            self.signs[word] ^= self.z_column(q)[word];
        }
        self.mask_padding();
    }

    fn x_column(&self, qubit: usize) -> &[u64] {
        let start = qubit * self.row_words;
        &self.x_columns[start..start + self.row_words]
    }

    fn x_column_mut(&mut self, qubit: usize) -> &mut [u64] {
        let start = qubit * self.row_words;
        &mut self.x_columns[start..start + self.row_words]
    }

    fn z_column(&self, qubit: usize) -> &[u64] {
        let start = qubit * self.row_words;
        &self.z_columns[start..start + self.row_words]
    }

    fn z_column_mut(&mut self, qubit: usize) -> &mut [u64] {
        let start = qubit * self.row_words;
        &mut self.z_columns[start..start + self.row_words]
    }

    fn mask_padding(&mut self) {
        let valid_rows = 2 * self.num_qubits;
        let tail_bits = valid_rows % 64;
        if tail_bits != 0 {
            let mask = (1u64 << tail_bits) - 1;
            let last = self.row_words - 1;
            self.signs[last] &= mask;
            for qubit in 0..self.num_qubits {
                self.x_column_mut(qubit)[last] &= mask;
                self.z_column_mut(qubit)[last] &= mask;
            }
        }
    }
}
```

- [ ] **Step 3: Run focused test and fix borrow-checker or phase mistakes**

Run: `cargo test -p rstim --test packed_inverse_direct_collapse -- --nocapture`

Expected: all five tests pass and print the standard Cargo test success summary.

- [ ] **Step 4: Commit implementation**

```bash
git add rstim/src/sim/packed_inverse_tableau.rs
git commit -m "feat: add direct packed inverse z collapse"
```

### Task 3: Verification and Branch Finish

**Files:**
- Modify only if verification exposes a concrete defect: `rstim/src/sim/packed_inverse_tableau.rs`
- Modify only if assertions need a concrete correction: `rstim/tests/packed_inverse_direct_collapse.rs`

**Interfaces:**
- Consumes: passing focused direct collapse tests.
- Produces: branch with focused and broad verification evidence, ready for PR.

- [ ] **Step 1: Run required focused verification**

Run: `cargo test -p rstim --test packed_inverse_direct_collapse -- --nocapture`

Expected: PASS for:

```text
deterministic_z_batch_avoids_transpose
random_z_collapse_matches_legacy_tableau
mixed_z_batch_reuses_one_transposed_view
direct_collapse_preserves_deterministic_one
direct_collapse_crosses_64_and_128_qubit_boundaries
```

- [ ] **Step 2: Run required broad verification**

Run: `cargo test`

Expected: all workspace tests pass.

- [ ] **Step 3: Inspect final diff and status**

Run: `git status --short`

Expected: clean working tree after final commit.

Run: `git log --oneline --decorate -n 5`

Expected: recent commits include the design, test, and implementation commits on `agent/issue-486-collapse-z-measurements-directly-in-the-packed-i-run-1`.

- [ ] **Step 4: Push and create PR**

```bash
git push -u origin agent/issue-486-collapse-z-measurements-directly-in-the-packed-i-run-1
gh pr create --repo nzy1997/rstim --base master --head agent/issue-486-collapse-z-measurements-directly-in-the-packed-i-run-1 --title "Collapse Z measurements directly in packed inverse tableau" --body "## Summary
- add direct packed-inverse Z collapse with deterministic sign scanning
- use one transposed working view per random-collapse batch
- cover deterministic signs, mixed batches, and word-boundary pivots

## Tests
- cargo test -p rstim --test packed_inverse_direct_collapse -- --nocapture
- cargo test

Closes #486"
```
