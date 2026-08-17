#![cfg(feature = "rmatching-runner")]

use std::fs;
use std::process::Command;

use rsinter::replay::{ReplayOptions, run_replay};

fn options(temp: &tempfile::TempDir) -> ReplayOptions {
    ReplayOptions {
        dem: temp.path().join("model.dem"),
        dets: temp.path().join("detectors.b8"),
        decoder: "rmatching".into(),
        decoder_config: None,
        predictions_out: temp.path().join("predictions.b8"),
        stats_out: temp.path().join("stats.json"),
        batch_size: 2,
        shots: None,
    }
}

#[test]
fn replay_rmatching_writes_predictions_and_stats() {
    let temp = tempfile::tempdir().unwrap();
    let options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0u8, 1, 0]).unwrap();

    let stats = run_replay(&options).unwrap();

    assert_eq!(fs::read(&options.predictions_out).unwrap(), [0, 1, 0]);
    assert_eq!(stats.num_shots, 3);
    assert_eq!(stats.num_detectors, 1);
    assert_eq!(stats.num_observables, 1);
    assert_eq!(stats.batches, 2);
    assert_eq!(stats.detector_bytes, 3);
    assert_eq!(stats.prediction_bytes, 3);
    assert_eq!(stats.dem_sha256.len(), 64);
    assert_eq!(stats.detectors_sha256.len(), 64);
    assert_eq!(stats.predictions_sha256.len(), 64);

    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(&options.stats_out).unwrap()).unwrap();
    assert_eq!(persisted["schema_version"], 1);
    assert_eq!(persisted["decoder"], "rmatching");
    assert_eq!(persisted["num_shots"], 3);
}

#[test]
fn replay_rejects_nonzero_padding_without_replacing_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0x80]).unwrap();
    fs::write(&options.predictions_out, b"old predictions").unwrap();
    fs::write(&options.stats_out, b"old stats").unwrap();

    let error = run_replay(&options).unwrap_err();

    assert!(error.contains("non-zero b8 padding bits"), "{error}");
    assert_eq!(
        fs::read(&options.predictions_out).unwrap(),
        b"old predictions"
    );
    assert_eq!(fs::read(&options.stats_out).unwrap(), b"old stats");
}

#[test]
fn replay_validates_shot_count_and_decoder_config() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0u8, 1]).unwrap();
    options.shots = Some(3);
    let error = run_replay(&options).unwrap_err();
    assert!(error.contains("does not match 2 rows"), "{error}");

    options.shots = None;
    let config = temp.path().join("decoder.toml");
    fs::write(&config, "osd_order = 1\n").unwrap();
    options.decoder_config = Some(config);
    let error = run_replay(&options).unwrap_err();
    assert!(error.contains("unknown field `osd_order`"), "{error}");
}

#[test]
fn replay_rejects_lexically_aliased_output_paths() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0u8]).unwrap();
    options.stats_out = options
        .predictions_out
        .parent()
        .unwrap()
        .join(".")
        .join("predictions.b8");

    let error = run_replay(&options).unwrap_err();

    assert!(error.contains("must use different paths"), "{error}");
    assert!(!options.predictions_out.exists());
}

#[test]
fn replay_surfaces_rmatching_hyperedge_error() {
    let temp = tempfile::tempdir().unwrap();
    let options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 D1 D2 L0\n").unwrap();
    fs::write(&options.dets, [0u8]).unwrap();

    let error = run_replay(&options).unwrap_err();

    assert!(error.contains("requires a graphlike DEM"), "{error}");
    assert!(!options.predictions_out.exists());
    assert!(!options.stats_out.exists());
}

#[test]
fn replay_rbposd_families_accept_non_graphlike_dems() {
    for decoder in ["rbposd", "rbplsd"] {
        let temp = tempfile::tempdir().unwrap();
        let mut options = options(&temp);
        options.decoder = decoder.into();
        fs::write(&options.dem, "error(0.1) D0 D1 D2 L0\n").unwrap();
        fs::write(&options.dets, [0u8]).unwrap();

        let stats = run_replay(&options).unwrap();

        assert_eq!(stats.decoder, decoder);
        assert_eq!(stats.num_detectors, 3);
        assert_eq!(fs::read(&options.predictions_out).unwrap().len(), 1);
    }
}

#[test]
fn replay_command_is_documented_in_cli_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args(["replay", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for option in [
        "--dem",
        "--dets",
        "--decoder",
        "--decoder-config",
        "--predictions-out",
        "--stats-out",
        "--batch-size",
        "--shots",
    ] {
        assert!(stdout.contains(option), "missing {option} in {stdout}");
    }
}
