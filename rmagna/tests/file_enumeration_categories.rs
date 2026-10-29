use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, OriginalNoiseOptions};
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{TaskOptions, parse_task};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn loaded(source: &str, registry: &ControllerRegistry, options: FileTaskOptions) -> FileTask {
    let parse = ParseOptions {
        source: Some("metadata-input.magna".to_owned()),
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(source, &parse).unwrap();
    let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    FileTask::from_loaded(
        physical,
        modules,
        registry,
        NoiseSampling::Physical,
        options,
        BTreeMap::new(),
    )
    .unwrap()
}
fn patterns() -> FileEnumerationOptions {
    let mut options = FileEnumerationOptions::default();
    options.enumeration.include_patterns = true;
    options
}
fn pair(value: &Value) -> Value {
    json!({"numerator":value[0].as_i64().unwrap().to_string(),
        "denominator":value[1].as_i64().unwrap().to_string()})
}
fn location(actual: &Value, expected: &Value) {
    assert_eq!(actual["source"], expected["filename"]);
    for field in ["line", "column", "repeat"] {
        assert_eq!(actual[field], expected[field], "{field}");
    }
}

#[test]
fn all_pinned_native_categories_export_exact_actions_sources_and_probabilities() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/original_noise_categories.json")).unwrap();
    assert_eq!(
        fixture["source_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 29);
    let mut checked = 0;
    for case in fixture["cases"].as_array().unwrap() {
        if case["rejected"] == true {
            assert!(
                parse_circuit(case["text"].as_str().unwrap(), &ParseOptions::default()).is_err()
            );
            continue;
        }
        let mut registry = ControllerRegistry::default();
        registry
            .register("opaque", "metadata-v1", |_: &[bool]| Ok(vec![false]))
            .unwrap();
        let file = loaded(
            case["text"].as_str().unwrap(),
            &registry,
            FileTaskOptions::default(),
        );
        let mut options = patterns();
        options.enumeration.noise.max_weight = 0;
        let compiled = file.compile().unwrap();
        let fresh = file.enumerate(&options).unwrap();
        let reused = compiled.enumerate(&options).unwrap();
        assert!(
            fresh.is_complete(),
            "{}: {:?}",
            case["name"],
            fresh.raw_result()
        );
        assert_eq!(fresh.raw_result(), reused.raw_result(), "{}", case["name"]);
        let value = fresh.to_json().unwrap();
        assert_eq!(
            value["original_noise_categories"],
            reused.to_json().unwrap()["original_noise_categories"]
        );
        let table = value["original_noise_categories"].as_array().unwrap();
        assert_eq!(
            table.len() as u64,
            case["categories"].as_u64().unwrap(),
            "{}",
            case["name"]
        );
        let instructions = case["circuit"]["instructions"].as_array().unwrap();
        let mut channel_count = 0;
        for (index, category) in table.iter().enumerate() {
            assert_eq!(category["original_category"], index);
            let probabilities = case["probabilities"][index].as_array().unwrap();
            let actions = case["actions"][index].as_array().unwrap();
            let channels = category["channels"].as_array().unwrap();
            assert_eq!(channels.len(), probabilities.len());
            let sum = Probability::checked_sum(
                probabilities
                    .iter()
                    .map(|p| format!("{}/{}", p[0], p[1]).parse::<Probability>().unwrap()),
            )
            .unwrap()
            .complement();
            assert_eq!(
                category["identity_probability"],
                json!({"numerator":sum.value().numer().to_string(),"denominator":sum.value().denom().to_string()})
            );
            let mut sources = Vec::new();
            for (j, channel) in channels.iter().enumerate() {
                channel_count += 1;
                assert_eq!(channel["channel"], j);
                assert_eq!(channel["probability"], pair(&probabilities[j]));
                let action = &actions[j];
                let native_source = if action[0] == "report" {
                    let record = action[1]
                        .as_str()
                        .unwrap()
                        .strip_prefix('m')
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    assert_eq!(
                        channel["action"],
                        json!({"kind":"report_flip","record":record})
                    );
                    instructions
                        .iter()
                        .find(|inst| inst["records"].as_array().unwrap().contains(&json!(record)))
                        .unwrap()
                } else {
                    let factors = action[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|g| json!({"qubit":g["qubits"][0],"basis":g["kind"]}))
                        .collect::<Vec<_>>();
                    assert_eq!(channel["action"], json!({"kind":"pauli","factors":factors}));
                    instructions
                        .iter()
                        .find(|inst| inst["op_id"] == action[0])
                        .unwrap()
                };
                location(&channel["source"], &native_source["source"]);
                let source = channel["source"].clone();
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            assert_eq!(category["source_operations"], json!(sources));
        }
        assert_eq!(channel_count as u64, case["action_count"].as_u64().unwrap());
        assert_eq!(
            table.iter().filter(|c| !c["guard"].is_null()).count() as u64,
            case["guarded"].as_u64().unwrap()
        );
        options.enumeration.include_patterns = false;
        let baseline = file.enumerate(&options).unwrap();
        let a = fresh.raw_result().unwrap();
        let b = baseline.raw_result().unwrap();
        assert_eq!(a.masses, b.masses);
        assert_eq!(a.by_weight, b.by_weight);
        assert_eq!(a.covered_probability, b.covered_probability);
        assert_eq!(a.omitted_probability, b.omitted_probability);
        checked += 1;
    }
    assert_eq!(checked, 26);
}

#[test]
fn implicit_readout_gap_keeps_internal_choice_and_adds_original_address() {
    let file = loaded(
        "M 0\nX_ERROR(1/3) 0",
        &ControllerRegistry::default(),
        FileTaskOptions::default(),
    );
    let options = patterns();
    let report = file.enumerate(&options).unwrap();
    let compiled = file.compile().unwrap();
    assert_eq!(
        report.raw_result(),
        compiled.enumerate(&options).unwrap().raw_result()
    );
    let value = report.to_json().unwrap();
    let category = &value["original_noise_categories"][0];
    assert_eq!(category["internal_site"], 1);
    assert_eq!(
        category["identity_probability"],
        json!({"numerator":"2","denominator":"3"})
    );
    assert_eq!(
        category["channels"][0]["action"],
        json!({"kind":"pauli","factors":[{"qubit":0,"basis":"X"}]})
    );
    assert_eq!(
        category["source_operations"],
        json!([{"operation":1,"source":"metadata-input.magna","line":2,"column":1,"repeat":[]}])
    );
    let rows = value["result"]["pattern_responses"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["choices"], json!([]));
    assert_eq!(
        rows[1]["choices"],
        json!([{"category":1,"channel":0,"original_category":0}])
    );
    assert_eq!(
        rows[1]["probability"],
        json!({"numerator":"1","denominator":"3"})
    );
    assert_eq!(rows[1]["response"]["accepted"]["value"], 1.0);
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-file-categories-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn captured_physical_table_excludes_recovery_and_target_after_remapping_and_deletion() {
    for measurement in ["M 0", "M(0) 0"] {
        let files = Files::new();
        files.put("physical.magna", "M 4\nX_ERROR(1/3) 4");
        files.put("recovery.magna", &format!("{measurement}\nX 0"));
        files.put("target.magna", "H 0\nCX 0 1");
        files.put("task.json", r#"{"outputs":[4],"recovery":"recovery.magna","ideal_state":{"circuit":"target.magna","outputs":[0],"postselection":{"1":0}}}"#);
        let file = FileTask::load(
            &files.0.join("physical.magna"),
            Some(&files.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        std::fs::remove_dir_all(&files.0).unwrap();
        let compiled = file.compile().unwrap();
        let fresh = file.enumerate(&patterns()).unwrap();
        let reused = compiled.enumerate(&patterns()).unwrap();
        assert_eq!(fresh.raw_result(), reused.raw_result());
        assert_eq!(
            fresh.to_json().unwrap()["original_noise_categories"],
            reused.metadata()["original_noise_categories"]
        );
        drop(compiled);
        drop(file);
        let value = fresh.to_json().unwrap();
        let table = value["original_noise_categories"].as_array().unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0]["internal_site"], 1);
        assert_eq!(
            table[0]["channels"][0]["action"],
            json!({"kind":"pauli","factors":[{"qubit":4,"basis":"X"}]})
        );
        assert!(
            table[0]["channels"][0]["source"]["source"]
                .as_str()
                .unwrap()
                .ends_with("physical.magna")
        );
        assert_eq!(value["result"]["masses"]["accepted"]["value"], 1.0);
        assert_eq!(
            value["result"]["masses"]["target_success"]["exact"]["coefficients"][0],
            json!({"numerator":"1","denominator":"3"})
        );
    }
}

#[test]
fn guarded_latent_assignments_export_canonical_decision_and_record_guards_in_both_strategies() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("copy", "1", move |bits: &[bool]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(bits.to_vec())
            })
            .unwrap();
        let mut file_options = FileTaskOptions::default();
        file_options.evaluation.runtime.strategy = strategy;
        let file = loaded(
            "H 0\nM 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCONTROL[copy] ctrl[0] -> ctrl[1]\nX_ERROR(1/3) ctrl[1] 1\nZ_ERROR(0) ctrl[0] 1\nM 1",
            &registry,
            file_options,
        );
        let report = file.enumerate(&patterns()).unwrap();
        let retained_calls = calls.swap(0, Ordering::SeqCst);
        let baseline = file.enumerate(&FileEnumerationOptions::default()).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), retained_calls);
        assert_eq!(
            report.raw_result().unwrap().masses,
            baseline.raw_result().unwrap().masses
        );
        let value = report.to_json().unwrap();
        let table = &value["original_noise_categories"];
        assert_eq!(
            table[0]["guard"],
            json!({"constant":false,"terms":[{"kind":"decision","call":0,"bit":0}]})
        );
        assert_eq!(
            table[1]["guard"],
            json!({"constant":false,"terms":[{"kind":"record","record":0}]})
        );
        let rows = value["result"]["pattern_responses"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1]["choices"],
            json!([{"category":1,"channel":0,"original_category":0}])
        );
        assert_eq!(rows[1]["histories"], 2);
        assert_eq!(rows[1]["response"]["accepted"]["value"], 1.0);
    }
}

#[test]
fn metadata_limits_fail_before_fresh_preparation_or_callbacks_and_also_bound_reuse() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("opaque", "1", move |_: &[bool]| {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(vec![false])
        })
        .unwrap();
    let file = loaded(
        "CONTROL[opaque] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 0",
        &registry,
        FileTaskOptions::default(),
    );
    let compiled = file.compile().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for (categories, channels, bits, field) in [
        (0, 1, 2, "max_categories"),
        (1, 0, 2, "max_channels"),
        (1, 1, 1, "max_probability_bits"),
    ] {
        let mut options = patterns();
        options.original_noise = OriginalNoiseOptions {
            max_categories: categories,
            max_channels: channels,
        };
        options.max_original_probability_bits = bits;
        options.execution_limits = ExecutionLimits {
            max_queries: 0,
            max_duration: Some(std::time::Duration::ZERO),
        };
        for result in [file.enumerate(&options), compiled.enumerate(&options)] {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("enumeration.original_noise") && error.contains(field),
                "{error}"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    }
    let mut admitted = patterns();
    admitted.original_noise = OriginalNoiseOptions {
        max_categories: 1,
        max_channels: 1,
    };
    admitted.max_original_probability_bits = 2;
    assert!(file.enumerate(&admitted).unwrap().is_complete());
    assert!(calls.load(Ordering::SeqCst) > 0);
}

#[test]
fn patterns_disabled_skips_all_metadata_limits_and_each_limit_changes_the_fingerprint() {
    let file = loaded(
        "M 0\nDEPOLARIZE1(1/3) 0",
        &ControllerRegistry::default(),
        FileTaskOptions::default(),
    );
    let baseline = file.enumerate(&FileEnumerationOptions::default()).unwrap();
    let disabled = FileEnumerationOptions {
        original_noise: OriginalNoiseOptions {
            max_categories: 0,
            max_channels: 0,
        },
        max_original_probability_bits: 0,
        ..Default::default()
    };
    let skipped = file.enumerate(&disabled).unwrap();
    assert!(skipped.is_complete());
    assert_eq!(baseline.raw_result(), skipped.raw_result());
    assert!(skipped.metadata()["original_noise_categories"].is_null());
    assert!(skipped.to_json().unwrap()["result"]["pattern_responses"].is_null());
    let original =
        file.enumerate(&patterns()).unwrap().metadata()["fingerprints"]["options"].clone();
    for index in 0..3 {
        let mut options = patterns();
        match index {
            0 => options.original_noise.max_categories -= 1,
            1 => options.original_noise.max_channels -= 1,
            _ => options.max_original_probability_bits -= 1,
        }
        let report = file.enumerate(&options).unwrap();
        assert_ne!(report.metadata()["fingerprints"]["options"], original);
        assert_eq!(
            report.metadata()["options"]["enumeration"]["original_noise"],
            json!({
                "max_categories":options.original_noise.max_categories,
                "max_channels":options.original_noise.max_channels,
                "max_probability_bits":options.max_original_probability_bits,
            })
        );
    }
}

#[test]
fn correlated_probability_budget_checks_final_exclusive_and_identity_integers() {
    let file = loaded(
        "CORRELATED_ERROR(1/5) X0\nELSE_CORRELATED_ERROR(1/5) Y0",
        &ControllerRegistry::default(),
        FileTaskOptions::default(),
    );
    let mut options = patterns();
    // Individual inputs fit three bits; 4/25 and the 16/25 identity need five.
    options.max_original_probability_bits = 4;
    assert!(
        file.enumerate(&options)
            .unwrap_err()
            .to_string()
            .contains("max_probability_bits requires 5")
    );
    options.max_original_probability_bits = 5;
    let value = file.enumerate(&options).unwrap().to_json().unwrap();
    assert_eq!(
        value["original_noise_categories"][0]["identity_probability"],
        json!({"numerator":"16","denominator":"25"})
    );
    assert_eq!(
        value["original_noise_categories"][0]["channels"][1]["probability"],
        json!({"numerator":"4","denominator":"25"})
    );
}

#[test]
fn owned_category_metadata_survives_failure_and_strict_fingerprint_preflight_is_preserved() {
    let registry = ControllerRegistry::default();
    let file = loaded("M 0\nX_ERROR(1/3) 0", &registry, FileTaskOptions::default());
    let mut options = patterns();
    options.execution_limits.max_queries = 0;
    let report = file.enumerate(&options).unwrap();
    assert!(!report.is_complete());
    let value = report.to_json().unwrap();
    assert!(value["result"].is_null());
    assert_eq!(
        value["original_noise_categories"].as_array().unwrap().len(),
        1
    );
    assert_eq!(value["original_noise_categories"][0]["internal_site"], 1);
    let mut configuration = file
        .inspect(&rmagna::task::TaskRunOptions::default())
        .unwrap()["options"]
        .clone();
    configuration.as_object_mut().unwrap().remove("run");
    let mut bounded = FileTaskOptions::default();
    bounded.fingerprints.max_json_bytes = serde_json::to_vec(&configuration).unwrap().len();
    let file = loaded("M 0\nX_ERROR(1/3) 0", &registry, bounded);
    options.original_noise.max_categories = 0;
    let error = file.enumerate(&options).unwrap_err();
    assert!(
        matches!(error, rmagna::file_api::FileApiError::Fingerprint(_)),
        "{error}"
    );
}
