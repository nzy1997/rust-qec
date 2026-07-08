# Issue 413 Depolarizing Scratch Reuse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reuse depolarizing noise scratch storage in `FrameSimulator` while preserving sampled distributions.

**Architecture:** `FrameSimulator` will own a private `DepolarizeScratch` workspace with reusable word buffers. `DEPOLARIZE1` and `DEPOLARIZE2` will call helper methods that refill and clear the workspace per target instead of allocating fresh `Vec<u64>` buffers inside the target loop.

**Tech Stack:** Rust 2024, `rand` 0.8, existing `rstim` parser/executor/frame simulator integration tests.

## Global Constraints

- Do not change public CLI output.
- Do not require the selected full d11/r100 benchmark to meet a specific speedup threshold.
- Preserve `DEPOLARIZE1(p)` and `DEPOLARIZE2(p)` sampled measurement/detector distributions.
- Use allocation or scratch instrumentation in tests instead of wall-clock thresholds.
- Preserve #412 integer-threshold event-mask semantics for depolarizing event selection.

---

## File Structure

- Modify `rstim/src/sim/frame.rs`: add private scratch storage, add a scratch-filling noise-mask helper, and route `DEPOLARIZE1`/`DEPOLARIZE2` through reusable buffers.
- Create `rstim/tests/frame_depolarize_alloc.rs`: add allocation-count and distribution smoke coverage required by #413.

### Task 1: Add Failing Allocation And Distribution Tests

**Files:**
- Create: `rstim/tests/frame_depolarize_alloc.rs`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines`, `rstim::executor::reference_sample`, `rstim::sim::frame::FrameSimulator`
- Produces: integration tests named `depolarize2_reuses_scratch_across_many_target_pairs` and `depolarize1_and_depolarize2_preserve_distribution_smoke`

- [ ] **Step 1: Write the failing test file**

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::FrameSimulator;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct CountingAllocator;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn run_frame(program: &str, num_qubits: usize, batch_size: usize, seed: u64) -> Vec<Vec<u64>> {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(num_qubits, batch_size);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let measurements = frame.measurements(&ref_sample);
    (0..measurements.num_major())
        .map(|row| measurements.row_words(row).to_vec())
        .collect()
}

fn count_measurement_ones(rows: &[Vec<u64>]) -> u32 {
    rows.iter()
        .flat_map(|row| row.iter())
        .map(|word| word.count_ones())
        .sum()
}

fn many_depolarize2_pairs(pair_count: usize) -> String {
    let mut program = String::from("DEPOLARIZE2(0.001)");
    for pair in 0..pair_count {
        let a = 2 * pair;
        let b = a + 1;
        program.push_str(&format!(" {a} {b}"));
    }
    program.push('\n');
    program
}

#[test]
fn depolarize2_reuses_scratch_across_many_target_pairs() {
    let pair_count = 256;
    let batch_size = 512;
    let program = many_depolarize2_pairs(pair_count);
    let instrs = parse_lines(&program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(19);
    let mut frame = FrameSimulator::new(pair_count * 2, batch_size);

    ALLOC_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Relaxed);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    COUNT_ALLOCATIONS.store(false, Ordering::Relaxed);

    let allocations = ALLOC_COUNT.load(Ordering::Relaxed);
    assert!(
        allocations < 160,
        "DEPOLARIZE2 should reuse scratch across {pair_count} target pairs; saw {allocations} allocations"
    );
}

#[test]
fn depolarize1_and_depolarize2_preserve_distribution_smoke() {
    let batch_size = 65_536;
    let dep1_rows = run_frame("DEPOLARIZE1(0.3) 0\nM 0\n", 1, batch_size, 7);
    let dep1_flips = count_measurement_ones(&dep1_rows);
    assert!(
        (12_500..=13_800).contains(&dep1_flips),
        "DEPOLARIZE1(0.3) should flip Z-basis measurements about 20% of the time; got {dep1_flips}"
    );

    let dep2_rows = run_frame("DEPOLARIZE2(0.3) 0 1\nM 0 1\n", 2, batch_size, 11);
    let dep2_flips = count_measurement_ones(&dep2_rows);
    assert!(
        (19_800..=22_100).contains(&dep2_flips),
        "DEPOLARIZE2(0.3) should flip about 16% of measured qubit results; got {dep2_flips}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test frame_depolarize_alloc`

Expected: `depolarize2_reuses_scratch_across_many_target_pairs` fails because the current implementation allocates fresh scratch vectors inside each target-pair loop, while `depolarize1_and_depolarize2_preserve_distribution_smoke` passes.

- [ ] **Step 3: Commit the failing test**

```bash
git add rstim/tests/frame_depolarize_alloc.rs
git commit -m "test: cover depolarize scratch reuse"
```

### Task 2: Reuse Depolarizing Scratch In FrameSimulator

**Files:**
- Modify: `rstim/src/sim/frame.rs`

**Interfaces:**
- Consumes: existing `random_bits_with_prob` probability behavior and `FrameSimulator::exec_op`
- Produces: private `DepolarizeScratch`, `random_bits_with_prob_into`, `exec_depolarize1`, and `exec_depolarize2`

- [ ] **Step 1: Add scratch state to `FrameSimulator`**

Add a `depolarize_scratch: DepolarizeScratch` field and initialize it in `FrameSimulator::new`.

```rust
pub struct FrameSimulator {
    pub num_qubits: usize,
    pub batch_size: usize,
    pub x_table: BitTable,
    pub z_table: BitTable,
    pub m_record: MeasureRecordBatch,
    last_correlated_error_occurred: Vec<u64>,
    depolarize_scratch: DepolarizeScratch,
    det_records: Vec<Vec<u64>>,
    obs_records: Vec<Vec<u64>>,
}
```

```rust
last_correlated_error_occurred: vec![0u64; words_per_row],
depolarize_scratch: DepolarizeScratch::new(),
det_records: Vec::new(),
obs_records: Vec::new(),
```

- [ ] **Step 2: Add scratch helpers**

Add near the noise helpers:

```rust
#[derive(Default)]
struct DepolarizeScratch {
    events: Vec<u64>,
    x_a: Vec<u64>,
    z_a: Vec<u64>,
    x_b: Vec<u64>,
    z_b: Vec<u64>,
}

impl DepolarizeScratch {
    fn new() -> Self {
        Self::default()
    }

    fn prepare_one(&mut self, words: usize) {
        resize_and_clear(&mut self.events, words);
        resize_and_clear(&mut self.x_a, words);
        resize_and_clear(&mut self.z_a, words);
    }

    fn prepare_two(&mut self, words: usize) {
        self.prepare_one(words);
        resize_and_clear(&mut self.x_b, words);
        resize_and_clear(&mut self.z_b, words);
    }
}

fn resize_and_clear(words: &mut Vec<u64>, len: usize) {
    words.resize(len, 0);
    words.fill(0);
}
```

- [ ] **Step 3: Add scratch-filling noise mask helper**

Replace `random_bits_with_prob` with a wrapper around a new helper.

```rust
fn random_bits_with_prob(words: usize, valid_bits: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    random_bits_with_prob_into(&mut result, valid_bits, p, rng);
    result
}

fn random_bits_with_prob_into(result: &mut [u64], valid_bits: usize, p: f64, rng: &mut impl Rng) {
    result.fill(0);
    if p <= 0.0 {
        return;
    }
    if p >= 1.0 {
        result.fill(!0u64);
        mask_unused_bits(result, valid_bits);
        return;
    }

    let threshold = probability_threshold_u64(p);
    if threshold == 0 {
        return;
    }

    for (word_idx, word) in result.iter_mut().enumerate() {
        let valid_in_word = valid_bits.saturating_sub(word_idx * 64).min(64);
        for bit in 0..valid_in_word {
            if rng.r#gen::<u64>() < threshold {
                *word |= 1u64 << bit;
            }
        }
    }
}
```

- [ ] **Step 4: Move depolarizing execution into helper methods**

Replace the existing `DEPOLARIZE1` match arm with:

```rust
"DEPOLARIZE1" => {
    let p = args.first().copied().unwrap_or(0.0);
    self.exec_depolarize1(targets, p, wpr, rng)?;
}
```

Replace the existing `DEPOLARIZE2` match arm with:

```rust
"DEPOLARIZE2" => {
    let p = args.first().copied().unwrap_or(0.0);
    self.exec_depolarize2(targets, p, wpr, rng)?;
}
```

Add methods inside `impl FrameSimulator`:

```rust
fn exec_depolarize1(
    &mut self,
    targets: &[StimTarget],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) -> Result<(), String> {
    if p <= 0.0 {
        return Ok(());
    }
    for q in qubits(targets)? {
        self.depolarize_scratch.prepare_one(wpr);
        random_bits_with_prob_into(
            &mut self.depolarize_scratch.events,
            self.batch_size,
            p,
            rng,
        );
        for w in 0..wpr {
            let mut bits = self.depolarize_scratch.events[w];
            while bits != 0 {
                let bit = bits.trailing_zeros();
                match rng.gen_range(0u8..3) {
                    0 => self.depolarize_scratch.x_a[w] |= 1u64 << bit,
                    1 => {
                        self.depolarize_scratch.x_a[w] |= 1u64 << bit;
                        self.depolarize_scratch.z_a[w] |= 1u64 << bit;
                    }
                    _ => self.depolarize_scratch.z_a[w] |= 1u64 << bit,
                }
                bits &= bits - 1;
            }
        }
        let x = self.x_table.row_words_mut(q);
        for w in 0..wpr {
            x[w] ^= self.depolarize_scratch.x_a[w];
        }
        let z = self.z_table.row_words_mut(q);
        for w in 0..wpr {
            z[w] ^= self.depolarize_scratch.z_a[w];
        }
    }
    Ok(())
}

fn exec_depolarize2(
    &mut self,
    targets: &[StimTarget],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) -> Result<(), String> {
    if p <= 0.0 {
        return Ok(());
    }
    for (qa, qb) in qubit_pairs(targets)? {
        self.depolarize_scratch.prepare_two(wpr);
        random_bits_with_prob_into(
            &mut self.depolarize_scratch.events,
            self.batch_size,
            p,
            rng,
        );
        for w in 0..wpr {
            let mut bits = self.depolarize_scratch.events[w];
            while bits != 0 {
                let bit = bits.trailing_zeros();
                let r = rng.gen_range(0u8..15);
                let (pa, pb) = two_qubit_pauli(r);
                apply_pauli_bits(
                    pa,
                    &mut self.depolarize_scratch.x_a,
                    &mut self.depolarize_scratch.z_a,
                    w,
                    bit,
                );
                apply_pauli_bits(
                    pb,
                    &mut self.depolarize_scratch.x_b,
                    &mut self.depolarize_scratch.z_b,
                    w,
                    bit,
                );
                bits &= bits - 1;
            }
        }
        let x = self.x_table.row_words_mut(qa);
        for w in 0..wpr {
            x[w] ^= self.depolarize_scratch.x_a[w];
        }
        let z = self.z_table.row_words_mut(qa);
        for w in 0..wpr {
            z[w] ^= self.depolarize_scratch.z_a[w];
        }
        let x = self.x_table.row_words_mut(qb);
        for w in 0..wpr {
            x[w] ^= self.depolarize_scratch.x_b[w];
        }
        let z = self.z_table.row_words_mut(qb);
        for w in 0..wpr {
            z[w] ^= self.depolarize_scratch.z_b[w];
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run focused tests**

Run: `cargo test -p rstim --test frame_depolarize_alloc`

Expected: both tests pass.

Run: `cargo test -p rstim --test frame_noise_masks`

Expected: all #412 mask tests still pass.

- [ ] **Step 6: Commit implementation**

```bash
git add rstim/src/sim/frame.rs rstim/tests/frame_depolarize_alloc.rs
git commit -m "fix: reuse depolarize frame scratch"
```

### Task 3: Final Verification And PR

**Files:**
- Modify: none expected beyond previous tasks

**Interfaces:**
- Consumes: committed implementation from Tasks 1-2
- Produces: pushed branch and pull request against `master`

- [ ] **Step 1: Run required verification**

Run: `cargo test -p rstim --test frame_depolarize_alloc`

Expected: pass.

Run: `cargo test`

Expected: pass for the default workspace test suite.

- [ ] **Step 2: Inspect git state**

Run: `git status --short --branch`

Expected: clean worker branch ahead of `origin/master`.

- [ ] **Step 3: Push and create PR**

Run: `git push -u origin agent/issue-413-remove-per-target-scratch-allocations-from-depol-run-1`

Run: `gh pr create --repo nzy1997/rstim --base master --head agent/issue-413-remove-per-target-scratch-allocations-from-depol-run-1 --title "Remove per-target depolarize scratch allocations" --body "<summary and tests>"`

Expected: PR URL printed.

## Self-Review

- Spec coverage: scratch reuse, distribution preservation, allocation test, and no CLI output changes are all covered.
- Placeholder scan: no TBD/TODO placeholders remain.
- Type consistency: helper names and field names match across tasks.
