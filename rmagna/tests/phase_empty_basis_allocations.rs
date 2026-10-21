//! Scalar pullbacks must not allocate coordinate-sized transpose rows.
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    // Constant TLS initialization does not allocate. Only this test thread's
    // measured query is counted; fixture/harness allocations are excluded.
    static REQUESTED: Cell<Option<usize>> = const { Cell::new(None) };
}
struct Allocator;
fn count(bytes: usize) {
    let _ = REQUESTED.try_with(|counter| {
        if let Some(total) = counter.get() {
            counter.set(Some(total.saturating_add(bytes)));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn empty_basis_scalar_pullbacks_keep_query_allocation_bounded_across_term_threshold() {
    for width in [129, 65536, 1048576] {
        for count in [63, 64, 65] {
            let terms = (0..count).map(|j| {
                (
                    BitVector::from_indices(width, [j]).unwrap(),
                    (j % 7 + 1) as u8,
                )
            });
            let phase = ParityPhase::new(width, 5, terms).unwrap();
            assert_eq!(phase.terms().len(), count);
            for offset in [0, 1] {
                let shift =
                    BitVector::from_indices(width, (0..count).filter(|j| (j + offset) % 3 == 0))
                        .unwrap();
                let expected = (5
                    + (0..count)
                        .filter(|j| (j + offset) % 3 == 0)
                        .map(|j| j % 7 + 1)
                        .sum::<usize>())
                    % 8;
                REQUESTED.with(|counter| {
                    assert_eq!(counter.replace(Some(0)), None);
                });
                let result = phase.restrict(&shift, &[]);
                let requested = REQUESTED.with(|counter| counter.replace(None).unwrap());
                assert!(
                    requested <= 65536,
                    "width={width},terms={count}: scalar query requested {requested} bytes"
                );
                let result = result.unwrap();
                assert_eq!(result.num_variables(), 0);
                assert!(result.terms().is_empty());
                assert_eq!(
                    result.evaluate(&BitVector::zeros(0).unwrap()).unwrap(),
                    expected as u8
                );
            }
        }
    }
}
