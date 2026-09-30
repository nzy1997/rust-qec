use std::fs;
use std::process::Command;

fn rstim() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn circuit_decode_uses_rmatching_on_detector_bits_only() {
    let root = tempfile::tempdir().unwrap();
    let dem = root.path().join("model.dem");
    let rows = root.path().join("detectors.01");
    let predictions = root.path().join("predictions.01");
    fs::write(&dem, "error(0.1) D0 D1\nerror(0.1) D1 L0\n").unwrap();
    fs::write(&rows, "00\n11\n01\n").unwrap();

    let output = rstim()
        .args([
            "circuit",
            "decode",
            "--decoder",
            "rmatching",
            "--dem",
            dem.to_str().unwrap(),
            "--in",
            rows.to_str().unwrap(),
            "--out",
            predictions.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "circuit.decode");
    assert_eq!(value["result"]["decoder"], "rmatching");
    assert_eq!(value["result"]["shots"], 3);
    assert_eq!(value["result"]["num_detectors"], 2);
    assert_eq!(value["result"]["num_observables"], 1);
    assert_eq!(fs::read_to_string(&predictions).unwrap(), "0\n0\n1\n");

    fs::write(&rows, "110\n").unwrap();
    let output = rstim()
        .args([
            "circuit",
            "decode",
            "--decoder",
            "rmatching",
            "--dem",
            dem.to_str().unwrap(),
            "--in",
            rows.to_str().unwrap(),
            "--out",
            predictions.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "circuit.decode");
    assert_eq!(error["error"]["code"], "invalid_detector_row");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("observable bits must not be included")
    );
    assert_eq!(fs::read_to_string(&predictions).unwrap(), "0\n0\n1\n");
}

#[test]
fn circuit_decode_requires_an_explicit_decoder() {
    let output = rstim().args(["circuit", "decode"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--decoder"));
}

#[test]
fn circuit_decode_rejects_hyperedges_without_writing_predictions() {
    let root = tempfile::tempdir().unwrap();
    let dem = root.path().join("hyperedge.dem");
    let rows = root.path().join("detectors.01");
    let predictions = root.path().join("predictions.01");
    fs::write(&dem, "error(0.1) D0 D1 D2\n").unwrap();
    fs::write(&rows, "000\n").unwrap();

    let output = rstim()
        .args([
            "circuit",
            "decode",
            "--decoder",
            "rmatching",
            "--dem",
            dem.to_str().unwrap(),
            "--in",
            rows.to_str().unwrap(),
            "--out",
            predictions.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "circuit.decode");
    assert_eq!(error["error"]["code"], "invalid_dem");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("requires a graphlike DEM")
    );
    assert!(!predictions.exists());
}

#[test]
fn capabilities_advertises_direct_rmatching_decode() {
    let output = rstim()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let capability = value["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "circuit.decode")
        .unwrap();
    assert_eq!(capability["argv"], serde_json::json!(["circuit", "decode"]));
    assert_eq!(capability["decoders"], serde_json::json!(["rmatching"]));
    assert!(
        capability["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| {
                arg["name"] == "decoder"
                    && arg["flag"] == "--decoder"
                    && arg["required"] == true
                    && arg["values"] == serde_json::json!(["rmatching"])
            })
    );
}
