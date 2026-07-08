# Issue 414 MeasureRecordBatch Contiguous Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store `MeasureRecordBatch` measurement rows in one contiguous row-major buffer while preserving existing lookback and XOR behavior.

**Architecture:** `MeasureRecordBatch` will keep `records: Vec<u64>` with rows addressed by `row_index * words_per_row`. An explicit `row_count` preserves zero-shot row semantics when `words_per_row == 0`, and a read-only `contiguous_words()` method lets tests verify the backing shape without exposing mutable storage.

**Tech Stack:** Rust 2024, existing `rstim` integration tests, deterministic `StdRng` sampling, existing b8 output writer for fixture parity fingerprints.

## Global Constraints

- Preserve `lookback_words`, `xor_lookback_into`, `push_row`, and `push_zeros` behavior.
- Keep existing public methods stable where possible.
- Back rows with contiguous storage that can be checked in tests.
- Do not change sampler semantics.
- Do not optimize detector algorithms beyond the storage layout.
- Verification command required by issue #414: `cargo test -p rstim --test measure_record_batch_storage`.
- Broader worker verification command required by Agent Desk: `cargo test`.

---

## File Structure

- Create `rstim/tests/measure_record_batch_storage.rs`: issue-level integration coverage for contiguous shape, lookback order, fixture detector parity, row alignment, and zero-shot row counting.
- Modify `rstim/src/sim/measure_record_batch.rs`: replace `Vec<Vec<u64>>` with `Vec<u64>` plus `row_count`, add private row slicing helpers, and add public `contiguous_words()`.

### Task 1: Add Failing MeasureRecordBatch Storage Tests

**Files:**
- Create: `rstim/tests/measure_record_batch_storage.rs`

**Interfaces:**
- Consumes: existing `MeasureRecordBatch::new`, `push_row`, `push_zeros`, `lookback_words`, `xor_lookback_into`, `len`, `words_per_row`, `batch_size`; existing `rstim::output::write_shots_b8`; existing sampler APIs.
- Produces: tests named `contiguous_storage_reports_expected_shape`, `lookback_words_match_pushed_rows`, `xor_lookback_preserves_detector_parity_for_known_fixture`, `push_zeros_preserves_row_alignment`, and `zero_shot_rows_remain_counted`.

- [ ] **Step 1: Write the failing test file**

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, sample_batch_with_options};
use rstim::sim::bit_table::BitTable;
use rstim::sim::measure_record_batch::MeasureRecordBatch;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn b8_fingerprint(table: &BitTable) -> (usize, u64) {
    let mut bytes = Vec::new();
    write_shots_b8(table, &mut bytes).expect("write b8");
    (bytes.len(), fnv64(&bytes))
}

#[test]
fn contiguous_storage_reports_expected_shape() {
    let mut batch = MeasureRecordBatch::new(130);

    batch.push_row(&[0x11, 0x22, 0x33, 0x44]);
    batch.push_row(&[0xaa, 0xbb]);

    assert_eq!(batch.batch_size(), 130);
    assert_eq!(batch.words_per_row(), 3);
    assert_eq!(batch.len(), 2);
    assert_eq!(
        batch.contiguous_words(),
        &[0x11, 0x22, 0x33, 0xaa, 0xbb, 0]
    );
    assert_eq!(batch.contiguous_words().len(), batch.len() * batch.words_per_row());
}

#[test]
fn lookback_words_match_pushed_rows() {
    let mut batch = MeasureRecordBatch::new(129);

    batch.push_row(&[0b001, 0b010, 0b100]);
    batch.push_row(&[0xf0]);
    batch.push_row(&[0xa0, 0xb0, 0xc0, 0xd0]);

    assert_eq!(batch.words_per_row(), 3);
    assert_eq!(batch.lookback_words(1), &[0xa0, 0xb0, 0xc0]);
    assert_eq!(batch.lookback_words(2), &[0xf0, 0, 0]);
    assert_eq!(batch.lookback_words(3), &[0b001, 0b010, 0b100]);
    assert!(batch.lookback(3, 0));
    assert!(!batch.lookback(3, 1));
}

#[test]
fn xor_lookback_preserves_detector_parity_for_known_fixture() {
    let instrs = parse_lines(SURFACE_D11_R100).expect("parse checked fixture");
    let mut rng = StdRng::seed_from_u64(20260708);

    let out = sample_batch_with_options(&instrs, 130, &mut rng, SampleOptions::default())
        .expect("sample checked fixture");

    assert_eq!(out.measurements.num_major(), 12121);
    assert_eq!(out.detections.num_major(), 12000);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.measurements.num_minor(), 130);
    assert_eq!(out.detections.num_minor(), 130);
    assert_eq!(out.observable_flips.num_minor(), 130);
    assert_eq!(out.detector_materializations, 12000);
    assert_eq!(out.observable_materializations, 1);
    assert_eq!(
        b8_fingerprint(&out.detections),
        (195000, 0xed59495c207d6221)
    );
    assert_eq!(
        b8_fingerprint(&out.observable_flips),
        (130, 0x8187b6cddeeef841)
    );
}

#[test]
fn push_zeros_preserves_row_alignment() {
    let mut batch = MeasureRecordBatch::new(65);

    batch.push_row(&[0x1111, 0x2222]);
    batch.push_zeros();
    batch.push_row(&[0x3333, 0x4444]);

    assert_eq!(batch.words_per_row(), 2);
    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch.contiguous_words(),
        &[0x1111, 0x2222, 0, 0, 0x3333, 0x4444]
    );

    let mut dest = vec![0xffff, 0xffff];
    batch.xor_lookback_into(2, &mut dest);
    assert_eq!(dest, &[0xffff, 0xffff]);
    batch.xor_lookback_into(1, &mut dest);
    assert_eq!(dest, &[0xcccc, 0xbbbb]);
}

#[test]
fn zero_shot_rows_remain_counted() {
    let mut batch = MeasureRecordBatch::new(0);

    batch.push_zeros();
    batch.push_row(&[0x1234, 0x5678]);

    assert_eq!(batch.words_per_row(), 0);
    assert_eq!(batch.len(), 2);
    assert!(batch.contiguous_words().is_empty());
    assert!(batch.lookback_words(1).is_empty());
    assert!(batch.lookback_words(2).is_empty());

    let mut dest = Vec::new();
    batch.xor_lookback_into(1, &mut dest);
    assert!(dest.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test measure_record_batch_storage`

Expected: FAIL at compile time with `no method named 'contiguous_words' found for struct 'MeasureRecordBatch'`. This proves the new test is exercising the storage inspection behavior that does not exist yet.

- [ ] **Step 3: Commit the failing test**

```bash
git add rstim/tests/measure_record_batch_storage.rs
git commit -m "test: cover measure record batch storage"
```

### Task 2: Convert MeasureRecordBatch To Contiguous Storage

**Files:**
- Modify: `rstim/src/sim/measure_record_batch.rs`

**Interfaces:**
- Consumes: the Task 1 tests and existing callers in `FrameSimulator`.
- Produces: `MeasureRecordBatch` backed by row-major `Vec<u64>`, with `contiguous_words() -> &[u64]` and unchanged behavior for all existing methods.

- [ ] **Step 1: Replace the struct fields**

Change the struct definition to:

```rust
#[derive(Debug, Clone)]
pub struct MeasureRecordBatch {
    batch_size: usize,
    words_per_row: usize,
    row_count: usize,
    records: Vec<u64>,
}
```

- [ ] **Step 2: Initialize the new fields**

Change `new` to:

```rust
pub fn new(batch_size: usize) -> Self {
    Self {
        batch_size,
        words_per_row: (batch_size + 63) / 64,
        row_count: 0,
        records: Vec::new(),
    }
}
```

- [ ] **Step 3: Add private row helpers**

Add these helpers inside the `impl MeasureRecordBatch` block:

```rust
fn row_range(&self, row: usize) -> std::ops::Range<usize> {
    let start = row * self.words_per_row;
    start..start + self.words_per_row
}

fn row_words(&self, row: usize) -> &[u64] {
    let range = self.row_range(row);
    &self.records[range]
}

fn lookback_row(&self, k: usize) -> usize {
    self.row_count - k
}
```

- [ ] **Step 4: Rewrite row appends**

Change `push_row` and `push_zeros` to:

```rust
/// Push a row of measurement bits (one word-slice per measurement)
pub fn push_row(&mut self, words: &[u64]) {
    let start = self.records.len();
    self.records.resize(start + self.words_per_row, 0);
    let copy_len = self.words_per_row.min(words.len());
    self.records[start..start + copy_len].copy_from_slice(&words[..copy_len]);
    self.row_count += 1;
}

pub fn push_zeros(&mut self) {
    self.records
        .resize(self.records.len() + self.words_per_row, 0);
    self.row_count += 1;
}
```

- [ ] **Step 5: Rewrite lookback accessors**

Change `lookback`, `lookback_words`, `xor_lookback_into`, and `len` to:

```rust
/// lookback(k, shot): get bit for rec[-k] for the given shot (k >= 1)
pub fn lookback(&self, k: usize, shot: usize) -> bool {
    let row = self.lookback_row(k);
    let word = shot / 64;
    let bit = shot % 64;
    (self.row_words(row)[word] >> bit) & 1 == 1
}

pub fn lookback_words(&self, k: usize) -> &[u64] {
    let row = self.lookback_row(k);
    self.row_words(row)
}

pub fn xor_lookback_into(&self, k: usize, dest: &mut [u64]) {
    let row = self.lookback_row(k);
    for (d, s) in dest.iter_mut().zip(self.row_words(row).iter()) {
        *d ^= *s;
    }
}

pub fn len(&self) -> usize {
    self.row_count
}
```

- [ ] **Step 6: Add the storage inspection method**

Add this public method after `words_per_row`:

```rust
pub fn contiguous_words(&self) -> &[u64] {
    &self.records
}
```

- [ ] **Step 7: Run focused tests**

Run: `cargo test -p rstim --test measure_record_batch_storage`

Expected: PASS. All five tests pass, including the deterministic fixture parity fingerprint.

- [ ] **Step 8: Run existing MeasureRecordBatch-adjacent tests**

Run: `cargo test -p rstim --test frame_sim measure_record_batch`

Expected: PASS for the existing `frame_sim` integration test filter.

- [ ] **Step 9: Commit the implementation**

```bash
git add rstim/src/sim/measure_record_batch.rs
git commit -m "feat: store measure records contiguously"
```

### Task 3: Verify And Prepare Branch

**Files:**
- No new source files beyond Tasks 1 and 2.

**Interfaces:**
- Consumes: committed tests and implementation.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run issue verification**

Run: `cargo test -p rstim --test measure_record_batch_storage`

Expected: PASS.

- [ ] **Step 2: Run Agent Desk verification**

Run: `cargo test`

Expected: PASS. If existing unrelated warnings are emitted, record them in the PR body.

- [ ] **Step 3: Inspect final diff**

Run: `git diff --stat origin/master..HEAD`

Expected: diff includes the issue #414 design, plan, storage test, and `MeasureRecordBatch` implementation only.

## Self-Review

- Spec coverage: Task 1 checks the direct contiguous shape, lookback behavior, fixture detector parity, zero row alignment, and zero-shot row counting. Task 2 changes only the requested storage layout while keeping existing methods. Task 3 runs the required focused command and broad `cargo test`.
- Placeholder scan: no `TBD`, `TODO`, or deferred implementation steps remain.
- Type consistency: the plan consistently uses `MeasureRecordBatch::contiguous_words() -> &[u64]`, `records: Vec<u64>`, and `row_count: usize`.
