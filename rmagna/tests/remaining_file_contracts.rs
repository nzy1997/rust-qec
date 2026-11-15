// SPDX-License-Identifier: MPL-2.0
// Source-bound detector cases from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_detector_feedback.py, test_quantum_output.py, test_query_reuse.py,
// test_reference_networks.py and tests/support/readout_target.py.
// Also tests/core/test_incremental_history.py's interference-zero prefix case.
// License: fixtures/original_many_record_input.LICENSE.
use rmagna::circuit::{BitAtom, BitExpr, Operation, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
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

fn quantum_readout(
    source: &str,
    records: usize,
    qubits: usize,
    rows: &[Vec<usize>],
) -> (String, Vec<usize>) {
    let mut text = format!(
        "{}\n# Noiseless quantum parity outputs for the declared ideal target.\n",
        source.trim_end()
    );
    let outputs = (qubits..qubits + rows.len()).collect::<Vec<_>>();
    for (&q, row) in outputs.iter().zip(rows) {
        text.push_str(&format!("I {q}\n"));
        for &record in row {
            assert!(record < records);
            text.push_str(&format!(
                "CX rec[{}] {q}\n",
                record as isize - records as isize
            ));
        }
    }
    (text, outputs)
}

#[test]
fn original_record_success_public_run_enumeration_task_file_and_source_reports() {
    use rmagna::field::FieldArithmetic;
    use serde_json::{Value, json};
    use std::process::Command;
    let source = "X_ERROR(1/5) 0\nM(1/7) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let (rewritten, outputs) = quantum_readout(source, 1, 1, &[vec![0]]);
    let files = Files::new();
    std::fs::write(files.0.join("physical.magna"), source).unwrap();
    std::fs::write(files.0.join("task.json"), r#"{"error_observables":[0]}"#).unwrap();
    std::fs::write(
        files.0.join("zero-target.magna"),
        "# All selected outputs are zero.\n",
    )
    .unwrap();
    let reference_task = json!({"outputs":outputs,"ideal_state":"zero-target.magna"}).to_string();
    let expected_error = FieldArithmetic::default()
        .from_rational("2/7".parse().unwrap())
        .unwrap();
    for strategy in strategies() {
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = strategy.0;
        options.evaluation.runtime.strategy = strategy.1;
        let native = FileTask::load(
            &files.0.join("physical.magna"),
            Some(&files.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        let target = files.load(
            &rewritten,
            &reference_task,
            &ControllerRegistry::default(),
            strategy,
        );
        let mut enumerate = FileEnumerationOptions::default();
        enumerate.enumeration.noise.max_weight = 2;
        let actual = native.enumerate(&enumerate).unwrap();
        let expected = target.enumerate(&enumerate).unwrap();
        assert!(actual.is_complete() && expected.is_complete());
        let a = actual.raw_result().unwrap();
        let e = expected.raw_result().unwrap();
        assert_eq!(
            (&a.masses.accepted, &a.masses.error, &a.masses.moments),
            (&e.masses.accepted, &e.masses.error, &e.masses.moments)
        );
        assert_eq!(a.masses.error, expected_error);
        assert_eq!(a.masses.accepted, ExactField::one());
        assert_eq!(a.covered_probability, e.covered_probability);
        assert_eq!(a.omitted_probability, e.omitted_probability);
        let draw = native.run(&run(40, 8)).unwrap();
        let reference = target.run(&run(40, 8)).unwrap();
        assert_eq!(draw.raw_result().status, RunStatus::Complete);
        let a = draw.to_json().unwrap();
        let b = reference.to_json().unwrap();
        assert_eq!(a["estimates"], b["estimates"]);
        let evaluation = json!({"ideal_state":null,"recovery":null,"output_mapping":[],"error_observables":[0],"sample_records":[],"moment_count":0,
            "error_metric":"wrong when any listed OBSERVABLE_INCLUDE parity is one","observables":"physical output"});
        let fingerprint = native.fingerprints(&run(40, 8)).unwrap().circuit;
        for value in [a.clone(), actual.to_json().unwrap()] {
            assert_eq!(value["schema_version"], 2);
            assert_eq!(value["evaluation"], evaluation);
            assert_eq!(value["fingerprints"]["circuit"], fingerprint);
            assert_eq!(value["circuit"]["qubits"], 1);
            assert_eq!(value["circuit"]["instructions"], 3);
            assert!(!value.to_string().contains("<record-success>"));
        }
        // Rust provenance hashes every category, including an absent ideal
        // target. The evaluation descriptor preserves its semantic absence.
        assert!(a["evaluation"]["ideal_state"].is_null());
        let inspected = native.compile().unwrap().inspect(&run(40, 8)).unwrap();
        assert_eq!(inspected["circuit"]["qubits"], 1);
        assert_eq!(inspected["circuit"]["instructions"], 3);
        assert_eq!(inspected["fingerprints"]["circuit"], fingerprint);
        assert_eq!(inspected["evaluation"], evaluation);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rmagna"))
        .current_dir(&files.0)
        .args([
            "validate",
            "physical.magna",
            "--task",
            "task.json",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let validated: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(validated["status"], "valid");
    assert_eq!(validated["schema_version"], 2);
    assert_eq!(validated["circuit"]["qubits"], 1);
    assert!(!validated.to_string().contains("<record-success>"));
    let zero = "M 0\nDETECTOR\nPOSTSELECT !det[0]";
    let result = files
        .load(
            zero,
            r#"{"error_observables":[]}"#,
            &ControllerRegistry::default(),
            (
                CompilerStrategy::ParityCopies,
                CausalStrategy::FixedTemplate,
            ),
        )
        .run(&run(2, 0))
        .unwrap();
    let stats = result.raw_result().statistics.as_ref().unwrap();
    assert_eq!(stats.error, Some(ExactField::zero()));
    assert!(stats.conditional_error.is_none());
}

#[test]
fn original_ordered_distinct_dependencies_keep_callback_duplicates() {
    let source = "M 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-1]\nCONTROL[wide] det[0] det[-1] det[0] -> ctrl[0]\nDETECTOR rec[-1]\nDETECTOR rec[-3]\nCONTROL[wide] det[-2] det[-1] -> ctrl[1]";
    let observed = Arc::new(Mutex::new(Vec::new()));
    let called = observed.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("wide", "test", move |bits: &[bool]| {
            called.lock().unwrap().push(bits.to_vec());
            Ok(vec![false])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let external = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control {
                call: Some(_),
                inputs,
                ..
            } => Some(inputs),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(external.len(), 2);
    assert_eq!(external[0].len(), 3);
    assert_eq!(external[1].len(), 2);
    let mut distinct = Vec::<BitExpr>::new();
    for input in external.iter().flat_map(|inputs| inputs.iter()) {
        if !distinct.contains(input) {
            distinct.push(input.clone());
        }
    }
    assert_eq!(distinct.len(), 3);
    assert_eq!(distinct[0], external[0][0]);
    assert_eq!(
        external[0],
        &vec![
            circuit.detectors()[0].clone(),
            circuit.detectors()[1].clone(),
            circuit.detectors()[0].clone()
        ]
    );
    let files = Files::new();
    let result = files
        .load(
            source,
            "{}",
            &registry,
            (
                CompilerStrategy::ParityCopies,
                CausalStrategy::FixedTemplate,
            ),
        )
        .run(&run(1, 0))
        .unwrap();
    assert_eq!(result.raw_result().status, RunStatus::Complete);
    assert_eq!(*observed.lock().unwrap(), [vec![false; 3], vec![false; 2]]);
}

#[test]
fn original_affine_support_offset_and_report_flip_keep_all_16_draws() {
    use rmagna::noise::SplitMix64;
    use rmagna::runtime::HistorySampler;
    use rmagna::task::{EvaluationOptions, TaskEvaluator};
    let source = "X 0\nH 1\nM(1) 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCX ctrl[0] 2";
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "test", |bits: &[bool]| {
            assert_eq!(bits.len(), 2);
            assert!(!bits[0]);
            Ok(vec![bits[0] & bits[1]])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(
        r#"{"outputs":[2],"sample_records":[0,1],"ideal_state":"zero.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &parse)
    .unwrap();
    for (compiler, causal) in strategies() {
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        options.runtime.strategy = causal;
        options.runtime.history_sampler = HistorySampler::AffineSupport;
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
        let mut random = SplitMix64::new(17);
        for _ in 0..16 {
            let draw = evaluator.sample(&mut random).unwrap();
            assert_eq!(draw.sampled_records.len(), 2);
            assert!(!draw.sampled_records[0]);
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.error, ExactField::zero());
            assert_eq!(draw.controller_calls.len(), 1);
        }
    }
}

#[test]
fn interference_zero_prefixes_finish_callbacks_guarded_noise_and_complex_terminal_mass() {
    use rmagna::noise::{RandomSource, SplitMix64};
    use rmagna::runtime::HistorySampler;
    use rmagna::scalar::ScalarArithmetic;
    use rmagna::task::{EvaluationOptions, TaskEvaluator};
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Rng,
        Controller(Vec<bool>),
    }
    struct Logged {
        inner: SplitMix64,
        events: Arc<Mutex<Vec<Event>>>,
    }
    impl RandomSource for Logged {
        fn next_u64(&mut self) -> u64 {
            self.events.lock().unwrap().push(Event::Rng);
            self.inner.next_u64()
        }
    }
    let triples = [(0, 1, 2), (0, 1, 3), (0, 2, 3)];
    let amplitudes = (0_usize..16)
        .map(|y| {
            (0_usize..16)
                .map(|x| {
                    let phase = triples
                        .iter()
                        .map(|&(i, j, k)| (x >> i & 1) * (x >> j & 1) * (x >> k & 1))
                        .sum::<usize>()
                        + (x & y).count_ones() as usize;
                    if phase % 2 == 0 { 1_i32 } else { -1 }
                })
                .sum::<i32>()
        })
        .collect::<Vec<_>>();
    let expected_zero = amplitudes
        .iter()
        .enumerate()
        .filter_map(|(j, &v)| (v == 0).then_some(j))
        .collect::<BTreeSet<_>>();
    assert!(expected_zero.iter().any(|v| v & 1 != 0));
    let files = Files::new();
    std::fs::write(files.0.join("zero-two.magna"), "I 0 1").unwrap();
    // Rust's existing namespace draws potential categorical faults before
    // histories and consumes no entropy for deterministic categories. Keep the
    // original probability-one fixture and a stochastic supplement; this is
    // deliberately not a claim of the original inter-callback RNG schedule.
    for probability in ["1", "1/2"] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("record", "1", move |bits: &[bool]| {
                seen.lock().unwrap().push(Event::Controller(bits.to_vec()));
                Ok(bits.to_vec())
            })
            .unwrap();
        let source = interference_source(probability);
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(&source, &parse).unwrap();
        let modules = parse_task(
            r#"{"outputs":[6,7],"sample_records":[0,1,2,3,4,5],"ideal_state":"zero-two.magna"}"#,
            &files.0,
            &TaskOptions::default(),
        )
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::AffineSupport;
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
        let mut zeros = BTreeSet::new();
        let a = ScalarArithmetic::default();
        let mut zero_guard_draws = 0;
        for seed in 0..256 {
            events.lock().unwrap().clear();
            let mut random = Logged {
                inner: SplitMix64::new(seed),
                events: events.clone(),
            };
            let draw = evaluator.sample(&mut random).unwrap_or_else(|error| {
                panic!(
                    "probability={probability}, seed={seed}, events={:?}: {error}",
                    events.lock().unwrap()
                )
            });
            let word = &draw.sampled_records;
            assert_eq!(word.len(), 6);
            let prefix = (0..4).fold(0, |v, j| v | usize::from(word[j]) << j);
            let events = events.lock().unwrap();
            let callbacks = events
                .iter()
                .enumerate()
                .filter_map(|(j, event)| match event {
                    Event::Controller(bits) => Some((j, bits.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                callbacks
                    .iter()
                    .map(|(_, bits)| bits.clone())
                    .collect::<Vec<_>>(),
                [word[..4].to_vec(), vec![word[0]], word[4..].to_vec()]
            );
            assert_eq!(draw.controller_calls.len(), 3);
            let entropy = &events[callbacks[0].0 + 1..callbacks[1].0];
            assert!(entropy.is_empty());
            assert_eq!(
                &events[..callbacks[0].0],
                if probability == "1/2" {
                    &[Event::Rng, Event::Rng][..]
                } else {
                    &[Event::Rng][..]
                }
            );
            let square = amplitudes[prefix] * amplitudes[prefix];
            let sign = if word[5] ^ word[0] { -1 } else { 1 };
            let expected = a
                .scalar(
                    [2 * square, sign * square, 0, -sign * square].map(Into::into),
                    -10,
                )
                .unwrap();
            assert_eq!(
                draw.history_mass, expected,
                "p={probability}, seed={seed}, prefix={prefix}"
            );
            if square != 0 {
                if probability == "1" {
                    assert_eq!(word[4], word[0]);
                }
                assert_eq!(
                    draw.error,
                    if word[4] || word[5] {
                        draw.accepted.clone()
                    } else {
                        ExactField::zero()
                    }
                );
            } else {
                zeros.insert(prefix);
                assert_eq!(draw.accepted, ExactField::zero());
                assert_eq!(draw.error, ExactField::zero());
                assert_eq!(draw.history_weight, ExactField::zero());
                if word[0] {
                    zero_guard_draws += 1;
                }
            }
        }
        assert_eq!(zeros, expected_zero);
        if probability == "1/2" {
            assert!(zero_guard_draws > 0);
        }
    }
}

fn interference_source(probability: &str) -> String {
    format!(
        "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3\nDETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[record] det[-4] det[-3] det[-2] det[-1] -> ctrl[0] ctrl[1] ctrl[2] ctrl[3]\nX_ERROR({probability}) ctrl[0] 4\nCONTROL[record] ctrl[0] -> ctrl[4]\nM 4\nH 5\nT 5\nCZ ctrl[4] 5\nMX 5\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[record] det[-2] det[-1] -> ctrl[5] ctrl[6]\nCX ctrl[5] 6\nCX ctrl[6] 7"
    )
}

#[test]
fn zero_prefix_completion_preserves_wide_repeated_records_limits_and_fingerprints() {
    use rmagna::noise::SplitMix64;
    use rmagna::runtime::HistorySampler;
    use rmagna::task::{EvaluationOptions, TaskEvaluator};
    use serde_json::json;
    let files = Files::new();
    std::fs::write(files.0.join("zero-two.magna"), "I 0 1").unwrap();
    let mut registry = ControllerRegistry::default();
    registry
        .register("record", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(&interference_source("1"), &parse).unwrap();
    let wide = (0..6).cycle().take(70).collect::<Vec<_>>();
    for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let mut short_record = None;
        let mut short_state = None;
        for records in [(0..6).collect::<Vec<_>>(), wide.clone()] {
            let task =
                json!({"outputs":[6,7],"sample_records":records,"ideal_state":"zero-two.magna"})
                    .to_string();
            let modules = parse_task(&task, &files.0, &TaskOptions::default())
                .unwrap()
                .load_modules(&circuit, &parse)
                .unwrap();
            let mut options = EvaluationOptions::default();
            options.runtime.history_sampler = HistorySampler::AffineSupport;
            options.runtime.strategy = causal;
            let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
            let mut rng = SplitMix64::new(3);
            let result = evaluator.sample(&mut rng).unwrap();
            assert_eq!(result.history_mass, rmagna::scalar::ExactScalar::zero());
            assert_eq!(result.accepted, ExactField::zero());
            assert_eq!(result.controller_calls.len(), 3);
            if let Some(short) = &short_record {
                let short: &Vec<bool> = short;
                assert_eq!(
                    result.sampled_records,
                    short.iter().copied().cycle().take(70).collect::<Vec<_>>()
                );
                assert_eq!(Some(rng.state()), short_state);
            } else {
                short_record = Some(result.sampled_records);
                short_state = Some(rng.state());
            }
            let mut bounded = options;
            bounded.runtime.max_zero_prefix_words = 0;
            let file = |evaluation| {
                FileTask::from_loaded(
                    circuit.clone(),
                    modules.clone(),
                    &registry,
                    NoiseSampling::Physical,
                    FileTaskOptions {
                        evaluation,
                        ..Default::default()
                    },
                    BTreeMap::new(),
                )
                .unwrap()
            };
            assert_ne!(
                file(options).fingerprints(&run(1, 3)).unwrap().options,
                file(bounded).fingerprints(&run(1, 3)).unwrap().options
            );
            let refused = TaskEvaluator::new(&circuit, &modules, &registry, bounded)
                .unwrap()
                .sample(&mut SplitMix64::new(3))
                .err()
                .unwrap();
            assert!(
                refused.to_string().contains("max_zero_prefix_words"),
                "{refused}"
            );
            assert!(evaluator.sample(&mut SplitMix64::new(3)).is_ok());
        }
    }
}
