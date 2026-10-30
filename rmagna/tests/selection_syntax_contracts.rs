//! Complete selection-syntax contracts from pinned
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_selection_syntax.py. Public Rust/CLI semantics, not Python ABI.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{TaskEnumeration, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-selection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn load(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        compiler: CompilerStrategy,
        causal: CausalStrategy,
    ) -> FileTask {
        self.put("c.magna", source);
        self.put("task.json", task);
        let mut configured = FileTaskOptions::default();
        configured.evaluation.runtime.compile.compiler_strategy = compiler;
        configured.evaluation.runtime.strategy = causal;
        FileTask::load(
            &self.0.join("c.magna"),
            Some(&self.0.join("task.json")),
            registry,
            NoiseSampling::Physical,
            configured,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
    fn totals(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        compiler: CompilerStrategy,
        causal: CausalStrategy,
    ) -> (TaskEnumeration, Value) {
        let file = self.load(source, task, registry, compiler, causal);
        let prepared = file.compile().unwrap();
        let mut options = FileEnumerationOptions::default();
        options.enumeration.noise.max_weight = 4;
        let fresh = file.enumerate(&options).unwrap();
        assert!(fresh.is_complete(), "{fresh:?}");
        let raw = fresh.raw_result().unwrap().clone();
        assert_eq!(raw.covered_probability, Probability::one());
        assert_eq!(raw.omitted_probability, Probability::zero());
        let value = fresh.to_json().unwrap();
        assert_eq!(value["status"], "complete");
        assert_eq!(value["circuit"]["format"], "magna-0.1");
        for _ in 0..2 {
            let reused = prepared.enumerate(&options).unwrap();
            assert!(reused.is_complete());
            assert_eq!(reused.raw_result().unwrap(), &raw);
            assert_eq!(
                reused.to_json().unwrap()["fingerprints"],
                value["fingerprints"]
            );
        }
        (raw, value)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn strategies() -> [(CompilerStrategy, CausalStrategy); 4] {
    [
        (
            CompilerStrategy::ParityCopies,
            CausalStrategy::FixedTemplate,
        ),
        (CompilerStrategy::ParityCopies, CausalStrategy::ColdPrefix),
        (
            CompilerStrategy::SharedCommutingLayer,
            CausalStrategy::FixedTemplate,
        ),
        (
            CompilerStrategy::SharedCommutingLayer,
            CausalStrategy::ColdPrefix,
        ),
    ]
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
fn selected(sign: i64) -> ExactField {
    // sqrt(2)=omega-omega^3: independent literal 1/2 +/- sqrt(2)/5.
    FieldArithmetic::default()
        .value([
            rational(1, 2),
            rational(sign, 5),
            rational(0, 1),
            rational(-sign, 5),
        ])
        .unwrap()
}
fn exact_json(value: &Value, name: &str, expected: &ExactField) {
    for (j, c) in expected.coefficients().iter().enumerate() {
        assert_eq!(
            value["result"]["masses"][name]["exact"]["coefficients"][j],
            json!({"numerator":c.numer().to_string(),"denominator":c.denom().to_string()})
        );
    }
}
fn majority() -> ControllerRegistry {
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", |bits: &[bool]| {
            assert_eq!(bits.len(), 3);
            Ok(vec![bits.iter().filter(|&&b| b).count() >= 2])
        })
        .unwrap();
    registry
}

#[test]
fn original_complemented_detector_selects_the_irrational_one_mass() {
    let files = Files::new();
    let base = "H 0\nT 0\nH 0\nX_ERROR(1/10) 0\nM 0\nDETECTOR rec[-1]\n";
    for (compiler, causal) in strategies() {
        let (zero, zjson) = files.totals(
            &(base.to_owned() + "POSTSELECT det[0]"),
            "{}",
            &ControllerRegistry::default(),
            compiler,
            causal,
        );
        let (one, ojson) = files.totals(
            &(base.to_owned() + "POSTSELECT !det[0]"),
            "{}",
            &ControllerRegistry::default(),
            compiler,
            causal,
        );
        assert_eq!(one.masses.accepted, selected(-1));
        assert_eq!(zero.masses.accepted, selected(1));
        assert_eq!(
            FieldArithmetic::default()
                .add(&one.masses.accepted, &zero.masses.accepted)
                .unwrap(),
            ExactField::one()
        );
        exact_json(&ojson, "accepted", &selected(-1));
        exact_json(&zjson, "accepted", &selected(1));
        assert!(
            (ojson["result"]["masses"]["accepted"]["value"]
                .as_f64()
                .unwrap()
                - (0.5 - 0.2 * 2.0_f64.sqrt()))
            .abs()
                < 1e-15
        );
        assert_eq!(
            parse_circuit(
                &(base.to_owned() + "POSTSELECT !det[0]"),
                &ParseOptions::default()
            )
            .unwrap()
            .format_version(),
            "magna-0.1"
        );
    }
}

#[test]
fn original_complemented_majority_register_equals_explicit_not_at_half_acceptance() {
    let files = Files::new();
    let base = "H 0\nM(1/10) 0\nM(1/10) 0\nM(1/10) 0\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\n";
    for (compiler, causal) in strategies() {
        let registry = majority();
        let (explicit, ejson) = files.totals(
            &(base.to_owned() + "CONTROL[NOT] ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]"),
            "{}",
            &registry,
            compiler,
            causal,
        );
        let (direct, djson) = files.totals(
            &(base.to_owned() + "POSTSELECT !ctrl[0]"),
            "{}",
            &registry,
            compiler,
            causal,
        );
        assert_eq!(direct.masses.accepted, field(1, 2));
        assert_eq!(explicit.masses.accepted, field(1, 2));
        assert_eq!(direct.masses.accepted, explicit.masses.accepted);
        assert_eq!(
            djson["result"]["masses"]["accepted"]["exact"],
            ejson["result"]["masses"]["accepted"]["exact"]
        );
        assert_eq!(djson["result"]["masses"]["accepted"]["value"], 0.5);
        exact_json(&djson, "accepted", &field(1, 2));
    }
}

#[test]
fn original_majority_observable_integrates_three_independent_readout_flips() {
    let files = Files::new();
    let source = "X_ERROR(1/5) 0\nM(1/10) 0\nM(1/10) 0\nM(1/10) 0\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nOBSERVABLE_INCLUDE(0) ctrl[0]";
    // Wrong majority probability is 7/250. Combining true-error probability
    // 1/5 gives (243+28)/1250, independent of the simulator's enumeration.
    for (compiler, causal) in strategies() {
        let (raw, value) = files.totals(
            source,
            r#"{"error_observables":[0]}"#,
            &majority(),
            compiler,
            causal,
        );
        assert_eq!(raw.masses.error, field(271, 1250));
        assert_eq!(raw.masses.accepted, ExactField::one());
        assert_eq!(raw.masses.target_success, None);
        exact_json(&value, "error", &field(271, 1250));
        assert!(
            (value["result"]["masses"]["error"]["value"]
                .as_f64()
                .unwrap()
                - 0.2168)
                .abs()
                < 1e-15
        );
        let mut options = ParseOptions::default();
        options.controllers.insert("majority".into());
        assert_eq!(
            parse_circuit(source, &options).unwrap().format_version(),
            "magna-0.1"
        );
    }
}

#[test]
fn original_observable_captures_register_before_overwrite_and_cancels_its_record() {
    let files = Files::new();
    for (source, error) in [
        (
            "CONTROL[XOR](1) -> ctrl[0]\nOBSERVABLE_INCLUDE(0) ctrl[0]\nCONTROL[XOR](0) -> ctrl[0]",
            1,
        ),
        (
            "X 0\nM 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nOBSERVABLE_INCLUDE(0) rec[-1] ctrl[0]",
            0,
        ),
    ] {
        let parsed = parse_circuit(source, &ParseOptions::default()).unwrap();
        let captured = parsed.observables().get(&0).unwrap();
        assert_eq!(captured.constant, error == 1);
        assert!(captured.terms.is_empty());
        for (compiler, causal) in strategies() {
            let (raw, value) = files.totals(
                source,
                r#"{"error_observables":[0]}"#,
                &ControllerRegistry::default(),
                compiler,
                causal,
            );
            assert_eq!(raw.masses.error, field(error, 1));
            assert_eq!(raw.masses.accepted, ExactField::one());
            exact_json(&value, "error", &field(error, 1));
        }
    }
}

#[test]
fn original_removed_task_fields_name_circuit_migrations_before_module_loading() {
    let files = Files::new();
    files.put("c.magna", "M 0\nDETECTOR rec[-1]");
    for (name, old_value, hint) in [
        ("detectors", json!({"0":0}), "POSTSELECT det"),
        (
            "postselection",
            json!([[0]]),
            "after a DETECTOR for each parity",
        ),
        ("outcomes", json!([0]), "POSTSELECT det"),
        ("error_checks", json!([[0]]), "OBSERVABLE_INCLUDE"),
        (
            "decoded_error",
            json!({"controller":"x","inputs":[]}),
            "OBSERVABLE_INCLUDE",
        ),
    ] {
        let mut task = json!({"ideal_state":"missing-target.magna"});
        task[name] = old_value;
        let text = task.to_string();
        let error = parse_task(&text, &files.0, &TaskOptions::default()).unwrap_err();
        assert_eq!(error.field, name);
        assert!(error.message.contains("no longer supported"));
        assert!(error.message.contains(hint), "{error}");
        assert!(error.to_string().contains(name));
        files.put("task.json", &text);
        let error = FileTask::load(
            &files.0.join("c.magna"),
            Some(&files.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .expect_err("removed fields must reject before module loading");
        assert!(
            error.to_string().contains(name) && error.to_string().contains(hint),
            "{error}"
        );
        let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&files.0)
            .args(["validate", "c.magna", "--task", "task.json", "--json"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        let message = value["failures"][0]["message"].as_str().unwrap();
        assert!(
            message.contains(name) && message.contains(hint),
            "{message}"
        );
        assert!(!files.0.join("missing-target.magna").exists());
    }
    // The removed field is root-only; projected target modules still retain
    // their independently supported nested postselection spelling.
    files.put("ideal.magna", "I 0\nH 1");
    let spec = parse_task(
        r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":0}}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    assert_eq!(
        spec.ideal_state().unwrap().postselection().get(&1),
        Some(&false)
    );
    assert!(
        spec.load_modules(
            &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
            &ParseOptions::default()
        )
        .is_ok()
    );
}

#[test]
fn original_external_controls_reject_records_with_detector_register_alternatives() {
    let mut options = ParseOptions {
        source: Some("selection-original.magna".into()),
        ..ParseOptions::default()
    };
    options.controllers.insert("majority".into());
    for source in [
        "M 0\nCONTROL[majority] rec[-1] -> ctrl[0]",
        "M 0\nCONTROL[majority] !rec[-1] -> ctrl[0]",
    ] {
        let error = parse_circuit(source, &options).unwrap_err();
        assert!(
            error
                .message
                .contains("external CONTROL inputs must be det[...] or ctrl[...]")
        );
        assert_eq!(error.location.line, 2);
        assert_eq!(
            error.location.source.as_deref(),
            Some("selection-original.magna")
        );
    }
    for source in [
        "M 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCX rec[-1] 0",
        "M 0\nCONTROL[NOT] rec[-1] -> ctrl[0]\nCX rec[-1] 0",
        "M 0\nDETECTOR rec[-1]\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCONTROL[majority] det[0] ctrl[0] !det[0] -> ctrl[1]",
    ] {
        assert_eq!(
            parse_circuit(source, &options).unwrap().format_version(),
            "magna-0.1"
        );
    }
}

#[test]
fn original_invalid_annotation_forms_keep_allowed_form_hints_and_source_locations() {
    let options = ParseOptions {
        source: Some("selection-original.magna".into()),
        ..ParseOptions::default()
    };
    for (source, hint, line) in [
        (
            "M 0\nPOSTSELECT rec[-1]",
            "POSTSELECT targets must be det",
            2,
        ),
        (
            "M 0\nDETECTOR rec[-1]\nPOSTSELECT(1) det[0]",
            "write !det",
            3,
        ),
        (
            "CONTROL[XOR] -> ctrl[0]\nOBSERVABLE_INCLUDE(0) !ctrl[0]",
            "OBSERVABLE_INCLUDE targets",
            2,
        ),
        (
            "CONTROL[XOR] -> ctrl[0]\nDETECTOR ctrl[0]",
            "DETECTOR targets must be measurement records",
            2,
        ),
        (
            "M 0\nOBSERVABLE_INCLUDE(0) !rec[-1]",
            "non-complemented rec",
            2,
        ),
        ("M 0\nDETECTOR !rec[-1]", "without ! complements", 2),
        (
            "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) det[0]",
            "non-complemented rec",
            3,
        ),
    ] {
        let error = parse_circuit(source, &options).unwrap_err();
        assert!(error.message.contains(hint), "{source}: {error}");
        assert_eq!(
            error.location.source.as_deref(),
            Some("selection-original.magna")
        );
        assert_eq!(error.location.line, line);
        assert_eq!(error.location.column, 1);
        assert!(error.location.repeat.is_empty());
        assert_eq!(parse_circuit("M 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\nCONTROL[XOR] rec[-1] -> ctrl[0]\nOBSERVABLE_INCLUDE(0) rec[-1] ctrl[0]",&options).unwrap().format_version(),"magna-0.1");
    }
    let repeated = parse_circuit("REPEAT 2 {\nM 0\nDETECTOR !rec[-1]\n}", &options).unwrap_err();
    assert_eq!(repeated.location.line, 3);
    assert_eq!(repeated.location.repeat, [0]);
    assert!(repeated.message.contains("without ! complements"));
}
