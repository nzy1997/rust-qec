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
    assert!(
        !indices.is_empty(),
        "seeded sparse run should produce events"
    );

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
