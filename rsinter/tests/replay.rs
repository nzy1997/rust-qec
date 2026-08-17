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
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rmatching-runner")]
fn replay_rejects_lexically_aliased_output_paths() {
    let temp = tempfile::tempdir().unwrap();
    let mut prediction_options = options(&temp);
    fs::write(&prediction_options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&prediction_options.dets, [0u8]).unwrap();
    prediction_options.stats_out = prediction_options
        .predictions_out
        .parent()
        .unwrap()
        .join(".")
        .join("predictions.b8");

    let error = run_replay(&prediction_options).unwrap_err();

    assert!(error.contains("must use different paths"), "{error}");
    assert!(!prediction_options.predictions_out.exists());
}

#[test]
fn replay_reports_invalid_dem_detector_and_config_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = options(&temp);
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("failed to read DEM")
    );

    fs::write(&options.dem, [0xff]).unwrap();
    assert!(run_replay(&options).unwrap_err().contains("is not UTF-8"));

    fs::write(&options.dem, "error(nope) D0\n").unwrap();
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("failed to parse DEM")
    );

    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("failed to open detectors")
    );

    fs::write(&options.dets, [0u8]).unwrap();
    options.decoder_config = Some(temp.path().join("missing.toml"));
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("failed to read decoder config")
    );

    options.decoder_config = None;
    options.batch_size = 0;
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("batch_size must be positive")
    );
}

#[test]
#[cfg(feature = "rbposd-runner")]
fn replay_validates_zero_detector_and_multibyte_row_lengths() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = options(&temp);
    fs::write(&options.dem, "error(0.1) D8 L0\n").unwrap();
    fs::write(&options.dets, [0u8]).unwrap();
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("not divisible by row width 2")
    );

    fs::write(&options.dem, "").unwrap();
    fs::write(&options.dets, []).unwrap();
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("--shots is required")
    );
    fs::write(&options.dets, [0u8]).unwrap();
    options.shots = Some(1);
    assert!(
        run_replay(&options)
            .unwrap_err()
            .contains("requires an empty detector input")
    );

    fs::write(&options.dets, []).unwrap();
    options.decoder = "rbposd".into();
    options.shots = Some(2);
    let stats = run_replay(&options).unwrap();
    assert_eq!(stats.num_shots, 2);
    assert_eq!(stats.num_detectors, 0);
    assert_eq!(stats.num_observables, 0);
    assert_eq!(stats.batches, 1);
    assert!(fs::read(&options.predictions_out).unwrap().is_empty());
}

#[test]
#[cfg(feature = "rmatching-runner")]
fn replay_reports_output_directory_creation_errors() {
    let temp = tempfile::tempdir().unwrap();
    let mut prediction_options = options(&temp);
    fs::write(&prediction_options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&prediction_options.dets, [0u8]).unwrap();
    let parent_file = temp.path().join("not-a-directory");
    fs::write(&parent_file, b"file").unwrap();
    prediction_options.predictions_out = parent_file.join("predictions.b8");

    let error = run_replay(&prediction_options).unwrap_err();

    assert!(
        error.contains("failed to create output directory"),
        "{error}"
    );

    let mut stats_options = options(&temp);
    stats_options.stats_out = parent_file.join("stats.json");
    let error = run_replay(&stats_options).unwrap_err();
    assert!(
        error.contains("failed to create output directory"),
        "{error}"
    );
}

#[test]
#[cfg(feature = "rmatching-runner")]
fn replay_accepts_an_inferred_zero_shot_dataset() {
    let temp = tempfile::tempdir().unwrap();
    let options = options(&temp);
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, []).unwrap();

    let stats = run_replay(&options).unwrap();

    assert_eq!(stats.num_shots, 0);
    assert_eq!(stats.batches, 0);
    assert_eq!(stats.shots_per_second, 0.0);
}

#[test]
#[cfg(feature = "rmatching-runner")]
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
#[cfg(feature = "rbposd-runner")]
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

#[test]
#[cfg(feature = "rmatching-runner")]
fn replay_command_runs_the_public_cli_path() {
    let temp = tempfile::tempdir().unwrap();
    let dem = temp.path().join("model.dem");
    let dets = temp.path().join("detectors.b8");
    let predictions = temp.path().join("predictions.b8");
    let stats = temp.path().join("stats.json");
    fs::write(&dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&dets, [1u8]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "replay",
            "--dem",
            dem.to_str().unwrap(),
            "--dets",
            dets.to_str().unwrap(),
            "--decoder",
            "rmatching",
            "--predictions-out",
            predictions.to_str().unwrap(),
            "--stats-out",
            stats.to_str().unwrap(),
            "--batch-size",
            "1",
            "--shots",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(predictions).unwrap(), [1]);
    assert!(stats.exists());
}

#[cfg(all(unix, feature = "rmatching-runner"))]
#[test]
fn replay_rejects_outputs_aliased_through_a_symlinked_parent() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let real = temp.path().join("real");
    let alias = temp.path().join("alias");
    fs::create_dir(&real).unwrap();
    symlink(&real, &alias).unwrap();
    let mut options = options(&temp);
    options.predictions_out = real.join("result");
    options.stats_out = alias.join("result");
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0u8]).unwrap();

    let error = run_replay(&options).unwrap_err();

    assert!(error.contains("must use different paths"), "{error}");
    assert!(!options.predictions_out.exists());
}

#[cfg(all(unix, feature = "rmatching-runner"))]
#[test]
fn replay_rejects_output_alias_with_parent_after_symlink() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base");
    let other = temp.path().join("other");
    let nested = other.join("nested");
    fs::create_dir(&base).unwrap();
    fs::create_dir_all(&nested).unwrap();
    symlink(&nested, base.join("link")).unwrap();
    let mut options = options(&temp);
    options.predictions_out = base.join("link").join("..").join("result");
    options.stats_out = other.join("result");
    fs::write(&options.dem, "error(0.1) D0 L0\n").unwrap();
    fs::write(&options.dets, [0u8]).unwrap();

    let error = run_replay(&options).unwrap_err();

    assert!(error.contains("must use different paths"), "{error}");
    assert!(!options.stats_out.exists());
}

#[cfg(feature = "rmatching-runner")]
#[test]
fn replay_rmatching_rejects_unrepresentable_observable_semantics() {
    for (dem, expected) in [
        ("error(0.1) D0 L64\n", "at most 64 observables"),
        ("error(1) L0\n", "observable-only DEM error components"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut options = options(&temp);
        fs::write(&options.dem, dem).unwrap();
        if dem.contains("D0") {
            fs::write(&options.dets, [0u8]).unwrap();
        } else {
            fs::write(&options.dets, []).unwrap();
            options.shots = Some(1);
        }

        let error = run_replay(&options).unwrap_err();

        assert!(error.contains(expected), "{error}");
        assert!(!options.predictions_out.exists());
        assert!(!options.stats_out.exists());
    }
}
