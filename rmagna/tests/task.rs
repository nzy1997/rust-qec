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
        r#"{"ideal_state":{"circuit":"i","postselection":{"0":true}}}"#,
        r#"{"ideal_state":{"circuit":"i","postselection":{"0":0,"00":1}}}"#,
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
    files.put("ideal.magna", "I 0");
    let spec = parse_task(
        r#"{"ideal_state":"ideal.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let error = spec
        .load_modules(
            &physical("I 0"),
            &ParseOptions {
                max_source_bytes: 2,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.field, "ideal_state");
    assert!(error.message.contains("max_source_bytes"));
    std::fs::write(files.0.join("ideal.magna"), [0xff]).unwrap();
    assert!(
        spec.load_modules(&physical("I 0"), &ParseOptions::default())
            .unwrap_err()
            .message
            .contains("UTF-8")
    );
    std::fs::remove_file(files.0.join("ideal.magna")).unwrap();
    let error = spec
        .load_modules(&physical("I 0"), &ParseOptions::default())
        .unwrap_err();
    assert!(error.message.contains("ideal.magna"));
}
