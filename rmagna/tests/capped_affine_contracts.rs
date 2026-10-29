//! Complete finite recovery/guarded-noise witnesses from pinned
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_capped_affine.py. No native imports or counter ABI claim.

use num_rational::BigRational;
use rmagna::circuit::{NoiseKind, Operation, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEnumerationOptions, TaskEvaluation, TaskEvaluator,
    TaskOptions, TaskRunOptions, parse_task,
};
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
        let files = Self(std::env::temp_dir().join(format!(
            "rmagna-original-capped-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&files.0).unwrap();
        files.put("target.magna", "I 0\n");
        files
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn evaluator(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
    ) -> TaskEvaluator {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let source = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(task, &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&source, &parse)
            .unwrap();
        TaskEvaluator::new(&source, &modules, registry, options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
type Calls = Arc<Mutex<Vec<Vec<bool>>>>;
fn echo() -> (ControllerRegistry, Calls) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let saved = seen.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            saved.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    (registry, seen)
}
fn options(compiler: CompilerStrategy, causal: CausalStrategy) -> EvaluationOptions {
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.strategy = causal;
    options.runtime.compile.compiler_strategy = compiler;
    options
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
fn as_rational(x: &ExactField) -> BigRational {
    assert!(x.coefficients()[1..].iter().all(|x| *x == rational(0, 1)));
    x.coefficients()[0].clone()
}
struct Words([u64; 2], usize);
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = self.0[self.1];
        self.1 += 1;
        word
    }
}
fn one_echo(row: &TaskEvaluation, bit: bool) {
    assert_eq!(row.controller_calls.len(), 1);
    let call = &row.controller_calls[0];
    assert_eq!(call.name, "echo");
    assert_eq!(call.inputs, [bit]);
    assert_eq!(call.outputs, [bit]);
}

#[test]
fn original_terminal_recovery_keeps_one_call_half_proposals_and_zero_error() {
    let files = Files::new();
    files.put(
        "recovery.magna",
        "M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0\n",
    );
    let scalar = ScalarArithmetic::default();
    let half = scalar.omega(0, -2).unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let (registry, seen) = echo();
            let program = files.evaluator(
                "H 0\n",
                r#"{"outputs":[0],"ideal_state":"target.magna","recovery":"recovery.magna"}"#,
                &registry,
                options(compiler, causal),
            );
            let mut random = SplitMix64::new(9);
            for _ in 0..16 {
                seen.lock().unwrap().clear();
                let row = program.sample(&mut random).unwrap();
                assert_eq!(row.accepted, ExactField::one());
                assert_eq!(row.error, ExactField::zero());
                assert_eq!(row.target_success, Some(ExactField::one()));
                assert_eq!(row.target_normalization, Some(ExactScalar::one()));
                assert_eq!(row.history_mass, half);
                assert_eq!(row.history_proposal_mass, Some(half.clone()));
                assert_eq!(row.controller_calls.len(), 1);
                one_echo(&row, row.controller_calls[0].inputs[0]);
                assert_eq!(
                    *seen.lock().unwrap(),
                    vec![row.controller_calls[0].inputs.clone()]
                );
            }
            // Independent Z-measurement branches of H|0>: each has mass 1/2;
            // the controlled X sends either post-measurement state to |0>.
            let mut mass = rational(0, 1);
            for bit in [false, true] {
                seen.lock().unwrap().clear();
                let row = program
                    .sample_prescribed(&[], &mut Words([u64::from(bit), 0], 0))
                    .unwrap();
                one_echo(&row, bit);
                assert_eq!(*seen.lock().unwrap(), vec![vec![bit]]);
                assert_eq!(row.history_mass, half);
                assert_eq!(row.history_proposal_mass, Some(half.clone()));
                assert_eq!(row.accepted, field(1, 1));
                assert_eq!(row.error, field(0, 1));
                assert_eq!(row.target_success, Some(field(1, 1)));
                assert_eq!(row.target_normalization, Some(ExactScalar::one()));
                mass += rational(1, 2);
            }
            assert_eq!(mass, rational(1, 1));
        }
    }
}

const GUARDED: &str = "H 0 1\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCS ctrl[0] 1\nMX 1\nX_ERROR(1/3) rec[-1] 2\nM 2\nCX rec[-1] 3\n";
const SAMPLED_TASK: &str =
    r#"{"outputs":[3],"sample_records":[0,1,2],"ideal_state":"target.magna"}"#;
const INTEGRATED_TASK: &str = r#"{"outputs":[3],"ideal_state":"target.magna"}"#;

#[test]
fn original_guarded_noise_feedback_matches_four_exact_branches_and_seeded_batch() {
    let files = Files::new();
    // M0 is fair; without S, MX1 is zero, while S|+> has fair X outcomes.
    // The guarded X occurs with probability1/3 only in the final quarter.
    let oracle = BTreeMap::from([
        (vec![false, false, false], rational(1, 2)),
        (vec![true, false, false], rational(1, 4)),
        (vec![true, true, false], rational(1, 6)),
        (vec![true, true, true], rational(1, 12)),
    ]);
    assert_eq!(
        oracle.values().cloned().sum::<BigRational>(),
        rational(1, 1)
    );
    assert_eq!(
        rational(1, 2) * rational(1, 2) * rational(1, 3),
        rational(1, 12)
    );
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let (registry, seen) = echo();
            let configured = options(compiler, causal);
            let program = files.evaluator(GUARDED, SAMPLED_TASK, &registry, configured);
            assert_eq!(
                program.execution_route(),
                match causal {
                    CausalStrategy::FixedTemplate => "causal_fixed_template",
                    CausalStrategy::ColdPrefix => "causal_cold_prefix",
                }
            );
            let parse = ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            };
            let physical = parse_circuit(GUARDED, &parse).unwrap();
            let sites = physical
                .operations()
                .iter()
                .filter(|op| {
                    matches!(
                        &op.operation,
                        Operation::Noise {
                            kind: NoiseKind::X,
                            ..
                        }
                    )
                })
                .filter_map(|op| program.potential_site(op.id))
                .collect::<BTreeSet<_>>();
            assert_eq!(sites.len(), 1);
            let fault = FaultSelection {
                site: *sites.first().unwrap(),
                alternative: 0,
            };
            let mut found = BTreeMap::new();
            for first in 0..2 {
                for second in 0..2 {
                    seen.lock().unwrap().clear();
                    let row = program
                        .sample_prescribed(&[], &mut Words([first, second], 0))
                        .unwrap();
                    one_echo(&row, row.sampled_records[0]);
                    assert_eq!(*seen.lock().unwrap(), vec![vec![row.sampled_records[0]]]);
                    assert_eq!(row.accepted, ExactField::one());
                    assert_eq!(row.error, ExactField::zero());
                    assert_eq!(row.target_success, Some(ExactField::one()));
                    assert_eq!(row.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(row.history_proposal_mass, Some(row.history_mass.clone()));
                    let history = FieldArithmetic::default()
                        .from_scalar(&row.history_mass)
                        .unwrap();
                    let joint = as_rational(&history) * &row.noise_probability;
                    assert_eq!(oracle.get(&row.sampled_records), Some(&joint));
                    found.insert(row.sampled_records, joint);
                }
            }
            seen.lock().unwrap().clear();
            let row = program
                .sample_prescribed(&[fault], &mut Words([1, 1], 0))
                .unwrap();
            assert_eq!(row.sampled_records, [true, true, true]);
            one_echo(&row, true);
            assert_eq!(*seen.lock().unwrap(), vec![vec![true]]);
            assert_eq!(row.accepted, ExactField::one());
            assert_eq!(row.error, ExactField::one());
            assert_eq!(row.target_success, Some(ExactField::zero()));
            assert_eq!(row.target_normalization, Some(ExactScalar::one()));
            assert_eq!(row.history_proposal_mass, Some(row.history_mass.clone()));
            let history = FieldArithmetic::default()
                .from_scalar(&row.history_mass)
                .unwrap();
            found.insert(
                row.sampled_records,
                as_rational(&history) * row.noise_probability,
            );
            assert_eq!(found, oracle);

            let exact = files.evaluator(GUARDED, INTEGRATED_TASK, &registry, configured);
            let mut enumeration = TaskEnumerationOptions::default();
            enumeration.noise.max_weight = 1;
            let all = exact.enumerate_physical(&enumeration).unwrap();
            assert_eq!(all.covered_probability, Probability::one());
            assert_eq!(all.omitted_probability, Probability::zero());
            assert_eq!(all.masses.accepted, field(1, 1));
            assert_eq!(all.masses.error, field(1, 12));
            assert_eq!(all.masses.target_success, Some(field(11, 12)));

            seen.lock().unwrap().clear();
            let result = program
                .run_batch(&TaskRunOptions {
                    shots: 512,
                    seed: 100,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(
                (
                    result.status,
                    result.requested,
                    result.completed,
                    result.failed,
                    result.not_attempted,
                    result.seed
                ),
                (RunStatus::Complete, 512, 512, 0, 0, 100)
            );
            let statistics = result.statistics.as_ref().unwrap();
            assert_eq!(statistics.accepted, ExactField::one());
            assert_eq!(statistics.error, statistics.conditional_error);
            let delta = as_rational(statistics.error.as_ref().unwrap()) - rational(1, 12);
            assert!(delta >= -rational(1, 25) && delta <= rational(1, 25));
            let records = result.sampled_records.as_ref().unwrap();
            let calls = seen.lock().unwrap();
            assert_eq!((records.len(), calls.len()), (512, 512));
            for (word, inputs) in records.iter().zip(calls.iter()) {
                assert_eq!(word.len(), 3);
                assert!(oracle.contains_key(word));
                assert_eq!(inputs, &[word[0]]);
            }
            let errors = records.iter().filter(|word| word[2]).count() as i64;
            assert_eq!(statistics.error, Some(field(errors, 512)));
        }
    }
}
