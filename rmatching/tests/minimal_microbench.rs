#[test]
#[ignore = "requires pymatching Python package and release bench binary"]
fn minimal_microbenchmark_smoke() {
    let status = std::process::Command::new("python3")
        .arg("benchmarks/run_minimal_benchmark.py")
        .arg("--warmup-rounds")
        .arg("2")
        .arg("--measure-rounds")
        .arg("5")
        .status()
        .expect("failed to run minimal benchmark driver");
    assert!(status.success(), "minimal benchmark driver failed");
}
