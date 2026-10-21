use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::task::{TaskOptions, parse_task};
use std::path::{Path, PathBuf};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-task-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
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
fn physical(text: &str) -> rmagna::circuit::Circuit {
    parse_circuit(text, &ParseOptions::default()).unwrap()
}
#[test]
fn task_defaults_ordered_outputs_and_arbitrary_precision_pauli_masks_are_exact() {
    let circuit = physical("I 128");
    let defaults = parse_task("{}", Path::new("/tmp"), &TaskOptions::default())
        .unwrap()
        .validate(&circuit)
        .unwrap();
    assert_eq!(defaults.outputs(), &(0..129).collect::<Vec<_>>());
    let mask = rmagna::BigInt::from(1) << 128;
    let text =
        format!(r#"{{"schema_version":1,"observables":[[{mask},{mask}]],"sample_records":[]}}"#);
    let parsed = parse_task(&text, Path::new("/tmp"), &TaskOptions::default()).unwrap();
    let task = parsed.validate(&circuit).unwrap();
    assert_eq!(task.observables().len(), 1);
    let word = &task.observables()[0];
    assert_eq!(word.phase(), 2);
    assert!(word.x().get(128).unwrap());
    assert!(word.z().get(128).unwrap());
    assert_eq!(word.x().ones().count(), 1);
    let ordered = parse_task(
        r#"{"outputs":[2,0]}"#,
        Path::new("/tmp"),
        &TaskOptions::default(),
    )
    .unwrap()
    .validate(&physical("I 2"))
    .unwrap();
    assert_eq!(ordered.outputs(), [2, 0]);
}
#[test]
fn raw_mask_tokens_preserve_low_bits_and_reject_noninteger_json_shapes() {
    let wide = (rmagna::BigInt::from(1) << 512) + 3;
    let text = format!(r#"{{"observables":[[{wide},0]]}}"#);
    let task = parse_task(&text, Path::new("."), &TaskOptions::default())
        .unwrap()
        .validate(&physical("I 512"))
        .unwrap();
    assert_eq!(
        task.observables()[0].x().ones().collect::<Vec<_>>(),
        [0, 1, 512]
    );
    for token in [
        "\"123\"",
        "true",
        "null",
        "[]",
        "{}",
        "1.0",
        "1e2",
        "1e10000000",
        "-1",
        r#"{"$serde_json::private::Number":"7"}"#,
    ] {
        let text = format!(r#"{{"observables":[[{token},0]]}}"#);
        let error = parse_task(&text, Path::new("."), &TaskOptions::default()).unwrap_err();
        assert_eq!(error.field, "observables[0]", "{token}: {error}");
    }
    for text in [
        "null",
        "[]",
        r#"{"observables":null}"#,
        r#"{"observables":[1]}"#,
    ] {
        assert!(parse_task(text, Path::new("."), &TaskOptions::default()).is_err());
    }
}
#[test]
fn classical_observables_and_records_validate_against_the_physical_contract() {
    let circuit = physical("M 0\nOBSERVABLE_INCLUDE(4) rec[-1]");
    let parsed = parse_task(
        r#"{"error_observables":[4],"sample_records":[0,0]}"#,
        Path::new("/tmp"),
        &TaskOptions::default(),
    )
    .unwrap();
    let task = parsed.validate(&circuit).unwrap();
    assert!(task.outputs().is_empty());
    assert_eq!(task.error_observables(), Some([4].as_slice()));
    assert_eq!(task.sample_records(), [0, 0]);
    for text in [
        r#"{"error_observables":[5]}"#,
        r#"{"sample_records":[1]}"#,
        r#"{"outputs":[0,0]}"#,
        r#"{"outputs":[1]}"#,
        r#"{"outputs":[],"observables":[[1,0]]}"#,
    ] {
        assert!(
            parse_task(text, Path::new("/tmp"), &TaskOptions::default())
                .unwrap()
                .validate(&circuit)
                .is_err(),
            "{text}"
        );
    }
}
#[test]
fn schema_unknown_fields_nulls_numbers_and_resource_limits_fail_explicitly() {
    for text in [
        "[]",
        r#"{"output":[0]}"#,
        r#"{"schema_version":2}"#,
        r#"{"schema_version":true}"#,
        r#"{"outputs":null}"#,
        r#"{"outputs":[1.0]}"#,
        r#"{"outputs":[true]}"#,
        r#"{"outputs":[-1]}"#,
        r#"{"observables":[[0.5,0]]}"#,
        r#"{"observables":[[-1,0]]}"#,
        r#"{"observables":[[0,0,0]]}"#,
        r#"{"error_observables":null}"#,
        r#"{"recovery":"r.magna"}"#,
        r#"{"ideal_state":"i.magna","error_observables":[]}"#,
        r#"{"ideal_state":{"circuit":"i","outputs":[],"postselection":{"0":true}}}"#,
        r#"{"ideal_state":{"circuit":"i","outputs":[],"postselection":{"0":0,"00":1}}}"#,
    ] {
        assert!(
            parse_task(text, Path::new("/tmp"), &TaskOptions::default()).is_err(),
            "{text}"
        );
    }
    let nulls = parse_task(
        r#"{"ideal_state":null,"recovery":null}"#,
        Path::new("/tmp"),
        &TaskOptions::default(),
    )
    .unwrap();
    assert!(nulls.ideal_state().is_none());
    assert!(
        parse_task(
            "{}",
            Path::new("/tmp"),
            &TaskOptions {
                max_json_bytes: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_task(
            r#"{"outputs":[0,1]}"#,
            Path::new("/tmp"),
            &TaskOptions {
                max_list_items: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_task(
            r#"{"observables":[[8,0]]}"#,
            Path::new("/tmp"),
            &TaskOptions {
                max_mask_bits: 3,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn relative_target_and_recovery_modules_preserve_local_maps_and_empty_zero_wires() {
    let files = Files::new();
    files.put("ideal.magna", "");
    files.put("recovery.magna", "M 0\nCX rec[-1] 0");
    let spec = parse_task(
        r#"{"outputs":[2,0],"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    assert_eq!(
        spec.ideal_state().unwrap().circuit_path(),
        files.0.join("ideal.magna")
    );
    let modules = spec
        .load_modules(&physical("I 2"), &ParseOptions::default())
        .unwrap();
    assert_eq!(modules.task().outputs(), [2, 0]);
    assert_eq!(modules.ideal_state().unwrap().outputs(), [0, 1]);
    assert_eq!(modules.ideal_state().unwrap().circuit().num_qubits(), 2);
    assert_eq!(modules.recovery().unwrap().num_qubits(), 2);
    assert_eq!(modules.recovery().unwrap().num_records(), 1);
    files.put("bell.magna", "H 0\nCX 0 1");
    let spec=parse_task(r#"{"outputs":[0],"ideal_state":{"circuit":"bell.magna","outputs":[1],"postselection":{"0":0}}}"#,&files.0,&TaskOptions::default()).unwrap();
    let modules = spec
        .load_modules(&physical("I 0"), &ParseOptions::default())
        .unwrap();
    let ideal = modules.ideal_state().unwrap();
    assert_eq!(ideal.outputs(), [1]);
    assert_eq!(ideal.postselection().get(&0), Some(&false));
}
#[test]
fn target_metadata_preserves_declarations_but_rejects_classical_execution() {
    let files = Files::new();
    let source = physical("I 0");
    for task in [
        r#"{"ideal_state":"ideal.magna"}"#,
        r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[0]}}"#,
    ] {
        let spec = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
        for text in [
            "DETECTOR\nI 0",
            "DETECTOR(1,2)\nI 0",
            "OBSERVABLE_INCLUDE(4)\nOBSERVABLE_INCLUDE(4)\nI 0",
            "REPEAT 2 {\nDETECTOR(1,2)\nOBSERVABLE_INCLUDE(4)\n}\nI 0",
        ] {
            files.put("ideal.magna", text);
            let loaded = spec
                .load_modules(&source, &ParseOptions::default())
                .unwrap();
            let target = loaded.ideal_state().unwrap();
            assert_eq!(target.outputs(), [0]);
            assert_eq!(target.circuit().num_records(), 0);
            assert_eq!(target.circuit().to_text(), text);
            for expression in target
                .circuit()
                .detectors()
                .iter()
                .chain(target.circuit().observables().values())
            {
                assert!(!expression.constant);
                assert!(expression.terms.is_empty());
            }
        }
        for text in [
            "M 0\nDETECTOR rec[-1]",
            "M 0\nOBSERVABLE_INCLUDE(4) rec[-1] rec[-1]",
            "DETECTOR\nOBSERVABLE_INCLUDE(4) det[0]\nI 0",
            "DETECTOR\nOBSERVABLE_INCLUDE(4) !det[0] !det[0]\nI 0",
            "REPEAT 2 {\nDETECTOR\nOBSERVABLE_INCLUDE(4) det[-1]\n}\nI 0",
            "DETECTOR\nPOSTSELECT det[0]\nI 0",
            "CONTROL[XOR](0) -> ctrl[0]\nDETECTOR\nI 0",
        ] {
            files.put("ideal.magna", text);
            let error = spec
                .load_modules(&source, &ParseOptions::default())
                .unwrap_err();
            assert_eq!(error.field, "ideal_state", "{text}: {error}");
            assert!(error.message.contains("ideal.magna:"), "{text}: {error}");
            assert!(
                error.message.contains("noiseless unitary"),
                "{text}: {error}"
            );
        }
    }
}
#[test]
fn target_and_recovery_validation_rejects_nonunitary_or_noisy_modules() {
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    files.put("recovery.magna", "");
    let source = physical("I 0");
    let spec = parse_task(
        r#"{"ideal_state":"ideal.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    for text in [
        "M 0",
        "R 0",
        "X_ERROR(0) 0",
        "H 1",
        "CONTROL[XOR](1) -> ctrl[0]",
    ] {
        files.put("ideal.magna", text);
        assert!(
            spec.load_modules(&source, &ParseOptions::default())
                .is_err(),
            "{text}"
        );
    }
    files.put("ideal.magna", "I 0");
    let spec = parse_task(
        r#"{"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    for text in [
        "X_ERROR(0) 0",
        "M(1/2) 0",
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
    ] {
        files.put("recovery.magna", text);
        assert!(
            spec.load_modules(&source, &ParseOptions::default())
                .is_err(),
            "{text}"
        );
    }
    let bad = parse_task(
        r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"0":0}}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    assert!(bad.load_modules(&source, &ParseOptions::default()).is_err());
}
#[test]
fn module_reads_are_bounded_and_preserve_file_and_field_context() {
    let files = Files::new();
    files.put("valid-target.magna", "");
    for (field, name, task) in [
        (
            "ideal_state",
            "ideal.magna",
            r#"{"ideal_state":"ideal.magna"}"#,
        ),
        (
            "recovery",
            "recovery.magna",
            r#"{"ideal_state":"valid-target.magna","recovery":"recovery.magna"}"#,
        ),
    ] {
        files.put(name, "I 0");
        let path = files.0.join(name).display().to_string();
        let spec = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
        let error = spec
            .load_modules(
                &physical("I 0"),
                &ParseOptions {
                    max_source_bytes: 2,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.field, field);
        assert!(error.message.contains(&path), "{error}");
        assert!(error.message.contains("max_source_bytes = 2"), "{error}");
        let loaded = spec
            .load_modules(
                &physical("I 0"),
                &ParseOptions {
                    max_source_bytes: 3,
                    ..Default::default()
                },
            )
            .unwrap();
        if field == "ideal_state" {
            assert_eq!(loaded.ideal_state().unwrap().circuit().num_qubits(), 1);
        } else {
            assert_eq!(loaded.recovery().unwrap().num_qubits(), 1);
        }
        std::fs::write(files.0.join(name), [0xff]).unwrap();
        let error = spec
            .load_modules(&physical("I 0"), &ParseOptions::default())
            .unwrap_err();
        assert_eq!(error.field, field);
        assert!(error.message.contains(&path), "{error}");
        assert!(error.message.contains("UTF-8"), "{error}");
        std::fs::remove_file(files.0.join(name)).unwrap();
        let error = spec
            .load_modules(&physical("I 0"), &ParseOptions::default())
            .unwrap_err();
        assert_eq!(error.field, field);
        assert!(error.message.contains(&path), "{error}");
    }
}

#[test]
fn numeric_and_container_types_reject_private_tags_across_serde_features() {
    let options = TaskOptions::default();
    let base = Path::new(".");
    for token in [
        r#"{"$serde_json::private::Number":"1"}"#,
        r#"{"$serde_json::private::RawValue":"1"}"#,
        "1.0",
        "1e0",
        "true",
        "\"1\"",
        "null",
        "[]",
        "{}",
        "18446744073709551616",
    ] {
        for (text, field) in [
            (format!(r#"{{"schema_version":{token}}}"#), "schema_version"),
            (format!(r#"{{"outputs":[{token}]}}"#), "outputs"),
            (
                format!(r#"{{"error_observables":[{token}]}}"#),
                "error_observables",
            ),
            (
                format!(r#"{{"sample_records":[{token}]}}"#),
                "sample_records",
            ),
            (
                format!(r#"{{"ideal_state":{{"circuit":"ideal.magna","outputs":[{token}]}}}}"#),
                "ideal_state.outputs",
            ),
            (
                format!(
                    r#"{{"ideal_state":{{"circuit":"ideal.magna","outputs":[],"postselection":{{"0":{token}}}}}}}"#
                ),
                "ideal_state.postselection",
            ),
        ] {
            let parsed = parse_task(&text, base, &options);
            assert!(parsed.is_err(), "accepted invalid JSON type: {text}");
            let error = parsed.unwrap_err();
            assert_eq!(error.field, field, "{text}: {error}");
        }
    }
    for (text, field) in [
        (
            r#"{"outputs":{"$serde_json::private::Number":"1"}}"#,
            "outputs",
        ),
        (
            r#"{"ideal_state":{"circuit":{"$serde_json::private::Number":"1"}}}"#,
            "ideal_state.circuit",
        ),
        (
            r#"{"ideal_state":{"$serde_json::private::Number":"1"}}"#,
            "ideal_state",
        ),
        (
            r#"{"ideal_state":"ideal.magna","recovery":{"$serde_json::private::Number":"1"}}"#,
            "recovery",
        ),
        (
            r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[],"postselection":{"$serde_json::private::Number":"1"}}}"#,
            "ideal_state.postselection",
        ),
    ] {
        let error = parse_task(text, base, &options).unwrap_err();
        assert_eq!(error.field, field, "{text}: {error}");
    }
    // Raw-value tags can otherwise substitute entire containers, paths or nulls.
    for (text, field) in [
        (
            r#"{"outputs":{"$serde_json::private::RawValue":"[2]"}}"#,
            "outputs",
        ),
        (
            r#"{"sample_records":{"$serde_json::private::RawValue":"[0]"}}"#,
            "sample_records",
        ),
        (
            r#"{"error_observables":{"$serde_json::private::RawValue":"[0]"}}"#,
            "error_observables",
        ),
        (
            r#"{"ideal_state":{"$serde_json::private::RawValue":"\"ideal.magna\""}}"#,
            "ideal_state",
        ),
        (
            r#"{"ideal_state":{"$serde_json::private::RawValue":"null"}}"#,
            "ideal_state",
        ),
        (
            r#"{"recovery":{"$serde_json::private::RawValue":"null"}}"#,
            "recovery",
        ),
        (
            r#"{"ideal_state":"ideal.magna","recovery":{"$serde_json::private::RawValue":"\"r.magna\""}}"#,
            "recovery",
        ),
        (
            r#"{"ideal_state":{"circuit":{"$serde_json::private::RawValue":"\"ideal.magna\""}}}"#,
            "ideal_state.circuit",
        ),
        (
            r#"{"ideal_state":{"circuit":"ideal.magna","outputs":{"$serde_json::private::RawValue":"[0]"}}}"#,
            "ideal_state.outputs",
        ),
        (
            r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[],"postselection":{"$serde_json::private::RawValue":"{\"0\":1}"}}}"#,
            "ideal_state.postselection",
        ),
    ] {
        let parsed = parse_task(text, base, &options);
        assert!(parsed.is_err(), "accepted substituted JSON shape: {text}");
        assert_eq!(parsed.unwrap_err().field, field, "{text}");
    }
    let valid = parse_task(
        "{\"schema_version\": 1, \"outputs\": [2, 0], \"sample_records\": [0, 0], \"ideal_state\": \n null, \"recovery\": null}",
        base,
        &options,
    ).unwrap().validate(&physical("I 2\nM 0")).unwrap();
    assert_eq!(valid.outputs(), [2, 0]);
    assert_eq!(valid.sample_records(), [0, 0]);
    let largest = format!(r#"{{"outputs":[{}]}}"#, usize::MAX);
    let parsed = parse_task(&largest, base, &options).unwrap();
    assert_eq!(parsed.outputs(), Some([usize::MAX].as_slice()));
}

#[test]
fn object_targets_require_explicit_outputs_within_declared_preparation() {
    let files = Files::new();
    let error = parse_task(
        r#"{"ideal_state":{"circuit":"ideal.magna"}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.field, "ideal_state.outputs");
    assert!(error.message.contains("explicit outputs list"), "{error}");
    for (preparation, mapping, source, index, width) in
        [("H 0", "[1,0]", "I 1", 1, 1), ("", "[0]", "I 0", 0, 0)]
    {
        files.put("ideal.magna", preparation);
        let text = format!(r#"{{"ideal_state":{{"circuit":"ideal.magna","outputs":{mapping}}}}}"#);
        let spec = parse_task(&text, &files.0, &TaskOptions::default()).unwrap();
        let error = spec
            .load_modules(&physical(source), &ParseOptions::default())
            .unwrap_err();
        assert_eq!(error.field, "ideal_state.outputs");
        assert_eq!(
            error.message,
            format!("index {index} is outside width {width}")
        );
    }
    files.put("ideal.magna", "H 0");
    let path = parse_task(
        r#"{"ideal_state":"ideal.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = path
        .load_modules(&physical("I 1"), &ParseOptions::default())
        .unwrap();
    assert_eq!(modules.ideal_state().unwrap().circuit().num_qubits(), 2);
    assert_eq!(modules.ideal_state().unwrap().outputs(), [0, 1]);
    files.put("ideal.magna", "H 0\nI 1");
    let object = parse_task(
        r#"{"ideal_state":{"circuit":"ideal.magna","outputs":[1,0]}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = object
        .load_modules(&physical("I 1"), &ParseOptions::default())
        .unwrap();
    assert_eq!(modules.ideal_state().unwrap().circuit().num_qubits(), 2);
    assert_eq!(modules.ideal_state().unwrap().outputs(), [1, 0]);
    files.put("ideal.magna", "");
    let empty = parse_task(
        r#"{"outputs":[],"ideal_state":{"circuit":"ideal.magna","outputs":[]}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = empty
        .load_modules(&physical(""), &ParseOptions::default())
        .unwrap();
    assert_eq!(modules.ideal_state().unwrap().circuit().num_qubits(), 0);
    assert!(modules.ideal_state().unwrap().outputs().is_empty());
}

#[test]
fn semantic_module_errors_identify_the_file_and_offending_source_location() {
    let files = Files::new();
    files.put("valid.magna", "I 0");
    for (field, name, task, operation, reason) in [
        (
            "ideal_state",
            "ideal.magna",
            r#"{"ideal_state":"ideal.magna"}"#,
            "M 0",
            "noiseless unitary",
        ),
        (
            "recovery",
            "recovery.magna",
            r#"{"ideal_state":"valid.magna","recovery":"recovery.magna"}"#,
            "X_ERROR(0) 0",
            "noise channels",
        ),
        (
            "recovery",
            "recovery.magna",
            r#"{"ideal_state":"valid.magna","recovery":"recovery.magna"}"#,
            "M(1/2) 0",
            "report-flip",
        ),
        (
            "recovery",
            "recovery.magna",
            r#"{"ideal_state":"valid.magna","recovery":"recovery.magna"}"#,
            "POSTSELECT det[0]",
            "preserve trace",
        ),
    ] {
        files.put(name, &format!("M 0\nDETECTOR rec[-1]\n  {operation}"));
        // Targets must reach their first semantic violation on the same line.
        if field == "ideal_state" {
            files.put(name, &format!("I 0\n# comment\n  {operation}"));
        }
        let spec = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
        let error = spec
            .load_modules(&physical("I 0"), &ParseOptions::default())
            .unwrap_err();
        assert_eq!(error.field, field);
        assert!(
            error
                .message
                .contains(&format!("{}:3:3:", files.0.join(name).display())),
            "{error}"
        );
        assert!(error.message.contains(reason), "{error}");
    }
}
