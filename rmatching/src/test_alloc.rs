use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct CountingAlloc;

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// Test-only global allocator used to assert specific operations stay allocation-free.
#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAlloc = CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|counting| {
            if counting.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|counting| {
            if counting.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|counting| {
            if counting.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

pub fn reset_allocation_count() {
    ALLOCATIONS.with(|allocations| allocations.set(0));
    COUNT_ALLOCATIONS.with(|counting| counting.set(true));
}

pub fn allocation_count() -> usize {
    let count = ALLOCATIONS.with(|allocations| allocations.get());
    COUNT_ALLOCATIONS.with(|counting| counting.set(false));
    count
}
