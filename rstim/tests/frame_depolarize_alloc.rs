use rand::rngs::StdRng;
use rand::SeedableRng;
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
        (22_500..=25_000).contains(&dep2_flips),
        "DEPOLARIZE2(0.3) should flip about 18.7% of measured qubit results; got {dep2_flips}"
    );
}
