// SPDX-License-Identifier: MPL-2.0
// Complete original gate-matrix, sparse-wire and wide-parity FileApi methods.
// Source: itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py. Matrix references use independent exact arithmetic.

use num_rational::BigRational;
use rmagna::circuit::{BitAtom, BitExpr, Operation, ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{
    FileApiError, FileEnumerationOptions, FileLoadOptions, FileRunReport, FileTask, FileTaskOptions,
};
use rmagna::noise::{NoiseSampling, RandomSource};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Deserialize)]
struct MatrixCase {
    gate: String,
    qubits: usize,
    circuit: String,
    paulis: Vec<[usize; 2]>,
    exact: Vec<[[String; 2]; 4]>,
    numpy: Vec<f64>,
}

#[derive(Deserialize)]
struct MatrixFixture {
    schema: String,
    upstream_commit: String,
    source_sha256: String,
    cases: Vec<MatrixCase>,
}

fn matrices() -> MatrixFixture {
    let fixture: MatrixFixture =
        serde_json::from_str(include_str!("data/original-file-matrices.json")).unwrap();
    assert_eq!(fixture.schema, "rstim.original-file-matrices.v1");
    assert_eq!(
        fixture.upstream_commit,
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        fixture.source_sha256,
        "03e38048973e84253d6df3f90e8672bb4ae1f07f7f5b0168c3057530e0256e30"
    );
    assert_eq!(fixture.cases.len(), 28);
    assert_eq!(
        fixture
            .cases
            .iter()
            .map(|case| case.paulis.len())
            .sum::<usize>(),
        312
    );
    fixture
}

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-matrices-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    fn load(&self, source: &str, task: &str, evaluation: EvaluationOptions) -> FileTask {
        let physical = self.write("physical.magna", source);
        let specification = self.write("task.json", task);
        FileTask::load(
            &physical,
            Some(&specification),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions {
                evaluation,
                ..Default::default()
            },
            &FileLoadOptions::default(),
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

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("the original exact circuit must not consume entropy")
    }
}

fn configurations() -> Vec<EvaluationOptions> {
    let mut configurations = Vec::new();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                configurations.push(options);
            }
        }
    }
    assert_eq!(configurations.len(), 8);
    configurations
}

fn fraction(numerator: i64, denominator: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(numerator.into(), denominator.into()))
        .unwrap()
}

fn complete(report: &FileRunReport, expected: &[ExactField], options: EvaluationOptions) -> Value {
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Complete);
    assert_eq!((raw.completed, raw.failed, raw.not_attempted), (1, 0, 0));
    assert!(raw.failures.is_empty());
    let statistics = raw.statistics.as_ref().unwrap();
    assert_eq!(statistics.accepted, ExactField::one());
    assert_eq!(statistics.error, None);
    assert_eq!(statistics.conditional_error, None);
    assert_eq!(statistics.moments, expected);
    let value = report.to_json().unwrap();
    assert_eq!(value["status"], "complete");
    assert_eq!(value["estimates"]["acceptance"].as_f64(), Some(1.0));
    assert!(value["estimates"]["error"].is_null());
    assert!(value["estimates"]["conditional_error"].is_null());
    assert_eq!(value["route"], "linear_affine");
    assert_eq!(
        value["compilation"]["compiler_diagnostics"]["strategy"],
        format!("{:?}", options.runtime.compile.compiler_strategy)
    );
    let sampler = match options.runtime.history_sampler {
        HistorySampler::ExactMarginal => "exact_marginal",
        HistorySampler::AffineSupport => "affine_support",
    };
    assert_eq!(value["history_sampler"], sampler);
    value
}

fn check_matrix(case: &MatrixCase) {
    let files = Files::new();
    let task = json!({"observables":case.paulis}).to_string();
    let expected: Vec<_> = case
        .exact
        .iter()
        .map(|coefficients| {
            FieldArithmetic::default()
                .value(std::array::from_fn(|j| {
                    BigRational::new(
                        coefficients[j][0].parse().unwrap(),
                        coefficients[j][1].parse().unwrap(),
                    )
                }))
                .unwrap()
        })
        .collect();
    assert_eq!(expected.len(), case.paulis.len());
    assert_eq!(case.numpy.len(), expected.len());
    let circuit = parse_circuit(&case.circuit, &ParseOptions::default()).unwrap();
    assert_eq!(circuit.num_qubits(), case.qubits);
    let operations = circuit.operations().to_vec();
    let modules = parse_task(&task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    let registry = ControllerRegistry::default();
    let mut run = TaskRunOptions::default();
    run.statistics.max_dimensions = 65;
    for options in configurations() {
        let file = files.load(&case.circuit, &task, options);
        if expected.len() == 63 {
            statistics_limit(file.fingerprints(&TaskRunOptions::default()).unwrap_err());
            statistics_limit(file.inspect(&TaskRunOptions::default()).unwrap_err());
            statistics_limit(file.run(&TaskRunOptions::default()).unwrap_err());
            let no_work = TaskRunOptions {
                execution_limits: ExecutionLimits {
                    max_queries: 0,
                    max_duration: Some(std::time::Duration::ZERO),
                },
                ..Default::default()
            };
            statistics_limit(file.run(&no_work).unwrap_err());
        }
        let fingerprints = file.fingerprints(&run).unwrap();
        let inspection = file.inspect(&run).unwrap();
        assert_eq!(
            inspection["options"]["run"]["statistics"]["max_dimensions"],
            65
        );
        assert_eq!(inspection["evaluation"]["moment_count"], expected.len());
        let report = file.run(&run).unwrap();
        let value = complete(&report, &expected, options);
        assert_eq!(value["fingerprints"], fingerprints.to_json());
        let moments = value["estimates"]["moments"].as_array().unwrap();
        assert_eq!(moments.len(), expected.len());
        for (moment, numpy) in moments.iter().zip(&case.numpy) {
            let actual = moment.as_f64().unwrap();
            assert!(
                (actual - numpy).abs() <= 2e-7 + 1e-7 * numpy.abs(),
                "{}: exported {actual}, original NumPy {numpy}",
                case.gate
            );
        }
        if expected.len() == 63 {
            let compiled = file.compile().unwrap();
            statistics_limit(compiled.inspect(&TaskRunOptions::default()).unwrap_err());
            statistics_limit(compiled.run(&TaskRunOptions::default()).unwrap_err());
            assert_eq!(
                compiled.inspect(&run).unwrap()["fingerprints"],
                fingerprints.to_json()
            );
            let reused = compiled.run(&run).unwrap();
            complete(&reused, &expected, options);
            assert_eq!(report.raw_result().execution_costs.compilation.attempted, 1);
            assert_eq!(reused.raw_result().execution_costs.compilation.attempted, 0);
            assert_eq!(
                report.raw_result().execution_costs.query.attempted,
                reused.raw_result().execution_costs.query.attempted
            );
            assert_eq!(
                reused.to_json().unwrap()["fingerprints"],
                fingerprints.to_json()
            );
        }
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
        assert_eq!(
            evaluator.compilation().strategy,
            options.runtime.compile.compiler_strategy
        );
        let draw = evaluator.sample(&mut NoEntropy).unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.raw_acceptance, ExactScalar::one());
        assert_eq!(draw.moments, expected);
        let raw: Vec<_> = draw
            .raw_moments
            .iter()
            .map(|v| FieldArithmetic::default().from_scalar(v).unwrap())
            .collect();
        assert_eq!(raw, expected);
        assert_eq!(draw.target_success, None);
        assert_eq!(draw.history_mass, ExactScalar::one());
        assert_eq!(draw.history_weight, ExactField::one());
        assert_eq!(draw.noise_probability, BigRational::from_integer(1.into()));
        assert_eq!(
            draw.noise_likelihood_ratio,
            BigRational::from_integer(1.into())
        );
        assert!(draw.controller_calls.is_empty());
        assert!(draw.sampled_records.is_empty());
        assert_eq!(circuit.operations(), operations);
        assert_eq!(circuit.to_text(), case.circuit);
        assert_eq!(
            std::fs::read_to_string(files.0.join("physical.magna")).unwrap(),
            case.circuit
        );
        assert_eq!(
            std::fs::read_to_string(files.0.join("task.json")).unwrap(),
            task
        );
    }
}

fn statistics_limit(error: FileApiError) {
    let FileApiError::Input(error) = error else {
        panic!("expected a run statistics validation error")
    };
    assert_eq!(error.field, "run.statistics");
    assert!(error.message.contains("65"));
    assert!(error.message.contains("64"));
}

#[test]
fn original_supported_clifford_gates_against_stim() {
    let fixture = matrices();
    let expected: Vec<_> = "I X Y Z H S S_DAG SQRT_X SQRT_X_DAG SQRT_Y SQRT_Y_DAG H_XY H_YZ C_XYZ C_ZYX CX CY CZ SWAP ISWAP ISWAP_DAG".split_whitespace().collect();
    assert_eq!(
        fixture.cases[..21]
            .iter()
            .map(|case| case.gate.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    for case in &fixture.cases[..21] {
        check_matrix(case);
    }
}

#[test]
fn original_magic_and_controlled_extensions_against_independent_matrices() {
    let fixture = matrices();
    assert_eq!(
        fixture.cases[21..]
            .iter()
            .map(|case| case.gate.as_str())
            .collect::<Vec<_>>(),
        ["T", "T_DAG", "CS", "CS_DAG", "CH", "CCX", "CCZ"]
    );
    for case in &fixture.cases[21..] {
        check_matrix(case);
    }
}

fn exact_json(value: &Value, expected: &ExactField) {
    let coefficients: Vec<_> = expected.coefficients().iter().map(|coefficient|
        json!({"numerator":coefficient.numer().to_string(),"denominator":coefficient.denom().to_string()})
    ).collect();
    assert_eq!(value["exact"]["coefficients"], json!(coefficients));
}

#[test]
fn original_untouched_qubit_indices_add_no_wires() {
    const SPARSE: &str =
        "H 3\nT 3\nX_ERROR(1/4) 3\nH 5\nM 5\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n";
    const COMPACT: &str =
        "H 0\nT 0\nX_ERROR(1/4) 0\nH 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n";
    const SPARSE_RECOVERY: &str = "H 4\nM 4\nCZ rec[-1] 0\n";
    const COMPACT_RECOVERY: &str = "H 1\nM 1\nCZ rec[-1] 0\n";
    let files = Files::new();
    files.write("target.magna", "H 0\nT 0\n");
    files.write("sparse-recovery.magna", SPARSE_RECOVERY);
    files.write("compact-recovery.magna", COMPACT_RECOVERY);
    let sparse_task =
        r#"{"outputs":[3],"ideal_state":"target.magna","recovery":"sparse-recovery.magna"}"#;
    let compact_task =
        r#"{"outputs":[0],"ideal_state":"target.magna","recovery":"compact-recovery.magna"}"#;
    let mut enumeration = FileEnumerationOptions::default();
    enumeration.enumeration.noise.max_weight = 1;
    for options in configurations() {
        let sparse = files
            .load(SPARSE, sparse_task, options)
            .enumerate(&enumeration)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(files.0.join("physical.magna")).unwrap(),
            SPARSE
        );
        let compact = files
            .load(COMPACT, compact_task, options)
            .enumerate(&enumeration)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(files.0.join("physical.magna")).unwrap(),
            COMPACT
        );
        for report in [&sparse, &compact] {
            let raw = report.raw_result().unwrap();
            assert_eq!(raw.masses.accepted, fraction(1, 2));
            assert_eq!(raw.masses.error, fraction(1, 4));
            assert_eq!(raw.masses.target_success, Some(fraction(1, 4)));
            assert_eq!(raw.omitted_probability.to_string(), "0");
            let value = report.to_json().unwrap();
            assert_eq!(value["status"], "complete");
            exact_json(&value["result"]["masses"]["accepted"], &fraction(1, 2));
            exact_json(&value["result"]["masses"]["error"], &fraction(1, 4));
            exact_json(
                &value["result"]["masses"]["target_success"],
                &fraction(1, 4),
            );
            assert_eq!(
                value["compilation"]["compiler_diagnostics"]["strategy"],
                format!("{:?}", options.runtime.compile.compiler_strategy)
            );
        }
        assert_eq!(
            sparse.raw_result().unwrap().masses,
            compact.raw_result().unwrap().masses
        );
        let a = sparse.to_json().unwrap();
        let b = compact.to_json().unwrap();
        println!(
            "original sparse/compact coherent widths: {}/{}",
            a["compilation"]["compiler_diagnostics"]["coherent_width"],
            b["compilation"]["compiler_diagnostics"]["coherent_width"]
        );
        // Width/resource optimization remains tracked by #1080.
    }
    assert_eq!(
        std::fs::read_to_string(files.0.join("target.magna")).unwrap(),
        "H 0\nT 0\n"
    );
    assert_eq!(
        std::fs::read_to_string(files.0.join("sparse-recovery.magna")).unwrap(),
        SPARSE_RECOVERY
    );
    assert_eq!(
        std::fs::read_to_string(files.0.join("compact-recovery.magna")).unwrap(),
        COMPACT_RECOVERY
    );
}

#[test]
fn original_wide_parity_has_balanced_lowering_and_owned_task() {
    let source = "M 0\n".repeat(1200)
        + "CONTROL[XOR] "
        + &(1..=1200)
            .map(|j| format!("rec[-{j}]"))
            .collect::<Vec<_>>()
            .join(" ")
        + " -> ctrl[0]\nCX ctrl[0] 1\n";
    let circuit = parse_circuit(&source, &ParseOptions::default()).unwrap();
    let operations = circuit.operations().to_vec();
    let controls: Vec<_> = circuit
        .operations()
        .iter()
        .filter_map(|located| {
            if let Operation::Control {
                name,
                inputs,
                outputs,
                ..
            } = &located.operation
            {
                Some((name, inputs, outputs))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(controls.len(), 1);
    let (name, inputs, outputs) = controls[0];
    assert_eq!(name, "XOR");
    assert_eq!(inputs.len(), 1200);
    for (j, expression) in inputs.iter().enumerate() {
        assert_eq!(expression, &BitExpr::atom(BitAtom::Record(1199 - j)));
    }
    assert_eq!(outputs.len(), 1);
    assert!(!outputs[0].expression.constant);
    assert_eq!(
        outputs[0].expression.terms,
        (0..1200).map(BitAtom::Record).collect::<BTreeSet<_>>()
    );
    let wide_task = r#"{"outputs":[1],"observables":[[0,1]]}"#;
    let wide_files = Files::new();
    let moment_files = Files::new();
    let modules = parse_task(wide_task, &wide_files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    let registry = ControllerRegistry::default();
    for options in configurations() {
        let file = wide_files.load(&source, wide_task, options);
        complete(
            &file.run(&TaskRunOptions::default()).unwrap(),
            &[ExactField::one()],
            options,
        );
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
        let draw = evaluator.sample(&mut NoEntropy).unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.moments, [ExactField::one()]);
        assert!(draw.controller_calls.is_empty());
        assert_eq!(circuit.operations(), operations);
        assert_eq!(circuit.to_text(), source);
        let mut caller = json!({"observables":[[0,1]]});
        let original = caller.to_string();
        let loaded = moment_files.load(
            "M 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n",
            &original,
            options,
        );
        let before = loaded.fingerprints(&TaskRunOptions::default()).unwrap();
        let compiled = loaded.compile().unwrap();
        caller["observables"][0]
            .as_array_mut()
            .unwrap()
            .push(json!(999));
        assert!(
            parse_task(
                &caller.to_string(),
                &moment_files.0,
                &TaskOptions::default()
            )
            .is_err()
        );
        assert_eq!(
            loaded.fingerprints(&TaskRunOptions::default()).unwrap(),
            before
        );
        assert_eq!(
            compiled.inspect(&TaskRunOptions::default()).unwrap()["fingerprints"],
            before.to_json()
        );
        let report = compiled.run(&TaskRunOptions::default()).unwrap();
        let value = complete(&report, &[ExactField::one()], options);
        assert_eq!(value["fingerprints"], before.to_json());
        assert_eq!(value["evaluation"]["moment_count"], 1);
        assert_eq!(
            std::fs::read_to_string(moment_files.0.join("task.json")).unwrap(),
            original
        );
    }
}
