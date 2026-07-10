# Issue 460 Rare Error Iterator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an internal instruction-wide `RareErrorIterator` over flattened opportunity indices with deterministic geometric skipping and focused acceptance tests.

**Architecture:** Keep the new primitive isolated in `rstim/src/rare_error_iterator.rs` and expose it as a hidden internal module from `rstim/src/lib.rs`. The iterator handles exact empty/all-event boundary modes without RNG draws and uses constant-space geometric skipping for `0.0 < p < 1.0`. Acceptance tests exercise the iterator directly without wiring simulator noise instructions to it.

**Tech Stack:** Rust 2024, `rand` 0.8, `StdRng::seed_from_u64(123)`, Cargo integration tests, thread-local allocation counting in the focused test binary.

## Global Constraints

- Do not wire noise instructions to the iterator in this issue.
- Keep the existing dense sampler available for probabilities above the sparse threshold.
- Yield strictly increasing, unique indices smaller than `attempt_count`.
- Yield nothing for `probability <= 0` or `attempt_count == 0`.
- Yield exactly `0..attempt_count` for `probability >= 1`.
- Use geometric skipping for `0.0 < probability < 1.0`.
- Expose iterator-build and RNG-core-draw telemetry for the acceptance tests.
- Allocate no dense bitmap or vector proportional to `attempt_count`.
- Seeded tests must use `rand::rngs::StdRng::seed_from_u64(123)`.
- For `attempt_count = 1_000_000`, `p = 0.001`, and seed `123`, require 800-1,200 events and fewer than 10,000 RNG-core draws.
- Ten 100,000-attempt windows must each contain an event, the window counts must not all be identical, and yielded gaps must contain more than 100 distinct values.
- Allocation measured while constructing the iterator and requesting its first event at `p = 1e-9` may grow by at most 4 KiB from `attempt_count = 1_000_000` to `1_000_000_000`.
- The focused acceptance test must print `PASS instruction-wide rare-error iterator`.
- Verification must include `cargo test -p rstim --test rare_error_iterator -- --nocapture`.
- Final verification must include `cargo test`.

---

## File Structure

- Create `rstim/tests/rare_error_iterator.rs`: focused acceptance tests, deterministic seeded checks, draw-count check, and thread-local allocation comparison.
- Create `rstim/src/rare_error_iterator.rs`: hidden internal iterator implementation and telemetry.
- Modify `rstim/src/lib.rs`: export the hidden internal module for integration tests and future internal wiring.

### Task 1: Add Rare Error Iterator and Acceptance Tests

**Files:**
- Create: `rstim/tests/rare_error_iterator.rs`
- Create: `rstim/src/rare_error_iterator.rs`
- Modify: `rstim/src/lib.rs`

**Interfaces:**
- Consumes: `rand::RngCore`, `rand::rngs::StdRng`, `rand::SeedableRng`
- Produces: `rstim::rare_error_iterator::RareErrorIterator<'a, R>`, `rstim::rare_error_iterator::rare_error_indices(probability: f64, attempt_count: usize, rng: &mut R)`, `RareErrorIterator::telemetry()`

- [ ] **Step 1: Write the failing acceptance test**

Create `rstim/tests/rare_error_iterator.rs` with this complete test file:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::rare_error_iterator::rare_error_indices;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashSet;
use std::hint::black_box;

struct CountingAllocator;

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED_BYTES: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        record_allocation(layout.size());
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        record_allocation(layout.size());
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        record_allocation(new_size);
        ptr
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

fn record_allocation(size: usize) {
    COUNT_ALLOCATIONS.with(|enabled| {
        if enabled.get() {
            ALLOCATED_BYTES.with(|bytes| {
                bytes.set(bytes.get().saturating_add(size));
            });
        }
    });
}

fn measure_allocated_bytes<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATED_BYTES.with(|bytes| bytes.set(0));
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(true));
    let result = f();
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
    let bytes = ALLOCATED_BYTES.with(|bytes| bytes.get());
    (result, bytes)
}

fn collect_seeded(probability: f64, attempt_count: usize) -> Vec<usize> {
    let mut rng = StdRng::seed_from_u64(123);
    rare_error_indices(probability, attempt_count, &mut rng).collect()
}

#[test]
fn boundary_probabilities_and_zero_attempts() {
    let mut rng = StdRng::seed_from_u64(123);
    let empty: Vec<usize> = rare_error_indices(0.0, 8, &mut rng).collect();
    assert!(empty.is_empty(), "p=0 must yield no rare events");

    let negative: Vec<usize> = rare_error_indices(-0.25, 8, &mut rng).collect();
    assert!(
        negative.is_empty(),
        "negative probabilities must yield no rare events"
    );

    let zero_attempts: Vec<usize> = rare_error_indices(0.5, 0, &mut rng).collect();
    assert!(
        zero_attempts.is_empty(),
        "zero attempts must yield no rare events"
    );

    let all: Vec<usize> = rare_error_indices(1.0, 8, &mut rng).collect();
    assert_eq!(all, (0..8).collect::<Vec<_>>());

    let above_one: Vec<usize> = rare_error_indices(2.0, 5, &mut rng).collect();
    assert_eq!(above_one, (0..5).collect::<Vec<_>>());

    let iter = rare_error_indices(1.0, 3, &mut rng);
    assert_eq!(iter.telemetry().iterator_builds, 1);
    assert_eq!(
        iter.telemetry().rng_core_draws,
        0,
        "dense boundary mode must not draw randomness"
    );
}

#[test]
fn indices_are_strictly_increasing_unique_and_in_range() {
    let attempt_count = 1_000_000;
    let indices = collect_seeded(0.001, attempt_count);
    assert!(!indices.is_empty(), "seeded sparse run should produce events");

    let mut previous = None;
    for &index in &indices {
        assert!(
            index < attempt_count,
            "index {index} must be smaller than attempt_count {attempt_count}"
        );
        if let Some(previous) = previous {
            assert!(
                index > previous,
                "indices must be strictly increasing: previous={previous}, current={index}"
            );
        }
        previous = Some(index);
    }

    let unique: HashSet<usize> = indices.iter().copied().collect();
    assert_eq!(
        unique.len(),
        indices.len(),
        "strict ordering must also imply no duplicate indices"
    );
}

#[test]
fn seeded_iterator_is_reproducible() {
    let first = collect_seeded(0.001, 1_000_000);
    let second = collect_seeded(0.001, 1_000_000);
    assert_eq!(first, second);
    assert!(
        !first.is_empty(),
        "seeded reproducibility check should cover a non-empty sparse stream"
    );
}

#[test]
fn sparse_frequency_windows_and_gaps_are_non_periodic() {
    let attempt_count = 1_000_000;
    let indices = collect_seeded(0.001, attempt_count);
    let event_count = indices.len();
    assert!(
        (800..=1_200).contains(&event_count),
        "expected 800-1,200 events for p=0.001 over {attempt_count} attempts, got {event_count}"
    );

    let mut windows = [0usize; 10];
    for &index in &indices {
        windows[index / 100_000] += 1;
    }
    assert!(
        windows.iter().all(|&count| count > 0),
        "every 100,000-attempt window must contain at least one event: {windows:?}"
    );
    assert!(
        windows.iter().any(|&count| count != windows[0]),
        "window counts must not all be identical: {windows:?}"
    );

    let distinct_gaps: HashSet<usize> = indices.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        distinct_gaps.len() > 100,
        "geometric gaps should have more than 100 distinct values, got {}",
        distinct_gaps.len()
    );

    println!("PASS instruction-wide rare-error iterator");
}

#[test]
fn sparse_draw_count_is_bounded() {
    let mut rng = StdRng::seed_from_u64(123);
    let mut iter = rare_error_indices(0.001, 1_000_000, &mut rng);
    let mut event_count = 0usize;
    while iter.next().is_some() {
        event_count += 1;
    }

    let telemetry = iter.telemetry();
    assert_eq!(telemetry.iterator_builds, 1);
    assert!(
        (800..=1_200).contains(&event_count),
        "draw-count test should exercise the expected sparse event frequency, got {event_count}"
    );
    assert!(
        telemetry.rng_core_draws < 10_000,
        "sparse iterator should not draw once per attempt; saw {} core RNG draws",
        telemetry.rng_core_draws
    );
}

#[test]
fn iterator_allocation_is_independent_of_attempt_count() {
    fn allocated_for_first_event(attempt_count: usize) -> usize {
        let mut rng = StdRng::seed_from_u64(123);
        let (first_event, bytes) = measure_allocated_bytes(|| {
            let mut iter = rare_error_indices(1e-9, attempt_count, &mut rng);
            black_box(iter.next())
        });
        assert!(
            first_event.is_none_or(|index| index < attempt_count),
            "first event must be in range when it exists"
        );
        bytes
    }

    let small = allocated_for_first_event(1_000_000);
    let large = allocated_for_first_event(1_000_000_000);
    assert!(
        large <= small + 4096,
        "larger flattened domain may allocate at most 4 KiB more; small={small}, large={large}"
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: FAIL because `rstim::rare_error_iterator` does not exist yet. If it fails for a typo in the test file instead of the missing iterator module, fix the test and rerun until the failure proves the missing feature.

- [ ] **Step 3: Add the hidden module export**

Append this module export to `rstim/src/lib.rs` after the existing module list:

```rust
#[doc(hidden)]
pub mod rare_error_iterator;
```

- [ ] **Step 4: Implement the iterator**

Create `rstim/src/rare_error_iterator.rs` with this complete implementation:

```rust
use rand::RngCore;

const F64_UNIT_INTERVAL_SCALE: f64 = 1.0 / ((1u64 << 53) as f64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RareErrorTelemetry {
    pub iterator_builds: usize,
    pub rng_core_draws: usize,
}

#[derive(Debug, Clone, Copy)]
enum RareErrorMode {
    Empty,
    Dense,
    Sparse { log_one_minus_p: f64 },
}

pub struct RareErrorIterator<'a, R: RngCore + ?Sized> {
    rng: &'a mut R,
    attempt_count: usize,
    next_candidate: usize,
    mode: RareErrorMode,
    telemetry: RareErrorTelemetry,
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
            rng,
            attempt_count,
            next_candidate: 0,
            mode,
            telemetry: RareErrorTelemetry {
                iterator_builds: 1,
                rng_core_draws: 0,
            },
        }
    }

    pub fn telemetry(&self) -> RareErrorTelemetry {
        self.telemetry
    }

    fn draw_open_unit_f64(&mut self) -> f64 {
        loop {
            let raw = self.rng.next_u64();
            self.telemetry.rng_core_draws += 1;
            let value = ((raw >> 11) as f64) * F64_UNIT_INTERVAL_SCALE;
            if value > 0.0 {
                return value;
            }
        }
    }
}

impl<R: RngCore + ?Sized> Iterator for RareErrorIterator<'_, R> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
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
                    let uniform = self.draw_open_unit_f64();
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
```

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: PASS. The output includes all six required test names and prints `PASS instruction-wide rare-error iterator`.

- [ ] **Step 6: Run formatter**

Run:

```sh
cargo fmt
```

Expected: PASS with no output.

- [ ] **Step 7: Re-run the focused test after formatting**

Run:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: PASS and the acceptance print remains visible.

- [ ] **Step 8: Commit the implementation**

Run:

```sh
git add rstim/src/lib.rs rstim/src/rare_error_iterator.rs rstim/tests/rare_error_iterator.rs
git commit -m "feat: add rare error iterator"
```

Expected: Commit succeeds with only the iterator implementation and focused test files staged.

### Task 2: Final Verification

**Files:**
- Verify: `rstim/src/rare_error_iterator.rs`
- Verify: `rstim/tests/rare_error_iterator.rs`
- Verify: `rstim/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 implementation and tests
- Produces: final verification evidence for the pull request

- [ ] **Step 1: Run the issue acceptance command**

Run:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

Expected: PASS and the output includes `PASS instruction-wide rare-error iterator`.

- [ ] **Step 2: Run the full workspace test command**

Run:

```sh
cargo test
```

Expected: PASS for the full workspace test suite. Existing warnings are acceptable only if they were present in the clean baseline and do not come from this change.

- [ ] **Step 3: Inspect the final diff**

Run:

```sh
git diff --stat master...HEAD
git diff --check
git status --short
```

Expected: The diff is limited to the design/plan docs plus `rstim/src/lib.rs`, `rstim/src/rare_error_iterator.rs`, and `rstim/tests/rare_error_iterator.rs`; `git diff --check` reports no whitespace errors; `git status --short` is clean after commits.

- [ ] **Step 4: Record verification outcome**

Add the focused command and full `cargo test` result to the final PR body. Do not merge the branch.

## Self-Review

- Spec coverage: Task 1 covers the iterator interface, boundary behavior, deterministic sparse skipping, telemetry, allocation independence, and all six required test names. Task 2 covers focused and full verification.
- Placeholder scan: no TBD, TODO, or deferred implementation steps remain.
- Type consistency: the plan uses one constructor function, `rare_error_indices`, one iterator type, `RareErrorIterator<'a, R>`, and one telemetry type, `RareErrorTelemetry`, consistently across implementation and tests.
