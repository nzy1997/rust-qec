// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original lossless-roundtrip method from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_circuit_file.py.
//! Rust resolves register reads into owned expressions and lowers grouped gates.
//! FileTask inspection and rmagna-provenance-v1 replace Python dictionary/hash ABI;
//! standard file I/O writes the retained source bytes without reformatting them.
use rmagna::circuit::{
    Basis, BitAtom, BitExpr, Circuit, Gate, Operation, ParseOptions, parse_circuit,
};
use rmagna::file_api::{FileApiError, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::probability::Probability;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{TaskOptions, TaskRunOptions, parse_task};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const TEXT: &str = "# physical sites\nR 0 1\nM 0\nCONTROL[XOR] rec[-1] -> ctrl[7]\nREPEAT 2 {\n MX(0.1) 1\n CONTROL[XOR](1) ctrl[7] rec[-1] -> ctrl[7]\n CZ ctrl[7] 0\n}\n";

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-roundtrip-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn circuit(&self) -> PathBuf {
        self.0.join("sample.magna")
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn expression(constant: bool, records: &[usize]) -> BitExpr {
    BitExpr {
        constant,
        terms: records.iter().copied().map(BitAtom::Record).collect(),
    }
}
fn file_task(circuit: Circuit) -> FileTask {
    let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    FileTask::from_loaded(
        circuit,
        modules,
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions::default(),
        BTreeMap::new(),
    )
    .unwrap()
}
fn load(path: &Path) -> Result<FileTask, FileApiError> {
    FileTask::load(
        path,
        None,
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions::default(),
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
}

#[test]
fn lossless_roundtrip_and_resolved_repeat() {
    let source = TEXT.to_owned();
    let options = ParseOptions {
        source: Some("example.magna".into()),
        ..Default::default()
    };
    let circuit = parse_circuit(&source, &options).unwrap();
    assert_eq!(circuit.to_text(), source);
    assert_eq!((circuit.num_records(), circuit.num_qubits()), (3, 2));
    assert_eq!(
        (circuit.num_instructions(), circuit.operations().len()),
        (9, 10)
    );
    let controls = circuit
        .operations()
        .iter()
        .filter_map(|located| match &located.operation {
            Operation::Control {
                name,
                call,
                inputs,
                outputs,
            } => {
                assert_eq!(name, "XOR");
                assert_eq!(*call, None);
                assert_eq!(outputs.len(), 1);
                Some((located, inputs, &outputs[0]))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(controls.len(), 3);
    assert_eq!(
        controls.iter().map(|x| x.2.version).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(controls.iter().all(|x| x.2.register == 7));
    assert_eq!(controls[0].1, &[expression(false, &[0])]);
    assert_eq!(
        controls[1].1,
        &[controls[0].2.expression.clone(), expression(false, &[1])]
    );
    // This read retains version1 before the same instruction installs version2.
    assert_eq!(
        controls[2].1,
        &[controls[1].2.expression.clone(), expression(false, &[2])]
    );
    assert_eq!(controls[0].2.expression, expression(false, &[0]));
    assert_eq!(controls[1].2.expression, expression(true, &[0, 1]));
    assert_eq!(controls[2].2.expression, expression(false, &[0, 1, 2]));
    assert!(controls[2].2.expression.is_affine());
    assert_eq!(controls[1].0.location.repeat, [0]);
    assert_eq!(controls[2].0.location.repeat, [1]);
    for row in &controls[1..] {
        assert_eq!((row.0.location.line, row.0.location.column), (7, 2));
        assert_eq!(row.0.location.source.as_deref(), Some("example.magna"));
    }
    let gates = circuit
        .operations()
        .iter()
        .filter_map(|located| match &located.operation {
            Operation::Unitary {
                gate,
                qubits,
                control,
            } => Some((gate, qubits, control)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(gates.len(), 2);
    for (gate, write) in gates.iter().zip(&controls[1..]) {
        assert_eq!(*gate.0, Gate::Z);
        assert_eq!(gate.1, &[0]);
        assert_eq!(gate.2.as_ref(), Some(&write.2.expression));
    }
    let measurements = circuit
        .operations()
        .iter()
        .filter_map(|located| match &located.operation {
            Operation::Measure {
                product,
                report_flip,
                record,
                ..
            } => Some((product, report_flip, *record)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        measurements.iter().map(|x| x.2).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(measurements[0].0.factors, [(0, Basis::Z)]);
    for measurement in &measurements[1..] {
        assert_eq!(measurement.0.factors, [(1, Basis::X)]);
        assert_eq!(measurement.1, &"1/10".parse::<Probability>().unwrap());
    }
    let ids = circuit
        .operations()
        .iter()
        .map(|x| x.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), circuit.operations().len());
    assert_eq!(ids, (0..10).collect());

    let run = TaskRunOptions::default();
    let memory = file_task(circuit.clone());
    let identity = memory.fingerprints(&run).unwrap();
    let descriptor = memory.inspect(&run).unwrap();
    assert_eq!(descriptor["circuit"]["records"], 3);
    assert_eq!(descriptor["circuit"]["qubits"], 2);
    assert_eq!(descriptor["circuit"]["instructions"], 9);
    assert_eq!(descriptor["circuit"]["operations"], 10);
    assert_eq!(descriptor["fingerprints"], identity.to_json());
    assert_eq!(
        descriptor["fingerprints"]["encoding"],
        "rmagna-provenance-v1"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&serde_json::to_vec(&descriptor).unwrap()).unwrap(),
        descriptor
    );

    let files = Files::new();
    let path = files.circuit();
    std::fs::write(&path, circuit.to_text().as_bytes()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), TEXT.as_bytes());
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.fingerprints(&run).unwrap(), identity);
    assert_eq!(loaded.inspect(&run).unwrap(), descriptor);
    let retained = loaded.clone();
    drop(loaded);
    drop(memory);
    drop(circuit);
    drop(source);
    drop(options);
    std::fs::remove_file(&path).unwrap();
    assert!(!path.exists());
    assert!(matches!(load(&path), Err(FileApiError::Input(error)) if error.field == "circuit"));
    assert_eq!(retained.fingerprints(&run).unwrap(), identity);
    assert_eq!(retained.inspect(&run).unwrap(), descriptor);
}

#[test]
fn captured_file_identity_survives_replacement_and_deletion() {
    let files = Files::new();
    let path = files.circuit();
    std::fs::write(&path, TEXT).unwrap();
    let snapshot = load(&path).unwrap();
    let run = TaskRunOptions::default();
    let original = snapshot.fingerprints(&run).unwrap();
    let inspection = snapshot.inspect(&run).unwrap();
    for replacement in [
        TEXT.replacen("# physical sites", "# edited comment", 1),
        TEXT.replacen("MX(0.1)", "MX(0.2)", 1),
    ] {
        std::fs::write(&path, &replacement).unwrap();
        let changed = load(&path).unwrap();
        let changed_identity = changed.fingerprints(&run).unwrap();
        assert_ne!(changed_identity.circuit, original.circuit);
        assert_eq!(changed_identity.task, original.task);
        assert_eq!(snapshot.fingerprints(&run).unwrap(), original);
        assert_eq!(snapshot.inspect(&run).unwrap(), inspection);
    }
    std::fs::write(&path, "REPEAT broken {").unwrap();
    assert!(matches!(load(&path), Err(FileApiError::Input(error)) if error.field == "circuit"));
    assert_eq!(snapshot.fingerprints(&run).unwrap(), original);
    std::fs::remove_file(&path).unwrap();
    assert!(load(&path).is_err());
    assert_eq!(snapshot.fingerprints(&run).unwrap(), original);
    assert_eq!(snapshot.inspect(&run).unwrap(), inspection);
}
