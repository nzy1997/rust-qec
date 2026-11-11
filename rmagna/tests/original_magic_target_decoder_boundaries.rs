// SPDX-License-Identifier: MPL-2.0
// Original FileApiTests magic-target/readout and rejected decoder boundaries
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py, exercised through the Rust file and task APIs.

use num_rational::BigRational;
use rmagna::circuit::{Circuit, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::file_api::{FileApiError, FileLoadOptions, FileRunReport, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, RandomSource};
use rmagna::runtime::{CausalStrategy, ControllerCall, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskError, TaskEvaluation, TaskEvaluator, TaskOptions,
    TaskRunOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const MAGIC: &str = "H 0\nT 0";
const ORTHOGONAL: &str = "H 0\nT 0\nZ 0";
const MAGIC_TASK: &str = r#"{"ideal_state":"magic-target.magna"}"#;
const READOUT: &str = "X 0\nM 0\nDETECTOR rec[-1]\nCONTROL[readout] det[-1] -> ctrl[0] ctrl[1]\nOBSERVABLE_INCLUDE(0) ctrl[1]";
const READOUT_TASK: &str = r#"{"error_observables":[0]}"#;
const UNINITIALIZED: &str =
    "M 0\nDETECTOR rec[-1]\nCONTROL[readout] det[-1] -> ctrl[0]\nOBSERVABLE_INCLUDE(0) ctrl[1]";

struct TestFiles(PathBuf);

impl TestFiles {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-magic-target-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TestFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("the original deterministic case must not request entropy")
    }
}

fn load_file(
    circuit: &Path,
    task: Option<&Path>,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
) -> Result<FileTask, FileApiError> {
    FileTask::load(
        circuit,
        task,
        registry,
        NoiseSampling::Physical,
        FileTaskOptions {
            evaluation,
            ..Default::default()
        },
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
}

fn field(bit: bool) -> ExactField {
    if bit {
        ExactField::one()
    } else {
        ExactField::zero()
    }
}

fn scalar(bit: bool) -> ExactScalar {
    if bit {
        ExactScalar::one()
    } else {
        ExactScalar::zero()
    }
}

fn assert_file_report(report: &FileRunReport, error: bool, route: &str) {
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Complete);
    assert_eq!(raw.completed, 1);
    assert_eq!(raw.failed, 0);
    let statistics = raw.statistics.as_ref().unwrap();
    assert_eq!(statistics.accepted, ExactField::one());
    assert_eq!(statistics.error, Some(field(error)));
    assert_eq!(statistics.conditional_error, Some(field(error)));
    let json = report.to_json().unwrap();
    let error = f64::from(u8::from(error));
    assert_eq!(json["status"], "complete");
    assert_eq!(json["completed"], 1);
    assert_eq!(json["route"], route);
    assert_eq!(json["estimates"]["acceptance"], 1.0);
    assert_eq!(json["estimates"]["error"], error);
    assert_eq!(json["estimates"]["conditional_error"], error);
}

fn evaluate(
    circuit: &Circuit,
    task: &str,
    base: &Path,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
    route: &str,
) -> TaskEvaluation {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let modules = parse_task(task, base, &TaskOptions::default())
        .unwrap()
        .load_modules(circuit, &parse)
        .unwrap();
    let evaluator = TaskEvaluator::new(circuit, &modules, registry, evaluation).unwrap();
    assert_eq!(evaluator.execution_route(), route);
    evaluator.sample(&mut NoEntropy).unwrap()
}

fn assert_exact_draw(draw: &TaskEvaluation, error: bool, target: Option<bool>) {
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.error, field(error));
    assert_eq!(draw.target_success, target.map(field));
    assert_eq!(draw.raw_acceptance, ExactScalar::one());
    assert_eq!(draw.raw_error, scalar(error));
    assert_eq!(draw.raw_target_success, target.map(scalar));
    assert_eq!(
        draw.target_normalization,
        target.map(|_| ExactScalar::one())
    );
    assert_eq!(draw.history_mass, ExactScalar::one());
    assert_eq!(draw.history_weight, ExactField::one());
    let one = BigRational::from_integer(1.into());
    assert_eq!(draw.noise_probability, one);
    assert_eq!(draw.noise_likelihood_ratio, one);
    assert!(draw.sampled_records.is_empty());
}

#[test]
fn original_exact_target_and_multibit_readout_decoder() {
    let files = TestFiles::new();
    let ideal = files.write("magic-target.magna", MAGIC);
    let magic_task = files.write("magic-task.json", MAGIC_TASK);
    let identical = files.write("identical.magna", MAGIC);
    let orthogonal = files.write("orthogonal.magna", ORTHOGONAL);
    let readout = files.write("readout.magna", READOUT);
    let readout_task = files.write("readout-task.json", READOUT_TASK);
    let empty = ControllerRegistry::default();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ControllerRegistry::default();
    registry
        .register("readout", "1", {
            let trace = Arc::clone(&trace);
            move |inputs: &[bool]| {
                let outputs = vec![false, inputs[0]];
                trace
                    .lock()
                    .unwrap()
                    .push((inputs.to_vec(), outputs.clone()));
                Ok(outputs)
            }
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let readout_circuit = parse_circuit(READOUT, &parse).unwrap();
    let expected_trace = (vec![true], vec![false, true]);
    let mut target_configurations = 0;
    let mut readout_configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for (causal, route) in [
            (CausalStrategy::FixedTemplate, "causal_fixed_template"),
            (CausalStrategy::ColdPrefix, "causal_cold_prefix"),
        ] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                for (path, source, error) in
                    [(&identical, MAGIC, false), (&orthogonal, ORTHOGONAL, true)]
                {
                    let file = load_file(path, Some(&magic_task), &empty, options).unwrap();
                    let report = file.run(&TaskRunOptions::default()).unwrap();
                    assert_file_report(&report, error, "linear_affine");
                    let physical = parse_circuit(source, &ParseOptions::default()).unwrap();
                    let draw = evaluate(
                        &physical,
                        MAGIC_TASK,
                        &files.0,
                        &empty,
                        options,
                        "linear_affine",
                    );
                    assert_exact_draw(&draw, error, Some(!error));
                    assert_eq!(draw.history_proposal_mass, None);
                    assert!(draw.controller_calls.is_empty());
                    assert_eq!(physical.to_text(), source);
                    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
                    assert_eq!(std::fs::read_to_string(&ideal).unwrap(), MAGIC);
                    assert_eq!(std::fs::read_to_string(&magic_task).unwrap(), MAGIC_TASK);
                    target_configurations += 1;
                }
                let file = load_file(&readout, Some(&readout_task), &registry, options).unwrap();
                assert!(trace.lock().unwrap().is_empty());
                let report = file.run(&TaskRunOptions::default()).unwrap();
                assert_file_report(&report, true, route);
                assert_eq!(
                    trace.lock().unwrap().as_slice(),
                    std::slice::from_ref(&expected_trace)
                );
                let draw = evaluate(
                    &readout_circuit,
                    READOUT_TASK,
                    &files.0,
                    &registry,
                    options,
                    route,
                );
                assert_exact_draw(&draw, true, None);
                assert_eq!(
                    draw.history_proposal_mass,
                    (sampler == HistorySampler::AffineSupport).then(ExactScalar::one)
                );
                assert_eq!(
                    draw.controller_calls,
                    [ControllerCall {
                        name: "readout".to_owned(),
                        call: 0,
                        inputs: vec![true],
                        outputs: vec![false, true],
                    }]
                );
                let observed = std::mem::take(&mut *trace.lock().unwrap());
                assert_eq!(observed, [expected_trace.clone(), expected_trace.clone()]);
                assert_eq!(readout_circuit.to_text(), READOUT);
                assert_eq!(std::fs::read_to_string(&readout).unwrap(), READOUT);
                assert_eq!(
                    std::fs::read_to_string(&readout_task).unwrap(),
                    READOUT_TASK
                );
                readout_configurations += 1;
            }
        }
    }
    assert_eq!(target_configurations, 16);
    assert_eq!(readout_configurations, 8);
}

fn input_error(error: FileApiError) -> TaskError {
    match error {
        FileApiError::Input(error) => error,
        other => panic!("expected a structured input error, got {other}"),
    }
}

#[test]
fn original_removed_effect_and_decoder_output_shape_fail_at_compile() {
    let files = TestFiles::new();
    let hadamard = files.write("hadamard.magna", "H 0");
    let measure = files.write("measure.magna", "M 0");
    let invalid = files.write("uninitialized.magna", UNINITIALIZED);
    let effect = files.write(
        "error-effect.json",
        r#"{"error_effect":{"constant":["1/3","1/4"]}}"#,
    );
    let decoded = files.write("decoded-error.json", r#"{"decoded_error":{"controller":"readout","inputs":[{"records":[0]}],"num_outputs":1,"output":1}}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    registry
        .register("readout", "1", {
            let calls = Arc::clone(&calls);
            move |_: &[bool]| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(vec![false])
            }
        })
        .unwrap();
    let options = EvaluationOptions::default();
    let error = input_error(load_file(&hadamard, Some(&effect), &registry, options).unwrap_err());
    assert_eq!(error.field, "<root>");
    assert_eq!(error.message, "unknown field \"error_effect\"");
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let error = input_error(load_file(&measure, Some(&decoded), &registry, options).unwrap_err());
    assert_eq!(error.field, "decoded_error");
    assert!(error.message.starts_with("is no longer supported;"));
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let options_for_parse = ParseOptions {
        source: Some(invalid.display().to_string()),
        controllers: registry.names(),
        ..Default::default()
    };
    let error =
        parse_circuit(UNINITIALIZED, &options_for_parse).expect_err("uninitialized decoder output");
    assert_eq!(error.location.source, options_for_parse.source);
    assert_eq!(error.location.line, 4);
    assert_eq!(error.location.column, 1);
    assert!(error.location.repeat.is_empty());
    assert_eq!(error.message, "control register 1 is uninitialized");
    assert!(error.resource_limit.is_none());
    let file_error = input_error(load_file(&invalid, None, &registry, options).unwrap_err());
    assert_eq!(file_error.field, "circuit");
    assert_eq!(file_error.message, error.to_string());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(std::fs::read_to_string(&invalid).unwrap(), UNINITIALIZED);
}
