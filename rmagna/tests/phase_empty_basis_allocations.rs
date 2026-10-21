//! Phase pullbacks must keep scalar and dense-basis allocations bounded.
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

#[test]
fn quadratic_pullbacks_do_not_materialize_dense_coordinate_lists() {
    use rmagna::quadratic::QuadraticForm;
    for width in [257, 513, 1025] {
        let mut linear = vec![0; width];
        linear[0] = 1;
        let phase = QuadraticForm::new(
            3,
            linear,
            (0..width)
                .map(|_| BitVector::zeros(width).unwrap())
                .collect(),
        )
        .unwrap();
        let shift = BitVector::zeros(width).unwrap();
        for mode in 0..3 {
            let basis = (0..64)
                .map(|j| {
                    BitVector::from_indices(
                        width,
                        (0..width).filter(|&i| match mode {
                            0 => i == j,
                            1 => i % 2 == 0,
                            _ => true,
                        }),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            REQUESTED.with(|counter| assert_eq!(counter.replace(Some(0)), None));
            let result = phase.restrict(&shift, &basis);
            let requested = REQUESTED.with(|counter| counter.replace(None).unwrap());
            // Includes every allocation/reallocation in the query. The budget
            // scales with packed polar storage, rather than one usize per set bit.
            let budget = 32 * width * width.div_ceil(64) + 65536;
            assert!(
                requested <= budget,
                "width={width},mode={mode}: requested {requested} bytes, budget {budget}"
            );
            let result = result.unwrap();
            for selected in [vec![], vec![0], vec![1], vec![0, 1], (0..64).collect()] {
                let first = if mode == 0 {
                    selected.contains(&0)
                } else {
                    selected.len() % 2 != 0
                };
                let expected = (3 + u8::from(first)) & 3;
                assert_eq!(
                    result
                        .evaluate(&BitVector::from_indices(64, selected).unwrap())
                        .unwrap(),
                    expected
                );
            }
        }
    }
}
