use rmagna::file_api::{
    FileLoadOptions, FilePreparationOptions, FileSamplerComparisonOptions, FileTask,
    FileTaskOptions,
};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-cli-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str], code: i32) -> Value {
        let out = self.run(args);
        assert_eq!(
            out.status.code(),
            Some(code),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn simple(&self) {
        self.put(
            "circuit.magna",
            "H 0\nT 0\nH 0\nM(1/7) 0\nI 1 2\nOBSERVABLE_INCLUDE(0) rec[-1]",
        );
        self.put(
            "task.json",
            r#"{"sample_records":[0],"error_observables":[0]}"#,
        );
    }
    fn loaded(&self) -> FileTask {
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy =
            CompilerStrategy::SharedCommutingLayer;
        options.evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
        options.evaluation.runtime.compile.probability_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.evaluation.runtime.compile.density_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.evaluation.runtime.compile.moment_preparation =
            QueryPreparation::FixedFibers(Default::default());
        FileTask::load(
            &self.0.join("circuit.magna"),
            Some(&self.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
    fn bindings(&self, source: &str) {
        self.put("decoder.py", source);
        self.put("controllers.json",&json!({"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"preparation-v1","config":{"calls":self.0.join("calls.txt").to_str().unwrap()}}}}).to_string());
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let cost = map.contains_key("attempted");
            let comparison = map.get("kind").is_some_and(|v| v == "sampler_comparison");
            map.retain(|k, _| {
                !(k.ends_with("_seconds")
                    || k == "seconds" && cost
                    || comparison
                        && [
                            "gamma",
                            "recommended_sampler",
                            "affine_support_seconds_per_draw",
                            "exact_marginal_seconds_per_draw",
                        ]
                        .contains(&k.as_str()))
            });
            for child in map.values_mut() {
                normalize(child);
            }
        }
        Value::Array(values) => {
            for v in values {
                normalize(v)
            }
        }
        _ => {}
    }
}
#[test]
fn preparation_json_with_nondefault_caps_matches_owned_file_api_and_saved_output() {
    let f = Files::new();
    f.simple();
    let mut actual = f.json(
        &[
            "prepare",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-preparation-requests",
            "12",
            "--max-preparation-history-rows",
            "32",
            "--max-preparation-layout-rows",
            "256",
            "--max-queries",
            "0",
            "--max-seconds",
            "60",
            "--output",
            "prepared.json",
            "--json",
        ],
        0,
    );
    assert_eq!(
        actual,
        serde_json::from_slice::<Value>(&std::fs::read(f.0.join("prepared.json")).unwrap())
            .unwrap()
    );
    assert_eq!(actual["kind"], "preparation");
    assert_eq!(actual["sampling_preparation"]["counts"]["created"], 2);
    assert_eq!(actual["execution_costs"]["query"]["attempted"], 0);
    let mut c = FilePreparationOptions::default();
    c.queries.max_requests = 12;
    c.queries.max_history_rows = 32;
    c.queries.max_layout_rows = 256;
    c.execution_limits.max_queries = 0;
    c.execution_limits.max_duration = Some(Duration::from_secs(60));
    let mut expected = f.loaded().prepare_sampling(&c).unwrap().report().to_json();
    expected["controller_process"] = Value::Null;
    normalize(&mut actual);
    normalize(&mut expected);
    assert_eq!(actual, expected);
    assert!(actual["options"].get("run").is_none());
    assert!(actual.get("seed").is_none());
    assert!(actual.get("estimates").is_none());
}
#[test]
fn comparison_json_preserves_independent_seeded_pilots_and_separate_phase_budgets() {
    let f = Files::new();
    f.simple();
    let mut actual = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--pilot-shots",
            "24",
            "--seed",
            "23",
            "--max-preparation-requests",
            "12",
            "--max-preparation-history-rows",
            "32",
            "--max-preparation-layout-rows",
            "256",
            "--max-preparation-queries",
            "0",
            "--max-preparation-seconds",
            "60",
            "--max-queries",
            "512",
            "--max-seconds",
            "90",
            "--max-record-values",
            "128",
            "--output",
            "comparison.json",
            "--json",
        ],
        0,
    );
    assert_eq!(
        actual,
        serde_json::from_slice::<Value>(&std::fs::read(f.0.join("comparison.json")).unwrap())
            .unwrap()
    );
    assert_eq!(actual["status"], "complete");
    assert_eq!(actual["pilots"]["affine_support"]["seed"], 23);
    assert_eq!(actual["pilots"]["exact_marginal"]["seed"], 24);
    assert_eq!(actual["automatic_switching"], false);
    assert_eq!(actual["production_started"], false);
    let mut c = FileSamplerComparisonOptions::default();
    c.pilot.shots = 24;
    c.pilot.seed = 23;
    c.pilot.max_retained_record_values = 128;
    c.pilot.execution_limits.max_queries = 512;
    c.pilot.execution_limits.max_duration = Some(Duration::from_secs(90));
    c.preparation.queries.max_requests = 12;
    c.preparation.queries.max_history_rows = 32;
    c.preparation.queries.max_layout_rows = 256;
    c.preparation.execution_limits.max_queries = 0;
    c.preparation.execution_limits.max_duration = Some(Duration::from_secs(60));
    let mut expected = f.loaded().compare_samplers(&c).unwrap().to_json();
    expected["controller_process"] = Value::Null;
    for method in ["affine_support", "exact_marginal"] {
        assert_eq!(
            actual["pilots"][method]["sampling"]["options"]["run"]["execution_limits"]["max_duration"]
                ["seconds"],
            90
        );
        assert_eq!(
            actual["pilots"][method]["preparation"]["options"]["sampling_preparation"]["execution_limits"]
                ["max_duration"]["seconds"],
            60
        );
    }
    normalize(&mut actual);
    normalize(&mut expected);
    assert_eq!(actual, expected);
}
#[test]
fn wrong_modes_bad_values_and_known_incompatible_policies_reject_before_imports() {
    let f = Files::new();
    f.simple();
    f.bindings(
        "open('imported.txt','w').write('unexpected')\nraise RuntimeError('must not import')",
    );
    let prep = [
        "--max-preparation-requests",
        "--max-preparation-history-rows",
        "--max-preparation-layout-rows",
    ];
    let compare = [
        "--pilot-shots",
        "--max-preparation-queries",
        "--max-preparation-seconds",
    ];
    for mode in ["validate", "inspect", "run", "enumerate"] {
        for flag in prep {
            let v = f.json(
                &[
                    mode,
                    "circuit.magna",
                    "--controllers",
                    "controllers.json",
                    flag,
                    "2",
                    "--json",
                ],
                2,
            );
            assert_eq!(v["failures"][0]["phase"], "arguments");
        }
    }
    for mode in ["validate", "inspect", "run", "enumerate", "prepare"] {
        for flag in compare {
            let v = f.json(
                &[
                    mode,
                    "circuit.magna",
                    "--controllers",
                    "controllers.json",
                    flag,
                    "2",
                    "--json",
                ],
                2,
            );
            assert_eq!(v["failures"][0]["phase"], "arguments");
        }
    }
    for (mode, flag, value) in [
        ("prepare", "--seed", "2"),
        ("prepare", "--shots", "2"),
        ("prepare", "--max-record-values", "2"),
        ("prepare", "--max-weight", "2"),
        ("compare-samplers", "--shots", "2"),
        ("compare-samplers", "--history-sampler", "affine_support"),
        ("compare-samplers", "--pilot-shots", "0"),
        ("compare-samplers", "--pilot-shots", "1"),
        ("compare-samplers", "--seed", "18446744073709551615"),
        ("compare-samplers", "--causal-strategy", "cold"),
        ("compare-samplers", "--probability-preparation", "trace"),
        ("compare-samplers", "--probability-preparation", "cold"),
        ("compare-samplers", "--max-preparation-seconds", "0"),
        ("compare-samplers", "--max-preparation-seconds", "NaN"),
        ("compare-samplers", "--max-preparation-seconds", "-1"),
        ("prepare", "--max-preparation-layout-rows", "-1"),
        (
            "prepare",
            "--max-preparation-history-rows",
            "18446744073709551616",
        ),
    ] {
        let v = f.json(
            &[
                mode,
                "circuit.magna",
                "--controllers",
                "controllers.json",
                flag,
                value,
                "--json",
            ],
            2,
        );
        assert_eq!(v["failures"][0]["phase"], "arguments");
    }
    f.json(
        &[
            "prepare",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--max-preparation-requests",
            "1",
            "--max-preparation-requests",
            "2",
            "--json",
        ],
        2,
    );
    f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--pilot-shots",
            "--json",
        ],
        2,
    );
    assert!(!f.0.join("imported.txt").exists());
}
#[test]
fn preparation_and_pilot_failures_have_nonzero_exit_status_and_keep_receipts() {
    let f = Files::new();
    f.simple();
    let failed = f.json(
        &[
            "prepare",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-preparation-requests",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["failure"]["phase"], "sampling_families");
    assert_eq!(failed["sampling_preparation"]["counts"]["attempted"], 0);
    let failed = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-queries",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(failed["reason"], "pilot_incomplete");
    for method in ["affine_support", "exact_marginal"] {
        assert_eq!(failed["pilots"][method]["sampling_receipt"]["completed"], 0);
        assert_eq!(
            failed["pilots"][method]["sampling_receipt"]["execution_costs"]["query"]["attempted"],
            0
        );
    }
    let failed = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--numeric-work-bits",
            "0",
            "--json",
        ],
        1,
    );
    for method in ["affine_support", "exact_marginal"] {
        assert!(failed["pilots"][method]["sampling"].is_null());
        assert_eq!(failed["pilots"][method]["sampling_receipt"]["completed"], 2);
        assert_eq!(
            failed["pilots"][method]["sampling_receipt"]["estimates_persisted"],
            false
        );
        assert!(failed["pilots"][method]["sampling_export_failure"].is_string());
    }
    // With no classical response family, affine needs one probability key;
    // exact sampling additionally needs its empty-prefix key.
    f.put("task.json", r#"{"sample_records":[0]}"#);
    let failed = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-prepared-probabilities",
            "1",
            "--json",
        ],
        1,
    );
    assert_eq!(failed["pilots"]["affine_support"]["status"], "complete");
    assert_eq!(
        failed["pilots"]["exact_marginal"]["preparation"]["status"],
        "failed"
    );
    assert_eq!(
        failed["pilots"]["affine_support"]["sampling_receipt"]["completed"],
        2
    );
}
#[test]
fn isolated_decoder_preparation_has_no_callbacks_and_pilots_make_only_requested_calls() {
    let f = Files::new();
    f.put("circuit.magna","H 0\nT 0\nH 0\nM(1/5) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nX_ERROR(1/7) 1\nM 1\nI 2\nOBSERVABLE_INCLUDE(0) rec[-1]");
    f.put("task.json", r#"{"error_observables":[0]}"#);
    f.bindings("try:\n import magna\nexcept ImportError:\n pass\nelse:\n raise RuntimeError('native import was not blocked')\ndef decode(bits, config):\n with open(config['calls'], 'a') as out:\n  out.write('called\\n')\n return [bits[0]]\n");
    let prepared = f.json(
        &[
            "prepare",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--json",
        ],
        0,
    );
    assert_eq!(prepared["execution_costs"]["decoder"]["attempted"], 0);
    assert!(!f.0.join("calls.txt").exists());
    assert!(prepared["controller_process"]["manifest"]["bindings"]["copy"].is_object());
    let compared = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--pilot-shots",
            "16",
            "--seed",
            "19",
            "--json",
        ],
        0,
    );
    assert_eq!(compared["status"], "complete");
    assert_eq!(compared["pilots"]["affine_support"]["seed"], 19);
    assert_eq!(compared["pilots"]["exact_marginal"]["seed"], 20);
    assert_eq!(
        std::fs::read_to_string(f.0.join("calls.txt"))
            .unwrap()
            .lines()
            .count(),
        32
    );
    for method in ["affine_support", "exact_marginal"] {
        assert_eq!(
            compared["pilots"][method]["preparation"]["execution_costs"]["decoder"]["attempted"],
            0
        );
        assert_eq!(
            compared["pilots"][method]["sampling_receipt"]["execution_costs"]["decoder"]["succeeded"],
            16
        );
    }
}
#[test]
fn inconclusive_human_output_seed_boundary_and_exclusive_destination_are_explicit() {
    let f = Files::new();
    f.simple();
    let prepared = f.run(&["prepare", "circuit.magna", "--task", "task.json"]);
    assert!(prepared.status.success());
    let text = String::from_utf8(prepared.stdout).unwrap();
    assert!(text.contains("Prepared families:") && text.contains("Preparation seconds:"));
    f.put("task.json", r#"{"sample_records":[0]}"#);
    let v = f.json(
        &[
            "compare-samplers",
            "circuit.magna",
            "--task",
            "task.json",
            "--seed",
            "18446744073709551614",
            "--json",
        ],
        0,
    );
    assert_eq!(v["status"], "inconclusive");
    assert_eq!(v["reason"], "no_error_metric");
    assert_eq!(v["pilots"]["exact_marginal"]["seed"], json!(u64::MAX));
    let out = f.run(&["compare-samplers", "circuit.magna", "--task", "task.json"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("No recommendation: no_error_metric")
            && text.contains("automatic switching disabled")
            && text.contains("affine_support:")
            && text.contains("exact_marginal:")
    );
    for mode in ["prepare", "compare-samplers"] {
        let help = f.run(&[mode, "--help"]);
        assert!(help.status.success());
        assert!(
            String::from_utf8(help.stdout)
                .unwrap()
                .contains("--max-preparation-queries")
        );
    }
    f.put("occupied.json", "sentinel");
    f.bindings(
        "open('imported.txt','w').write('unexpected')\nraise RuntimeError('must not import')",
    );
    for mode in ["prepare", "compare-samplers"] {
        let v = f.json(
            &[
                mode,
                "circuit.magna",
                "--controllers",
                "controllers.json",
                "--output",
                "occupied.json",
                "--json",
            ],
            1,
        );
        assert_eq!(v["failures"][0]["phase"], "output");
    }
    assert_eq!(
        std::fs::read_to_string(f.0.join("occupied.json")).unwrap(),
        "sentinel"
    );
    assert!(!f.0.join("imported.txt").exists());
}
