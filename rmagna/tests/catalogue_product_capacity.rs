//! Run the enormous logical boundary in an isolated process with bounded allocation.
use rmagna::catalogue::{
    CatalogueLimits, CliffordIndicator, PhaseCatalogue, PhaseDecompositionInput,
    builtin_phase_catalogue,
};
use rmagna::phase::ParityPhase;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, Ordering};

struct GuardedAllocator;
static GUARD_ACTIVE: AtomicBool = AtomicBool::new(false);
const MAX_ALLOCATION: usize = 1 << 20;

// This integration-test binary contains only this regression. Activate the guard
// after builtin initialization and isolate aborts from the parent test process.
unsafe impl GlobalAlloc for GuardedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if GUARD_ACTIVE.load(Ordering::Relaxed) && layout.size() > MAX_ALLOCATION {
            std::process::abort();
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: GuardedAllocator = GuardedAllocator;

#[test]
fn impossible_table_is_rejected_before_cartesian_allocation() {
    const CHILD_FLAG: &str = "RMAGNA_PRODUCT_CAPACITY_CHILD";
    if std::env::var_os(CHILD_FLAG).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "impossible_table_is_rejected_before_cartesian_allocation",
            ])
            .env(CHILD_FLAG, "1")
            .output()
            .unwrap();
        assert!(result.status.success(), "child failed: {result:?}");
        return;
    }
    let source = builtin_phase_catalogue().unwrap();
    let before = source.usage();
    let dimension = (usize::BITS - 1) as usize;
    assert!(Layout::array::<u8>(1_usize << dimension).is_err());
    let limits = CatalogueLimits {
        max_dimension: usize::MAX,
        max_table_values: usize::MAX,
        max_entries: usize::MAX,
        max_terms: usize::MAX,
        max_constraint_rows: usize::MAX,
        max_parity_rows: usize::MAX,
        max_verification_work: u128::MAX,
        ..Default::default()
    };
    GUARD_ACTIVE.store(true, Ordering::Relaxed);
    let result = source.paired_t_product(dimension, &limits, &ScalarArithmetic::default());
    GUARD_ACTIVE.store(false, Ordering::Relaxed);
    let error = result.unwrap_err();
    assert_eq!(error.field, "catalogue.product.phase_table_capacity");
    assert_eq!(
        error.message,
        format!(
            "needs at least {}; configured limit is {}",
            1_usize << dimension,
            isize::MAX
        ),
    );
    assert_eq!(source.usage(), before);

    // An eight-term pair makes the indicator vector unrepresentable while the
    // phase table, logical counts and u128 work still fit their platform limits.
    let pair = source.get(2, &[0, 1, 1, 2]).unwrap();
    let mut terms = pair.terms().to_vec();
    terms.extend((0..6).map(|_| CliffordIndicator {
        coefficient: ExactScalar::zero(),
        constraints: vec![],
        phase: ParityPhase::new(2, 0, []).unwrap(),
    }));
    let mut extended = PhaseCatalogue::new();
    extended
        .insert(
            PhaseDecompositionInput {
                dimension: 2,
                phase_table: vec![0, 1, 1, 2],
                terms,
            },
            &CatalogueLimits::default(),
            &ScalarArithmetic::default(),
        )
        .unwrap();
    let pairs = (1_u32..)
        .find(|n| {
            8_usize
                .checked_pow(*n)
                .is_some_and(|count| Layout::array::<CliffordIndicator>(count).is_err())
        })
        .unwrap();
    let count = 8_usize.pow(pairs);
    let dimension = 2 * pairs as usize;
    assert!(Layout::array::<u8>(1_usize << dimension).is_ok());
    let before = extended.usage();
    GUARD_ACTIVE.store(true, Ordering::Relaxed);
    let result = extended.paired_t_product(dimension, &limits, &ScalarArithmetic::default());
    GUARD_ACTIVE.store(false, Ordering::Relaxed);
    let error = result.unwrap_err();
    assert_eq!(error.field, "catalogue.product.terms_capacity");
    assert!(
        error
            .message
            .starts_with(&format!("needs at least {count};"))
    );
    assert_eq!(extended.usage(), before);
}
