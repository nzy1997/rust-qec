# Phase 4: Frame Simulator — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Pauli frame simulator that processes many shots in parallel using bit-packed `u64` operations, enabling fast batch sampling with detection event and observable flip output.

**Architecture:** The frame simulator tracks X and Z Pauli error frames per qubit across N shots simultaneously, packed into `u64` words (64 shots per word). A one-time noiseless "reference sample" from the existing tableau simulator provides baseline measurement outcomes. Gates update frames via XOR/swap on bit-rows. Measurements record the relevant frame bit (Z measurement reads X frame), XOR'd with the reference to produce actual outcomes. The public API is `sample_batch(circuit, n_shots) -> BatchOutput` containing measurements, detection events, and observable flips.

**Tech Stack:** Rust, `rand` crate, `cargo test`

---

### Task 1: BitTable Data Structure

**Files:**
- Create: `src/sim/bit_table.rs`
- Modify: `src/sim/mod.rs` (add `pub mod bit_table;`)
- Create: `tests/bit_table.rs`

**Context:**

`BitTable` is a 2D bit array with row-major layout, packed into `u64` words. Row = major axis (e.g. qubit index), column = minor axis (e.g. shot index). Each row is `ceil(num_minor / 64)` words. This is the core data structure for the frame simulator.

**Step 1: Write failing tests**

In `tests/bit_table.rs`:

```rust
use rstim::sim::bit_table::BitTable;

#[test]
fn new_table_is_all_zeros() {
    let t = BitTable::new(4, 128);
    for r in 0..4 {
        for c in 0..128 {
            assert_eq!(t.get(r, c), false);
        }
    }
}

#[test]
fn set_and_get() {
    let mut t = BitTable::new(3, 200);
    t.set(1, 77, true);
    assert_eq!(t.get(1, 77), true);
    assert_eq!(t.get(1, 78), false);
    assert_eq!(t.get(0, 77), false);
    t.set(1, 77, false);
    assert_eq!(t.get(1, 77), false);
}

#[test]
fn xor_row() {
    let mut t = BitTable::new(3, 128);
    t.set(0, 10, true);
    t.set(0, 50, true);
    t.set(1, 50, true);
    t.set(1, 90, true);
    t.xor_row(0, 1); // row[0] ^= row[1]
    assert_eq!(t.get(0, 10), true);  // unchanged
    assert_eq!(t.get(0, 50), false); // 1 ^ 1 = 0
    assert_eq!(t.get(0, 90), true);  // 0 ^ 1 = 1
}

#[test]
fn swap_rows() {
    let mut t = BitTable::new(2, 64);
    t.set(0, 0, true);
    t.set(1, 63, true);
    t.swap_rows(0, 1);
    assert_eq!(t.get(0, 0), false);
    assert_eq!(t.get(0, 63), true);
    assert_eq!(t.get(1, 0), true);
    assert_eq!(t.get(1, 63), false);
}

#[test]
fn clear_row() {
    let mut t = BitTable::new(2, 128);
    t.set(0, 5, true);
    t.set(0, 100, true);
    t.clear_row(0);
    assert_eq!(t.get(0, 5), false);
    assert_eq!(t.get(0, 100), false);
}

#[test]
fn num_minor_and_major() {
    let t = BitTable::new(5, 130);
    assert_eq!(t.num_major(), 5);
    assert_eq!(t.num_minor(), 130);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test bit_table 2>&1 | tail -20`

**Step 3: Implement**

In `src/sim/bit_table.rs`:

```rust
use rand::Rng;

#[derive(Debug, Clone)]
pub struct BitTable {
    num_major: usize,
    num_minor: usize,
    words_per_row: usize,
    data: Vec<u64>,
}

impl BitTable {
    pub fn new(num_major: usize, num_minor: usize) -> Self {
        let words_per_row = (num_minor + 63) / 64;
        Self {
            num_major,
            num_minor,
            words_per_row,
            data: vec![0u64; num_major * words_per_row],
        }
    }

    pub fn num_major(&self) -> usize { self.num_major }
    pub fn num_minor(&self) -> usize { self.num_minor }

    pub fn get(&self, major: usize, minor: usize) -> bool {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        (self.data[word_idx] >> bit_idx) & 1 == 1
    }

    pub fn set(&mut self, major: usize, minor: usize, val: bool) {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        if val {
            self.data[word_idx] |= 1u64 << bit_idx;
        } else {
            self.data[word_idx] &= !(1u64 << bit_idx);
        }
    }

    pub fn toggle(&mut self, major: usize, minor: usize) {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        self.data[word_idx] ^= 1u64 << bit_idx;
    }

    pub fn xor_row(&mut self, dst: usize, src: usize) {
        let dst_start = dst * self.words_per_row;
        let src_start = src * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[dst_start + w] ^= self.data[src_start + w];
        }
    }

    pub fn swap_rows(&mut self, a: usize, b: usize) {
        let a_start = a * self.words_per_row;
        let b_start = b * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data.swap(a_start + w, b_start + w);
        }
    }

    pub fn clear_row(&mut self, row: usize) {
        let start = row * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[start + w] = 0;
        }
    }

    pub fn randomize_row(&mut self, row: usize, rng: &mut impl Rng) {
        let start = row * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[start + w] = rng.gen();
        }
    }

    pub fn row_words(&self, row: usize) -> &[u64] {
        let start = row * self.words_per_row;
        &self.data[start..start + self.words_per_row]
    }

    pub fn row_words_mut(&mut self, row: usize) -> &mut [u64] {
        let start = row * self.words_per_row;
        &mut self.data[start..start + self.words_per_row]
    }
}
```

In `src/sim/mod.rs`, add:
```rust
pub mod bit_table;
```

**Step 4: Run tests**

Run: `cargo test --test bit_table 2>&1 | tail -20`
Expected: all pass

Run: `cargo test 2>&1 | tail -5`
Expected: all pass

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: BitTable data structure for frame simulator"
```

---

### Task 2: MeasureRecordBatch + Reference Sample

**Files:**
- Create: `src/sim/measure_record_batch.rs`
- Modify: `src/sim/mod.rs`
- Modify: `src/executor.rs` (add `reference_sample` function)
- Create: `tests/frame_sim.rs`

**Context:**

`MeasureRecordBatch` is a batched version of `Recorder`. Each measurement is a row of bits across all shots. It supports lookback by record offset (negative indexing from the end, like `rec[-1]`). It uses `BitTable` for storage plus a circular index.

The reference sample is the noiseless measurement outcome vector, computed by running the circuit through the existing tableau executor with all noise removed and random measurements biased to 0. We'll add a `reference_sample` function to the executor module.

**Step 1: Write failing tests**

In `tests/frame_sim.rs`:

```rust
use rstim::sim::measure_record_batch::MeasureRecordBatch;
use rstim::sim::bit_table::BitTable;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;

#[test]
fn measure_record_batch_push_and_lookback() {
    let mut mrb = MeasureRecordBatch::new(64);
    // Push a row where shot 0 = true, shot 1 = false
    let mut row = BitTable::new(1, 64);
    row.set(0, 0, true);
    mrb.push_row(row.row_words(0));
    assert_eq!(mrb.lookback(1, 0), true);   // rec[-1] for shot 0
    assert_eq!(mrb.lookback(1, 1), false);   // rec[-1] for shot 1
}

#[test]
fn measure_record_batch_multiple_rows() {
    let mut mrb = MeasureRecordBatch::new(64);
    // Push two rows
    let mut r1 = BitTable::new(1, 64);
    r1.set(0, 0, true);
    mrb.push_row(r1.row_words(0));

    let mut r2 = BitTable::new(1, 64);
    r2.set(0, 1, true);
    mrb.push_row(r2.row_words(0));

    // rec[-1] is the most recent (r2), rec[-2] is r1
    assert_eq!(mrb.lookback(1, 0), false);  // r2, shot 0
    assert_eq!(mrb.lookback(1, 1), true);   // r2, shot 1
    assert_eq!(mrb.lookback(2, 0), true);   // r1, shot 0
    assert_eq!(mrb.lookback(2, 1), false);  // r1, shot 1
}

#[test]
fn reference_sample_simple() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    // Noiseless: H then CNOT creates Bell state, measurements are correlated
    // With bias toward 0: both should be 0
    assert_eq!(ref_sample.len(), 2);
    assert_eq!(ref_sample[0], ref_sample[1]); // correlated
}

#[test]
fn reference_sample_deterministic() {
    // |0⟩ → X → M gives deterministic 1
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
}

#[test]
fn reference_sample_no_noise() {
    // X_ERROR(1) should be ignored in reference sample
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]); // noise skipped
}
```

**Step 2: Run tests to verify they fail**

**Step 3: Implement MeasureRecordBatch**

In `src/sim/measure_record_batch.rs`:

```rust
#[derive(Debug, Clone)]
pub struct MeasureRecordBatch {
    batch_size: usize,
    words_per_row: usize,
    records: Vec<Vec<u64>>,  // each entry is one measurement row
}

impl MeasureRecordBatch {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            words_per_row: (batch_size + 63) / 64,
            records: Vec::new(),
        }
    }

    pub fn push_row(&mut self, words: &[u64]) {
        let mut row = vec![0u64; self.words_per_row];
        let copy_len = row.len().min(words.len());
        row[..copy_len].copy_from_slice(&words[..copy_len]);
        self.records.push(row);
    }

    pub fn push_zeros(&mut self) {
        self.records.push(vec![0u64; self.words_per_row]);
    }

    /// lookback(k, shot): get bit for rec[-(k as i32)] for the given shot
    pub fn lookback(&self, k: usize, shot: usize) -> bool {
        let idx = self.records.len() - k;
        let word = shot / 64;
        let bit = shot % 64;
        (self.records[idx][word] >> bit) & 1 == 1
    }

    /// Get a mutable reference to the last pushed row's words
    pub fn last_row_mut(&mut self) -> &mut [u64] {
        self.records.last_mut().unwrap()
    }

    /// XOR lookback row into dest words
    pub fn xor_lookback_into(&self, k: usize, dest: &mut [u64]) {
        let idx = self.records.len() - k;
        for (d, s) in dest.iter_mut().zip(self.records[idx].iter()) {
            *d ^= *s;
        }
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn batch_size(&self) -> usize { self.batch_size }
    pub fn words_per_row(&self) -> usize { self.words_per_row }
}
```

In `src/sim/mod.rs`:
```rust
pub mod bit_table;
pub mod measure_record_batch;
```

**Step 4: Implement reference_sample**

In `src/executor.rs`, add a public function. The reference sample runs the circuit with:
- All noise operations skipped (DEPOLARIZE1, X_ERROR, etc. become no-ops)
- Random measurements biased to return 0

```rust
pub fn reference_sample(instrs: &[StimInstr]) -> Result<Vec<bool>, String> {
    reference_sample_inner(instrs)
}

fn reference_sample_inner(instrs: &[StimInstr]) -> Result<Vec<bool>, String> {
    let n = max_qubit(instrs)?;
    let mut state = StabilizerState::new(n);
    let mut measurements = Vec::new();
    // Use a dummy RNG that always returns false for booleans (bias toward 0)
    let mut rng = BiasedRng;
    ref_sample_process(&mut state, &mut measurements, instrs, &mut rng)?;
    Ok(measurements)
}
```

The key is a `BiasedRng` that forces `gen::<bool>()` to return `false` (measurement outcome = 0 for random measurements). The simplest approach: use a real RNG seeded to 0, but override the measurement path to always return 0. Since we can't easily override `gen::<bool>()`, instead create a specialized version of the execution loop that:
1. Skips all noise instructions
2. For measurements, uses a modified `measure_z` that forces random outcomes to 0

Actually, simpler: add a `measure_z_biased` method to `StabilizerState` that's identical to `measure_z` but returns 0 for random outcomes instead of sampling. Then create a stripped-down execution loop.

Add to `src/sim/tableau.rs`:

```rust
pub fn measure_z_biased(&mut self, q: usize) -> u8 {
    // Same as measure_z but always returns 0 for random outcomes
    // (equivalent to sign_bias = +1 in Stim)
    let mut p = None;
    for i in self.n..2 * self.n {
        if self.x[i][q] { p = Some(i); break; }
    }
    if let Some(p) = p {
        let r: u8 = 0; // Always 0
        for i in 0..2 * self.n {
            if i != p && self.x[i][q] { self.row_mult(i, p); }
        }
        let d = p - self.n;
        self.copy_row(p, d);
        self.x[p].fill(false);
        self.z[p].fill(false);
        self.z[p][q] = true;
        self.phase[p] = 0;
        return r;
    }
    // Deterministic
    let mut temp_x = vec![false; self.n];
    let mut temp_z = vec![false; self.n];
    temp_z[q] = true;
    let mut temp_phase: u8 = 0;
    for i in 0..self.n {
        if self.x[i][q] {
            self.row_mult_temp(&mut temp_x, &mut temp_z, &mut temp_phase, i + self.n);
        }
    }
    if temp_phase % 4 == 2 { 1 } else { 0 }
}
```

Then in `src/executor.rs`, implement `reference_sample` by walking the instruction list, skipping noise, and using `measure_z_biased` for measurements. The function should handle H/S/CX/etc. gates, measurements (M/MX/MY/MR/MRX/MRY), resets, MPP, MPAD, DETECTOR, OBSERVABLE_INCLUDE, REPEAT — but skip all noise instructions. Write a dedicated inner loop (don't reuse `Executor::run`; it's simpler to write a clean loop that only handles what's needed).

The noise instructions to skip: `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, `DEPOLARIZE2`, `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`, `CORRELATED_ERROR`, `E`, `ELSE_CORRELATED_ERROR`, `I_ERROR`, `II_ERROR`.

For `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1`: in the reference sample, these push `false` (no herald) per target since no noise occurs.

For `MPAD`: push the deterministic pad bits (same logic as executor: bit = q != 0, no noise).

**Step 5: Run tests, commit**

Run: `cargo test --test frame_sim && cargo test`
Commit: `git add -A && git commit -m "feat: MeasureRecordBatch + reference_sample"`

---

### Task 3: FrameSimulator Core + Clifford Gates

**Files:**
- Create: `src/sim/frame.rs`
- Modify: `src/sim/mod.rs`
- Modify: `tests/frame_sim.rs`

**Context:**

The `FrameSimulator` processes a circuit for `batch_size` shots simultaneously. For each qubit, it maintains an X-frame row and a Z-frame row (each `batch_size` bits wide). Gates update frames via bitwise XOR/swap.

Frame update rules (from Stim):
- `H`: swap x_table[q] and z_table[q]
- `S`: z_table[q] ^= x_table[q]
- `S_DAG`: same as S (in the Pauli frame picture, S and S_DAG have identical frame propagation)
- `SQRT_X`: x_table[q] ^= z_table[q]  (H;S;H in frame picture: swap, z^=x, swap → x^=z)
- `SQRT_X_DAG`: same as SQRT_X
- `SQRT_Y`: swap + z_table[q] ^= x_table[q] (combined H_YZ effect)
- `SQRT_Y_DAG`: same as SQRT_Y
- `X`: z_table[q] is unaffected (X commutes with X, anticommutes with Z only in phase)
  Actually in frame picture: X gate doesn't change the frame at all (it's a Pauli itself)
- `Y`: no frame change
- `Z`: no frame change
- `CX(c,t)`: x_table[t] ^= x_table[c]; z_table[c] ^= z_table[t]
- `CZ(a,b)`: z_table[a] ^= x_table[b]; z_table[b] ^= x_table[a]
  (But must use temp since both read x! Copy x_table[a] first.)
- `SWAP(a,b)`: swap x_table[a]/x_table[b] and z_table[a]/z_table[b]
- `ISWAP(a,b)`: SWAP then S(a) S(b) CZ(a,b) in frame terms:
  swap x and z rows; then z_a ^= x_a; z_b ^= x_b; z_a ^= x_b; z_b ^= x_a
  (Simpler: swap x rows, swap z rows, then x_table[a] ^= x_table[b]; x_table[b] ^= x_table[a] ... actually just use the Stim definitions)

For the frame update rules, the key insight: **Pauli gates (X, Y, Z) don't change the frame**. Only Clifford gates that aren't Paulis change the frame. The frame tracks how Pauli errors propagate; applying a Pauli gate doesn't change error propagation.

**Measurements in frame sim:**
- `M` / `MZ` on qubit q: The measurement result (relative to reference) = x_table[q]. Push x_table[q] into m_record. Then clear x_table[q] and randomize z_table[q].
- `MX` on qubit q: Result = z_table[q]. Push z_table[q] into m_record. Clear z_table[q], randomize x_table[q].
- `MY` on qubit q: Result = x_table[q] XOR z_table[q]. Push that. Clear both, randomize both.
- `MR` / `MRZ`: Same as M but also clear z_table[q] (reset clears all frame).
- `R` / `RZ`: Clear x_table[q], randomize z_table[q].
- `RX`: Clear z_table[q], randomize x_table[q].
- `RY`: Clear x and z, randomize both. (Actually: clear both, randomize both.)

Wait, let me be more precise about measurement + reset:
- `MR`: measure Z (push x_frame), then reset. After reset: x cleared, z randomized.
- `MRX`: measure X (push z_frame), then reset to |+>. After reset: z cleared, x randomized.
- `MRY`: measure Y (push x^z frame), then reset to |+i>. After: clear both, randomize both.

For inversions (`!0`): XOR the inversion into the reference sample, not the frame. Actually in Stim, inversions in measurements affect the reference sample. For the frame sim, inversions are handled by flipping the corresponding bit in the reference sample. Since we compute the reference sample separately, inversions are already accounted for there. The frame sim doesn't need to handle inversions specially.

**Step 1: Write failing tests**

Append to `tests/frame_sim.rs`:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::sim::frame::FrameSimulator;
use rstim::ir::StimInstr;

#[test]
fn frame_sim_no_noise_matches_reference() {
    // X 0; M 0 → deterministic 1
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let measurements = frame.measurements(&ref_sample);
    // All 64 shots should give 1 (true) since no noise
    for shot in 0..64 {
        assert_eq!(measurements.get(0, shot), true, "shot {shot}");
    }
}

#[test]
fn frame_sim_h_cnot_bell() {
    // H 0; CNOT 0 1; M 0 1 → correlated random
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 256);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    // All shots: m0 == m1 (correlated)
    for shot in 0..256 {
        assert_eq!(m.get(0, shot), m.get(1, shot), "shot {shot}");
    }
}

#[test]
fn frame_sim_identity_no_noise() {
    // M 0 on |0⟩ → all false
    let instrs = parse_lines("M 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 128);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..128 {
        assert_eq!(m.get(0, shot), false);
    }
}
```

**Step 2: Implement FrameSimulator**

In `src/sim/frame.rs`:

```rust
use rand::Rng;
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::sim::bit_table::BitTable;
use crate::sim::measure_record_batch::MeasureRecordBatch;

pub struct FrameSimulator {
    pub num_qubits: usize,
    pub batch_size: usize,
    pub x_table: BitTable,
    pub z_table: BitTable,
    pub m_record: MeasureRecordBatch,
}

impl FrameSimulator {
    pub fn new(num_qubits: usize, batch_size: usize) -> Self {
        Self {
            num_qubits,
            batch_size,
            x_table: BitTable::new(num_qubits, batch_size),
            z_table: BitTable::new(num_qubits, batch_size),
            m_record: MeasureRecordBatch::new(batch_size),
        }
    }
}
```

Implement a `run` method that processes instructions. For each gate, apply the frame update rule. For measurements, push the appropriate frame row into m_record and clear/randomize.

Implement a `measurements` method that returns a `BitTable` where `result[m][shot] = m_record[m][shot] XOR ref_sample[m]`.

Gate implementations operate on `BitTable` rows:
- `h(q)`: `x_table.swap_rows(q, ...)` — actually the x and z are separate tables. Use:
  ```rust
  fn h(&mut self, q: usize) {
      // swap x_table row q with z_table row q
      let wpr = self.x_table.words_per_row();  // need to expose this
      // swap word-by-word between x_table and z_table for row q
  }
  ```
  Since x_table and z_table are separate `BitTable`s, swapping rows between them requires swapping word-by-word. Add a `swap_rows_between(table_a, row_a, table_b, row_b)` helper or a method that takes two mutable row slices.

  Simplest: add `BitTable::swap_row_with(&mut self, row: usize, other: &mut BitTable, other_row: usize)` that swaps word-by-word. Or just use raw word access:
  ```rust
  fn h(&mut self, q: usize) {
      let x_words = self.x_table.row_words_mut(q);
      let z_words = self.z_table.row_words_mut(q);
      // Can't do this because both borrow self mutably!
  }
  ```
  
  To handle this, store x_table and z_table data in a single combined table, or use indices. Simplest fix: extract both row slices using raw pointer tricks, or restructure so that x and z data are in one Vec with known offsets.

  **Better approach**: Store frame data as a single `Vec<u64>` with x and z interleaved, or use a combined struct. But simplest for correctness: implement a `swap_row_pair` free function that takes `&mut BitTable, &mut BitTable, usize`.

  Actually in Rust, the cleanest approach: since `x_table` and `z_table` are separate fields of `FrameSimulator`, we can borrow them separately:
  ```rust
  fn h(&mut self, q: usize) {
      let wpr = (self.batch_size + 63) / 64;
      for w in 0..wpr {
          let x = self.x_table.row_words_mut(q);
          let z = self.z_table.row_words_mut(q);
          // This won't work due to borrow rules on self
      }
  }
  ```

  **Solution**: Don't use `&mut self` methods for gate operations. Instead, implement gates as functions that take `&mut BitTable, &mut BitTable`:
  ```rust
  pub fn do_h(x_table: &mut BitTable, z_table: &mut BitTable, q: usize) {
      let wpr = x_table.words_per_row();
      let x = x_table.row_words_mut(q);
      let z = z_table.row_words_mut(q);
      for w in 0..wpr {
          std::mem::swap(&mut x[w], &mut z[w]);
      }
  }
  ```

  Actually that still won't work because we need `words_per_row()` but also `row_words_mut()`. Let's just add a `words_per_row(&self) -> usize` method and use it before the mutable borrow. Or better: the `row_words_mut` already returns a slice, so:
  ```rust
  pub fn swap_row_between(a: &mut BitTable, b: &mut BitTable, row: usize) {
      let wa = a.row_words_mut(row);
      let wb = b.row_words_mut(row);
      for i in 0..wa.len() {
          std::mem::swap(&mut wa[i], &mut wb[i]);
      }
  }
  ```
  This works because `a` and `b` are separate mutable references.

Implement all Clifford gates, measurements, and resets as described above. Use `run` to dispatch instructions (similar to `Executor::run` but with frame operations).

**Step 3: Run tests, commit**

Run: `cargo test --test frame_sim && cargo test`
Commit: `git add -A && git commit -m "feat: FrameSimulator core with Clifford gates and measurements"`

---

### Task 4: Frame Simulator Noise + Advanced Operations

**Files:**
- Modify: `src/sim/frame.rs`
- Modify: `tests/frame_sim.rs`

**Context:**

Noise in the frame simulator flips bits in x_table/z_table probabilistically:
- `X_ERROR(p)`: for each qubit, for each shot, flip x_table[q][shot] with prob p
- `Z_ERROR(p)`: flip z_table[q][shot]
- `Y_ERROR(p)`: flip both x and z
- `DEPOLARIZE1(p)`: with prob p, apply random X/Y/Z (flip x, z, or both)
- `DEPOLARIZE2(p)`: with prob p, apply random 2-qubit Pauli pair
- `CORRELATED_ERROR(p) X0 Y1 Z2`: with prob p (per shot), flip relevant frame bits for all specified qubits
- `ELSE_CORRELATED_ERROR`: conditional on per-shot flag
- `PAULI_CHANNEL_1(px,py,pz)`: sample per qubit per shot
- `PAULI_CHANNEL_2(15 probs)`: sample per pair per shot
- `HERALDED_ERASE(p)`: push herald bit row, then apply random Pauli on erased qubits
- `HERALDED_PAULI_CHANNEL_1(pi,px,py,pz)`: push herald bit row, apply Pauli
- `I_ERROR`, `II_ERROR`: no-ops
- `MPP`: for each Pauli product, XOR appropriate frame rows into measurement result.
  For a Z term: XOR x_table[q]; for an X term: XOR z_table[q]; for a Y term: XOR both.
- `SPP` / `SPP_DAG`: apply Pauli product phase gate in frame picture. SPP Z0*Z1 means: wherever x_table[0] XOR x_table[1] is 1, flip z for both qubits. More precisely, SPP for a Pauli product P applies S to the eigenspace. In frame picture this is complex; simplest correct implementation: decompose into basis change + CX fold + S + uncompute (same as tableau, but using frame operations).
- `MXX`/`MYY`/`MZZ`: desugar to MPP.
- `MPAD`: push deterministic bits (same as reference sample handling).

For noise, the per-shot sampling is done by generating random bits:
- For `X_ERROR(p)` on qubit q: generate a random bitvector where each bit is 1 with prob p, then XOR it into x_table[q].
- The bitvector generation: for each u64 word, generate bits with probability p. Simple approach: for each of 64 bits, sample `rng.gen::<f64>() < p`. Faster: use `rng.gen::<u64>()` and compute a threshold.

For `CORRELATED_ERROR`, the same per-shot random bit pattern applies to ALL targets simultaneously (correlated). So generate one random bitvector and XOR it into each target's appropriate frame table.

**Step 1: Write failing tests**

Append to `tests/frame_sim.rs`:

```rust
#[test]
fn frame_sim_x_error_flips() {
    // X_ERROR(1) 0; M 0 → all shots measure 1
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]); // noiseless = 0
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), true); // all flipped
    }
}

#[test]
fn frame_sim_depolarize1_statistical() {
    // DEPOLARIZE1(0.75) 0; M 0 → ~50% of shots flipped (X or Y flip measurement)
    // X flips M, Y flips M, Z doesn't flip M → 2/3 of 75% = 50%
    let instrs = parse_lines("DEPOLARIZE1(0.75) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let n = 10000;
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, n);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    let count: usize = (0..n).filter(|&s| m.get(0, s)).count();
    // Expected: 50% (0.75 * 2/3 = 0.5)
    assert!((count as f64 / n as f64 - 0.5).abs() < 0.05, "count={count}");
}

#[test]
fn frame_sim_mpp_bell() {
    // H 0; CNOT 0 1; MPP Z0*Z1 → deterministic 0
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Z0*Z1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 128);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..128 {
        assert_eq!(m.get(0, shot), ref_sample[0], "shot {shot}");
    }
}

#[test]
fn frame_sim_correlated_error() {
    let instrs = parse_lines("CORRELATED_ERROR(1) X0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), true);
    }
}
```

**Step 2: Implement noise operations and MPP/SPP**

Add noise handling to `FrameSimulator::run`. For each noise channel, generate per-shot random bits and XOR into frame tables.

Helper for generating biased random bits:
```rust
fn random_bits_with_prob(words: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    for w in &mut result {
        for bit in 0..64u32 {
            if rng.gen::<f64>() < p {
                *w |= 1u64 << bit;
            }
        }
    }
    result
}
```

For MPP: iterate Pauli products, for each product XOR the appropriate frame rows into a result row, push to m_record.

For SPP/SPP_DAG: use the same decomposition as the tableau (basis change → CX fold → S → uncompute) but with frame operations.

**Step 3: Run tests, commit**

Commit: `git add -A && git commit -m "feat: frame simulator noise channels and MPP/SPP"`

---

### Task 5: Batch Sample API + Detection Events

**Files:**
- Create: `src/sampler.rs`
- Modify: `src/lib.rs`
- Modify: `tests/frame_sim.rs`

**Context:**

The public API ties everything together:

```rust
pub struct BatchOutput {
    pub measurements: BitTable,  // [measurement_idx][shot]
    pub detections: BitTable,    // [detector_idx][shot]
    pub observable_flips: BitTable, // [observable_idx][shot]
}

pub fn sample_batch(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
) -> Result<BatchOutput, String>
```

Detection events: for each `DETECTOR rec[-a] rec[-b] ...`, XOR the measurement record lookback rows. Observable flips: for each `OBSERVABLE_INCLUDE(idx) rec[-a] ...`, XOR lookback rows into observable row.

The frame simulator's `run` method should handle DETECTOR and OBSERVABLE_INCLUDE by maintaining detection and observable BitTables.

Processing loop:
1. Compute reference sample
2. Count num_qubits, create FrameSimulator
3. Process all instructions through frame simulator
4. Extract measurements (XOR m_record with reference sample)
5. Extract detections and observable flips
6. Return BatchOutput

For REPEAT blocks: iterate the body `count` times through the frame simulator (same state, just repeat the instructions).

**Step 1: Write failing tests**

```rust
use rstim::sampler::{sample_batch, BatchOutput};

#[test]
fn sample_batch_deterministic() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 100, &mut rng).unwrap();
    for shot in 0..100 {
        assert_eq!(out.measurements.get(0, shot), true);
    }
}

#[test]
fn sample_batch_detector() {
    // M 0; M 0; DETECTOR rec[-1] rec[-2] → detection = M1 XOR M0
    // Without noise, both measurements of |0⟩ give 0, so detector = 0
    let instrs = parse_lines("M 0\nR 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.detections.get(0, shot), false);
    }
}

#[test]
fn sample_batch_detector_with_noise() {
    // X_ERROR(1) between two measurements flips the second
    let instrs = parse_lines("M 0\nR 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.detections.get(0, shot), true); // noise detected
    }
}

#[test]
fn sample_batch_observable() {
    let instrs = parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.observable_flips.get(0, shot), true);
    }
}

#[test]
fn sample_batch_matches_tableau() {
    // Run same noiseless circuit through both tableau and frame, verify same results
    let program = "H 0\nCNOT 0 1\nM 0 1\n";
    let instrs = parse_lines(program).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 1000, &mut rng).unwrap();
    // Both measurements should be correlated in every shot
    for shot in 0..1000 {
        assert_eq!(out.measurements.get(0, shot), out.measurements.get(1, shot));
    }
}

#[test]
fn sample_batch_repeat() {
    let instrs = parse_lines("REPEAT 3 {\nX 0\nM 0\nR 0\n}\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    // 3 measurements, all true (X then M gives 1, R resets)
    for shot in 0..64 {
        for m in 0..3 {
            assert_eq!(out.measurements.get(m, shot), true);
        }
    }
}
```

**Step 2: Implement**

Create `src/sampler.rs` with the `sample_batch` function and `BatchOutput` struct. Wire together reference sample computation, frame simulator creation, and result extraction.

Handle DETECTOR in the frame simulator: maintain a detection event list. For each DETECTOR instruction, create a result row by XOR-ing measurement record lookback rows, push to detection list.

Handle OBSERVABLE_INCLUDE: maintain an observable flip table (indexed by observable ID). For each OBSERVABLE_INCLUDE, XOR measurement lookback rows into the observable row.

Handle REPEAT: iterate body instructions through the same frame simulator state.

Add `pub mod sampler;` to `src/lib.rs`.

**Step 3: Run tests, commit**

Run: `cargo test --test frame_sim && cargo test`
Commit: `git add -A && git commit -m "feat: sample_batch API with detection events and observable flips"`
