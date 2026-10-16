use std::fs;
use std::path::Path;

use rstim::cli;
use rstim::executor::Executor;
use rstim::ir::circuit_to_string;
use rstim::qp101::{export_qp101, export_qp101_with_sample_trace};
use rstim::showcase::mixed_noise_rotated_memory_x_d3_r3;

const SAMPLE_SEED: u64 = 7;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rstim crate should live under repo root");

    let instrs = mixed_noise_rotated_memory_x_d3_r3();
    let stim_text = circuit_to_string(&instrs);
    let base = export_qp101(&instrs).expect("showcase base export should succeed");

    let mut ex = Executor::from_instrs(instrs.clone()).expect("showcase should execute");
    let mut rng = cli::make_rng(Some(SAMPLE_SEED));
    let (_out, trace) = ex
        .run_with_trace(&mut rng)
        .expect("showcase sample trace should succeed");
    let sample = export_qp101_with_sample_trace(&instrs, &trace)
        .expect("showcase sample export should succeed");

    write(
        &root.join("qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim"),
        &stim_text,
    );
    write_json(
        &root.join("qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.qp101.json"),
        &base,
    );
    write_json(
        &root.join(
            "qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss-sample.qp101.json",
        ),
        &sample,
    );
    write_json(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise.json",
        ),
        &base,
    );
    write_json(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json",
        ),
        &sample,
    );
}

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap_or_else(|err| {
        panic!("failed to write {}: {err}", path.display());
    });
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    let text = serde_json::to_string_pretty(value).expect("json serialization should succeed");
    write(path, &(text + "\n"));
}
