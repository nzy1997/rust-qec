use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn microbench_accepts_request_and_emits_predictions() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rmatching_microbench"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("microbenchmark binary should start");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(
            br#"{"dem":"error(0.1) D0 D1\nerror(0.1) D2 D3\nerror(0.1) D0 D2\nerror(0.1) D1 D3\nerror(0.1) D0 D3 L0\nerror(0.05) D0\nerror(0.05) D1\nerror(0.05) D2\nerror(0.05) D3\n","syndromes":[[1,0,0,1],[1,1,0,0]],"warmup_rounds":0,"measure_rounds":1}"#,
        )
        .expect("request should be written");

    let output = child.wait_with_output().expect("binary should finish");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(response["predictions"], serde_json::json!([[1], [0]]));
    assert_eq!(response["decode_latencies_us"].as_array().unwrap().len(), 1);
}

#[test]
fn pipeline_benchmark_runs_a_stim_file_and_keeps_csv_contract() {
    let dir = tempfile::tempdir().expect("temporary directory should be created");
    let circuit = dir
        .path()
        .join("surface_code_rotated_memory_x_5_0.001.stim");
    std::fs::write(
        &circuit,
        "X_ERROR(0.001) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("circuit should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_rmatching_bench"))
        .arg(circuit)
        .arg("4")
        .output()
        .expect("pipeline benchmark should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let fields: Vec<_> = stdout.trim().split(',').collect();
    assert_eq!(fields.len(), 5, "{stdout}");
    assert_eq!(&fields[..3], ["rmatching", "0.001", "5"]);
    assert!(fields[3].parse::<f64>().is_ok());
    assert!(fields[4].parse::<f64>().is_ok());
}
