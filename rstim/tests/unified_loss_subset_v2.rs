//! Public import/decode conformance for opt-in native loss circuits.
#![cfg(feature = "unified-cli")]

use std::fs;
use std::path::Path;
use std::process::Command;

use rstim::dem::DetectorErrorModel;
use rstim::showcase::dem_semantic_summary;
use serde_json::Value;

const NATIVE: &str = include_str!("fixtures/loss_visible_native_v2.stim");
const REFERENCE: &str = include_str!("fixtures/loss_visible_native_v2_reference.stim");

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn decoders() -> &'static [&'static str] {
    #[cfg(feature = "ilp")]
    return &["envelope-matching", "envelope-mle"];
    #[cfg(not(feature = "ilp"))]
    &["envelope-matching"]
}

fn import(root: &Path, text: &str, shots: &[u8], subset: Option<&str>) -> std::process::Output {
    fs::write(root.join("circuit.stim"), text).unwrap();
    fs::write(root.join("shots.b8"), shots).unwrap();
    let mut command = cli();
    command
        .args(["dataset", "import", "--shots-format", "b8", "--circuit"])
        .arg(root.join("circuit.stim"))
        .arg("--shots")
        .arg(root.join("shots.b8"))
        .arg("--out")
        .arg(root.join("bundle"));
    if let Some(subset) = subset {
        command.args(["--circuit-subset", subset]);
    }
    command.output().unwrap()
}

fn decode(root: &Path, decoder: &str, subset: Option<&str>) -> std::process::Output {
    let mut command = cli();
    command
        .args(["decode", "--decoder", decoder, "--dataset"])
        .arg(root.join("bundle"))
        .arg("--out")
        .arg(root.join(format!("{decoder}.b8")))
        .arg("--stats-out")
        .arg(root.join(format!("{decoder}.json")));
    if let Some(subset) = subset {
        command.args(["--circuit-subset", subset]);
    }
    command.output().unwrap()
}

fn success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn rejected(output: &std::process::Output, command: &str, code: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["command"], command);
    assert_eq!(value["error"]["code"], code);
}

#[test]
fn native_import_and_decode_require_explicit_v2_and_preserve_the_payload() {
    let invalid = tempfile::tempdir().unwrap();
    rejected(
        &import(invalid.path(), NATIVE, &[0], Some("v99")),
        "dataset.import",
        "invalid_arguments",
    );
    assert!(!invalid.path().join("bundle").exists());
    for subset in [None, Some("v1")] {
        let root = tempfile::tempdir().unwrap();
        let output = import(root.path(), NATIVE, &[0], subset);
        rejected(&output, "dataset.import", "unsupported_circuit");
        assert!(String::from_utf8_lossy(&output.stderr).contains("instruction Z"));
        assert!(!root.path().join("bundle").exists());
        assert!(
            !fs::read_dir(root.path()).unwrap().any(|f| f
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("staging"))
        );
    }
    let root = tempfile::tempdir().unwrap();
    let shots: Vec<_> = (0..64).collect();
    success(&import(root.path(), NATIVE, &shots, Some("v2")));
    assert_eq!(
        fs::read_to_string(root.path().join("bundle/circuit.stim")).unwrap(),
        NATIVE
    );
    assert_eq!(
        fs::read(root.path().join("bundle/shots.b8")).unwrap(),
        shots
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.path().join("bundle/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["schema_version"], 1,
        "circuit subset and dataset schema are independent"
    );
    assert_eq!(manifest["row"]["bits"], 6);
    for &decoder in decoders() {
        for subset in [None, Some("v1")] {
            rejected(
                &decode(root.path(), decoder, subset),
                "decode",
                "unsupported_circuit",
            );
            assert!(!root.path().join(format!("{decoder}.b8")).exists());
        }
        rejected(
            &decode(root.path(), decoder, Some("v99")),
            "decode",
            "invalid_arguments",
        );
        success(&decode(root.path(), decoder, Some("v2")));
        let predictions = fs::read(root.path().join(format!("{decoder}.b8"))).unwrap();
        // In this fixture every mechanism's logical label equals D0. A lost
        // q0 value is canonicalized to one, independent of its stored bit.
        let expected: Vec<_> = shots.iter().map(|&row| u8::from(row & 3 != 0)).collect();
        assert_eq!(predictions, expected, "{decoder}");
        let stats: Value =
            serde_json::from_slice(&fs::read(root.path().join(format!("{decoder}.json"))).unwrap())
                .unwrap();
        assert_eq!(stats["circuit_subset"], "v2");
        assert_eq!(stats["shot_count"], 64);
        assert_eq!(stats["circuit_compilations"], 1);
        assert_eq!(stats["distinct_loss_patterns"], 8);
        let builds = if decoder == "envelope-matching" {
            "matching_graph_builds"
        } else {
            "mle_model_builds"
        };
        assert_eq!(stats[builds], 8);
        assert_eq!(stats["cache_hits"], 56);
    }
}

#[test]
fn invalid_import_subset_respects_the_error_channel() {
    for (arguments, json) in [
        (vec!["--error-format", "json"], true),
        (vec!["--format", "json"], true),
        (vec!["--error-format", "human"], false),
        (vec!["--format", "human"], false),
    ] {
        let output = cli()
            .args(["dataset", "import", "--circuit-subset", "v99"])
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        if json {
            rejected(&output, "dataset.import", "invalid_arguments");
        } else {
            assert!(output.stdout.is_empty());
            assert!(serde_json::from_slice::<Value>(&output.stderr).is_err());
            assert!(String::from_utf8_lossy(&output.stderr).contains("v99"));
        }
    }
}

#[test]
fn native_no_flag_cli_predictions_match_the_ordinary_reference() {
    let native = tempfile::tempdir().unwrap();
    let reference = tempfile::tempdir().unwrap();
    let shots: Vec<u8> = (0..8)
        .map(|s| ((s & 1) << 1) | ((s & 2) << 2) | ((s & 4) << 3))
        .collect();
    success(&import(native.path(), NATIVE, &shots, Some("v2")));
    success(&import(reference.path(), REFERENCE, &shots, None));
    for &decoder in decoders() {
        success(&decode(native.path(), decoder, Some("v2")));
        success(&decode(reference.path(), decoder, None));
        let read = |root: &Path| fs::read(root.join(format!("{decoder}.b8"))).unwrap();
        assert_eq!(read(native.path()), [0, 1, 0, 1, 0, 1, 0, 1]);
        assert_eq!(read(native.path()), read(reference.path()));
        let stats: Value = serde_json::from_slice(
            &fs::read(reference.path().join(format!("{decoder}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(stats["circuit_subset"], "v1");
    }
}

#[test]
fn v2_import_keeps_target_lifetime_and_graph_rejections_transactional() {
    for text in [
        NATIVE.replace("CZ 0 1", "CZ 0 1 1 2"),
        NATIVE.replace("CZ 0 1", "CZ 0"),
        NATIVE.replace("Z 0", "Z 0 0"),
        NATIVE.replace("SQRT_Y 0", "SQRT_Y !0"),
        NATIVE.replace("ML 0 1 2", "ML 0\nCZ 0 1\nML 1 2"),
        NATIVE.replace("LOSS(0.1) 0", "LOSS(0.1) 0\nR 0"),
        NATIVE.replace("Z 0", "REPEAT 1 {\nZ 0\n}"),
        NATIVE.replace("DETECTOR(0,0,0) rec[-5]", "DETECTOR(0,0,0) rec[-6]"),
        NATIVE.replace("DETECTOR(0,0,0) rec[-5]\n", ""),
        NATIVE.replace("SQRT_Y 0", "S 0"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = import(root.path(), &text, &[0], Some("v2"));
        assert_eq!(output.status.code(), Some(2), "{text}");
        let value: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert!(
            matches!(
                value["error"]["code"].as_str(),
                Some("unsupported_circuit" | "invalid_circuit")
            ),
            "{value}"
        );
        assert!(!root.path().join("bundle").exists());
    }
}

#[test]
fn capabilities_and_help_advertise_both_subset_versions_and_default_v1() {
    let output = cli()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    for name in ["decode", "dataset.import"] {
        let command = value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        let selector = command["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["flag"] == "--circuit-subset")
            .unwrap();
        assert_eq!(selector["values"], serde_json::json!(["v1", "v2"]));
        assert_eq!(selector["default"], "v1");
    }
    for command in [
        vec!["decode", "--help"],
        vec!["dataset", "import", "--help"],
    ] {
        let output = cli().args(command).output().unwrap();
        success(&output);
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("--circuit-subset"));
        assert!(help.contains("[default: v1]"));
        assert!(help.contains("v1, v2"));
    }
}

#[test]
fn native_and_reference_pauli_dems_agree_with_independent_stim() {
    let root = tempfile::tempdir().unwrap();
    let mut summaries = Vec::new();
    for (index, text) in [NATIVE, REFERENCE].iter().enumerate() {
        // Hand-written no-loss projection preserves interleaved flag slots.
        let ordinary = text
            .lines()
            .filter(|line| !line.starts_with("LOSS("))
            .map(|line| {
                if line.starts_with("ML ") {
                    line[3..]
                        .split_whitespace()
                        .map(|q| format!("MPAD 0\nM {q}\n"))
                        .collect::<String>()
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        let path = root.path().join(format!("ordinary-{index}.stim"));
        fs::write(&path, &ordinary).unwrap();
        let output =
            Command::new(std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string()))
                .args(["analyze_errors", "--decompose_errors", "--in"])
                .arg(&path)
                .output()
                .unwrap();
        success(&output);
        let stim = DetectorErrorModel::parse(&String::from_utf8(output.stdout).unwrap()).unwrap();
        let rstim = rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem_decomposed(
            &rstim::validation::parse_and_validate(&ordinary).unwrap(),
        )
        .unwrap();
        summaries.push(dem_semantic_summary(&stim));
        summaries.push(dem_semantic_summary(&rstim));
    }
    for summary in &summaries[1..] {
        assert_eq!(summary.annotation_lines, summaries[0].annotation_lines);
        assert_eq!(
            summary.error_probabilities.keys().collect::<Vec<_>>(),
            summaries[0].error_probabilities.keys().collect::<Vec<_>>()
        );
        for (effect, &probability) in &summary.error_probabilities {
            assert!(
                (probability - summaries[0].error_probabilities[effect]).abs() < 1e-12,
                "{effect}"
            );
        }
    }
    assert_eq!(summaries[0].error_probabilities.len(), 4);
}
