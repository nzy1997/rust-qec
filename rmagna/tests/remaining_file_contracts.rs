// SPDX-License-Identifier: MPL-2.0
// Source-bound detector cases from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_detector_feedback.py. License: fixtures/original_many_record_input.LICENSE.
use rmagna::circuit::{BitAtom, BitExpr, Operation, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{RunStatus, TaskOptions, TaskRunOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-remaining-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("zero.magna"), "I 0").unwrap();
        Self(path)
    }
    fn load(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        strategy: (CompilerStrategy, CausalStrategy),
    ) -> FileTask {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(task, &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = strategy.0;
        options.evaluation.runtime.strategy = strategy.1;
        FileTask::from_loaded(
            circuit,
            modules,
            registry,
            NoiseSampling::Physical,
            options,
            BTreeMap::new(),
        )
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn strategies() -> impl Iterator<Item = (CompilerStrategy, CausalStrategy)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate]
            .into_iter()
            .map(move |causal| (compiler, causal))
    })
}
fn run(shots: u64, seed: u64) -> TaskRunOptions {
    TaskRunOptions {
        shots,
        seed,
        ..Default::default()
    }
}

#[test]
fn original_detector_future_and_ambiguous_uses_reject_including_module_roles() {
    for source in [
        "CONTROL[XOR] det[0] -> ctrl[0]",
        "POSTSELECT det[-1]",
        "M 0\nDETECTOR rec[-1]\nCONTROL[XOR] det[1] -> ctrl[0]",
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT(2) det[0]",
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT(0) det[0]",
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT(1) det[0]",
        "M 0\nPOSTSELECT rec[-1]",
        "M 0\nDETECTOR rec[-1]\nCX det[0] 0",
        "POSTSELECT",
    ] {
        assert!(
            parse_circuit(source, &ParseOptions::default()).is_err(),
            "{source}"
        );
    }
    let files = Files::new();
    let selected = "DETECTOR\nPOSTSELECT det[0]";
    parse_circuit(selected, &ParseOptions::default()).unwrap();
    std::fs::write(files.0.join("selected.magna"), selected).unwrap();
    let physical = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    for (task, role, expected) in [
        (
            r#"{"ideal_state":"selected.magna"}"#,
            "ideal_state",
            "noiseless unitary",
        ),
        (
            r#"{"ideal_state":"zero.magna","recovery":"selected.magna"}"#,
            "recovery",
            "preserve trace",
        ),
    ] {
        let task = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
        let error = task
            .load_modules(&physical, &ParseOptions::default())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(role) && error.contains(expected), "{error}");
    }
}

#[test]
fn original_detector_order_duplicates_inversions_and_prior_register_state() {
    const SOURCE: &str = "M(1) 0\nDETECTOR rec[-1]\nM 1\nDETECTOR rec[-1]\nCONTROL[XOR](0) -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[decode] det[0] det[1] det[0] !det[1] det[-1] ctrl[0] -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[decode] det[-4] det[-3] det[0] !det[1] det[-1] ctrl[0] -> ctrl[0]\nCX ctrl[0] 2\n";
    let files = Files::new();
    for strategy in strategies() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("decode", "1", move |bits: &[bool]| {
                observed.lock().unwrap().push(bits.to_vec());
                Ok(vec![bits[0] ^ bits[bits.len() - 1]])
            })
            .unwrap();
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(SOURCE, &parse).unwrap();
        assert_eq!(circuit.format_version(), "magna-0.1");
        assert_eq!(
            parse_circuit("M 0\nDETECTOR rec[-1]", &ParseOptions::default())
                .unwrap()
                .format_version(),
            "magna-0.1"
        );
        let roundtrip = parse_circuit(circuit.to_text(), &parse).unwrap();
        assert_eq!(roundtrip.operations(), circuit.operations());
        assert_eq!(roundtrip.control_analysis(), circuit.control_analysis());
        let task = r#"{"outputs":[2],"observables":[[0,1]]}"#;
        let file = files.load(SOURCE, task, &registry, strategy);
        let roundtrip_file = files.load(roundtrip.to_text(), task, &registry, strategy);
        assert_eq!(
            file.fingerprints(&run(3, 11)).unwrap(),
            roundtrip_file.fingerprints(&run(3, 11)).unwrap()
        );
        let report = file.run(&run(3, 11)).unwrap();
        let result = report.raw_result();
        assert_eq!((result.status, result.completed), (RunStatus::Complete, 3));
        assert_eq!(
            result.statistics.as_ref().unwrap().moments,
            [ExactField::one()]
        );
        let pair = [
            vec![true, false, true, true, false, false],
            vec![true, false, true, true, false, true],
        ];
        assert_eq!(
            *calls.lock().unwrap(),
            (0..3)
                .flat_map(|_| pair.iter().cloned())
                .collect::<Vec<_>>()
        );
        serde_json::to_string(&report.to_json().unwrap()).unwrap();
    }
}

#[test]
fn original_detector_repeat_numbering_and_parity_cancellation() {
    let source = "M 0\nDETECTOR rec[-1] rec[-1]\nREPEAT 3 {\n M 0\n DETECTOR rec[-1] rec[-2]\n CONTROL[XOR](1) det[-1] det[0] -> ctrl[0]\n}\nPOSTSELECT det[-1] det[0]\n";
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let controls = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control { inputs, .. } => Some(inputs),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(controls.len(), 3);
    assert_eq!(circuit.detectors().len(), 4);
    assert!(circuit.detectors()[0].terms.is_empty());
    assert!(!circuit.detectors()[0].constant);
    // Rust owns resolved expressions rather than unresolved detector targets.
    // The first argument at each repeated CONTROL is detector 1/2/3.
    for (i, inputs) in controls.iter().enumerate() {
        assert_eq!(inputs[0], circuit.detectors()[i + 1]);
        assert_eq!(inputs[1], circuit.detectors()[0]);
        assert_eq!(
            inputs[0].terms,
            BTreeSet::from([BitAtom::Record(i), BitAtom::Record(i + 1)])
        );
    }
}

#[test]
fn original_detector_selection_and_target_do_not_bias_earlier_decoder_inputs() {
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[decode] det[0] -> ctrl[0]\nCX ctrl[0] 1\n";
    let files = Files::new();
    for strategy in strategies() {
        let histories = Arc::new(Mutex::new(Vec::new()));
        let observed = histories.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("decode", "1", move |bits: &[bool]| {
                observed.lock().unwrap().push(bits.to_vec());
                Ok(bits.to_vec())
            })
            .unwrap();
        let task = r#"{"outputs":[1],"ideal_state":"zero.magna"}"#;
        let first = files
            .load(source, task, &registry, strategy)
            .run(&run(32, 9))
            .unwrap();
        assert_eq!(first.raw_result().status, RunStatus::Complete);
        let original = std::mem::take(&mut *histories.lock().unwrap());
        let second = files
            .load(
                &format!("{source}POSTSELECT det[0]"),
                task,
                &registry,
                strategy,
            )
            .run(&run(32, 9))
            .unwrap();
        assert_eq!(second.raw_result().status, RunStatus::Complete);
        assert_eq!(*histories.lock().unwrap(), original);
        assert_eq!(
            original.iter().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([vec![false], vec![true]])
        );
        assert_eq!(
            second.raw_result().statistics.as_ref().unwrap().error,
            Some(ExactField::zero())
        );
    }
}

#[test]
fn original_detector_separate_selection_rows_and_combination() {
    let prefix = "X 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[NOT] det[0] -> ctrl[0]\nCONTROL[NOT] det[1] -> ctrl[1]\n";
    let cases = [
        ("POSTSELECT det[0] det[1]", false),
        ("POSTSELECT ctrl[0] ctrl[1]", true),
        ("POSTSELECT ctrl[0] ctrl[0]\nPOSTSELECT !det[0]", true),
        ("POSTSELECT ctrl[0]\nPOSTSELECT det[0]", false),
        ("POSTSELECT ctrl[0]\nPOSTSELECT det[0]", false),
        (
            "POSTSELECT ctrl[0]\nDETECTOR rec[-2] rec[-1]\nPOSTSELECT !det[2]",
            false,
        ),
    ];
    let files = Files::new();
    for strategy in strategies() {
        for (selection, accepted) in cases {
            let report = files
                .load(
                    &format!("{prefix}{selection}"),
                    "{}",
                    &ControllerRegistry::default(),
                    strategy,
                )
                .run(&run(1, 0))
                .unwrap();
            assert_eq!(
                report.raw_result().status,
                RunStatus::Complete,
                "{selection}"
            );
            assert_eq!(
                report.raw_result().statistics.as_ref().unwrap().accepted,
                if accepted {
                    ExactField::one()
                } else {
                    ExactField::zero()
                },
                "{selection}"
            );
        }
    }
}

#[test]
fn original_detector_1025_inputs_keep_one_parity_and_affine_route() {
    let files = Files::new();
    let width = 1025;
    let mut source = format!(
        "M 0\n{}CONTROL[decode] ",
        "DETECTOR rec[-1]\n".repeat(width)
    );
    source.push_str(
        &(0..width)
            .map(|i| format!("det[{i}]"))
            .collect::<Vec<_>>()
            .join(" "),
    );
    source.push_str(" -> ctrl[0]\nCX ctrl[0] 1");
    for strategy in strategies() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("decode", "1", move |bits: &[bool]| {
                observed.lock().unwrap().push(bits.to_vec());
                Ok(vec![false])
            })
            .unwrap();
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(&source, &parse).unwrap();
        let inputs = circuit
            .operations()
            .iter()
            .find_map(|op| match &op.operation {
                Operation::Control { inputs, .. } => Some(inputs),
                _ => None,
            })
            .unwrap();
        assert_eq!(inputs.len(), width);
        assert!(
            inputs
                .iter()
                .all(|input| *input == BitExpr::atom(BitAtom::Record(0)))
        );
        assert_eq!(
            inputs
                .iter()
                .map(|e| (e.constant, e.terms.iter().cloned().collect::<Vec<_>>()))
                .collect::<BTreeSet<_>>()
                .len(),
            1
        );
        let report = files
            .load(&source, "{}", &registry, strategy)
            .run(&run(1, 0))
            .unwrap();
        assert_eq!(report.raw_result().status, RunStatus::Complete);
        assert_eq!(*calls.lock().unwrap(), [vec![false; width]]);
        let affine = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[XOR] det[0] -> ctrl[0]\nCX ctrl[0] 0";
        let circuit = parse_circuit(affine, &ParseOptions::default()).unwrap();
        assert!(!circuit.needs_history_sampling());
        assert_eq!(circuit.external_calls(), 0);
        let report = files
            .load(
                affine,
                r#"{"observables":[[0,1]]}"#,
                &ControllerRegistry::default(),
                strategy,
            )
            .run(&run(1, 0))
            .unwrap();
        assert_eq!(
            report.raw_result().statistics.as_ref().unwrap().moments,
            [ExactField::one()]
        );
    }
}

#[test]
fn quantum_gate_detector_controls_reject_with_register_migration_hint() {
    for gate in ["CX det[0] 0", "CY !det[0] 0", "CZ det[-1] 0", "CZ 0 det[0]"] {
        let source = format!("M 0\nDETECTOR rec[-1]\n{gate}");
        let error = parse_circuit(&source, &ParseOptions::default()).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("CONTROL")
                && message.contains("rec[...]")
                && message.contains("ctrl[...]"),
            "{message}"
        );
    }
    for control in ["rec[-1]", "ctrl[0]"] {
        let source =
            format!("M 0\nDETECTOR rec[-1]\nCONTROL[XOR] det[0] -> ctrl[0]\nCX {control} 0");
        parse_circuit(&source, &ParseOptions::default()).unwrap();
    }
}
