# Issue 462 Instruction-Wide DEPOLARIZE2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply sparse `DEPOLARIZE2` with one instruction-wide rare-event iterator over the flattened pair/shot domain while preserving dense fallback behavior.

**Architecture:** Keep interpreted and compiled execution sharing `FrameSimulator::exec_depolarize2_pairs`. Refactor `rare_error_iterator` to provide crate-visible sampler state that can be advanced with a short-lived RNG borrow, then use that sampler in a sparse instruction-wide `DEPOLARIZE2` helper. Add debug-only frame telemetry and focused integration tests for decode order, branch mapping, iterator-build counts, dense fallback selection, and distribution preservation.

**Tech Stack:** Rust 2024, `rand` 0.8, Cargo integration tests, existing `rstim` frame simulator and compiled sampler APIs, Python distribution verifier.

## Global Constraints

- Flatten and decode exactly as `event_index = pair_index * shots + shot_index`, `pair_index = event_index / shots`, and `shot_index = event_index % shots`.
- Branch indices `0..14` map exactly to `IX IY IZ XI XX XY XZ YI YX YY YZ ZI ZX ZY ZZ`.
- `II` is never valid.
- Interpreted and compiled execution must share the helper and expose `sampling_path`, `iterator_builds`, and `attempt_count`.
- For `p <= SPARSE_BERNOULLI_MAX_PROBABILITY = 0.02`, use the instruction-wide iterator.
- For `p > 0.02`, retain the dense fallback.
- Do not change the `DEPOLARIZE2` probability model.
- Do not add SIMD or parallel execution.
- The pinned `stim_depolarize2_two_measured_qubits` oracle remains `00=.92`, `01=.02666666666666667`, `10=.02666666666666667`, `11=.02666666666666667`.
- The distribution command must print `PASS distribution correctness cases=8 mismatch=0`.

---

## File Structure

- Modify `rstim/src/rare_error_iterator.rs`: add crate-visible sampler state while keeping the public iterator constructor and debug telemetry behavior intact.
- Modify `rstim/src/sim/frame.rs`: add debug-only `DEPOLARIZE2` sampling telemetry, explicit branch table/helpers, sparse instruction-wide execution, and dense fallback split.
- Create `rstim/tests/frame_instruction_wide_depolarize2.rs`: focused acceptance tests required by issue #462.
- Keep `rstim/tests/frame_depolarize_alloc.rs` as the allocation regression suite; run it after implementation.

### Task 1: Add Failing Instruction-Wide DEPOLARIZE2 Tests

**Files:**
- Create: `rstim/tests/frame_instruction_wide_depolarize2.rs`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines`, `rstim::executor::reference_sample`, `rstim::compiled::compile_circuit`, `rstim::sim::frame::FrameSimulator`, and debug-only frame helper functions that Task 3 will add.
- Produces: focused tests that fail before implementation because the debug helper surface and instruction-wide telemetry do not exist.

- [ ] **Step 1: Write the failing test file**

Create `rstim/tests/frame_instruction_wide_depolarize2.rs` with this complete file:

```rust
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rstim::compiled::compile_circuit;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::{
    depolarize2_branch_label_for_test, depolarize2_decode_event_for_test,
    depolarize2_sampling_telemetry, reset_depolarize2_sampling_telemetry,
    sample_depolarize2_branch_index_for_test, FrameSimulator,
};

struct ScriptedRng {
    draws: Vec<u64>,
    next: usize,
}

impl ScriptedRng {
    fn from_u64s(draws: Vec<u64>) -> Self {
        Self { draws, next: 0 }
    }
}

impl RngCore for ScriptedRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let value = self.draws.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        value
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_ne_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn many_depolarize2_pairs(pair_count: usize, probability: f64) -> String {
    let mut program = format!("DEPOLARIZE2({probability})");
    for pair in 0..pair_count {
        let a = 2 * pair;
        let b = a + 1;
        program.push_str(&format!(" {a} {b}"));
    }
    program.push('\n');
    program
}

fn run_interpreted(program: &str, num_qubits: usize, shots: usize, seed: u64) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(num_qubits, shots);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
}

fn run_compiled(program: &str, shots: usize, seed: u64) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let compiled = compile_circuit(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(compiled.num_qubits, shots);
    frame
        .run_compiled_blocks(&compiled.blocks, &ref_sample, &mut rng)
        .unwrap();
}

#[test]
fn decode_helper_is_pair_major_at_boundaries() {
    let shots = 1024;
    let cases = [
        (0, (0, 0)),
        (shots - 1, (0, shots - 1)),
        (shots, (1, 0)),
        (2 * shots - 1, (1, shots - 1)),
        (109 * shots, (109, 0)),
        (110 * shots - 1, (109, shots - 1)),
    ];

    for (event_index, expected) in cases {
        assert_eq!(
            depolarize2_decode_event_for_test(event_index, shots),
            expected,
            "event_index={event_index}"
        );
    }
}

#[test]
fn branch_indices_map_to_non_identity_paulis_in_order() {
    let labels: Vec<&'static str> = (0..15)
        .map(|branch| depolarize2_branch_label_for_test(branch).unwrap())
        .collect();
    assert_eq!(
        labels,
        vec![
            "IX", "IY", "IZ", "XI", "XX", "XY", "XZ", "YI", "YX", "YY", "YZ", "ZI", "ZX",
            "ZY", "ZZ",
        ]
    );
    assert_eq!(depolarize2_branch_label_for_test(15), None);
}

#[test]
fn seeded_branch_draws_are_uniform_and_never_ii() {
    let mut rng = StdRng::seed_from_u64(123);
    let mut counts = [0usize; 15];
    for _ in 0..1_500_000 {
        let branch = sample_depolarize2_branch_index_for_test(&mut rng);
        let label = depolarize2_branch_label_for_test(branch).unwrap();
        assert_ne!(label, "II");
        counts[branch] += 1;
    }

    for (branch, count) in counts.into_iter().enumerate() {
        assert!(
            (98_000..=102_000).contains(&count),
            "branch {branch} count {count} outside 98,000-102,000"
        );
    }
}

#[test]
fn scripted_branch_zero_applies_ix_not_ii() {
    let instrs = parse_lines("DEPOLARIZE2(0.001) 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = ScriptedRng::from_u64s(vec![u64::MAX, 0]);
    let mut frame = FrameSimulator::new(2, 1);

    reset_depolarize2_sampling_telemetry();
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let measurements = frame.measurements(&ref_sample);

    assert_eq!(measurements.row_words(0), &[0], "IX leaves qubit 0 unchanged");
    assert_eq!(measurements.row_words(1), &[1], "IX flips qubit 1");
    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, 1);
}

#[test]
fn sparse_interpreted_depolarize2_uses_one_instruction_wide_iterator() {
    let pair_count = 110;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.001);

    reset_depolarize2_sampling_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 462);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
}

#[test]
fn sparse_compiled_depolarize2_uses_one_instruction_wide_iterator() {
    let pair_count = 110;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.001);

    reset_depolarize2_sampling_telemetry();
    run_compiled(&program, shots, 462);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
}

#[test]
fn dense_probability_keeps_dense_fallback() {
    let pair_count = 3;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.3);

    reset_depolarize2_sampling_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 463);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "dense");
    assert_eq!(telemetry.iterator_builds, 0);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
}
```

- [ ] **Step 2: Run the focused test and verify the expected RED failure**

Run:

```sh
cargo test -p rstim --test frame_instruction_wide_depolarize2 -- --nocapture
```

Expected: compilation fails because `depolarize2_branch_label_for_test`,
`depolarize2_decode_event_for_test`, `depolarize2_sampling_telemetry`,
`reset_depolarize2_sampling_telemetry`, and
`sample_depolarize2_branch_index_for_test` do not exist yet.

- [ ] **Step 3: Commit the failing test**

```sh
git add rstim/tests/frame_instruction_wide_depolarize2.rs
git commit -m "test: cover instruction-wide depolarize2"
```

### Task 2: Refactor RareErrorIterator For Short-Lived RNG Borrows

**Files:**
- Modify: `rstim/src/rare_error_iterator.rs`

**Interfaces:**
- Consumes: existing `rare_error_indices(probability, attempt_count, rng)` API and telemetry tests.
- Produces: crate-visible `RareErrorIndexSampler::new(probability, attempt_count)` and `RareErrorIndexSampler::next_index(rng)` while preserving existing iterator behavior.

- [ ] **Step 1: Refactor sampler state without changing external behavior**

Update `rstim/src/rare_error_iterator.rs` so the mode and candidate state live in
`RareErrorIndexSampler`, and `RareErrorIterator` wraps that sampler plus the
existing RNG reference:

```rust
use rand::RngCore;
#[cfg(debug_assertions)]
use std::cell::Cell;

const F64_UNIT_INTERVAL_SCALE: f64 = 1.0 / ((1u64 << 53) as f64);

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RareErrorTelemetry {
    pub iterator_builds: usize,
    pub rng_core_draws: usize,
}

#[cfg(debug_assertions)]
thread_local! {
    static ITERATOR_BUILDS: Cell<usize> = const { Cell::new(0) };
    static RNG_CORE_DRAWS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reset_rare_error_telemetry() {
    ITERATOR_BUILDS.with(|builds| builds.set(0));
    RNG_CORE_DRAWS.with(|draws| draws.set(0));
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn rare_error_telemetry() -> RareErrorTelemetry {
    RareErrorTelemetry {
        iterator_builds: ITERATOR_BUILDS.with(Cell::get),
        rng_core_draws: RNG_CORE_DRAWS.with(Cell::get),
    }
}

#[cfg(debug_assertions)]
fn record_iterator_build() {
    ITERATOR_BUILDS.with(|builds| builds.set(builds.get().saturating_add(1)));
}

#[cfg(debug_assertions)]
fn record_rng_core_draw() {
    RNG_CORE_DRAWS.with(|draws| draws.set(draws.get().saturating_add(1)));
}

#[derive(Debug, Clone, Copy)]
enum RareErrorMode {
    Empty,
    Dense,
    Sparse { log_one_minus_p: f64 },
}

pub(crate) struct RareErrorIndexSampler {
    attempt_count: usize,
    next_candidate: usize,
    mode: RareErrorMode,
}

impl RareErrorIndexSampler {
    pub(crate) fn new(probability: f64, attempt_count: usize) -> Self {
        #[cfg(debug_assertions)]
        record_iterator_build();

        let mode = if attempt_count == 0 || probability <= 0.0 || probability.is_nan() {
            RareErrorMode::Empty
        } else if probability >= 1.0 {
            RareErrorMode::Dense
        } else {
            RareErrorMode::Sparse {
                log_one_minus_p: (-probability).ln_1p(),
            }
        };

        Self {
            attempt_count,
            next_candidate: 0,
            mode,
        }
    }

    pub(crate) fn next_index<R: RngCore + ?Sized>(&mut self, rng: &mut R) -> Option<usize> {
        match self.mode {
            RareErrorMode::Empty => None,
            RareErrorMode::Dense => {
                if self.next_candidate >= self.attempt_count {
                    return None;
                }
                let index = self.next_candidate;
                self.next_candidate += 1;
                Some(index)
            }
            RareErrorMode::Sparse { log_one_minus_p } => {
                while self.next_candidate < self.attempt_count {
                    let uniform = draw_open_unit_f64(rng);
                    let skip = (uniform.ln() / log_one_minus_p).floor();
                    let skip = if skip.is_finite() && skip >= 0.0 {
                        skip as usize
                    } else {
                        usize::MAX
                    };
                    let index = self.next_candidate.saturating_add(skip);
                    if index >= self.attempt_count {
                        self.next_candidate = self.attempt_count;
                        return None;
                    }
                    self.next_candidate = index + 1;
                    return Some(index);
                }
                None
            }
        }
    }
}

pub struct RareErrorIterator<'a, R: RngCore + ?Sized> {
    rng: &'a mut R,
    sampler: RareErrorIndexSampler,
}

pub fn rare_error_indices<'a, R: RngCore + ?Sized>(
    probability: f64,
    attempt_count: usize,
    rng: &'a mut R,
) -> RareErrorIterator<'a, R> {
    RareErrorIterator::new(probability, attempt_count, rng)
}

impl<'a, R: RngCore + ?Sized> RareErrorIterator<'a, R> {
    pub fn new(probability: f64, attempt_count: usize, rng: &'a mut R) -> Self {
        Self {
            rng,
            sampler: RareErrorIndexSampler::new(probability, attempt_count),
        }
    }
}

impl<R: RngCore + ?Sized> Iterator for RareErrorIterator<'_, R> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.sampler.next_index(self.rng)
    }
}

fn draw_open_unit_f64<R: RngCore + ?Sized>(rng: &mut R) -> f64 {
    loop {
        #[cfg(debug_assertions)]
        record_rng_core_draw();
        let raw = rng.next_u64();
        let value = ((raw >> 11) as f64) * F64_UNIT_INTERVAL_SCALE;
        if value > 0.0 {
            return value;
        }
    }
}
```

- [ ] **Step 2: Run rare iterator regression tests**

Run:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: all rare iterator tests pass and the output still includes
`PASS instruction-wide rare-error iterator`.

- [ ] **Step 3: Commit the refactor**

```sh
git add rstim/src/rare_error_iterator.rs
git commit -m "refactor: expose rare error sampler state"
```

### Task 3: Implement Sparse Instruction-Wide DEPOLARIZE2

**Files:**
- Modify: `rstim/src/sim/frame.rs`

**Interfaces:**
- Consumes: `RareErrorIndexSampler`, `SPARSE_BERNOULLI_MAX_PROBABILITY`, shared `exec_depolarize2_pairs`.
- Produces: sparse instruction-wide `DEPOLARIZE2` helper, dense fallback helper, explicit branch table, and debug-only telemetry/test helpers.

- [ ] **Step 1: Add imports, telemetry, and branch constants**

At the top of `rstim/src/sim/frame.rs`, import the sampler and add debug-only
telemetry storage:

```rust
use rand::Rng;
#[cfg(debug_assertions)]
use std::cell::RefCell;

use crate::compiled::{CompiledBasis, CompiledBlock, CompiledOp};
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::rare_error_iterator::RareErrorIndexSampler;
use crate::sim::bit_table::BitTable;
use crate::sim::measure_record_batch::MeasureRecordBatch;

#[cfg(debug_assertions)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Depolarize2SamplingTelemetry {
    pub sampling_path: &'static str,
    pub iterator_builds: usize,
    pub attempt_count: usize,
}

#[cfg(debug_assertions)]
impl Default for Depolarize2SamplingTelemetry {
    fn default() -> Self {
        Self {
            sampling_path: "none",
            iterator_builds: 0,
            attempt_count: 0,
        }
    }
}

#[cfg(debug_assertions)]
thread_local! {
    static DEPOLARIZE2_SAMPLING_TELEMETRY: RefCell<Depolarize2SamplingTelemetry> =
        const { RefCell::new(Depolarize2SamplingTelemetry {
            sampling_path: "none",
            iterator_builds: 0,
            attempt_count: 0,
        }) };
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reset_depolarize2_sampling_telemetry() {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| {
        *telemetry.borrow_mut() = Depolarize2SamplingTelemetry::default();
    });
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_sampling_telemetry() -> Depolarize2SamplingTelemetry {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| *telemetry.borrow())
}

#[cfg(debug_assertions)]
fn record_depolarize2_sampling(
    sampling_path: &'static str,
    iterator_builds: usize,
    attempt_count: usize,
) {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| {
        *telemetry.borrow_mut() = Depolarize2SamplingTelemetry {
            sampling_path,
            iterator_builds,
            attempt_count,
        };
    });
}
```

Near the noise helpers, add the explicit branch table and test helpers:

```rust
const DEPOLARIZE2_BRANCHES: [(u8, u8); 15] = [
    (0, 1), (0, 2), (0, 3), (1, 0), (1, 1),
    (1, 2), (1, 3), (2, 0), (2, 1), (2, 2),
    (2, 3), (3, 0), (3, 1), (3, 2), (3, 3),
];

#[cfg(debug_assertions)]
const DEPOLARIZE2_BRANCH_LABELS: [&str; 15] = [
    "IX", "IY", "IZ", "XI", "XX", "XY", "XZ", "YI", "YX", "YY", "YZ", "ZI", "ZX",
    "ZY", "ZZ",
];

fn sample_depolarize2_branch_index(rng: &mut impl Rng) -> usize {
    rng.gen_range(0..DEPOLARIZE2_BRANCHES.len())
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn sample_depolarize2_branch_index_for_test(rng: &mut impl Rng) -> usize {
    sample_depolarize2_branch_index(rng)
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_branch_label_for_test(branch_index: usize) -> Option<&'static str> {
    DEPOLARIZE2_BRANCH_LABELS.get(branch_index).copied()
}

fn decode_depolarize2_event(event_index: usize, shots: usize) -> (usize, usize) {
    debug_assert!(shots > 0);
    (event_index / shots, event_index % shots)
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_decode_event_for_test(event_index: usize, shots: usize) -> (usize, usize) {
    decode_depolarize2_event(event_index, shots)
}
```

- [ ] **Step 2: Split `exec_depolarize2_pairs` into sparse and dense paths**

Replace the current `exec_depolarize2_pairs` body with:

```rust
fn exec_depolarize2_pairs(
    &mut self,
    pairs: &[(usize, usize)],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) {
    if p <= 0.0 || pairs.is_empty() || self.batch_size == 0 {
        #[cfg(debug_assertions)]
        record_depolarize2_sampling("empty", 0, pairs.len().saturating_mul(self.batch_size));
        return;
    }

    let attempt_count = pairs.len() * self.batch_size;
    if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
        self.exec_depolarize2_pairs_sparse_instruction_wide(pairs, p, attempt_count, rng);
    } else {
        self.exec_depolarize2_pairs_dense(pairs, p, wpr, rng);
    }
}
```

- [ ] **Step 3: Add the sparse instruction-wide helper**

Add this method next to `exec_depolarize2_pairs`:

```rust
fn exec_depolarize2_pairs_sparse_instruction_wide(
    &mut self,
    pairs: &[(usize, usize)],
    p: f64,
    attempt_count: usize,
    rng: &mut impl Rng,
) {
    #[cfg(debug_assertions)]
    record_depolarize2_sampling("sparse", 1, attempt_count);

    let mut events = RareErrorIndexSampler::new(p, attempt_count);
    while let Some(event_index) = events.next_index(rng) {
        let (pair_index, shot_index) = decode_depolarize2_event(event_index, self.batch_size);
        let (qa, qb) = pairs[pair_index];
        let branch = sample_depolarize2_branch_index(rng);
        let (pa, pb) = two_qubit_pauli(branch);
        let word = shot_index / 64;
        let bit = (shot_index % 64) as u32;
        let mask = 1u64 << bit;
        apply_pauli_mask_to_tables(pa, &mut self.x_table, &mut self.z_table, qa, word, mask);
        apply_pauli_mask_to_tables(pb, &mut self.x_table, &mut self.z_table, qb, word, mask);
    }
}
```

Add the direct table helper near `apply_pauli_bits`:

```rust
fn apply_pauli_mask_to_tables(
    p: u8,
    x_table: &mut BitTable,
    z_table: &mut BitTable,
    q: usize,
    word: usize,
    mask: u64,
) {
    match p {
        1 => x_table.row_words_mut(q)[word] ^= mask,
        2 => {
            x_table.row_words_mut(q)[word] ^= mask;
            z_table.row_words_mut(q)[word] ^= mask;
        }
        3 => z_table.row_words_mut(q)[word] ^= mask,
        _ => {}
    }
}
```

- [ ] **Step 4: Move the current per-pair implementation into the dense helper**

Add `exec_depolarize2_pairs_dense` with the old per-pair scratch logic, but call
the shared branch sampler:

```rust
fn exec_depolarize2_pairs_dense(
    &mut self,
    pairs: &[(usize, usize)],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) {
    #[cfg(debug_assertions)]
    record_depolarize2_sampling("dense", 0, pairs.len() * self.batch_size);

    for &(qa, qb) in pairs {
        {
            let scratch = &mut self.depolarize_scratch;
            scratch.prepare_two(wpr);
            random_bits_with_prob_into(&mut scratch.events, self.batch_size, p, rng);
            for w in 0..wpr {
                let mut bits = scratch.events[w];
                while bits != 0 {
                    let bit = bits.trailing_zeros();
                    let branch = sample_depolarize2_branch_index(rng);
                    let (pa, pb) = two_qubit_pauli(branch);
                    apply_pauli_bits(pa, &mut scratch.x_a, &mut scratch.z_a, w, bit);
                    apply_pauli_bits(pb, &mut scratch.x_b, &mut scratch.z_b, w, bit);
                    bits &= bits - 1;
                }
            }
        }
        let scratch = &self.depolarize_scratch;
        let x = self.x_table.row_words_mut(qa);
        for w in 0..wpr {
            x[w] ^= scratch.x_a[w];
        }
        let z = self.z_table.row_words_mut(qa);
        for w in 0..wpr {
            z[w] ^= scratch.z_a[w];
        }
        let x = self.x_table.row_words_mut(qb);
        for w in 0..wpr {
            x[w] ^= scratch.x_b[w];
        }
        let z = self.z_table.row_words_mut(qb);
        for w in 0..wpr {
            z[w] ^= scratch.z_b[w];
        }
    }
}
```

Update `two_qubit_pauli` to use the explicit branch table:

```rust
fn two_qubit_pauli(branch: usize) -> (u8, u8) {
    DEPOLARIZE2_BRANCHES.get(branch).copied().unwrap_or((0, 0))
}
```

- [ ] **Step 5: Run focused tests**

Run:

```sh
cargo test -p rstim --test frame_instruction_wide_depolarize2 -- --nocapture
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: both commands pass. The first command covers issue #462 behavior; the
second proves the rare iterator refactor did not regress #460.

- [ ] **Step 6: Commit the implementation**

```sh
git add rstim/src/sim/frame.rs rstim/src/rare_error_iterator.rs rstim/tests/frame_instruction_wide_depolarize2.rs
git commit -m "feat: apply depolarize2 sparse events instruction-wide"
```

### Task 4: Run Allocation, Build, Distribution, And Full Verification

**Files:**
- No source edits expected. If a command fails, fix the narrow cause and rerun the failing command before continuing.

**Interfaces:**
- Consumes: the issue's required verification commands.
- Produces: verified branch ready for final review and PR creation.

- [ ] **Step 1: Run allocation regression**

Run:

```sh
cargo test -p rstim --test frame_depolarize_alloc -- --nocapture
```

Expected: passes and continues to show near-constant allocation growth across many pairs.

- [ ] **Step 2: Build release CLI**

Run:

```sh
cargo build --release -p rstim --bin rstim
```

Expected: release build exits successfully.

- [ ] **Step 3: Run distribution verifier**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --rstim target/release/rstim --shots 100000 --seeds 7 \
  --out /tmp/rstim-instruction-wide-depolarize2.json
```

Expected: output includes `PASS distribution correctness cases=8 mismatch=0`,
including the pinned `stim_depolarize2_two_measured_qubits` oracle:
`00=.92`, `01=.02666666666666667`, `10=.02666666666666667`,
`11=.02666666666666667`.

- [ ] **Step 4: Run full Cargo test suite**

Run:

```sh
cargo test
```

Expected: exits successfully. Existing unrelated warnings are acceptable if no
test fails.

- [ ] **Step 5: Run diff hygiene check**

Run:

```sh
git diff --check master...HEAD
```

Expected: exits successfully with no whitespace errors.

- [ ] **Step 6: Commit any verification-driven fixes**

If verification required fixes, commit them with:

```sh
git add <changed-files>
git commit -m "fix: stabilize instruction-wide depolarize2 verification"
```

If no files changed during verification, do not create an empty commit.

## Self-Review

- Spec coverage: Tasks cover helper sharing, exact flatten/decode, exact branch order, branch distribution, sparse iterator count, dense fallback, allocation regression, release build, and distribution verifier.
- Placeholder scan: no `TBD`, `TODO`, or open-ended implementation steps remain.
- Type consistency: helper names in Task 1 match the telemetry/test helper names produced in Task 3, and `RareErrorIndexSampler` from Task 2 is the type consumed by Task 3.
