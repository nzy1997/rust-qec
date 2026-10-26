use rmagna::field::FieldArithmetic;
use rmagna::statistics::{CheckpointOptions, ExactStatistics, StatisticsOptions};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_ALLOCATION: usize = 16 << 20;
static GUARDED: AtomicBool = AtomicBool::new(false);

struct GuardedAllocator;
fn admit(size: usize) {
    if GUARDED.load(Ordering::Relaxed) && size > MAX_ALLOCATION {
        std::process::exit(86);
    }
}
unsafe impl GlobalAlloc for GuardedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        admit(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        admit(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        admit(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: GuardedAllocator = GuardedAllocator;

#[test]
fn malformed_triangular_checkpoint_rejects_before_accumulator_allocation() {
    const MODE: &str = "RMAGNA_CHECKPOINT_SHAPE_TEST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        if mode == "sentinel" {
            GUARDED.store(true, Ordering::Relaxed);
            let layout = Layout::from_size_align(MAX_ALLOCATION + 1, 1).unwrap();
            // The guard exits before delegating this negative-control request.
            std::hint::black_box(unsafe { ALLOCATOR.alloc(layout) });
            panic!("allocation guard did not reject the control request");
        }
        assert_eq!(mode, "restore");
        let text = ExactStatistics::new(1, StatisticsOptions::default())
            .unwrap()
            .checkpoint_json(CheckpointOptions::default())
            .unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&text).unwrap();
        wire["sums"] = serde_json::json!(vec![wire["sums"][0].clone(); 32768]);
        wire["products"] = serde_json::json!([]);
        let text = wire.to_string();
        GUARDED.store(true, Ordering::Relaxed);
        let result = ExactStatistics::from_checkpoint_json(
            &text,
            StatisticsOptions {
                max_dimensions: 32768,
                arithmetic: FieldArithmetic::default(),
            },
            CheckpointOptions::default(),
        );
        GUARDED.store(false, Ordering::Relaxed);
        let error = result.err().expect("malformed triangular shape accepted");
        assert_eq!(error.field, "products");
        assert!(error.message.contains("triangular product count"));
        return;
    }
    for (mode, code) in [("sentinel", 86), ("restore", 0)] {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "malformed_triangular_checkpoint_rejects_before_accumulator_allocation",
                "--nocapture",
            ])
            .env(MODE, mode)
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(code),
            "mode={mode}; stdout={}; stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
