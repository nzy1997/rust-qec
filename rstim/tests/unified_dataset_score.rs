use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

struct Fixture {
    dir: tempfile::TempDir,
    public: PathBuf,
    private: PathBuf,
    predictions: PathBuf,
}

impl Fixture {
    fn new(observables: usize, shots: usize) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let public = dir.path().join("public");
        let private = dir.path().join("private");
        let predictions = dir.path().join("predictions.b8");
        let circuit = dir.path().join("circuit.stim");
        let mut text = String::from("R 0\nM 0\nDETECTOR rec[-1]\n");
        for observable in 0..observables {
            text.push_str(&format!("OBSERVABLE_INCLUDE({observable}) rec[-1]\n"));
        }
        fs::write(&circuit, text).unwrap();
        let output = command()
            .args([
                "dataset",
                "export",
                "--mode",
                "detectors",
                "--shots",
                &shots.to_string(),
                "--seed",
                "7",
            ])
            .arg("--circuit")
            .arg(circuit)
            .arg("--public-out")
            .arg(&public)
            .arg("--private-out")
            .arg(&private)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::copy(private.join("answers.b8"), &predictions).unwrap();
        Self {
            dir,
            public,
            private,
            predictions,
        }
    }

    fn score(&self) -> Command {
        let mut cmd = command();
        cmd.args(["dataset", "score"])
            .arg("--dataset")
            .arg(&self.public)
            .arg("--private")
            .arg(&self.private)
            .arg("--predictions")
            .arg(&self.predictions);
        cmd
    }

    fn stats(&self) -> PathBuf {
        let path = self.dir.path().join("decode-stats.json");
        let public = read_json(&self.public.join("manifest.json"));
        write_json(
            &path,
            &json!({
                "schema_version": "rustqec.decode-stats.v1",
                "circuit_sha256": public["circuit"]["sha256"],
                "shot_count": public["shots"],
                "attempted_shot_count": public["shots"],
                "timeout_count": 0,
                "infeasible_shot_count": 0,
                "distinct_loss_patterns": 1,
                "distinct_loss_patterns_exact": true,
            }),
        );
        path
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn result(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "dataset.score");
    assert_eq!(value["schema_version"], "rustqec.cli.v1");
    assert_eq!(value["status"], "ok");
    value["result"].clone()
}

fn assert_rejected(mut cmd: Command, code: &str) {
    let output = cmd.args(["--error-format", "json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "must not report a score before validating all rows"
    );
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["command"], "dataset.score");
    assert_eq!(value["error"]["code"], code, "{value}");
}

#[test]
fn multiple_wrong_observables_count_as_one_logical_error_per_shot() {
    let fixture = Fixture::new(10, 3);
    fs::write(&fixture.predictions, [3, 1, 0, 0, 0, 1]).unwrap();
    let scored = result(fixture.score().args(["--format", "json"]).output().unwrap());
    assert_eq!(scored["shot_count"], 3);
    assert_eq!(scored["observables"], 10);
    assert_eq!(scored["logical_errors"], 2);
    assert_eq!(scored["logical_error_rate"], 2.0 / 3.0);
    assert!(scored.get("distinct_loss_patterns").is_none());
    let output = fixture.score().output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Scored shots: 3\nLogical errors: 2 / 3\n"
    );
}

#[test]
fn scoring_streams_across_blocks_and_handles_empty_bundles() {
    let fixture = Fixture::new(1, 1025);
    let mut predictions = fs::read(&fixture.predictions).unwrap();
    predictions[1024] ^= 1;
    fs::write(&fixture.predictions, predictions).unwrap();
    let scored = result(fixture.score().args(["--format", "json"]).output().unwrap());
    assert_eq!(scored["logical_errors"], 1);
    assert_eq!(scored["shot_count"], 1025);
    let empty = Fixture::new(1, 1);
    // The exporter requires positive shots, but scoring can read an empty
    // bundle whose public material and identity consistently declare zero.
    fs::write(&empty.predictions, []).unwrap();
    fs::write(empty.public.join("shots.b8"), []).unwrap();
    fs::write(empty.private.join("answers.b8"), []).unwrap();
    let mut public = read_json(&empty.public.join("manifest.json"));
    public["shots"] = json!(0);
    public["shots_file"]["sha256"] = json!(format!("{:x}", Sha256::digest([])));
    let id = format!(
        "{:x}",
        Sha256::digest(rstim::decoder_dataset::dataset_id_material(
            1,
            rstim::decoder_dataset::DecoderDatasetMode::Detectors,
            public["circuit"]["sha256"].as_str().unwrap(),
            0,
            public["row"]["bits"].as_u64().unwrap() as usize,
            public["shots_file"]["sha256"].as_str().unwrap(),
        ))
    );
    public["dataset_id"] = json!(id);
    write_json(&empty.public.join("manifest.json"), &public);
    let mut private = read_json(&empty.private.join("manifest.json"));
    private["shots"] = json!(0);
    private["dataset_id"] = public["dataset_id"].clone();
    private["answers_file"]["sha256"] = public["shots_file"]["sha256"].clone();
    write_json(&empty.private.join("manifest.json"), &private);
    let scored = result(empty.score().args(["--format", "json"]).output().unwrap());
    assert_eq!(scored["shot_count"], 0);
    assert_eq!(scored["logical_errors"], 0);
    assert!(scored["logical_error_rate"].is_null());
}

#[test]
fn predictions_must_have_complete_rows_and_zero_padding() {
    let fixture = Fixture::new(1, 3);
    for bad in [vec![0, 0], vec![0, 0, 0, 0], vec![0, 0, 128]] {
        fs::write(&fixture.predictions, bad).unwrap();
        assert_rejected(fixture.score(), "invalid_predictions");
    }
}

#[test]
fn scoring_rejects_mixed_bundles_and_tampered_answers() {
    let fixture = Fixture::new(1, 3);
    let manifest_path = fixture.private.join("manifest.json");
    let original = read_json(&manifest_path);
    for (key, value) in [
        ("dataset_id", json!("unrelated")),
        ("shots", json!(2)),
        ("mode", json!("measurements_blinded")),
        ("schema_version", json!(2)),
        ("format", json!("other")),
    ] {
        let mut changed = original.clone();
        changed[key] = value;
        write_json(&manifest_path, &changed);
        assert_rejected(fixture.score(), "invalid_dataset");
    }
    write_json(&manifest_path, &original);
    let answers_path = fixture.private.join("answers.b8");
    fs::write(&answers_path, [1, 0, 0]).unwrap();
    assert_rejected(fixture.score(), "invalid_dataset");
    // Even a payload with an updated checksum must satisfy b8 padding rules.
    let bad_answers = [0, 0, 128];
    fs::write(&answers_path, bad_answers).unwrap();
    let mut manifest = original;
    manifest["answers_file"]["sha256"] = json!(format!("{:x}", Sha256::digest(bad_answers)));
    write_json(&manifest_path, &manifest);
    assert_rejected(fixture.score(), "invalid_dataset");
}

#[test]
fn provided_decode_stats_must_describe_a_matching_successful_run() {
    let fixture = Fixture::new(1, 3);
    let stats_path = fixture.stats();
    let original = read_json(&stats_path);
    for (key, value) in [
        ("schema_version", json!("unknown")),
        ("circuit_sha256", json!("unrelated")),
        ("shot_count", json!(2)),
        ("attempted_shot_count", json!(2)),
        ("timeout_count", json!(1)),
        ("infeasible_shot_count", json!(1)),
    ] {
        let mut changed = original.clone();
        changed[key] = value;
        write_json(&stats_path, &changed);
        let mut cmd = fixture.score();
        cmd.arg("--decode-stats").arg(&stats_path);
        assert_rejected(cmd, "invalid_decode_stats");
    }
    write_json(&stats_path, &original);
    let output = fixture
        .score()
        .arg("--decode-stats")
        .arg(&stats_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Scored shots: 3\nLogical errors: 0 / 3\n"
    );
    let mut estimated = original;
    estimated["distinct_loss_patterns_exact"] = json!(false);
    write_json(&stats_path, &estimated);
    let output = fixture
        .score()
        .arg("--decode-stats")
        .arg(&stats_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Scored shots: 3\nLogical errors: 0 / 3\n"
    );
}

#[test]
fn scoring_is_discoverable_and_parse_errors_use_its_contract() {
    let output = command()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    let capabilities: Value = serde_json::from_slice(&output.stdout).unwrap();
    let score = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "dataset.score")
        .unwrap();
    assert_eq!(score["argv"], json!(["dataset", "score"]));
    let mut cmd = command();
    cmd.args(["dataset", "score", "--format", "json"]);
    assert_rejected(cmd, "invalid_arguments");
}
