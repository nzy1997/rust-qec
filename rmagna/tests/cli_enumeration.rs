use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
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
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-cli-enumeration-{}-{}",
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
            "X_ERROR(1/3) 0\nX_ERROR(1/5) 1\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2] rec[-1]",
        );
        self.put("task.json", r#"{"error_observables":[0]}"#);
    }
    fn decoder(&self, source: &str) {
        self.put("decoder.py", source);
        self.put("controllers.json",r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"enumeration-v1","config":{}}}}"#);
        self.put("circuit.magna","X_ERROR(1/3) 1\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nPOSTSELECT det[0]");
        self.put("task.json", r#"{"error_observables":[0]}"#);
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
            map.retain(|key, _| !(key.ends_with("_seconds") || key == "seconds" && cost));
            for child in map.values_mut() {
                normalize(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize(child);
            }
        }
        _ => {}
    }
}
#[test]
fn all_enumeration_options_and_non_time_results_match_the_owned_file_api() {
    let files = Files::new();
    files.simple();
    let mut value = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-weight",
            "1",
            "--max-patterns",
            "32",
            "--max-fault-entries",
            "128",
            "--max-enumeration-sites",
            "8",
            "--max-site-weight-pairs",
            "256",
            "--max-enumeration-probability-bits",
            "2048",
            "--max-history-branches",
            "32",
            "--max-history-record-bits",
            "8",
            "--max-marginal-queries",
            "1024",
            "--max-replay-attempts",
            "2048",
            "--max-queries",
            "64",
            "--max-seconds",
            "60",
            "--json",
            "--output",
            "result.json",
        ],
        0,
    );
    assert_eq!(
        value,
        serde_json::from_slice::<Value>(&std::fs::read(files.0.join("result.json")).unwrap())
            .unwrap()
    );
    assert_eq!(value["result"]["masses"]["error"]["value"], 0.4);
    assert_eq!(
        value["result"]["full_error"]["upper"]["exact"]["coefficients"][0],
        json!({"numerator":"7","denominator":"15"})
    );
    assert!(value.get("seed").is_none());
    assert!(value.get("completed").is_none());
    let mut options = FileTaskOptions::default();
    let runtime = &mut options.evaluation.runtime;
    runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    runtime.history_sampler = HistorySampler::AffineSupport;
    runtime.compile.probability_preparation = QueryPreparation::FixedFibers(Default::default());
    runtime.compile.density_preparation = QueryPreparation::FixedFibers(Default::default());
    runtime.compile.moment_preparation = QueryPreparation::FixedFibers(Default::default());
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
    let mut enumeration = FileEnumerationOptions::default();
    let n = &mut enumeration.enumeration.noise;
    n.max_weight = 1;
    n.max_patterns = 32;
    n.max_fault_entries = 128;
    n.max_sites = 8;
    n.max_site_weight_pairs = 256;
    n.max_probability_bits = 2048;
    let h = &mut enumeration.enumeration.histories;
    h.max_branches = 32;
    h.max_record_bits = 8;
    h.max_marginal_queries = 1024;
    h.max_replay_attempts = 2048;
    enumeration.execution_limits.max_queries = 64;
    enumeration.execution_limits.max_duration = Some(std::time::Duration::from_secs(60));
    let mut expected = task.enumerate(&enumeration).unwrap().to_json().unwrap();
    expected["controller_process"] = Value::Null;
    normalize(&mut expected);
    normalize(&mut value);
    assert_eq!(value, expected);
    let full = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-weight",
            "2",
            "--json",
        ],
        0,
    );
    assert_eq!(
        full["result"]["omitted_probability"],
        json!({"numerator":"0","denominator":"1"})
    );
    assert_eq!(
        full["result"]["conditional_error"]["lower"],
        full["result"]["conditional_error"]["upper"]
    );
    let human = files.run(&["enumerate", "circuit.magna", "--task", "task.json"]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    for text_part in [
        "Covered physical probability: 14/15",
        "Omitted physical probability: 1/15",
        "Covered joint error mass: 0.4",
        "Full conditional error interval: [0.4,",
        "Full joint error interval: [0.4,",
    ] {
        assert!(text.contains(text_part), "{text}");
    }
    assert!(
        text.contains(&format!("Full acceptance interval: [{}, 1]", 14.0 / 15.0)),
        "{text}"
    );
}
#[test]
fn every_mode_mismatch_and_malformed_flag_rejects_before_decoder_import() {
    let files = Files::new();
    files.decoder("from pathlib import Path\nPath('imported').touch()\ndef decode(bits,config): return bits\n");
    let flags = [
        "--max-original-categories",
        "--max-original-channels",
        "--max-original-probability-bits",
        "--max-weight",
        "--max-patterns",
        "--max-fault-entries",
        "--max-enumeration-sites",
        "--max-site-weight-pairs",
        "--max-enumeration-probability-bits",
        "--max-history-branches",
        "--max-history-record-bits",
        "--max-marginal-queries",
        "--max-replay-attempts",
    ];
    for mode in ["run", "validate", "inspect"] {
        for flag in flags {
            let value = files.json(
                &[
                    mode,
                    "missing.magna",
                    "--controllers",
                    "controllers.json",
                    flag,
                    "1",
                    "--json",
                ],
                2,
            );
            assert!(
                value["failures"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("requires enumerate")
            );
            assert!(!files.0.join("imported").exists());
        }
    }
    for extra in [
        vec!["--shots", "1"],
        vec!["--seed", "0"],
        vec!["--noise-proposal-scale", "1"],
        vec!["--history-sampler", "exact_marginal"],
        vec!["--max-record-values", "2"],
        vec!["--max-weight", "-1"],
        vec!["--max-weight", "wrong"],
        vec!["--max-weight", "340282366920938463463374607431768211455x"],
        vec!["--max-patterns", "0"],
        vec!["--max-weight", "1", "--max-weight", "2"],
        vec!["--max-weight"],
    ] {
        let mut args = vec![
            "enumerate",
            "missing.magna",
            "--controllers",
            "controllers.json",
            "--json",
        ];
        args.extend(extra);
        files.json(&args, 2);
        assert!(!files.0.join("imported").exists());
    }
    files.put("report.json", "preserved");
    files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--output",
            "report.json",
            "--json",
        ],
        1,
    );
    assert_eq!(
        std::fs::read_to_string(files.0.join("report.json")).unwrap(),
        "preserved"
    );
    assert!(!files.0.join("imported").exists());
    let help = files.run(&["enumerate", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("--max-weight"));
    assert!(text.contains("all positive histories"));
}

#[test]
fn physical_category_limits_resolve_and_export_choices_without_changing_internal_addresses() {
    let files = Files::new();
    files.put("circuit.magna", "M 0\nX_ERROR(1/3) 0");
    let args = [
        "enumerate",
        "circuit.magna",
        "--include-patterns",
        "--max-original-categories",
        "1",
        "--max-original-channels",
        "1",
        "--max-original-probability-bits",
        "2",
        "--json",
    ];
    let value = files.json(&args, 0);
    assert_eq!(value["original_noise_categories"][0]["internal_site"], 1);
    assert_eq!(
        value["original_noise_categories"][0]["channels"][0]["action"],
        json!({"kind":"pauli","factors":[{"qubit":0,"basis":"X"}]})
    );
    assert_eq!(
        value["result"]["pattern_responses"][1]["choices"],
        json!([{"category":1,"channel":0,"original_category":0}])
    );
    assert_eq!(
        value["options"]["enumeration"]["original_noise"],
        json!({"max_categories":1,"max_channels":1,"max_probability_bits":2})
    );
    let disabled = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--max-original-categories",
            "0",
            "--max-original-channels",
            "0",
            "--max-original-probability-bits",
            "0",
            "--json",
        ],
        0,
    );
    assert!(disabled["original_noise_categories"].is_null());
    assert!(disabled["result"]["pattern_responses"].is_null());
    assert_eq!(disabled["result"]["masses"], value["result"]["masses"]);
    assert_eq!(
        disabled["result"]["by_weight"],
        value["result"]["by_weight"]
    );
    for (flag, bound) in [
        ("--max-original-categories", "0"),
        ("--max-original-channels", "0"),
        ("--max-original-probability-bits", "1"),
    ] {
        let failed = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--include-patterns",
                flag,
                bound,
                "--json",
            ],
            1,
        );
        assert!(
            failed["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("enumeration.original_noise")
        );
    }
}

#[test]
fn malformed_or_boolean_physical_metadata_limits_fail_before_controller_import() {
    let files = Files::new();
    files.decoder("from pathlib import Path\nPath('imported').touch()\ndef decode(bits,config): return bits\n");
    for flag in [
        "--max-original-categories",
        "--max-original-channels",
        "--max-original-probability-bits",
    ] {
        for invalid in [
            "true",
            "false",
            "-1",
            "1.0",
            "oops",
            "184467440737095516160",
        ] {
            let value = files.json(
                &[
                    "enumerate",
                    "circuit.magna",
                    "--controllers",
                    "controllers.json",
                    "--include-patterns",
                    flag,
                    invalid,
                    "--json",
                ],
                2,
            );
            assert!(
                value["failures"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains(flag)
            );
            assert!(!files.0.join("imported").exists());
        }
        files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--controllers",
                "controllers.json",
                flag,
                "--json",
            ],
            2,
        );
        assert!(!files.0.join("imported").exists());
    }
}
#[test]
fn noise_budgets_and_numeric_export_failures_preserve_receipts() {
    let files = Files::new();
    files.simple();
    for (flag, limit) in [
        ("--max-patterns", "1"),
        ("--max-fault-entries", "0"),
        ("--max-enumeration-sites", "1"),
        ("--max-site-weight-pairs", "1"),
        ("--max-enumeration-probability-bits", "1"),
        ("--max-queries", "0"),
    ] {
        let value = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--task",
                "task.json",
                flag,
                limit,
                "--json",
            ],
            1,
        );
        assert_eq!(value["status"], "failed");
        assert!(value["result"].is_null());
        assert_eq!(value["failure_phase"], "enumeration");
        assert_eq!(value["execution_costs"]["query"]["attempted"], 0);
        assert!(!value["fingerprints"]["options"].is_null());
    }
    let numeric = files.json(
        &[
            "enumerate",
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
    assert!(numeric["result"].is_null());
    assert_eq!(numeric["execution_costs"]["query"]["succeeded"], 6);
    assert_eq!(numeric["traversal_counts"]["patterns"], 3);
    assert_eq!(numeric["traversal_counts"]["histories"], 3);
    assert_eq!(numeric["numeric_export"]["exact_results_persisted"], false);
    assert!(
        numeric["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("max_work_bits")
    );
    let deadline = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--max-seconds",
            "0.000000001",
            "--json",
        ],
        1,
    );
    assert_eq!(
        deadline["options"]["enumeration"]["execution_limits"]["max_duration"],
        json!({"seconds":0,"nanoseconds":1})
    );
    assert!(deadline["result"].is_null());
}
#[test]
fn nonlinear_external_decoders_enumerate_all_physical_histories_and_caps() {
    let files = Files::new();
    files.decoder("from pathlib import Path\ndef decode(bits,config):\n try:\n  import magna\n  raise RuntimeError('native import unexpectedly succeeded')\n except ImportError: pass\n with Path('callbacks').open('a') as f: f.write('x')\n return bits\n");
    for strategy in ["fixed", "cold"] {
        let value = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--task",
                "task.json",
                "--controllers",
                "controllers.json",
                "--causal-strategy",
                strategy,
                "--json",
            ],
            0,
        );
        assert_eq!(value["result"]["histories"], 4);
        assert_eq!(value["result"]["masses"]["accepted"]["value"], 0.5);
        assert_eq!(value["result"]["masses"]["error"]["value"], 1.0 / 6.0);
        assert_eq!(value["execution_costs"]["query"]["succeeded"], 20);
        assert_eq!(value["execution_costs"]["decoder"]["succeeded"], 4);
        assert_eq!(
            value["controller_process"]["manifest"]["native_backend"],
            "unavailable"
        );
        assert_eq!(
            value["execution_costs"]["compilation"]["succeeded"],
            if strategy == "fixed" { 1 } else { 11 }
        );
    }
    for (flag, limit) in [
        ("--max-history-branches", "1"),
        ("--max-history-record-bits", "0"),
        ("--max-marginal-queries", "0"),
        ("--max-replay-attempts", "0"),
    ] {
        let value = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--task",
                "task.json",
                "--controllers",
                "controllers.json",
                flag,
                limit,
                "--json",
            ],
            1,
        );
        assert!(value["result"].is_null());
        assert_eq!(value["failure_phase"], "enumeration");
        assert_eq!(value["execution_costs"]["decoder"]["attempted"], 0);
    }
    assert_eq!(
        std::fs::read_to_string(files.0.join("callbacks")).unwrap(),
        "xxxxxxxx"
    );
}
#[test]
fn callback_failure_and_sampled_record_requests_do_not_publish_partial_sums() {
    let files = Files::new();
    files.decoder("def decode(bits,config): raise RuntimeError('enumeration decoder failure')\n");
    let value = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--json",
        ],
        1,
    );
    assert!(value["result"].is_null());
    assert_eq!(value["execution_costs"]["decoder"]["attempted"], 1);
    assert_eq!(value["execution_costs"]["decoder"]["failed"], 1);
    assert!(
        value["failure"]["message"]
            .as_str()
            .unwrap()
            .contains("enumeration decoder failure")
    );
    files.put("task.json", r#"{"sample_records":[0]}"#);
    let value = files.json(
        &[
            "enumerate",
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
        value["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("sample_records")
    );
    let missing = files.json(&["enumerate", "missing.magna", "--json"], 1);
    assert_eq!(missing["status"], "failed");
}
#[test]
fn scientific_tiny_error_and_undefined_conditional_intervals_remain_visible() {
    let files = Files::new();
    let denominator = (num_bigint::BigUint::from(1_u8) << 1200_usize).to_string();
    files.put(
        "circuit.magna",
        &format!("X_ERROR(1/{denominator}) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]"),
    );
    files.put("task.json", r#"{"error_observables":[0]}"#);
    let value = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--json",
        ],
        0,
    );
    assert!(value["result"]["masses"]["error"]["value"].is_null());
    assert_eq!(
        value["result"]["masses"]["error"]["scientific"],
        json!({"mantissa":0.5,"exponent2":-1199})
    );
    let human = files.run(&["enumerate", "circuit.magna", "--task", "task.json"]);
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("Covered joint error mass: 0.5 * 2^-1199")
    );
    files.put(
        "circuit.magna",
        "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    let value = files.json(
        &[
            "enumerate",
            "circuit.magna",
            "--task",
            "task.json",
            "--json",
        ],
        0,
    );
    assert!(value["result"]["conditional_error"].is_null());
    let human = files.run(&["enumerate", "circuit.magna", "--task", "task.json"]);
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("Full conditional error interval: n/a")
    );
}

#[test]
fn oversized_decimal_weight_cutoffs_cover_the_finite_sector_and_preserve_quotas() {
    let files = Files::new();
    files.simple();
    let huge = "9".repeat(512);
    for cutoff in [
        "2",
        "18446744073709551615",
        "340282366920938463463374607431768211456",
        "+000000000000000000000000000000000000002",
        huge.as_str(),
    ] {
        let value = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--task",
                "task.json",
                "--max-weight",
                cutoff,
                "--json",
            ],
            0,
        );
        assert_eq!(value["result"]["patterns"], 4, "cutoff={cutoff}");
        assert_eq!(value["result"]["masses"]["accepted"]["value"], 1.0);
        assert_eq!(
            value["result"]["masses"]["error"]["exact"]["coefficients"][0],
            json!({"numerator":"2","denominator":"5"})
        );
        assert_eq!(
            value["result"]["covered_probability"],
            json!({"numerator":"1","denominator":"1"})
        );
        assert_eq!(
            value["result"]["omitted_probability"],
            json!({"numerator":"0","denominator":"1"})
        );
        assert!(value["fingerprints"]["options"].is_string());
        for (flag, limit) in [
            ("--max-patterns", "3"),
            ("--max-fault-entries", "0"),
            ("--max-queries", "0"),
        ] {
            let failed = files.json(
                &[
                    "enumerate",
                    "circuit.magna",
                    "--task",
                    "task.json",
                    "--max-weight",
                    cutoff,
                    flag,
                    limit,
                    "--json",
                ],
                1,
            );
            assert_eq!(failed["failure_phase"], "enumeration");
            assert!(failed["result"].is_null());
            assert_eq!(failed["execution_costs"]["query"]["attempted"], 0);
        }
    }
    for cutoff in [
        format!("{huge}x"),
        format!("-{huge}"),
        format!("{huge}.0"),
        "+".to_owned(),
    ] {
        let value = files.json(
            &[
                "enumerate",
                "missing.magna",
                "--max-weight",
                &cutoff,
                "--json",
            ],
            2,
        );
        assert!(
            value["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("max-weight")
        );
    }
}

#[test]
fn projected_target_norms_are_exported_without_rescaling_physical_masses() {
    let files = Files::new();
    files.put("circuit.magna", "X_ERROR(1/5) 0");
    files.put("task.json", r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}}}"#);
    for (target, coefficients, power) in [
        ("H 0\nCX 0 1", [1, 0, 0, 0], -1),
        ("H 0\nT 0\nH 0\nI 1", [2, 1, 0, -1], -2),
    ] {
        // Projecting Bell gives norm1/2; projecting HTH gives (2+sqrt(2))/4.
        // The selected target output is zero in both normalized states.
        files.put("target.magna", target);
        let value = files.json(
            &[
                "enumerate",
                "circuit.magna",
                "--task",
                "task.json",
                "--json",
            ],
            0,
        );
        let result = &value["result"];
        assert_eq!(
            result["target_normalization"]["coefficients"],
            json!(coefficients.map(|n| n.to_string()))
        );
        assert_eq!(result["target_normalization"]["power"], power);
        assert_eq!(result["target_normalization"]["scale"], "2**power");
        for (name, numerator, denominator) in [
            ("accepted", "1", "1"),
            ("error", "1", "5"),
            ("target_success", "4", "5"),
        ] {
            assert_eq!(
                result["masses"][name]["exact"]["coefficients"][0],
                json!({"numerator":numerator,"denominator":denominator})
            );
            for k in 1..4 {
                assert_eq!(
                    result["masses"][name]["exact"]["coefficients"][k],
                    json!({"numerator":"0","denominator":"1"})
                );
            }
        }
        assert_eq!(value["execution_costs"]["query"]["attempted"], 5);
        assert_eq!(result["patterns"], 2);
        assert_eq!(
            result["covered_probability"],
            json!({"numerator":"1","denominator":"1"})
        );
    }
}
