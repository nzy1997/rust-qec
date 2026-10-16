//! Resource regression: invalid borrowed rows must not be cloned before rejection.
//! Only malformed inputs are passed to synthesis; no application code is encoded.
use qec_code::Pauli;
use qec_code::phased_pauli::{Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup};
use qec_code::signed_encoder::{EncoderError, EncoderLimits, synthesize_signed_encoder};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    // Const TLS avoids allocation in the probe. Other test-harness threads
    // cannot contaminate this thread's measured call.
    static LARGEST: Cell<Option<usize>> = const { Cell::new(None) };
}

struct Probe;

fn record(size: usize) {
    let _ = LARGEST.try_with(|cell| {
        if let Some(previous) = cell.get() {
            cell.set(Some(previous.max(size)));
        }
    });
}

// This test-only allocator forwards each operation unchanged to System.
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Probe = Probe;

fn axis(q: usize, x_axis: bool) -> PhasedPauli {
    let mut x = vec![0; 2];
    let mut z = vec![0; 2];
    if x_axis {
        x[q] = 1;
    } else {
        z[q] = 1;
    }
    PhasedPauli::new(Pauli::from_xz_bits(x, z).unwrap(), Phase::PlusOne)
}

#[test]
fn every_borrowed_logical_width_is_rejected_before_any_in_call_allocation() {
    for kind in ["X", "Z"] {
        for index in 0..2 {
            let group = SignedStabilizerGroup::new(2, vec![]).unwrap();
            let mut xs = vec![axis(0, true), axis(1, true)];
            let mut zs = vec![axis(0, false), axis(1, false)];
            if kind == "X" {
                xs[index] = PhasedPauli::identity(65_536);
            } else {
                zs[index] = PhasedPauli::identity(65_536);
            }
            LARGEST.with(|cell| cell.set(Some(0)));
            let result =
                synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits { max_gates: 0 });
            let largest = LARGEST.with(|cell| cell.replace(None)).unwrap();
            assert!(matches!(result,
                Err(EncoderError::InvalidFrame(PhaseAlgebraError::LogicalWidth {
                    kind: actual_kind, index: actual_index, expected: 2, actual: 65_536,
                })) if actual_kind == kind && actual_index == index
            ));
            assert_eq!(largest, 0, "reject {kind}[{index}] before copying supports");
        }
    }
}
