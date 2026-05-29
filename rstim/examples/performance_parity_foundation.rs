use rstim::perf::{PerfRunOptions, run_benchmark_suite_to_writer};

fn main() {
    let mut out = std::io::stdout().lock();
    run_benchmark_suite_to_writer(&mut out, PerfRunOptions::default())
        .expect("run performance parity foundation");
}
