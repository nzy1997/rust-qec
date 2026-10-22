use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::numeric::{NumericError, NumericOptions, approximate_real, approximate_sqrt};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct AllocationProbe;
thread_local! {
    // Only the calling test thread is measured, after constructing its inputs.
    static LARGEST: Cell<Option<usize>> = const { Cell::new(None) };
}
fn record(size: usize) {
    let _ = LARGEST.try_with(|largest| {
        if let Some(previous) = largest.get() {
            largest.set(Some(previous.max(size)));
        }
    });
}
// SAFETY: every operation forwards the original pointer/layout to System.
// The counter observes requested sizes without allocating or touching payloads.
unsafe impl GlobalAlloc for AllocationProbe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: AllocationProbe = AllocationProbe;

fn measured(
    f: impl FnOnce() -> Result<rmagna::numeric::RoundedReal, NumericError>,
) -> (Result<rmagna::numeric::RoundedReal, NumericError>, usize) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            LARGEST.with(|largest| largest.set(None));
        }
    }
    LARGEST.with(|largest| assert_eq!(largest.replace(Some(0)), None));
    let _stop = Stop;
    let result = f();
    let largest = LARGEST.with(|largest| largest.replace(None).unwrap());
    (result, largest)
}

#[test]
fn oversized_coefficients_are_rejected_without_copying_their_integer_storage() {
    let f = FieldArithmetic::new(2_000_000);
    let big = BigInt::one() << 1_000_000usize;
    let mut values: Vec<(ExactField, NumericError)> = Vec::new();
    for b in [
        BigRational::from_integer(big.clone()),
        BigRational::from_integer(-&big),
        BigRational::new(BigInt::one(), &big + 1),
        BigRational::new(-BigInt::one(), &big + 1),
    ] {
        let x = f
            .value([BigRational::zero(), b.clone(), BigRational::zero(), -b])
            .unwrap();
        values.push((
            x,
            NumericError::ResourceLimit {
                limit: 64,
                required: 1_000_001,
            },
        ));
    }
    // Nonreal inputs retain their domain error, without cloning oversized d.
    for b in [BigRational::one(), BigRational::zero()] {
        values.push((
            f.value([
                BigRational::zero(),
                b,
                BigRational::zero(),
                BigRational::from_integer(big.clone()),
            ])
            .unwrap(),
            NumericError::NonReal,
        ));
    }
    for (value, expected) in values {
        for sqrt in [false, true] {
            let (result, largest) = measured(|| {
                if sqrt {
                    approximate_sqrt(&value, NumericOptions { max_work_bits: 64 })
                } else {
                    approximate_real(&value, NumericOptions { max_work_bits: 64 })
                }
            });
            assert_eq!(result, Err(expected.clone()));
            // This permits small bookkeeping allocations; a coefficient copy
            // would request at least 125,000 bytes for the literal witness.
            assert!(
                largest <= 512,
                "oversized coefficient allocation: {largest}"
            );
        }
    }
}
