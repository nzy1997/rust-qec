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
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = strategy.0;
        options.evaluation.runtime.strategy = strategy.1;
        self.with_options(source, task, registry, options, NoiseSampling::Physical)
    }
    fn with_options(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        options: FileTaskOptions,
        sampling: NoiseSampling,
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
        FileTask::from_loaded(
            circuit,
            modules,
            registry,
            sampling,
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
    // Select causal guarded timing to preserve the original callback/noise
    // order, including an entropy word for a deterministic active category.
    // SplitMix64 replaces Python Random; no cross-language seed identity.
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
        options.runtime.guarded_noise_timing = rmagna::runtime::GuardedNoiseTiming::Causal;
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
            let noise_probability = if probability == "1/2" && word[0] {
                "1/2"
            } else {
                "1"
            };
            assert_eq!(draw.noise_probability, noise_probability.parse().unwrap());
            let entropy = &events[callbacks[0].0 + 1..callbacks[1].0];
            assert_eq!(entropy, if word[0] { &[Event::Rng][..] } else { &[][..] });
            assert_eq!(&events[..callbacks[0].0], &[Event::Rng]);
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

#[test]
fn causal_guarded_noise_keeps_active_laws_likelihoods_entropy_and_supplied_routes() {
    use rmagna::instrument::FaultSelection;
    use rmagna::noise::RandomSource;
    use rmagna::runtime::GuardedNoiseTiming;
    use rmagna::task::{EvaluationOptions, TaskEvaluator};
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Rng,
        Gate,
        After,
    }
    struct FixedWord {
        value: u64,
        remaining: usize,
        events: Arc<Mutex<Vec<Event>>>,
    }
    impl RandomSource for FixedWord {
        fn next_u64(&mut self) -> u64 {
            assert!(self.remaining > 0, "unexpected entropy");
            self.remaining -= 1;
            self.events.lock().unwrap().push(Event::Rng);
            self.value
        }
    }
    let source = "X_ERROR(1/4) 0\nCONTROL[gate] -> ctrl[0]\nX_ERROR(1/2) ctrl[0] 1\nCONTROL[after] ctrl[0] -> ctrl[1]\nM 0 1";
    let files = Files::new();
    std::fs::write(files.0.join("zero-two.magna"), "I 0 1").unwrap();
    let importance = NoiseSampling::Importance {
        proposal_scale: "2".parse().unwrap(),
    };
    for active in [false, true] {
        for timing in [GuardedNoiseTiming::PreDrawn, GuardedNoiseTiming::Causal] {
            for value in [0, 1] {
                let events = Arc::new(Mutex::new(Vec::new()));
                let called = events.clone();
                let mut registry = ControllerRegistry::default();
                registry
                    .register("gate", "1", move |bits: &[bool]| {
                        assert!(bits.is_empty());
                        called.lock().unwrap().push(Event::Gate);
                        Ok(vec![active])
                    })
                    .unwrap();
                let called = events.clone();
                registry
                    .register("after", "1", move |bits: &[bool]| {
                        assert_eq!(bits, [active]);
                        called.lock().unwrap().push(Event::After);
                        Ok(bits.to_vec())
                    })
                    .unwrap();
                let parse = ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                };
                let circuit = parse_circuit(source, &parse).unwrap();
                let modules = parse_task(
                    r#"{"outputs":[0,1],"sample_records":[0,1],"ideal_state":"zero-two.magna"}"#,
                    &files.0,
                    &TaskOptions::default(),
                )
                .unwrap()
                .load_modules(&circuit, &parse)
                .unwrap();
                let mut options = EvaluationOptions::default();
                options.runtime.guarded_noise_timing = timing;
                let evaluator = TaskEvaluator::new_with_sampling(
                    &circuit,
                    &modules,
                    &registry,
                    options,
                    &importance,
                )
                .unwrap();
                let words = if timing == GuardedNoiseTiming::PreDrawn || active {
                    2
                } else {
                    1
                };
                let mut rng = FixedWord {
                    value,
                    remaining: words,
                    events: events.clone(),
                };
                let (draw, diagnostics) = evaluator
                    .sample_with_diagnostics(&mut rng, &Default::default())
                    .unwrap();
                let selected = value == 1;
                let likelihood = if selected { "1/2" } else { "3/2" };
                let weighted = rmagna::field::FieldArithmetic::default()
                    .from_rational(likelihood.parse().unwrap())
                    .unwrap();
                assert_eq!(draw.sampled_records, [selected, active && selected]);
                assert_eq!(draw.accepted, weighted);
                assert_eq!(
                    draw.error,
                    if selected {
                        weighted
                    } else {
                        ExactField::zero()
                    }
                );
                assert_eq!(draw.noise_likelihood_ratio, likelihood.parse().unwrap());
                assert_eq!(
                    draw.noise_probability,
                    match (active, selected) {
                        (true, true) => "1/8",
                        (false, true) => "1/4",
                        (true, false) => "3/8",
                        (false, false) => "3/4",
                    }
                    .parse()
                    .unwrap()
                );
                assert_eq!(diagnostics.noise_entropy_words, words);
                assert_eq!(diagnostics.history_entropy_words, 0);
                assert_eq!(rng.remaining, 0);
                let expected = match (timing, active) {
                    (GuardedNoiseTiming::PreDrawn, _) => {
                        vec![Event::Rng, Event::Rng, Event::Gate, Event::After]
                    }
                    (GuardedNoiseTiming::Causal, false) => {
                        vec![Event::Rng, Event::Gate, Event::After]
                    }
                    (GuardedNoiseTiming::Causal, true) => {
                        vec![Event::Rng, Event::Gate, Event::Rng, Event::After]
                    }
                };
                assert_eq!(*events.lock().unwrap(), expected);
                if timing == GuardedNoiseTiming::Causal {
                    let mut legacy = options;
                    legacy.runtime.guarded_noise_timing = GuardedNoiseTiming::PreDrawn;
                    let fingerprint = |evaluation| {
                        FileTask::from_loaded(
                            circuit.clone(),
                            modules.clone(),
                            &registry,
                            importance.clone(),
                            FileTaskOptions {
                                evaluation,
                                ..Default::default()
                            },
                            BTreeMap::new(),
                        )
                        .unwrap()
                        .fingerprints(&run(1, 0))
                        .unwrap()
                    };
                    assert_ne!(fingerprint(options).options, fingerprint(legacy).options);
                    assert_eq!(fingerprint(options).circuit, fingerprint(legacy).circuit);
                    let mut limited = options;
                    limited.runtime.noise.max_entropy_words = 1;
                    let bounded = TaskEvaluator::new_with_sampling(
                        &circuit,
                        &modules,
                        &registry,
                        limited,
                        &importance,
                    )
                    .unwrap();
                    events.lock().unwrap().clear();
                    let mut rng = FixedWord {
                        value,
                        remaining: 1,
                        events: events.clone(),
                    };
                    let result = bounded.sample(&mut rng);
                    if active {
                        assert!(result.err().unwrap().message.contains("random-word"));
                        assert_eq!(*events.lock().unwrap(), [Event::Rng, Event::Gate]);
                    } else {
                        assert!(result.is_ok());
                    }
                    // Explicit caller assignments never generate categorical entropy,
                    // even when the evaluator requests causal guarded timing.
                    events.lock().unwrap().clear();
                    let faults = [
                        FaultSelection {
                            site: 0,
                            alternative: 0,
                        },
                        FaultSelection {
                            site: 1,
                            alternative: 0,
                        },
                    ];
                    let mut rng = FixedWord {
                        value,
                        remaining: 0,
                        events: events.clone(),
                    };
                    let supplied = evaluator.sample_prescribed(&faults, &mut rng).unwrap();
                    assert_eq!(supplied.sampled_records, [true, active]);
                    assert_eq!(supplied.accepted, ExactField::one());
                    assert_eq!(supplied.noise_likelihood_ratio, "1".parse().unwrap());
                    assert_eq!(*events.lock().unwrap(), [Event::Gate, Event::After]);
                }
            }
        }
    }
}

#[test]
fn causal_guarded_cli_options_reach_execution_and_preserve_zero_prefix_limits() {
    use serde_json::{Value, json};
    use std::process::Command;
    let files = Files::new();
    std::fs::write(files.0.join("physical.magna"), interference_source("1")).unwrap();
    std::fs::write(
        files.0.join("task.json"),
        r#"{"outputs":[6,7],"sample_records":[0,1,2,3,4,5],"ideal_state":"zero-two.magna"}"#,
    )
    .unwrap();
    std::fs::write(files.0.join("zero-two.magna"), "I 0 1").unwrap();
    std::fs::write(
        files.0.join("decoder.py"),
        "def record(bits, config):\n    return bits\n",
    )
    .unwrap();
    std::fs::write(files.0.join("controllers.json"),r#"{"schema_version":1,"bindings":{"record":{"import":"decoder:record","version":"1","config":{}}}}"#).unwrap();
    let base = [
        "run",
        "physical.magna",
        "--task",
        "task.json",
        "--controllers",
        "controllers.json",
        "--shots",
        "1",
        "--seed",
        "3",
        "--json",
        "--guarded-noise-timing",
        "causal",
    ];
    for (cap, code) in [("16777216", 0), ("0", 1)] {
        let output = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&files.0)
            .args(base)
            .args(["--max-zero-prefix-words", cap])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["options"]["guarded_noise_timing"], "causal");
        if code == 0 {
            assert_eq!(value["status"], "complete");
            assert_eq!(value["completed"], 1);
            assert_eq!(value["estimates"]["acceptance"], 0.0);
            assert_eq!(value["estimates"]["error"], 0.0);
            assert_eq!(value["execution_costs"]["decoder"]["succeeded"], 3);
            assert!(value["options"].get("max_zero_prefix_words").is_none());
        } else {
            assert_eq!(value["status"], "failed");
            assert_eq!(value["completed"], 0);
            assert_eq!(value["options"]["max_zero_prefix_words"], json!(0));
            assert!(
                value["failures"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("max_zero_prefix_words")
            );
            assert_eq!(value["execution_costs"]["decoder"]["succeeded"], 1);
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rmagna"))
        .current_dir(&files.0)
        .args([
            "enumerate",
            "physical.magna",
            "--guarded-noise-timing",
            "causal",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("unavailable in enumerate")
    );
}

fn prepared_options() -> FileTaskOptions {
    use rmagna::instrument::QueryPreparation;
    let mut options = FileTaskOptions::default();
    let compile = &mut options.evaluation.runtime.compile;
    compile.probability_preparation = QueryPreparation::FixedFibers(Default::default());
    compile.density_preparation = QueryPreparation::FixedFibers(Default::default());
    compile.moment_preparation = QueryPreparation::FixedFibers(Default::default());
    options
}

#[test]
fn original_compiled_runs_match_fresh_estimates_identities_and_call_local_work() {
    let files = Files::new();
    std::fs::write(files.0.join("ideal.magna"), "H 0\nT 0").unwrap();
    let source = "H 0\nT 0\nDEPOLARIZE1(1/20) 0\nM(1/50) 0\nCX rec[-1] 1\nH 1\nT 1";
    let task = r#"{"outputs":[1],"ideal_state":"ideal.magna"}"#;
    let registry = ControllerRegistry::default();
    let file = files.with_options(
        source,
        task,
        &registry,
        prepared_options(),
        NoiseSampling::Physical,
    );
    let fresh = file.run(&run(64, 11)).unwrap();
    let compiled = file.compile().unwrap();
    let first = compiled.run(&run(64, 11)).unwrap();
    let second = compiled.run(&run(64, 11)).unwrap();
    let fresh_json = fresh.to_json().unwrap();
    let first_json = first.to_json().unwrap();
    let second_json = second.to_json().unwrap();
    for result in [&first, &second] {
        assert_eq!(result.raw_result().status, RunStatus::Complete);
        assert_eq!(result.raw_result().completed, 64);
        let json = result.to_json().unwrap();
        for key in [
            "estimates",
            "standard_errors",
            "estimates_scientific",
            "fingerprints",
            "route",
        ] {
            assert_eq!(json[key], fresh_json[key], "{key}");
        }
        assert_eq!(
            result.raw_result().execution_costs.query.attempted,
            first.raw_result().execution_costs.query.attempted
        );
        assert_eq!(result.raw_result().execution_costs.compilation.attempted, 0);
    }
    assert_eq!(
        first_json["compilation"]["preparation_seconds"],
        second_json["compilation"]["preparation_seconds"]
    );
    assert!(
        first.raw_result().execution_costs.query.attempted
            <= fresh.raw_result().execution_costs.query.attempted
    );
    assert!(
        !first_json["compilation"]["preparation_included_in_this_call"]
            .as_bool()
            .unwrap()
    );
    assert!(
        fresh_json["compilation"]["preparation_included_in_this_call"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn original_compiled_task_and_sampling_are_fixed_before_any_run() {
    let files = Files::new();
    let mut source = "X_ERROR(1/10) 0".to_string();
    let mut task = r#"{"outputs":[0],"ideal_state":"zero.magna"}"#.to_string();
    let registry = ControllerRegistry::default();
    let file = files.with_options(
        &source,
        &task,
        &registry,
        prepared_options(),
        NoiseSampling::Importance {
            proposal_scale: "2".parse().unwrap(),
        },
    );
    let compiled = file.compile().unwrap();
    let before = compiled.run(&run(16, 12)).unwrap().to_json().unwrap();
    source.clear();
    task.clear();
    std::fs::write(files.0.join("zero.magna"), "X 0").unwrap();
    let unrelated = files.with_options(
        "X_ERROR(1/2) 0",
        "{}",
        &registry,
        prepared_options(),
        NoiseSampling::Physical,
    );
    assert_ne!(
        unrelated.fingerprints(&run(16, 12)).unwrap(),
        file.fingerprints(&run(16, 12)).unwrap()
    );
    let after = compiled.run(&run(16, 12)).unwrap().to_json().unwrap();
    assert_eq!(after["sampling"]["method"], "importance");
    assert_eq!(
        after["sampling"]["configuration"]["proposal_scale"]["numerator"],
        "2"
    );
    assert_eq!(
        after["evaluation"]["output_mapping"],
        serde_json::json!([0])
    );
    assert!(!after["evaluation"]["ideal_state"].is_null());
    for key in [
        "estimates",
        "standard_errors",
        "fingerprints",
        "sampling",
        "evaluation",
    ] {
        assert_eq!(before[key], after[key], "{key}");
    }
    // Compile-fail examples on CompiledFileTask::run prove task/sampling
    // overrides cannot enter this call or execute a callback in typed Rust.
}

#[test]
fn original_each_compiled_run_starts_fresh_query_and_deadline_budgets() {
    use std::time::Duration;
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", |bits: &[bool]| {
            assert_eq!(bits.len(), 3);
            Ok(vec![bits.iter().filter(|&&b| b).count() >= 2])
        })
        .unwrap();
    let source = "H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[-3] det[-2] det[-1] -> ctrl[0]\nCX ctrl[0] 3";
    let file = files.with_options(
        source,
        "{}",
        &registry,
        prepared_options(),
        NoiseSampling::Physical,
    );
    let compiled = file.compile().unwrap();
    let baseline = compiled.run(&run(5, 1)).unwrap();
    assert_eq!(baseline.raw_result().completed, 5);
    let queries = baseline.raw_result().execution_costs.query.attempted;
    assert!(queries > 0);
    for seed in 0..4 {
        let mut options = run(5, seed);
        options.execution_limits.max_queries = queries + 1;
        let report = compiled.run(&options).unwrap();
        assert_eq!(report.raw_result().status, RunStatus::Complete);
        assert_eq!(report.raw_result().completed, 5);
        assert!(report.raw_result().execution_costs.query.attempted <= queries + 1);
    }
    let mut timed = run(2, 0);
    timed.execution_limits.max_duration = Some(Duration::from_millis(500));
    assert_eq!(
        compiled.run(&timed).unwrap().raw_result().status,
        RunStatus::Complete
    );
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        compiled.run(&timed).unwrap().raw_result().status,
        RunStatus::Complete
    );
    let mut zero = run(1, 0);
    zero.execution_limits.max_queries = 0;
    let refused = compiled.run(&zero).unwrap();
    assert_eq!(refused.raw_result().status, RunStatus::Failed);
    assert_eq!(refused.raw_result().completed, 0);
    assert_eq!(
        compiled.run(&run(5, 1)).unwrap().raw_result().status,
        RunStatus::Complete
    );
}

#[test]
fn original_noiseless_batch_keeps_rng_and_70_noisy_draws_match_single_stream() {
    use rmagna::instrument::{CompileOptions, compile_affine};
    use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
    use rmagna::scalar::ScalarArithmetic;
    let sampler = |source: &str, sampling: &NoiseSampling| {
        let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
        let compiled = compile_affine(
            &circuit,
            &CompileOptions::default(),
            &ScalarArithmetic::default(),
        )
        .unwrap();
        let model = NoiseModel::physical(&compiled, &NoiseOptions::default()).unwrap();
        NoiseSampler::configured(model, sampling, &BTreeSet::new(), NoiseOptions::default())
            .unwrap()
    };
    let empty = sampler("H 0", &NoiseSampling::Physical);
    let mut rng = SplitMix64::new(12);
    let state = rng.state();
    let draws = empty
        .draws(&mut rng, 7)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rng.state(), state);
    assert_eq!(draws.len(), 7);
    for draw in draws {
        assert!(draw.faults.is_empty());
        assert_eq!(draw.entropy_words, 0);
        assert_eq!(draw.physical_probability, "1".parse().unwrap());
        assert_eq!(draw.proposal_probability, draw.physical_probability);
        assert_eq!(draw.likelihood_ratio, draw.physical_probability);
    }
    let noisy = sampler(
        "DEPOLARIZE2(1/10) 0 1\nM(1/5) 0",
        &NoiseSampling::Importance {
            proposal_scale: "2".parse().unwrap(),
        },
    );
    let mut single = SplitMix64::new(7);
    let expected = (0..70)
        .map(|_| noisy.draw(&mut single).unwrap())
        .collect::<Vec<_>>();
    let mut batch = SplitMix64::new(7);
    let actual = noisy
        .draws(&mut batch, 70)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(batch.state(), single.state());
    for draw in &actual {
        let depol = draw.faults.iter().any(|f| f.site == 0);
        let report = draw.faults.iter().any(|f| f.site == 1);
        let ratio = match (depol, report) {
            (false, false) => "3/2",
            (false, true) => "9/16",
            (true, false) => "2/3",
            (true, true) => "1/4",
        };
        assert_eq!(draw.likelihood_ratio, ratio.parse().unwrap());
    }
    assert!(actual.iter().any(|d| d.faults.is_empty()));
    assert!(actual.iter().any(|d| !d.faults.is_empty()));
}

#[test]
fn original_70_draw_internal_batch_preserves_all_weights_target_recovery_and_work() {
    use rmagna::costs::{ExecutionLimits, ExecutionMeter};
    use rmagna::noise::SplitMix64;
    use rmagna::task::TaskEvaluator;
    let files = Files::new();
    std::fs::write(files.0.join("ideal.magna"), "H 0\nT 0").unwrap();
    std::fs::write(files.0.join("recovery.magna"), "H 1\nM 1\nCX rec[-1] 0").unwrap();
    let source = "H 0\nT 0\nDEPOLARIZE1(1/7) 0\nH 1\nM(1/5) 1\nCX rec[-1] 0";
    let parse = ParseOptions::default();
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules=parse_task(r#"{"outputs":[0],"observables":[[1,0]],"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,&files.0,&TaskOptions::default()).unwrap().load_modules(&circuit,&parse).unwrap();
    let options = prepared_options().evaluation;
    let registry = ControllerRegistry::default();
    let sampling = NoiseSampling::Importance {
        proposal_scale: "2".parse().unwrap(),
    };
    let batch_cost = ExecutionMeter::new(ExecutionLimits::default());
    let single_cost = ExecutionMeter::new(ExecutionLimits::default());
    let batch = TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        &registry,
        options,
        &sampling,
        &batch_cost,
    )
    .unwrap();
    let single = TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        &registry,
        options,
        &sampling,
        &single_cost,
    )
    .unwrap();
    assert_eq!(batch_cost.snapshot().compilation.succeeded, 2);
    assert_eq!(single_cost.snapshot().compilation.succeeded, 2);
    let mut left = SplitMix64::new(12);
    let expected = (0..70)
        .map(|_| single.sample_with_costs(&mut left, &single_cost).unwrap())
        .collect::<Vec<_>>();
    let mut right = SplitMix64::new(12);
    let actual = batch
        .samples_with_costs(&mut right, 70, &batch_cost)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(actual.len(), 70);
    assert_eq!(right.state(), left.state());
    for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
        assert!(
            a == b,
            "all exact/sample/resource fields differ at draw {i}"
        );
        assert_eq!(a.network_compilations, 0);
        assert!(a.sampled_records.is_empty());
        assert!(a.target_success.is_some());
        assert_eq!(a.moments.len(), 1);
        assert!(
            ["8/5", "3/5", "2/3", "1/4"]
                .iter()
                .any(|s| a.noise_likelihood_ratio == s.parse().unwrap())
        );
    }
    assert_eq!(
        batch_cost.snapshot().query.attempted,
        single_cost.snapshot().query.attempted
    );
    assert_eq!(batch_cost.snapshot().compilation.succeeded, 2);
    let cached = batch
        .prepared_probability_statistics()
        .unwrap()
        .map_or(0, |s| s.cached_queries)
        + batch
            .prepared_density_statistics()
            .unwrap()
            .map_or(0, |s| s.cached_queries)
        + batch
            .prepared_moment_statistics()
            .unwrap()
            .map_or(0, |s| s.cached_queries);
    assert!(cached <= 4, "retained query families: {cached}");
}

#[test]
fn lazy_sample_batches_stop_once_on_error_and_empty_batches_have_no_effects() {
    use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
    use rmagna::task::TaskEvaluator;
    let mut rng = SplitMix64::new(5);
    let before = rng.state();
    let sampler = NoiseSampler::physical(
        NoiseModel::new(
            vec![vec!["1/2".parse().unwrap(); 2]],
            &NoiseOptions::default(),
        )
        .unwrap(),
        NoiseOptions {
            max_entropy_words: 0,
            ..Default::default()
        },
    );
    let mut draws = sampler.draws(&mut rng, 3);
    assert!(draws.next().unwrap().is_err());
    assert!(draws.next().is_none());
    assert!(draws.next().is_none());
    drop(draws);
    assert_eq!(rng.state(), before);
    assert_eq!(sampler.draws(&mut rng, 0).count(), 0);
    let called = Arc::new(AtomicUsize::new(0));
    let counter = called.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("bad", "1", move |_: &[bool]| {
            counter.fetch_add(1, Ordering::SeqCst);
            Err("batch stop".to_owned())
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit("CONTROL[bad] -> ctrl[0]", &parse).unwrap();
    let files = Files::new();
    let modules = parse_task("{}", &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, Default::default()).unwrap();
    let mut draws = evaluator.samples(&mut rng, 3);
    assert!(
        draws
            .next()
            .unwrap()
            .err()
            .unwrap()
            .message
            .contains("batch stop")
    );
    assert!(draws.next().is_none());
    assert!(draws.next().is_none());
    drop(draws);
    assert_eq!(called.load(Ordering::SeqCst), 1);
    assert_eq!(rng.state(), before);
    assert_eq!(evaluator.samples(&mut rng, 0).count(), 0);
    assert_eq!(called.load(Ordering::SeqCst), 1);
}

#[test]
fn original_prepared_scalar_queries_match_40_cold_queries_with_report_flip_controls() {
    use rmagna::costs::{ExecutionLimits, ExecutionMeter};
    use rmagna::instrument::{CompileOptions, FaultSelection, QueryPreparation, compile_affine};
    use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
    use rmagna::scalar::{ExactScalar, ScalarArithmetic};
    use rmagna::task::TaskEvaluator;
    let files = Files::new();
    std::fs::write(files.0.join("ideal.magna"), "H 0").unwrap();
    let source = "H 0\nT 0\nCX 0 1\nDEPOLARIZE2(1/4) 0 1\nM(1/3) 1\nCX rec[-1] 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let registry = ControllerRegistry::default();
    let modules = parse_task(
        r#"{"outputs":[0],"observables":[[1,0],[1,1]],"ideal_state":"ideal.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &ParseOptions::default())
    .unwrap();
    let warm_options = prepared_options().evaluation;
    let mut cold_options = warm_options;
    cold_options.runtime.compile.probability_preparation = QueryPreparation::Cold;
    cold_options.runtime.compile.density_preparation = QueryPreparation::Cold;
    cold_options.runtime.compile.moment_preparation = QueryPreparation::Cold;
    let warm_cost = ExecutionMeter::new(ExecutionLimits::default());
    let cold_cost = ExecutionMeter::new(ExecutionLimits {
        max_queries: 10000,
        ..Default::default()
    });
    let warm = TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        &registry,
        warm_options,
        &NoiseSampling::Physical,
        &warm_cost,
    )
    .unwrap();
    let cold = TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        &registry,
        cold_options,
        &NoiseSampling::Physical,
        &cold_cost,
    )
    .unwrap();
    let a = ScalarArithmetic::default();
    let physical = compile_affine(&circuit, &CompileOptions::default(), &a).unwrap();
    for (j, site) in physical.noise_sites().iter().enumerate() {
        for &op in site.source_operations() {
            assert_eq!(warm.potential_site(op), Some(j));
            assert_eq!(cold.potential_site(op), Some(j));
        }
    }
    let sampler = NoiseSampler::physical(
        NoiseModel::physical(&physical, &NoiseOptions::default()).unwrap(),
        NoiseOptions::default(),
    );
    let mut noise_rng = SplitMix64::new(8);
    let noises = sampler
        .draws(&mut noise_rng, 40)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(noises.len(), 40);
    assert!(noises.iter().any(|n| n.faults.iter().any(|f| f.site == 1)));
    let mut cold_rng = SplitMix64::new(8);
    for (index, noise) in noises.iter().enumerate() {
        let x = warm
            .sample_supplied_noise(
                &noise.faults,
                &"1".parse().unwrap(),
                &mut SplitMix64::new(1),
            )
            .unwrap();
        let y = cold.sample_with_costs(&mut cold_rng, &cold_cost).unwrap();
        assert!(x == y, "prepared/cold exact fields differ at draw {index}");
    }
    assert_eq!(cold_rng.state(), noise_rng.state());
    assert_eq!(cold_cost.snapshot().query.attempted, 161);
    assert!(
        warm.prepared_probability_statistics()
            .unwrap()
            .unwrap()
            .cached_queries
            > 0
    );
    assert_eq!(
        cold.prepared_probability_statistics()
            .unwrap()
            .unwrap()
            .cached_queries,
        0
    );
    // Supplemental Z distinguishes the original report inversion from a true
    // record and detects dropping the final detector selection.
    let modules = parse_task(
        r#"{"outputs":[0],"observables":[[1,0],[1,1],[0,1]],"ideal_state":"ideal.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &ParseOptions::default())
    .unwrap();
    let z = TaskEvaluator::new(&circuit, &modules, &registry, warm_options).unwrap();
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    for flips in [false, true] {
        let faults = if flips {
            vec![FaultSelection {
                site: 1,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        let draw = z
            .sample_prescribed(&faults, &mut SplitMix64::new(1))
            .unwrap();
        assert_eq!(draw.raw_acceptance, half);
        assert_eq!(
            draw.raw_moments[..2],
            [ExactScalar::zero(), ExactScalar::zero()]
        );
        assert_eq!(
            draw.raw_moments[2],
            if flips { half.negated() } else { half.clone() }
        );
    }
}

fn selections(records: usize, rows: &[Vec<usize>]) -> String {
    let mut text = String::new();
    for row in rows {
        text.push_str("DETECTOR");
        for &record in row {
            text.push_str(&format!(" rec[{}]", record as isize - records as isize));
        }
        text.push('\n');
    }
    for j in 0..rows.len() {
        text.push_str(&format!("POSTSELECT det[{j}]\n"));
    }
    text
}
fn checks(records: usize, rows: &[Vec<usize>]) -> String {
    let mut text = String::new();
    for (j, row) in rows.iter().enumerate() {
        text.push_str(&format!("OBSERVABLE_INCLUDE({j})"));
        for &record in row {
            text.push_str(&format!(" rec[{}]", record as isize - records as isize));
        }
        text.push('\n');
    }
    text
}
fn exact_evaluator(
    files: &Files,
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: rmagna::task::EvaluationOptions,
    meter: &rmagna::costs::ExecutionMeter,
) -> rmagna::task::TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    rmagna::task::TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        registry,
        options,
        &NoiseSampling::Physical,
        meter,
    )
    .unwrap()
}

#[test]
fn original_report_parities_equal_quantum_readout_and_independent_selected_event() {
    use rmagna::costs::{ExecutionLimits, ExecutionMeter};
    use rmagna::instrument::FaultSelection;
    use rmagna::noise::SplitMix64;
    use rmagna::scalar::ScalarArithmetic;
    use serde_json::json;
    let source = "H 0\nT 0\nH 0\nM(1/3) 0\nH 1\nM 1\n";
    let files = Files::new();
    std::fs::write(
        files.0.join("empty-zero.magna"),
        "# All zero on the selected outputs.\n",
    )
    .unwrap();
    let registry = ControllerRegistry::default();
    let row_sets = [
        vec![],
        vec![vec![0]],
        vec![vec![0, 1]],
        vec![vec![0], vec![1]],
        vec![vec![0, 0]],
        vec![vec![], vec![0]],
    ];
    let mut compared = 0;
    let arithmetic = ScalarArithmetic::default();
    for rows in row_sets {
        let (rewritten, outputs) = quantum_readout(source, 2, 2, &rows);
        let selected = selections(2, &[vec![1]]);
        let target_cost = ExecutionMeter::new(ExecutionLimits::default());
        let native_cost = ExecutionMeter::new(ExecutionLimits::default());
        let reference_cost = ExecutionMeter::new(ExecutionLimits::default());
        let target = exact_evaluator(
            &files,
            &format!("{rewritten}{selected}"),
            &json!({"outputs":outputs,"ideal_state":"empty-zero.magna"}).to_string(),
            &registry,
            prepared_options().evaluation,
            &target_cost,
        );
        let native = exact_evaluator(
            &files,
            &format!("{source}{selected}{}", checks(2, &rows)),
            &json!({"error_observables":(0..rows.len()).collect::<Vec<_>>()} ).to_string(),
            &registry,
            prepared_options().evaluation,
            &native_cost,
        );
        let mut reference_rows = vec![vec![1]];
        reference_rows.extend(rows.clone());
        let reference = exact_evaluator(
            &files,
            &format!("{source}{}", selections(2, &reference_rows)),
            r#"{"outputs":[]}"#,
            &registry,
            prepared_options().evaluation,
            &reference_cost,
        );
        // Rust explicitly constructs the requested zero-width ideal as well;
        // both compiler invocations are charged even when there are no rows.
        assert_eq!(target_cost.snapshot().compilation.succeeded, 2);
        for flips in [vec![], vec![0], vec![1], vec![0, 1]] {
            let faults = flips
                .iter()
                .map(|&site| FaultSelection {
                    site,
                    alternative: 0,
                })
                .collect::<Vec<_>>();
            let actual = target
                .sample_prescribed(&faults, &mut SplitMix64::new(1))
                .unwrap();
            let fast = native
                .sample_prescribed(&faults, &mut SplitMix64::new(1))
                .unwrap();
            let expected = reference
                .sample_prescribed(&faults, &mut SplitMix64::new(1))
                .unwrap();
            assert_eq!(actual.raw_acceptance, fast.raw_acceptance);
            assert_eq!(actual.raw_error, fast.raw_error);
            assert_eq!(actual.accepted, fast.accepted);
            assert_eq!(actual.error, fast.error);
            assert_eq!(
                arithmetic
                    .subtract(&actual.raw_acceptance, &actual.raw_error)
                    .unwrap(),
                expected.raw_acceptance
            );
            assert!(actual.sampled_records.is_empty());
            compared += 1;
        }
        assert_eq!(target_cost.snapshot().compilation.succeeded, 2);
        let retained = target.prepared_density_statistics().unwrap().unwrap();
        assert_eq!(retained.cached_queries, 1);
        if !rows.is_empty() {
            assert!(target.prepared_density_reuses().unwrap() > 0);
        }
    }
    assert_eq!(compared, 24);
}

#[test]
fn original_causal_readout_callbacks_match_both_history_methods_and_report_flips() {
    use rmagna::costs::{ExecutionLimits, ExecutionMeter};
    use rmagna::instrument::FaultSelection;
    use rmagna::noise::SplitMix64;
    use rmagna::runtime::HistorySampler;
    use serde_json::json;
    let files = Files::new();
    std::fs::write(files.0.join("empty-zero.magna"), "# Zero target.\n").unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            observed.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    let source =
        "H 0\nM(1/3) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1\nM 1\n";
    let (rewritten, outputs) = quantum_readout(source, 2, 2, &[vec![0, 1]]);
    for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
        let mut options = prepared_options().evaluation;
        options.runtime.history_sampler = sampler;
        let target_cost = ExecutionMeter::new(ExecutionLimits::default());
        let native_cost = ExecutionMeter::new(ExecutionLimits::default());
        let target = exact_evaluator(
            &files,
            &rewritten,
            &json!({"outputs":outputs,"ideal_state":"empty-zero.magna"}).to_string(),
            &registry,
            options,
            &target_cost,
        );
        let native = exact_evaluator(
            &files,
            &format!("{source}OBSERVABLE_INCLUDE(0) rec[-2] rec[-1]\n"),
            r#"{"error_observables":[0]}"#,
            &registry,
            options,
            &native_cost,
        );
        for flips in [false, true] {
            calls.lock().unwrap().clear();
            let faults = if flips {
                vec![FaultSelection {
                    site: 0,
                    alternative: 0,
                }]
            } else {
                vec![]
            };
            let mut left = SplitMix64::new(19);
            let actual = target.sample_prescribed(&faults, &mut left).unwrap();
            let expected_calls = std::mem::take(&mut *calls.lock().unwrap());
            let mut right = SplitMix64::new(19);
            let expected = native.sample_prescribed(&faults, &mut right).unwrap();
            assert_eq!(actual.raw_acceptance, expected.raw_acceptance);
            assert_eq!(actual.raw_error, expected.raw_error);
            assert_eq!(actual.raw_moments, expected.raw_moments);
            assert_eq!(left.state(), right.state());
            assert_eq!(*calls.lock().unwrap(), expected_calls);
            assert_eq!(actual.accepted, ExactField::one());
            assert_eq!(actual.error, ExactField::zero());
            assert_eq!(actual.controller_calls.len(), 1);
        }
        assert_eq!(target_cost.snapshot().compilation.succeeded, 2);
        assert_eq!(
            target
                .prepared_density_statistics()
                .unwrap()
                .unwrap()
                .cached_queries,
            1
        );
        assert!(target.prepared_density_reuses().unwrap() > 0);
    }
}

#[test]
fn original_annotations_manual_faults_and_sampled_source_snapshot_match_readout() {
    use rmagna::costs::{ExecutionLimits, ExecutionMeter};
    use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
    use rmagna::noise::{NoiseModel, NoiseOptions, SplitMix64};
    use rmagna::runtime::HistorySampler;
    use rmagna::scalar::ScalarArithmetic;
    use serde_json::json;
    let source = "X_ERROR(1/5) 0\nM(1/7) 0\nOBSERVABLE_INCLUDE(4) rec[-1]\nH 1\nM 1\nOBSERVABLE_INCLUDE(4) rec[-1]\nOBSERVABLE_INCLUDE(9)\nOBSERVABLE_INCLUDE(0) rec[-2] rec[-2]\n";
    let rows = vec![vec![0, 0], vec![0, 1], vec![], vec![0, 1]];
    let (rewritten, outputs) = quantum_readout(source, 2, 2, &rows);
    let physical = parse_circuit(source, &ParseOptions::default()).unwrap();
    let explicit = parse_circuit(&rewritten, &ParseOptions::default()).unwrap();
    assert_eq!(physical.to_text(), source);
    assert_eq!(physical.num_qubits(), 2);
    assert_eq!(physical.num_records(), 2);
    assert_eq!(
        &explicit.operations()[..physical.operations().len()],
        physical.operations()
    );
    assert_eq!(explicit.num_records(), 2);
    let arithmetic = ScalarArithmetic::default();
    let left = compile_affine(&physical, &CompileOptions::default(), &arithmetic).unwrap();
    let right = compile_affine(&explicit, &CompileOptions::default(), &arithmetic).unwrap();
    let p = NoiseModel::physical(&left, &NoiseOptions::default()).unwrap();
    let q = NoiseModel::physical(&right, &NoiseOptions::default()).unwrap();
    assert_eq!(p.num_sites(), q.num_sites());
    assert_eq!(p.num_sites(), 3);
    for j in 0..p.num_sites() {
        assert_eq!(p.probabilities(j), q.probabilities(j));
        assert_eq!(
            left.noise_sites()[j].source_operations(),
            right.noise_sites()[j].source_operations()
        );
    }
    let files = Files::new();
    std::fs::write(
        files.0.join("empty-zero.magna"),
        "# Zero on the selected outputs.\n",
    )
    .unwrap();
    let registry = ControllerRegistry::default();
    let raw_task = json!({"outputs":[],"error_observables":[0,4,9,4]});
    let native_cost = ExecutionMeter::new(ExecutionLimits::default());
    let target_cost = ExecutionMeter::new(ExecutionLimits::default());
    let native = exact_evaluator(
        &files,
        source,
        &raw_task.to_string(),
        &registry,
        prepared_options().evaluation,
        &native_cost,
    );
    let target_task = json!({"outputs":outputs,"ideal_state":"empty-zero.magna"});
    let target = exact_evaluator(
        &files,
        &rewritten,
        &target_task.to_string(),
        &registry,
        prepared_options().evaluation,
        &target_cost,
    );
    for faults in [
        vec![],
        vec![
            FaultSelection {
                site: 0,
                alternative: 0,
            },
            FaultSelection {
                site: 1,
                alternative: 0,
            },
        ],
    ] {
        let x = native
            .sample_prescribed(&faults, &mut SplitMix64::new(5))
            .unwrap();
        let y = target
            .sample_prescribed(&faults, &mut SplitMix64::new(5))
            .unwrap();
        assert_eq!(x.raw_acceptance, y.raw_acceptance);
        assert_eq!(x.raw_error, y.raw_error);
        assert_eq!(x.noise_probability, y.noise_probability);
        assert_eq!(x.history_mass, y.history_mass);
    }
    let file = files.with_options(
        source,
        &raw_task.to_string(),
        &registry,
        prepared_options(),
        NoiseSampling::Physical,
    );
    let descriptor = file.inspect(&run(1, 0)).unwrap();
    assert_eq!(descriptor["circuit"]["qubits"], 2);
    assert_eq!(descriptor["circuit"]["records"], 2);
    assert_eq!(
        descriptor["circuit"]["operations"],
        physical.operations().len()
    );
    assert_eq!(
        descriptor["evaluation"]["output_mapping"],
        raw_task["outputs"]
    );
    assert_eq!(
        descriptor["evaluation"]["error_observables"],
        raw_task["error_observables"]
    );
    for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
        let mut options = prepared_options().evaluation;
        options.runtime.history_sampler = sampler;
        let native_cost = ExecutionMeter::new(ExecutionLimits::default());
        let target_cost = ExecutionMeter::new(ExecutionLimits::default());
        let mut sampled_task = raw_task.clone();
        sampled_task["sample_records"] = json!([0]);
        let native = exact_evaluator(
            &files,
            source,
            &sampled_task.to_string(),
            &registry,
            options,
            &native_cost,
        );
        let mut sampled_target = target_task.clone();
        sampled_target["sample_records"] = json!([0]);
        let target = exact_evaluator(
            &files,
            &rewritten,
            &sampled_target.to_string(),
            &registry,
            options,
            &target_cost,
        );
        for seed in 0..8 {
            let mut a = SplitMix64::new(seed);
            let mut b = SplitMix64::new(seed);
            let x = native.sample(&mut a).unwrap();
            let y = target.sample(&mut b).unwrap();
            assert_eq!(x.sampled_records, y.sampled_records);
            assert_eq!(x.raw_acceptance, y.raw_acceptance);
            assert_eq!(x.raw_error, y.raw_error);
            assert_eq!(x.history_mass, y.history_mass);
            assert_eq!(x.noise_probability, y.noise_probability);
            assert_eq!(a.state(), b.state());
        }
        assert_eq!(target_cost.snapshot().compilation.succeeded, 2);
        assert!(target.prepared_density_reuses().unwrap() > 0);
    }
    assert_eq!(physical.to_text(), source);
    assert_eq!(physical.operations().len(), 8);
}

impl Files {
    fn cli_fixture() -> Self {
        let files = Self::new();
        std::fs::write(files.0.join("physical.magna"), "X_ERROR(1/1000) 0").unwrap();
        std::fs::write(files.0.join("ideal.magna"), "I 0").unwrap();
        std::fs::write(
            files.0.join("task.json"),
            r#"{"schema_version":1,"ideal_state":"ideal.magna"}"#,
        )
        .unwrap();
        files
    }
    fn cli(&self, args: &[&str], code: i32) -> std::process::Output {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "args={args:?}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

#[test]
fn original_cli_summary_full_json_and_output_file_preserve_all_200_draws() {
    use serde_json::Value;
    let files = Files::cli_fixture();
    let args = [
        "run",
        "physical.magna",
        "--task",
        "task.json",
        "--shots",
        "200",
        "--seed",
        "7",
    ];
    let human = files.cli(&args, 0);
    assert!(human.stderr.is_empty());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.starts_with("rmagna: complete\n"), "{text}");
    assert!(text.contains("Completed draws: 200/200"), "{text}");
    assert!(text.contains("acceptance: 1 ± 0"), "{text}");
    assert!(text.contains("conditional error:"), "{text}");
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = files.cli(&json_args, 0);
    assert!(json.stderr.is_empty());
    let value: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["completed"], 200);
    assert_eq!(value["seed"], 7);
    assert_eq!(value["estimates"]["acceptance"], 1.0);
    assert_eq!(value["standard_errors"]["acceptance"], 0.0);
    let destination = files.0.join("result.json");
    let mut output_args = args.to_vec();
    output_args.extend(["--output", destination.to_str().unwrap()]);
    let human = files.cli(&output_args, 0);
    assert!(human.stderr.is_empty());
    assert!(String::from_utf8(human.stdout).unwrap().contains(&format!(
        "Full result written to {}.",
        destination.display()
    )));
    let saved: Value = serde_json::from_slice(&std::fs::read(destination).unwrap()).unwrap();
    assert_eq!(saved["completed"], 200);
    for field in [
        "estimates",
        "standard_errors",
        "estimates_scientific",
        "fingerprints",
        "weights",
        "seed",
        "status",
    ] {
        assert_eq!(saved[field], value[field], "{field}");
    }
}

#[test]
fn original_cli_command_summaries_enforce_option_scopes_and_explicit_weight() {
    let files = Files::cli_fixture();
    for (mode, extra, prefix, summary) in [
        ("validate", vec![], "rmagna: valid", "Circuit: 1 qubits"),
        ("inspect", vec![], "rmagna: complete", "Route:"),
        (
            "enumerate",
            vec!["--max-weight", "1"],
            "rmagna: complete",
            "Covered physical probability: 1",
        ),
    ] {
        let mut args = vec![mode, "physical.magna", "--task", "task.json"];
        args.extend(extra);
        let output = files.cli(&args, 0);
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.starts_with(prefix) && text.contains(summary), "{text}");
        let mut json_args = args.clone();
        json_args.push("--json");
        let value: serde_json::Value =
            serde_json::from_slice(&files.cli(&json_args, 0).stdout).unwrap();
        if mode == "inspect" {
            // Rust inspect is dry compilation. Eager query building is prepare.
            assert_eq!(value["compilation"]["status"], "compiled");
            assert_eq!(value["compilation"]["query_preparation"], "on_demand");
        }
    }
    for args in [
        vec!["validate", "physical.magna", "--shots", "8"],
        vec![
            "enumerate",
            "physical.magna",
            "--max-weight",
            "1",
            "--history-sampler",
            "exact_marginal",
        ],
        vec!["enumerate", "physical.magna", "--sampling", "task.json"],
        vec!["run", "physical.magna", "--max-weight", "1"],
        vec!["enumerate", "physical.magna"],
    ] {
        let output = files.cli(&args, 2);
        assert!(output.stdout.is_empty(), "{args:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("rmagna: invalid\n"), "{error}");
        assert!(error.contains("--"), "{error}");
    }
}

#[test]
fn original_cli_version_and_command_help_name_the_rust_package_and_draw_count() {
    let files = Files::new();
    let output = files.cli(&["--version"], 0);
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "rmagna ",
            env!("CARGO_PKG_VERSION"),
            " (independent Rust implementation)\n"
        )
    );
    let output = files.cli(&["run", "--help"], 0);
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(
        help.contains("--shots N") && help.contains("number of draws"),
        "{help}"
    );
}

#[test]
fn original_cli_invalid_input_uses_stderr_or_invalid_json_with_spelling_help() {
    let files = Files::cli_fixture();
    // The Rust CLI exposes typed flags instead of a Python options dictionary.
    for tail in [vec!["--max-term", "4"], vec!["--task", "invalid.json"]] {
        std::fs::write(files.0.join("invalid.json"), r#"{"output":[0]}"#).unwrap();
        let mut args = vec!["run", "physical.magna"];
        args.extend(tail);
        let output = files.cli(&args, 2);
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        let (key, fix) = if args.contains(&"--max-term") {
            ("--max-term", "--max-terms")
        } else {
            ("output", "outputs")
        };
        assert!(
            error.contains(key) && error.contains(fix) && error.contains("did you mean"),
            "{error}"
        );
        args.push("--json");
        let output = files.cli(&args, 2);
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "invalid");
        assert!(value["error"].as_str().unwrap().contains(fix));
    }
}

#[test]
fn original_cli_budget_failure_uses_failed_exit_one_instead_of_invalid_input() {
    let files = Files::new();
    std::fs::write(files.0.join("t.magna"), "H 0\nT 0\nH 0\nT 0\nH 0\nM 0").unwrap();
    for (mode, budget, field) in [
        // Dry inspect has no contraction: reject at its path-budget boundary.
        ("inspect", "--max-path-bits", "max_path_bits"),
        // Eager prepare exercises the original zero contraction-term budget.
        ("prepare", "--max-terms", "limit is 0"),
    ] {
        let args = [mode, "t.magna", budget, "0"];
        let output = files.cli(&args, 1);
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("rmagna: failed") && error.contains(field),
            "{error}"
        );
        let mut args = args.to_vec();
        args.push("--json");
        let output = files.cli(&args, 1);
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "failed");
        assert!(value.to_string().contains(field));
    }
}

#[test]
fn original_compiled_option_task_range_cache_and_sampling_errors_suggest_repairs() {
    use rmagna::noise::{SamplingDeclarationOptions, parse_sampling_declaration};
    let files = Files::cli_fixture();
    let output = files.cli(&["run", "physical.magna", "--max-term", "4", "--json"], 2);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let error = value["error"].as_str().unwrap();
    assert!(
        error.contains("--max-term")
            && error.contains("did you mean --max-terms?")
            && error.contains("--help"),
        "{error}"
    );
    let error = parse_task(r#"{"output":[0]}"#, &files.0, &TaskOptions::default())
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.contains("output") && error.contains("did you mean") && error.contains("outputs"),
        "{error}"
    );
    let circuit = parse_circuit("X_ERROR(1/10) 0\nM 0", &ParseOptions::default()).unwrap();
    let error = parse_task(r#"{"outputs":[3]}"#, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.contains("outputs") && error.contains("index 3") && error.contains("width 1"),
        "{error}"
    );
    let mut options = prepared_options();
    options
        .evaluation
        .runtime
        .compile
        .max_prepared_probabilities = 0;
    let file = files.with_options(
        "X_ERROR(1/10) 0",
        "{}",
        &ControllerRegistry::default(),
        options,
        NoiseSampling::Physical,
    );
    let error = file.compile().err().unwrap().to_string();
    assert!(
        error.contains("max_prepared_probabilities") && error.contains("0") && error.contains("1"),
        "{error}"
    );
    let error = parse_sampling_declaration(
        r#"{"method":"tilted"}"#,
        &SamplingDeclarationOptions::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("sampling.method")
            && error.contains("physical")
            && error.contains("importance"),
        "{error}"
    );
    // Suggestions stay bounded and decline ties instead of recommending at random.
    assert_eq!(
        rmagna::validation::closest_name("abc", ["abd", "abe"]),
        None
    );
    assert_eq!(
        rmagna::validation::closest_name(&"x".repeat(81), ["max_terms"]),
        None
    );
    assert_eq!(
        rmagna::validation::closest_name("max_term", ["max_terms", "max_terms"]),
        Some("max_terms")
    );
}
