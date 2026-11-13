// SPDX-License-Identifier: MPL-2.0
// Complete QuantumOutputTests.test_entry_validation from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_quantum_output.py.
// Typed module paths and TaskError fields translate Python circuit objects and
// exception classes; conflicting declarations must fail before module I/O.
use rmagna::circuit::{Circuit, ParseOptions, parse_circuit};
use rmagna::task::{TaskError, TaskOptions, parse_task};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
const PHYSICAL: &str = "M 0\nOBSERVABLE_INCLUDE(0) rec[-1]";
fn validate(physical: &Circuit, task: &Value, base: &Path) -> Result<(), TaskError> {
    parse_task(
        &serde_json::to_string(task).unwrap(),
        base,
        &TaskOptions::default(),
    )?
    .load_modules(physical, &ParseOptions::default())
    .map(|_| ())
}
#[test]
fn original_entry_validation() {
    let physical = parse_circuit(PHYSICAL, &ParseOptions::default()).unwrap();
    let base = std::env::temp_dir().join(format!(
        "rmagna-entry-modules-must-not-open-{}",
        std::process::id()
    ));
    assert!(!base.exists());
    for retired in [
        json!({"error_checks":[[-1]]}),
        json!({"error_checks":[[1]]}),
        json!({"error_checks":[[true]]}),
        json!({"error_checks":[["0"]]}),
        json!({"error_checks":[]}),
        json!({"decoded_error":{}}),
    ] {
        let field = retired.as_object().unwrap().keys().next().unwrap();
        let error =
            validate(&physical, &retired, &base).expect_err("original retired field was accepted");
        assert_eq!(&error.field, field);
        assert!(error.message.contains("no longer supported"));
    }
    for text in [
        "M 0\nOBSERVABLE_INCLUDE(0) rec[-2]",
        "M 0\nOBSERVABLE_INCLUDE(0) rec[0]",
    ] {
        let error = parse_circuit(text, &ParseOptions::default())
            .expect_err("non-earlier original record was accepted");
        assert!(error.to_string().contains("earlier measurement"));
    }
    for invalid in [
        json!({"error_observables":[1]}),
        json!({"error_observables":[-1]}),
        json!({"error_observables":[0.5]}),
        json!({"error_observables":[true]}),
        json!({"error_observables":["0"]}),
        json!({"error_observables":[0],"typo":1}),
        json!({"error_observables":[0],"outputs":null}),
        json!({"error_observables":[0],"observables":false}),
    ] {
        assert!(
            validate(&physical, &invalid, &base).is_err(),
            "invalid original task accepted: {invalid}"
        );
    }
    for conflict in [
        json!({"outputs":[0]}),
        json!({"observables":[[0,1]]}),
        json!({"ideal_state":"zero.magna"}),
        json!({"recovery":"zero.magna"}),
    ] {
        let mut task = json!({"error_observables":[]});
        task.as_object_mut()
            .unwrap()
            .extend(conflict.as_object().unwrap().clone());
        let error = validate(&physical, &task, &base)
            .expect_err("classical/quantum original conflict accepted");
        assert_eq!(error.field, "error_observables", "conflict {conflict}");
        assert!(
            error.message.contains("classical error definition"),
            "{error}"
        );
    }
    assert!(!base.exists());
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-entry-positive-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("zero.magna"), "I 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn valid_classical_quantum_declarations_and_standalone_recovery_dependency_remain() {
    let files = Files::new();
    let physical = parse_circuit(PHYSICAL, &ParseOptions::default()).unwrap();
    let valid = [
        json!({"error_observables":[]}),
        json!({"error_observables":[0]}),
        json!({"error_observables":[0,0]}),
        json!({"error_observables":[0],"outputs":[]}),
        json!({"error_observables":[0],"observables":[]}),
        json!({"error_observables":[0],"outputs":[],"observables":[]}),
        json!({"outputs":[0],"ideal_state":"zero.magna"}),
    ];
    assert_eq!(valid.len(), 7);
    for task in valid {
        validate(&physical, &task, &files.0).unwrap();
    }
    let error = validate(&physical, &json!({"recovery":"zero.magna"}), &files.0)
        .err()
        .unwrap();
    assert_eq!(error.field, "recovery");
    assert!(
        error
            .message
            .contains("terminal recovery requires ideal_state")
    );
}
