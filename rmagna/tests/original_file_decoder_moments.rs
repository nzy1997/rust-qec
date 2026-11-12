// SPDX-License-Identifier: MPL-2.0
// Complete original FileApiTests decoder/moment and XOR/postselection methods
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py. Rust RNG trajectories are independent of native.

use num_rational::BigRational;
use rmagna::circuit::{BitAtom, BitExpr, Operation, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileLoadOptions, FileRunReport, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, RandomSource, SplitMix64};
use rmagna::runtime::{
    CausalProgram, CausalStrategy, ControllerCall, ControllerRegistry, HistorySampler,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluation, TaskEvaluator, TaskOptions, TaskRunOptions,
    parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

const MIXED: &str = "H 0\nH 1\nT 1\nM 0\nDETECTOR rec[-1]\nCONTROL[choose] det[-1] -> ctrl[0] ctrl[1] ctrl[2]\nCS ctrl[0] 1\nCH ctrl[1] 1\nCY ctrl[2] 1\n";
const XOR: &str = "H 0 1 2\nM 0 1 2\nCONTROL[XOR] rec[-3] rec[-2] rec[-1] -> ctrl[0]\nCONTROL[echo] ctrl[0] -> ctrl[1]\nCX ctrl[1] 3\nDETECTOR rec[-3]\nPOSTSELECT det[0]\n";

#[derive(Clone, Copy)]
enum Case {
    Mixed,
    Xor,
}
impl Case {
    fn source(self) -> &'static str {
        match self {
            Self::Mixed => MIXED,
            Self::Xor => XOR,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Mixed => "choose",
            Self::Xor => "echo",
        }
    }
    fn task(self) -> Value {
        match self {
            // Original moments() order is Z, X, Y, including every mask.
            Self::Mixed => json!({"outputs":[1],"observables":[[0,1],[1,0],[1,1]]}),
            Self::Xor => json!({"outputs":[3],"ideal_state":"zero.magna"}),
        }
    }
    fn seeds(self) -> u64 {
        match self {
            Self::Mixed => 8,
            Self::Xor => 4,
        }
    }
    fn outputs(self, bit: bool) -> Vec<bool> {
        vec![
            bit;
            match self {
                Self::Mixed => 3,
                Self::Xor => 1,
            }
        ]
    }
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-file-decoder-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        // Upstream ZERO_TARGET is the empty vacuum circuit with inferred width.
        // The Rust target explicitly owns the same one-wire zero state.
        std::fs::write(path.join("zero.magna"), "I 0\n").unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct CountedRandom {
    stream: SplitMix64,
    fixed: Option<u64>,
    words: usize,
}
impl CountedRandom {
    fn endpoint(word: u64) -> Self {
        Self {
            stream: SplitMix64::new(0),
            fixed: Some(word),
            words: 0,
        }
    }
    fn seeded(seed: u64) -> Self {
        Self {
            stream: SplitMix64::new(seed),
            fixed: None,
            words: 0,
        }
    }
}
impl RandomSource for CountedRandom {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        assert_eq!(
            self.words, 1,
            "only the required parity may consume entropy"
        );
        self.fixed.unwrap_or_else(|| self.stream.next_u64())
    }
}

type Calls = Arc<Mutex<Vec<Vec<bool>>>>;
fn registry(case: Case) -> (ControllerRegistry, Calls) {
    let calls = Arc::new(Mutex::new(vec![]));
    let captured = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register(case.name(), "1", move |bits: &[bool]| {
            assert_eq!(bits.len(), 1);
            captured.lock().unwrap().push(bits.to_vec());
            Ok(case.outputs(bits[0]))
        })
        .unwrap();
    (registry, calls)
}
fn configurations() -> Vec<EvaluationOptions> {
    let mut result = vec![];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                result.push(options);
            }
        }
    }
    assert_eq!(result.len(), 8);
    result
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn half() -> ExactScalar {
    ScalarArithmetic::default().omega(0, -2).unwrap()
}
fn mixed_moments(bit: bool) -> Vec<ExactField> {
    let s = FieldArithmetic::default()
        .from_scalar(&ScalarArithmetic::default().omega(0, -1).unwrap())
        .unwrap();
    // Independent Bloch rotations: T|+> has (X,Y,Z)=(s,s,0).
    // S:(x,y,z)->(-y,x,z); H:(x,y,z)->(z,-y,x);
    // Y:(x,y,z)->(-x,y,-z), hence Y H S gives (0,-s,s).
    if bit {
        vec![s.clone(), ExactField::zero(), s.negated()]
    } else {
        vec![ExactField::zero(), s.clone(), s]
    }
}
fn trace(case: Case, calls: &[ControllerCall], observed: &Calls) -> bool {
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(call.name, case.name());
    assert_eq!(call.call, 0);
    assert_eq!(call.inputs.len(), 1);
    let bit = call.inputs[0];
    assert_eq!(call.outputs, case.outputs(bit));
    assert_eq!(*observed.lock().unwrap(), [vec![bit]]);
    bit
}
fn assert_draw(
    case: Case,
    draw: &TaskEvaluation,
    calls: &Calls,
    options: EvaluationOptions,
) -> bool {
    let bit = trace(case, &draw.controller_calls, calls);
    assert_eq!(draw.history_mass, half());
    assert_eq!(
        draw.history_proposal_mass,
        (options.runtime.history_sampler == HistorySampler::AffineSupport).then(half)
    );
    assert_eq!(draw.history_weight, ExactField::one());
    assert_eq!(
        draw.history_queries, 1,
        "one observation group, not all marginal contractions"
    );
    assert!(draw.sampled_records.is_empty());
    assert_eq!(draw.noise_probability, BigRational::from_integer(1.into()));
    assert_eq!(
        draw.noise_likelihood_ratio,
        BigRational::from_integer(1.into())
    );
    let f = FieldArithmetic::default();
    match case {
        Case::Mixed => {
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.error, ExactField::zero());
            assert_eq!(draw.target_success, None);
            assert_eq!(draw.target_normalization, None);
            assert_eq!(draw.raw_target_success, None);
            assert_eq!(draw.raw_error, ExactScalar::zero());
            assert_eq!(draw.raw_acceptance, half());
            assert_eq!(draw.moments, mixed_moments(bit));
            let raw: Vec<_> = draw
                .raw_moments
                .iter()
                .map(|s| f.from_scalar(s).unwrap())
                .collect();
            assert_eq!(
                raw,
                mixed_moments(bit)
                    .iter()
                    .map(|s| f.multiply(s, &field(1, 2)).unwrap())
                    .collect::<Vec<_>>()
            );
        }
        Case::Xor => {
            assert_eq!(
                draw.accepted,
                field(1, 2),
                "unobserved postselection is integrated"
            );
            assert_eq!(draw.error, field(i64::from(bit), 2));
            assert_eq!(draw.target_success, Some(field(i64::from(!bit), 2)));
            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
            assert_eq!(
                draw.raw_acceptance,
                ScalarArithmetic::default().omega(0, -4).unwrap()
            );
            assert_eq!(
                f.from_scalar(&draw.raw_error).unwrap(),
                field(i64::from(bit), 4)
            );
            assert_eq!(
                f.from_scalar(draw.raw_target_success.as_ref().unwrap())
                    .unwrap(),
                field(i64::from(!bit), 4)
            );
            assert!(draw.moments.is_empty() && draw.raw_moments.is_empty());
        }
    }
    if options.runtime.strategy == CausalStrategy::FixedTemplate {
        assert_eq!(draw.network_compilations, 0);
    } else {
        assert!(draw.network_compilations > 0);
    }
    bit
}
fn assert_report(
    case: Case,
    report: &FileRunReport,
    calls: &Calls,
    options: EvaluationOptions,
    seed: u64,
) -> bool {
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Complete);
    assert_eq!(
        (raw.requested, raw.completed, raw.failed, raw.not_attempted),
        (1, 1, 0, 0)
    );
    assert_eq!(raw.seed, seed);
    assert!(raw.failures.is_empty());
    assert!(raw.partial_statistics.is_none());
    assert_eq!(raw.execution_costs.decoder.attempted, 1);
    assert_eq!(raw.execution_costs.decoder.succeeded, 1);
    assert_eq!(raw.execution_costs.decoder.failed, 0);
    assert_eq!(raw.completed_history_groups, 1);
    assert!(
        raw.execution_costs.query.succeeded > 1,
        "group count excludes other scalar contractions"
    );
    assert!(raw.sampled_records.is_none());
    let observed = calls.lock().unwrap();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].len(), 1);
    let bit = observed[0][0];
    drop(observed);
    let stats = raw.statistics.as_ref().unwrap();
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.mean_weight, ExactField::one());
    assert_eq!(stats.effective_sample_size, ExactField::one());
    let json = report.to_json().unwrap();
    assert_eq!(json["status"], "complete");
    assert_eq!(json["seed"], seed);
    assert_eq!(json["completed_resources"]["history_groups"], 1);
    assert_eq!(json["execution_costs"]["decoder"]["succeeded"], 1);
    assert_eq!(
        json["compilation"]["compiler_diagnostics"]["strategy"],
        format!("{:?}", options.runtime.compile.compiler_strategy)
    );
    assert_eq!(
        json["history_sampler"],
        match options.runtime.history_sampler {
            HistorySampler::ExactMarginal => "exact_marginal",
            HistorySampler::AffineSupport => "affine_support",
        }
    );
    assert_eq!(
        json["route"],
        match options.runtime.strategy {
            CausalStrategy::FixedTemplate => "causal_fixed_template",
            CausalStrategy::ColdPrefix => "causal_cold_prefix",
        }
    );
    match case {
        Case::Mixed => {
            assert_eq!(stats.accepted, ExactField::one());
            assert_eq!(stats.error, None);
            assert_eq!(stats.conditional_error, None);
            assert_eq!(stats.moments, mixed_moments(bit));
            assert_eq!(json["estimates"]["acceptance"], 1.0);
            assert!(
                json["estimates"]["error"].is_null()
                    && json["estimates"]["conditional_error"].is_null()
            );
            let s = 0.5_f64.sqrt();
            let expected = if bit { [s, 0.0, -s] } else { [0.0, s, s] };
            assert_eq!(json["estimates"]["moments"].as_array().unwrap().len(), 3);
            for (actual, expected) in json["estimates"]["moments"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected)
            {
                assert!((actual.as_f64().unwrap() - expected).abs() < 1e-12);
            }
        }
        Case::Xor => {
            assert_eq!(stats.accepted, field(1, 2));
            assert_eq!(stats.error, Some(field(i64::from(bit), 2)));
            assert_eq!(stats.conditional_error, Some(field(i64::from(bit), 1)));
            assert!(stats.moments.is_empty());
            assert_eq!(
                json["estimates"],
                json!({"acceptance":0.5,"error":if bit {0.5} else {0.0},"conditional_error":if bit {1.0} else {0.0},"moments":[]})
            );
        }
    }
    bit
}

fn exercise(case: Case) {
    let files = Files::new();
    let (registry, calls) = registry(case);
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(case.source(), &parse).unwrap();
    let operations = circuit.operations().to_vec();
    let text = case.task().to_string();
    let task = parse_task(&text, &files.0, &TaskOptions::default()).unwrap();
    let modules = task.load_modules(&circuit, &parse).unwrap();
    let physical = files.write("physical.magna", case.source());
    let specification = files.write("task.json", &text);
    let scalar = ScalarArithmetic::default();
    for options in configurations() {
        calls.lock().unwrap().clear();
        let program = CausalProgram::new(&circuit, &registry, options.runtime, &scalar).unwrap();
        let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
        assert!(evaluator.sampled_history());
        let file = FileTask::load(
            &physical,
            Some(&specification),
            &registry,
            NoiseSampling::Physical,
            FileTaskOptions {
                evaluation: options,
                ..Default::default()
            },
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        let compiled = file.compile().unwrap();
        let inspected = compiled.inspect(&TaskRunOptions::default()).unwrap();
        assert_eq!(inspected["route"], evaluator.execution_route());
        assert!(
            calls.lock().unwrap().is_empty(),
            "preparation and inspection invoke no decoder"
        );
        let mut endpoints = [false; 2];
        for word in [0, u64::MAX] {
            calls.lock().unwrap().clear();
            let mut random = CountedRandom::endpoint(word);
            let history = program.resolve(&[], &mut random, &scalar).unwrap();
            let bit = trace(case, history.controller_calls(), &calls);
            assert_eq!(random.words, 1);
            assert_eq!(history.entropy_words(), (0, 1));
            assert_eq!(history.history_queries(), 1);
            assert_eq!(history.history_mass(), &half());
            assert!(history.requested_values().is_empty());
            let parity = match case {
                Case::Mixed => BitExpr::atom(BitAtom::Record(0)),
                Case::Xor => {
                    let mut p = BitExpr::default();
                    for record in 0..3 {
                        p.xor_assign(&BitExpr::atom(BitAtom::Record(record)));
                    }
                    p
                }
            };
            assert_eq!(history.conditions(), [(parity, bit)]);
            calls.lock().unwrap().clear();
            let mut random = CountedRandom::endpoint(word);
            let draw = evaluator.sample_prescribed(&[], &mut random).unwrap();
            assert_eq!(assert_draw(case, &draw, &calls, options), bit);
            assert_eq!(draw.controller_calls, history.controller_calls());
            assert_eq!(random.words, 1);
            endpoints[usize::from(bit)] = true;
        }
        assert_eq!(
            endpoints,
            [true, true],
            "endpoint streams force both decoder branches"
        );
        let mut seen = [false; 2];
        // Preserve every original seeded file call. Compare only paired Rust
        // streams; SplitMix64 does not promise native Random(seed) trajectories.
        for seed in 0..case.seeds() {
            calls.lock().unwrap().clear();
            let report = compiled
                .run(&TaskRunOptions {
                    seed,
                    ..Default::default()
                })
                .unwrap();
            let bit = assert_report(case, &report, &calls, options, seed);
            seen[usize::from(bit)] = true;
            calls.lock().unwrap().clear();
            let mut random = CountedRandom::seeded(seed);
            let draw = evaluator.sample_prescribed(&[], &mut random).unwrap();
            assert_eq!(assert_draw(case, &draw, &calls, options), bit);
            assert_eq!(random.words, 1);
        }
        assert_eq!(seen, [true, true]);
        assert_eq!(circuit.operations(), operations);
        assert_eq!(circuit.to_text(), case.source());
        assert_eq!(std::fs::read_to_string(&physical).unwrap(), case.source());
        assert_eq!(std::fs::read_to_string(&specification).unwrap(), text);
        assert_eq!(
            std::fs::read_to_string(files.0.join("zero.magna")).unwrap(),
            "I 0\n"
        );
    }
}

#[test]
fn original_nonlinear_mixed_gates_use_actual_decoder_and_global_conditional_moments() {
    exercise(Case::Mixed);
}

#[test]
fn original_only_required_xor_information_is_sampled_and_postselection_is_preserved() {
    let mut options = ParseOptions::default();
    options.controllers.insert("echo".to_string());
    let circuit = parse_circuit(XOR, &options).unwrap();
    let controls: Vec<_> = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control {
                name,
                inputs,
                outputs,
                ..
            } => Some((name, inputs, outputs)),
            _ => None,
        })
        .collect();
    assert_eq!(controls.len(), 2);
    assert_eq!(controls[0].0, "XOR");
    assert_eq!(
        controls[0].1,
        &(0..3)
            .map(|j| BitExpr::atom(BitAtom::Record(j)))
            .collect::<Vec<_>>()
    );
    let mut parity = BitExpr::default();
    for j in 0..3 {
        parity.xor_assign(&BitExpr::atom(BitAtom::Record(j)));
    }
    assert_eq!(controls[0].2[0].expression, parity);
    assert_eq!(controls[1].0, "echo");
    assert_eq!(controls[1].1, &[parity]);
    exercise(Case::Xor);
}
