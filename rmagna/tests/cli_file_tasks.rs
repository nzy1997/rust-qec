use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::TaskRunOptions;
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str], code: i32) -> Value {
        let output = self.run(args);
        assert_eq!(
            output.status.code(),
            Some(code),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn decoder(&self, source: &str) {
        self.put("decoder.py", source);
        self.put("controllers.json", r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"test-v1","config":{}}}}"#);
        self.put("circuit.magna", "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]");
        self.put(
            "task.json",
            r#"{"error_observables":[0],"sample_records":[0,1]}"#,
        );
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_noisy_draws_and_seeded_results_match_the_library() {
    let files = Files::new();
    files.put(
        "circuit.magna",
        "X_ERROR(1/3) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    files.put(
        "task.json",
        r#"{"error_observables":[0],"sample_records":[0]}"#,
    );
    let value = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "24",
            "--seed",
            "713",
            "--json",
            "--output",
            "report.json",
        ],
        0,
    );
    assert_eq!(
        value,
        serde_json::from_slice::<Value>(&std::fs::read(files.0.join("report.json")).unwrap())
            .unwrap()
    );
    let mut options = FileTaskOptions::default();
    options.evaluation.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
    options.evaluation.runtime.compile.probability_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.density_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.moment_preparation =
        QueryPreparation::FixedFibers(Default::default());
    let task = FileTask::load(
        &files.0.join("circuit.magna"),
        Some(&files.0.join("task.json")),
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        options,
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
    .unwrap();
    let expected = task
        .run(&TaskRunOptions {
            shots: 24,
            seed: 713,
            ..Default::default()
        })
        .unwrap()
        .to_json()
        .unwrap();
    for field in [
        "estimates",
        "estimates_scientific",
        "standard_errors",
        "weights",
        "sampled_records",
        "fingerprints",
        "options",
        "status",
        "execution_limits",
    ] {
        assert_eq!(value[field], expected[field], "{field}");
    }
    assert_eq!(value["completed"], 24);
    let errors = value["sampled_records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row[0] == true)
        .count();
    assert!(errors > 0 && errors < 24);
    assert_eq!(
        value["estimates"]["error"].as_f64().unwrap(),
        errors as f64 / 24.0
    );
    files.put(
        "circuit.magna",
        "X_ERROR(1) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    let endpoint = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "3",
            "--json",
        ],
        0,
    );
    assert_eq!(endpoint["estimates"]["error"], 1.0);
    assert_eq!(endpoint["sampled_records"], json!([[true], [true], [true]]));
    files.put(
        "circuit.magna",
        "X_ERROR(0) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    let endpoint = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "3",
            "--json",
        ],
        0,
    );
    assert_eq!(endpoint["estimates"]["error"], 0.0);
}

#[test]
fn inspection_validation_and_human_output_have_distinct_execution_scopes() {
    let files = Files::new();
    files.put(
        "circuit.magna",
        "H 0\nCX 0 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
    );
    files.put("target.magna", "H 0\nCX 0 1");
    files.put("recovery.magna", "Z 0");
    files.put("task.json", r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}},"recovery":"recovery.magna"}"#);
    let valid = files.json(
        &["validate", "circuit.magna", "--task", "task.json", "--json"],
        0,
    );
    assert_eq!(valid["status"], "valid");
    assert_eq!(valid["route"], "not_compiled");
    assert_eq!(valid["compilation"]["status"], "not_compiled");
    let inspect = files.json(
        &["inspect", "circuit.magna", "--task", "task.json", "--json"],
        0,
    );
    assert_eq!(inspect["fingerprints"], valid["fingerprints"]);
    assert_eq!(inspect["compilation"]["status"], "compiled");
    assert_eq!(inspect["compilation"]["query_preparation"], "on_demand");
    // Preparing the target normalization is a real quantum query, even though
    // inspection never samples a history or invokes a decoder callback.
    assert_eq!(
        inspect["compilation"]["preparation_costs"]["query"]["attempted"],
        1
    );
    assert_eq!(
        inspect["compilation"]["preparation_costs"]["decoder"]["attempted"],
        0
    );
    let value = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "3",
            "--json",
        ],
        0,
    );
    assert_eq!(value["estimates"]["acceptance"], 0.5);
    assert_eq!(value["estimates"]["error"], 0.0);
    assert_eq!(value["estimates"]["conditional_error"], 0.0);
    let human = files.run(&["run", "circuit.magna", "--task", "task.json"]);
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(human.status.success());
    assert!(
        text.contains("acceptance: 0.5")
            && text.contains("joint error mass: 0")
            && text.contains("conditional error: 0"),
        "{text}"
    );
    files.put("empty.magna", "H 0");
    let undefined = files.run(&["run", "empty.magna"]);
    assert!(
        String::from_utf8(undefined.stdout)
            .unwrap()
            .contains("conditional error: n/a")
    );
    for flag in ["--help", "--version"] {
        let output = files.run(&[flag]);
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains("rmagna"));
    }
    assert!(files.run(&["run", "--help"]).status.success());
    let duration = files.json(
        &[
            "validate",
            "empty.magna",
            "--max-seconds",
            "0.000000003",
            "--json",
        ],
        0,
    );
    assert_eq!(
        duration["options"]["run"]["execution_limits"]["max_duration"],
        json!({"seconds":0,"nanoseconds":3})
    );
}

#[test]
fn explicit_modes_exact_proposals_and_resource_failures_are_reported() {
    let files = Files::new();
    files.put(
        "circuit.magna",
        "X_ERROR(1/4) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    files.put("task.json", r#"{"error_observables":[0]}"#);
    for sampler in ["affine_support", "exact_marginal"] {
        for preparation in ["cold", "trace", "fixed"] {
            let value = files.json(
                &[
                    "run",
                    "circuit.magna",
                    "--task",
                    "task.json",
                    "--shots",
                    "16",
                    "--seed",
                    "29",
                    "--noise-proposal-scale",
                    "2e0",
                    "--history-sampler",
                    sampler,
                    "--probability-preparation",
                    preparation,
                    "--density-preparation",
                    preparation,
                    "--compiler",
                    "parity_copies",
                    "--causal-strategy",
                    "cold",
                    "--insertion-preparation",
                    "prepared",
                    "--max-insertion-table-bytes",
                    "4096",
                    "--max-insertion-catalog-entries",
                    "4",
                    "--json",
                ],
                0,
            );
            assert_eq!(value["sampling"]["method"], "importance");
            assert_eq!(value["history_sampler"], sampler);
            assert_eq!(value["completed"], 16);
            assert_eq!(
                value["proposal_summary"]["proposal_scale"],
                json!({"numerator":"2","denominator":"1"})
            );
        }
    }
    let fraction = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--noise-proposal-scale",
            "3/2",
            "--json",
        ],
        0,
    );
    assert_eq!(
        fraction["proposal_summary"]["proposal_scale"]["numerator"],
        "3"
    );
    let quota = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "3",
            "--max-queries",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(quota["completed"], 0);
    assert_eq!(quota["not_attempted"], 2);
    assert_eq!(quota["failed"], 1);
    assert!(quota["estimates"].is_null());
    assert!(
        quota["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("max_queries")
    );
    let failed = files.json(
        &[
            "run",
            "circuit.magna",
            "--max-coherent-qubits",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(failed["compilation"]["status"], "preparation_failed");
    assert_eq!(failed["completed"], 0);
    let numeric = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--numeric-work-bits",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(numeric["engine_status"], "complete");
    assert_eq!(numeric["completed"], 1);
    assert_eq!(numeric["failures"][0]["phase"], "export");
    assert!(numeric["estimates"].is_null() && numeric["partial_statistics"].is_null());
    for key in [
        "circuit",
        "task",
        "ideal_state",
        "recovery",
        "controllers",
        "options",
        "sampling",
    ] {
        assert_eq!(numeric["fingerprints"][key].as_str().unwrap().len(), 64);
    }
    assert_eq!(numeric["numeric_export"]["exact_results_persisted"], false);
}

#[test]
fn arguments_and_existing_outputs_fail_before_decoder_import() {
    let files = Files::new();
    files.decoder("from pathlib import Path\nPath('imported').touch()\ndef decode(bits,config): return bits\n");
    for extra in [
        vec!["--shots", "0"],
        vec!["--seed", "-1"],
        vec!["--seed", "1", "--seed", "2"],
        vec!["--max-seconds", "NaN"],
        vec!["--max-seconds", "inf"],
        vec!["--max-seconds", "0"],
        vec!["--max-seconds", "1e-99"],
        vec!["--compiler", "typo"],
        vec!["--moment-preparation", "typo"],
        vec!["--noise-proposal-scale", "0"],
        vec!["--noise-proposal-scale", "-1/3"],
        vec!["--noise-proposal-scale", "1/0"],
        vec!["--max-insertion-table-bytes", "10"],
        vec![
            "--probability-preparation",
            "cold",
            "--density-preparation",
            "cold",
            "--moment-preparation",
            "cold",
            "--fiber-search-rank",
            "16",
        ],
        vec!["--max-gates", "-1"],
        vec!["--max-task-bytes", "0"],
        vec!["--unsupported", "value"],
    ] {
        let mut args = vec![
            "run",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--json",
        ];
        args.extend(extra);
        let value = files.json(&args, 2);
        assert_eq!(value["failures"][0]["phase"], "arguments");
        assert!(!files.0.join("imported").exists());
    }
    files.put("existing.json", "preserve me");
    let value = files.json(
        &[
            "run",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--output",
            "existing.json",
            "--json",
        ],
        1,
    );
    assert_eq!(value["failures"][0]["phase"], "output");
    assert_eq!(
        std::fs::read_to_string(files.0.join("existing.json")).unwrap(),
        "preserve me"
    );
    assert!(!files.0.join("imported").exists());
    for args in [
        vec!["inspect", "circuit.magna", "--shots", "2", "--json"],
        vec!["run", "circuit.magna", "--python", "python3", "--json"],
    ] {
        files.json(&args, 2);
    }
    let value = files.json(
        &["run", "missing.magna", "--output", "failure.json", "--json"],
        1,
    );
    assert_eq!(
        value,
        serde_json::from_slice::<Value>(&std::fs::read(files.0.join("failure.json")).unwrap())
            .unwrap()
    );
    files.put("invalid.json", "{");
    files.json(
        &["run", "circuit.magna", "--task", "invalid.json", "--json"],
        1,
    );
    files.json(
        &[
            "validate",
            "circuit.magna",
            "--max-source-bytes",
            "1",
            "--json",
        ],
        1,
    );
}

#[test]
fn embedded_decoder_manifest_and_failed_prefix_survive_end_to_end() {
    let files = Files::new();
    let source = "from pathlib import Path\ndef decode(bits,config):\n print('decoder diagnostic')\n with Path('callbacks').open('a') as f: f.write('x')\n return bits\n";
    files.decoder(source);
    for mode in ["validate", "inspect"] {
        let value = files.json(
            &[
                mode,
                "circuit.magna",
                "--task",
                "task.json",
                "--controllers",
                "controllers.json",
                "--decoder-timeout",
                "10",
                "--json",
            ],
            0,
        );
        assert_eq!(
            value["controller_process"]["manifest"]["native_backend"],
            "unavailable"
        );
        assert_eq!(
            value["controller_process"]["manifest"]["module_sources"]["decoder.py"],
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        assert!(!files.0.join("callbacks").exists());
    }
    let value = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--shots",
            "8",
            "--seed",
            "41",
            "--json",
        ],
        0,
    );
    assert_eq!(
        std::fs::read_to_string(files.0.join("callbacks")).unwrap(),
        "xxxxxxxx"
    );
    for row in value["sampled_records"].as_array().unwrap() {
        assert_eq!(row[0], row[1]);
    }
    assert!(
        value["execution_costs"]["decoder"]["attempted"]
            .as_u64()
            .unwrap()
            >= 8
    );
    assert_eq!(value["route"], "causal_fixed_template");
    let source = "calls=0\ndef decode(bits,config):\n global calls\n calls+=1\n if calls>1: raise RuntimeError('fail second callback')\n return bits\n";
    files.put("decoder.py", source);
    let partial = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--shots",
            "4",
            "--json",
        ],
        1,
    );
    assert_eq!(
        (
            partial["completed"].as_u64(),
            partial["failed"].as_u64(),
            partial["not_attempted"].as_u64()
        ),
        (Some(1), Some(1), Some(2))
    );
    assert!(partial["estimates"].is_null());
    assert_eq!(
        partial["partial_statistics"]["estimates"]["acceptance"],
        1.0
    );
    assert_eq!(partial["sampled_records"].as_array().unwrap().len(), 1);
    assert_ne!(
        partial["fingerprints"]["controllers"],
        value["fingerprints"]["controllers"]
    );
    files.put(
        "decoder.py",
        "def decode(bits,config):\n import magna\n return bits\n",
    );
    let forbidden = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--json",
        ],
        1,
    );
    assert!(
        forbidden["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("cannot import a Magna quantum backend")
    );
}

#[test]
fn moment_preparation_options_cache_limits_and_fingerprints_are_observable() {
    let files = Files::new();
    files.put("circuit.magna", "H 0\nT 0\nH 0");
    files.put("task.json", r#"{"observables":[[0,1]]}"#);
    let mut digests = std::collections::BTreeSet::new();
    for mode in ["cold", "trace", "fixed"] {
        let value = files.json(
            &[
                "run",
                "circuit.magna",
                "--task",
                "task.json",
                "--shots",
                "2",
                "--moment-preparation",
                mode,
                "--max-prepared-moments",
                "2",
                "--json",
            ],
            0,
        );
        assert_eq!(
            value["estimates"]["moments"][0],
            std::f64::consts::FRAC_1_SQRT_2
        );
        assert_eq!(value["standard_errors"]["moments"][0], 0.0);
        assert_eq!(
            value["options"]["compile"]["moment_preparation"]["mode"],
            mode
        );
        assert_eq!(value["options"]["compile"]["max_prepared_moments"], 2);
        assert_eq!(
            value["compilation"]["retained_caches"]["moment"]["cached_queries"],
            u64::from(mode != "cold")
        );
        assert_eq!(
            value["compilation"]["retained_caches"]["moment"]["fixed_resources"],
            u64::from(mode == "fixed")
        );
        assert!(
            digests.insert(
                value["fingerprints"]["options"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            )
        );
    }
    let inspected = files.json(
        &[
            "inspect",
            "circuit.magna",
            "--probability-preparation",
            "cold",
            "--density-preparation",
            "cold",
            "--moment-preparation",
            "fixed",
            "--fiber-search-rank",
            "1",
            "--json",
        ],
        0,
    );
    assert_eq!(
        inspected["options"]["compile"]["moment_preparation"]["max_search_rank"],
        1
    );
    assert_eq!(
        inspected["compilation"]["retained_caches"]["moment"]["cached_queries"],
        0
    );
    files.put("task.json", r#"{"observables":[[0,1],[0,0]]}"#);
    let limited = files.json(
        &[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-prepared-moments",
            "1",
            "--json",
        ],
        1,
    );
    assert_eq!(limited["completed"], 0);
    assert_eq!(limited["execution_costs"]["query"]["attempted"], 3);
    assert_eq!(limited["execution_costs"]["query"]["failed"], 1);
    assert_eq!(
        limited["compilation"]["retained_caches"]["moment"]["cached_queries"],
        1
    );
    assert!(
        limited["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("max_prepared_moments")
    );
    let invalid = files.json(
        &[
            "run",
            "circuit.magna",
            "--max-prepared-moments",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(invalid["compilation"]["status"], "preparation_failed");
    assert_eq!(invalid["execution_costs"]["query"]["attempted"], 0);
}
