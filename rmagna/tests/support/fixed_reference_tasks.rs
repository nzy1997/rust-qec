// SPDX-License-Identifier: MPL-2.0
//! Complete original reference-network task fixtures and exact finite oracles.
use super::preparations;
use rmagna::{
    circuit::{ParseOptions, parse_circuit},
    field::{ExactField, FieldArithmetic},
    instrument::QueryPreparation,
    noise::SplitMix64,
    runtime::{
        CausalProgram, CausalStrategy, ControllerRegistry, HistoryEnumerationOptions,
        HistorySampler,
    },
    scalar::{ExactScalar, ScalarArithmetic},
    task::{EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task},
    unitary::CompilerStrategy,
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
const COMPILERS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
const SAMPLERS: [HistorySampler; 2] =
    [HistorySampler::AffineSupport, HistorySampler::ExactMarginal];
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-reference-source-followup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("zero.magna"), "I 0\n").unwrap();
        std::fs::write(path.join("magic.magna"), "H 0\nT 0\n").unwrap();
        Self(path)
    }
    fn evaluator(
        &self,
        source: &str,
        task: serde_json::Value,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
    ) -> TaskEvaluator {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let physical = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&physical, &parse)
            .unwrap();
        TaskEvaluator::new(&physical, &modules, registry, options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(compiler: CompilerStrategy, sampler: HistorySampler) -> EvaluationOptions {
    let mut options = EvaluationOptions::default();
    options.runtime.compile.compiler_strategy = compiler;
    options.runtime.history_sampler = sampler;
    options.runtime.sampling.max_terms = 256;
    let mode = QueryPreparation::Native(Default::default());
    options.runtime.compile.probability_preparation = mode;
    options.runtime.compile.density_preparation = mode;
    options.runtime.compile.moment_preparation = mode;
    options
}
fn field(value: &ExactScalar) -> ExactField {
    FieldArithmetic::default().from_scalar(value).unwrap()
}
fn quarter() -> ExactField {
    field(&ScalarArithmetic::default().omega(0, -4).unwrap())
}
#[test]
fn exact_source_nonzero_constrained_domains_reuse_fixed_factories_and_preserve_biased_weights() {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let minus = a
        .scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
        .unwrap();
    let mut draws = 0;
    for compiler in COMPILERS {
        for sampler in SAMPLERS {
            let calls = Arc::new(Mutex::new(vec![]));
            let seen = Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", move |b: &[bool]| {
                    seen.lock().unwrap().push(b.to_vec());
                    Ok(b.to_vec())
                })
                .unwrap();
            let evaluator=files.evaluator("H 0\nT 0\nMX 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCZ ctrl[0] 0\nMX 0",json!({"outputs":[0],"sample_records":[0,1],"ideal_state":"magic.magna"}),&registry,options(compiler,sampler));
            let prep = prepare(&evaluator);
            assert!(prep.all_families_retained());
            assert!(calls.lock().unwrap().is_empty());
            let before = evaluator.query_work_statistics();
            let mut found = std::collections::BTreeSet::new();
            for seed in 0..32 {
                calls.lock().unwrap().clear();
                let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
                let first = draw.sampled_records[0];
                assert!(!draw.sampled_records[1]);
                assert_eq!(*calls.lock().unwrap(), vec![vec![first]]);
                let p = if first {
                    minus.clone()
                } else {
                    a.subtract(&ExactScalar::one(), &minus).unwrap()
                };
                assert_eq!(draw.history_mass, p);
                assert_eq!(draw.raw_acceptance, p);
                let weight = if sampler == HistorySampler::AffineSupport {
                    a.multiply(&p, &a.omega(0, 2).unwrap()).unwrap()
                } else {
                    ExactScalar::one()
                };
                assert_eq!(draw.accepted, field(&weight));
                assert_eq!(draw.history_weight, field(&weight));
                assert_eq!(
                    draw.history_proposal_mass,
                    if sampler == HistorySampler::AffineSupport {
                        Some(a.omega(0, -2).unwrap())
                    } else {
                        None
                    }
                );
                assert_eq!(
                    draw.error,
                    f.multiply(&field(&weight), &field(&minus)).unwrap()
                );
                assert_eq!(draw.raw_error, a.multiply(&p, &minus).unwrap());
                assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                assert_eq!(draw.network_compilations, 0);
                found.insert(first);
                draws += 1;
            }
            assert_eq!(found.len(), 2);
            assert_eq!(
                preparations(evaluator.query_work_statistics().source.unwrap()),
                preparations(before.source.unwrap())
            );
            assert_eq!(evaluator.prepared_density_reuses(), Some(32));
        }
    }
    assert_eq!(draws, 128);
    println!(
        "128 original nonzero-domain draws: both compilers/samplers, exact biased physical and proposal masses, both actual callback RHS values, zero hot observed construction and32targetreuses per configuration"
    );
}
#[test]
fn original_three_bit_selection_has_literal_joint_masses_and_exact_three_quarter_one_quarter_totals()
 {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let mut draws = 0;
    let source = "H 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCX ctrl[0] 2\nM 2\nDETECTOR rec[-1]\nPOSTSELECT det[2]";
    for compiler in COMPILERS {
        for sampler in SAMPLERS {
            let calls = Arc::new(Mutex::new(vec![]));
            let seen = Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("and", "1", move |b: &[bool]| {
                    seen.lock().unwrap().push(b.to_vec());
                    Ok(vec![b[0] & b[1]])
                })
                .unwrap();
            let e = files.evaluator(
                source,
                json!({"outputs":[0],"sample_records":[0,1],"ideal_state":"zero.magna"}),
                &registry,
                options(compiler, sampler),
            );
            assert!(prepare(&e).all_families_retained());
            assert!(calls.lock().unwrap().is_empty());
            let before = e.query_work_statistics();
            let mut random = SplitMix64::new(20260928);
            let mut found = std::collections::BTreeSet::new();
            let mut rejected = 0;
            for _ in 0..128 {
                calls.lock().unwrap().clear();
                let draw = e.sample(&mut random).unwrap();
                let word = &draw.sampled_records;
                assert_eq!(word.len(), 2);
                let accepted = !(word[0] & word[1]);
                let error = word[0] & accepted;
                assert_eq!(*calls.lock().unwrap(), vec![word.clone()]);
                assert_eq!(
                    draw.accepted,
                    if accepted {
                        ExactField::one()
                    } else {
                        ExactField::zero()
                    }
                );
                assert_eq!(
                    draw.error,
                    if error {
                        ExactField::one()
                    } else {
                        ExactField::zero()
                    }
                );
                assert_eq!(draw.history_mass, a.omega(0, -4).unwrap());
                assert_eq!(draw.network_compilations, 0);
                found.insert(word.clone());
                rejected += usize::from(!accepted);
                draws += 1;
            }
            assert_eq!(found.len(), 4);
            assert!(rejected > 0);
            assert_eq!(
                preparations(e.query_work_statistics().source.unwrap()),
                preparations(before.source.unwrap())
            );
            assert_eq!(e.prepared_density_reuses(), Some(128));
            let exact = files.evaluator(
                source,
                json!({"outputs":[0],"ideal_state":"zero.magna"}),
                &registry,
                options(compiler, sampler),
            );
            let mut enumeration = TaskEnumerationOptions::default();
            enumeration.noise.max_weight = 0;
            let result = exact.enumerate_physical(&enumeration).unwrap();
            assert_eq!(
                result.masses.accepted,
                f.add(&quarter(), &f.add(&quarter(), &quarter()).unwrap())
                    .unwrap()
            );
            assert_eq!(result.masses.error, quarter());
            assert_eq!(
                result.masses.target_success,
                Some(field(&a.omega(0, -2).unwrap()))
            );
            assert_eq!(
                f.divide(
                    result.masses.target_success.as_ref().unwrap(),
                    &result.masses.accepted
                )
                .unwrap(),
                f.from_rational(num_rational::BigRational::new(2.into(), 3.into()))
                    .unwrap()
            );
            assert_eq!(result.histories, 4);
        }
    }
    assert_eq!(draws, 512);
    println!(
        "512 original three-bit selection draws and4exact enumerations: all input pairs, actual AND calls, rejected histories retained, literal A3/4 E1/4 and zero hot construction"
    );
}
#[test]
fn twelve_redundant_decoded_labels_enumerate_two_histories_under_one_independent_bit() {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let mut source = "H 0\n".to_owned();
    for j in 0..12 {
        source +=
            &format!("M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[{j}]\nCX ctrl[{j}] 1\n");
    }
    let mut cases = 0;
    for compiler in COMPILERS {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let calls = Arc::new(AtomicUsize::new(0));
            let seen = Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", move |b: &[bool]| {
                    seen.fetch_add(1, Ordering::Relaxed);
                    Ok(b.to_vec())
                })
                .unwrap();
            let mut configured = options(compiler, HistorySampler::ExactMarginal);
            configured.runtime.strategy = strategy;
            let e = files.evaluator(
                &source,
                json!({"outputs":[1],"ideal_state":"zero.magna"}),
                &registry,
                configured,
            );
            let mut enumeration = TaskEnumerationOptions::default();
            enumeration.noise.max_weight = 0;
            enumeration.histories.max_branches = 2;
            enumeration.histories.max_record_bits = 1;
            let result = e.enumerate_physical(&enumeration).unwrap();
            assert_eq!(result.histories, 2);
            assert_eq!(result.masses.accepted, ExactField::one());
            assert_eq!(result.masses.error, ExactField::zero());
            let parse = ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            };
            let circuit = parse_circuit(&source, &parse).unwrap();
            let program = CausalProgram::new(&circuit, &registry, configured.runtime, &a).unwrap();
            let histories = program
                .enumerate_prescribed_histories(&[], &[], enumeration.histories, &a)
                .unwrap();
            assert_eq!(histories.histories.len(), 2);
            for history in &histories.histories {
                assert_eq!(history.history_mass(), &a.omega(0, -2).unwrap());
                assert_eq!(history.controller_calls().len(), 12);
                let all_records = (0..12)
                    .map(|j| rmagna::circuit::BitExpr::atom(rmagna::circuit::BitAtom::Record(j)))
                    .collect::<Vec<_>>();
                let support = history
                    .instrument()
                    .reported_support(&all_records, history.faults(), &a)
                    .unwrap();
                assert_eq!((support.width(), support.dimension()), (12, 1));
                let bit = history.controller_calls()[0].inputs[0];
                for (j, call) in history.controller_calls().iter().enumerate() {
                    assert_eq!(call.call, j);
                    assert_eq!(call.inputs, vec![bit]);
                    assert_eq!(call.outputs, vec![bit]);
                }
            }
            for limits in [
                HistoryEnumerationOptions {
                    max_branches: 1,
                    ..enumeration.histories
                },
                HistoryEnumerationOptions {
                    max_record_bits: 0,
                    ..enumeration.histories
                },
            ] {
                calls.store(0, Ordering::Relaxed);
                assert!(
                    program
                        .enumerate_prescribed_histories(&[], &[], limits, &a)
                        .is_err()
                );
                assert_eq!(calls.load(Ordering::Relaxed), 0);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 4);
    println!(
        "4 original twelve-label compiler/causal configurations: one free bit/two complete histories, all twelve actual logical callback inputs and exact A1 E0; branch-one and rank-zero refusals precede callbacks"
    );
}
#[test]
fn original_affine_tasks_have_only_terminal_roles_and_batch_target_reuse_preserves_the_word_stream()
{
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let mut draws = 0;
    for compiler in COMPILERS {
        for sampler in SAMPLERS {
            let registry = ControllerRegistry::default();
            let two = files.evaluator(
                "H 0\nT 0\nH 1\nM 1\nCX rec[-1] 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
                json!({"outputs":[0],"ideal_state":"magic.magna"}),
                &registry,
                options(compiler, sampler),
            );
            assert_eq!(two.execution_route(), "linear_affine");
            let prep = prepare(&two);
            assert!(prep.all_families_retained());
            assert_eq!(prep.counts.planned, 2);
            assert!(!prep.by_kind.contains_key("reported_support"));
            assert!(!prep.by_kind.contains_key("reported_probability"));
            for seed in 0..4 {
                let draw = two.sample(&mut SplitMix64::new(seed)).unwrap();
                assert_eq!(draw.raw_acceptance, a.omega(0, -2).unwrap());
                assert!(draw.raw_error.is_zero());
                assert!(draw.controller_calls.is_empty());
                draws += 1;
            }
            let e = files.evaluator(
                "H 0\nT 0\nZ_ERROR(1/4) 0",
                json!({"outputs":[0],"ideal_state":"magic.magna"}),
                &registry,
                options(compiler, sampler),
            );
            let prep = prepare(&e);
            assert_eq!(prep.counts.planned, 2);
            assert!(!prep.by_kind.contains_key("reported_support"));
            let before = e.query_work_statistics();
            let mut batched = SplitMix64::new(9);
            let rows = e
                .samples(&mut batched, 16)
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            let mut single = SplitMix64::new(9);
            let mut errors = std::collections::BTreeSet::new();
            for expected in rows {
                let actual = e.sample(&mut single).unwrap();
                assert!(actual == expected);
                assert_eq!(actual.raw_acceptance, ExactScalar::one());
                assert_eq!(actual.accepted, ExactField::one());
                assert_eq!(actual.error, field(&actual.raw_error));
                assert_eq!(actual.target_normalization, Some(ExactScalar::one()));
                assert_eq!(
                    actual.raw_target_success,
                    Some(a.subtract(&ExactScalar::one(), &actual.raw_error).unwrap())
                );
                assert_eq!(actual.history_queries, 0);
                assert!(actual.controller_calls.is_empty());
                assert!(actual.sampled_records.is_empty());
                assert!(
                    actual.raw_error == ExactScalar::zero()
                        || actual.raw_error == ExactScalar::one()
                );
                assert_eq!(actual.history_mass, ExactScalar::one());
                assert_eq!(actual.history_weight, ExactField::one());
                let numerator = if actual.raw_error.is_zero() { 3 } else { 1 };
                assert_eq!(
                    actual.noise_probability,
                    num_rational::BigRational::new(numerator.into(), 4.into())
                );
                errors.insert(actual.raw_error.is_zero());
                draws += 2;
            }
            assert_eq!(single.state(), batched.state());
            assert_eq!(errors.len(), 2);
            assert_eq!(e.prepared_density_reuses(), Some(32));
            assert_eq!(
                preparations(e.query_work_statistics().source.unwrap()),
                preparations(before.source.unwrap())
            );
        }
    }
    assert_eq!(draws, 144);
    println!(
        "144 original affine-task draws: two terminal request roles/no sampling parent, magic target and postselection,16batch/16single exact fields and caller word state,32actual target reuses per configuration"
    );
}

fn prepare(e: &TaskEvaluator) -> rmagna::task::SamplingPreparationReport {
    let report = e.prepare_sampling_queries(
        Default::default(),
        &rmagna::costs::ExecutionMeter::default(),
    );
    assert!(report.result.is_ok(), "{:?}", report.result);
    report
}

#[test]
fn original_parity_feedback_uses_all_eight_manual_report_offsets_without_reconstructing_query_families()
 {
    use rmagna::task::{SuppliedActionLimits, SuppliedNoiseActions};
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let mut draws = 0;
    // The original input has no physical readout channels. Use the existing
    // supplied-action contract; do not add probability-1/2 channels to it.
    let source = "X 0\nH 1 2\nM 0 1\nM 1\nCONTROL[XOR] rec[-3] rec[-2] -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[echo] ctrl[0] det[-1] -> ctrl[1] ctrl[2]\nCS ctrl[1] 2\nCX ctrl[2] 4\nMX 2\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[3]\nCX ctrl[3] 3\nDETECTOR rec[-4] rec[-2]\nPOSTSELECT det[2]";
    for compiler in COMPILERS {
        for sampler in SAMPLERS {
            let calls = Arc::new(Mutex::new(vec![]));
            let seen = Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", move |b: &[bool]| {
                    seen.lock().unwrap().push(b.to_vec());
                    Ok(b.to_vec())
                })
                .unwrap();
            let parse = ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            };
            let physical = parse_circuit(source, &parse).unwrap();
            let modules = parse_task(
                r#"{"outputs":[3],"ideal_state":"zero.magna"}"#,
                &files.0,
                &TaskOptions::default(),
            )
            .unwrap()
            .load_modules(&physical, &parse)
            .unwrap();
            // A separately declared readout law supplies the same quantum
            // offsets through one owner across all eight assignments. Its
            // physical probability is explicitly 1/8 rather than the manual
            // zero-noise probability checked below.
            let channel_source = source
                .replace("M 0 1\nM 1", "M(1/2) 0\nM 1\nM(1/2) 1")
                .replace("MX 2", "MX(1/2) 2");
            let channel_physical = parse_circuit(&channel_source, &parse).unwrap();
            // Every category in this fixture is one measurement's readout
            // flip, including the unchanged zero-probability second record.
            // FaultSelection uses the category index, not LocatedOperation::id.
            let sites = channel_physical
                .operations()
                .iter()
                .filter_map(|op| match op.operation {
                    rmagna::circuit::Operation::Measure { record, .. } => Some(record),
                    _ => None,
                })
                .enumerate()
                .filter_map(|(site, record)| [0, 2, 3].contains(&record).then_some((record, site)))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(sites.len(), 3);
            let channelled = files.evaluator(
                &channel_source,
                json!({"outputs":[3],"ideal_state":"zero.magna"}),
                &registry,
                options(compiler, sampler),
            );
            calls.lock().unwrap().clear();
            assert!(prepare(&channelled).all_families_retained());
            assert!(calls.lock().unwrap().is_empty());
            let retained_before = channelled.query_work_statistics();
            for mask in 0..8 {
                let f0 = mask & 1 != 0;
                let f2 = mask & 2 != 0;
                let f3 = mask & 4 != 0;
                let flips = [(0, f0), (2, f2), (3, f3)]
                    .into_iter()
                    .filter_map(|(r, f)| f.then_some(r))
                    .collect::<Vec<_>>();
                let supplied = TaskEvaluator::prepare_supplied_actions(
                    &physical,
                    &modules,
                    &registry,
                    options(compiler, sampler),
                    SuppliedNoiseActions {
                        paulis: &[],
                        report_flips: &flips,
                        likelihood: &num_rational::BigRational::from_integer(1.into()),
                    },
                    SuppliedActionLimits::default(),
                )
                .unwrap();
                calls.lock().unwrap().clear();
                let prep = supplied.prepare_sampling_queries(
                    Default::default(),
                    &rmagna::costs::ExecutionMeter::default(),
                );
                assert!(prep.result.is_ok(), "{:?}", prep.result);
                assert!(prep.all_families_retained());
                assert!(calls.lock().unwrap().is_empty());
                let before = supplied.query_work_statistics();
                let mut found = std::collections::BTreeSet::new();
                for seed in 0..32 {
                    calls.lock().unwrap().clear();
                    let mut manual_rng = SplitMix64::new(seed);
                    let mut row = supplied.sample(&mut manual_rng).unwrap();
                    let actual = calls.lock().unwrap().clone();
                    assert_eq!(actual.len(), 2);
                    assert_eq!(actual[0].len(), 2);
                    assert_eq!(actual[1].len(), 1);
                    let s = actual[0][0];
                    let reported_copy = actual[0][1];
                    let b = actual[1][0];
                    let input = reported_copy ^ f2;
                    assert_eq!(s, true ^ f0 ^ input);
                    if !s {
                        assert_eq!(b, f3);
                    }
                    assert_eq!(
                        row.history_mass,
                        a.omega(0, if s { -4 } else { -2 }).unwrap()
                    );
                    let accepted = (true ^ f0) == reported_copy;
                    assert_eq!(
                        row.accepted,
                        if accepted {
                            ExactField::one()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(
                        row.error,
                        if accepted && b {
                            ExactField::one()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(
                        row.raw_acceptance,
                        if accepted {
                            row.history_mass.clone()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                    assert_eq!(row.history_weight, ExactField::one());
                    assert_eq!(
                        row.noise_likelihood_ratio,
                        num_rational::BigRational::from_integer(1.into())
                    );
                    // Manual flips outside the zero-noise law have physical mass
                    // zero; conditional quantum contributions are still defined.
                    assert_eq!(
                        row.noise_probability,
                        num_rational::BigRational::from_integer(
                            (if mask == 0 { 1 } else { 0 }).into()
                        )
                    );
                    assert_eq!(row.network_compilations, 0);
                    calls.lock().unwrap().clear();
                    let faults = flips
                        .iter()
                        .map(|r| rmagna::instrument::FaultSelection {
                            site: sites[r],
                            alternative: 0,
                        })
                        .collect::<Vec<_>>();
                    let mut channel_rng = SplitMix64::new(seed);
                    let paired = channelled
                        .sample_prescribed(&faults, &mut channel_rng)
                        .unwrap();
                    assert_eq!(
                        paired.noise_probability,
                        num_rational::BigRational::new(1.into(), 8.into())
                    );
                    assert_eq!(*calls.lock().unwrap(), actual);
                    assert_eq!(manual_rng.state(), channel_rng.state());
                    row.noise_probability = paired.noise_probability.clone();
                    assert!(
                        row == paired,
                        "all conditional evaluation fields and callback/RNG streams must agree; only the declared physical noise law differs"
                    );
                    found.insert((input, b));
                    draws += 2;
                }
                let expected = (0..2)
                    .flat_map(|input| {
                        let input = input != 0;
                        let count = if true ^ f0 ^ input { 2 } else { 1 };
                        (0..count).map(move |b| (input, (b != 0) ^ f3))
                    })
                    .collect();
                assert_eq!(found, expected);
                assert_eq!(
                    preparations(supplied.query_work_statistics().source.unwrap()),
                    preparations(before.source.unwrap())
                );
            }
            assert_eq!(
                preparations(channelled.query_work_statistics().source.unwrap()),
                preparations(retained_before.source.unwrap())
            );
        }
    }
    assert_eq!(draws, 2048);
    println!(
        "2048 complete original/manual and persistent-declared parity/offset draws: both compilers and samplers, eight manual report-flip combinations, literal support/mass/acceptance/error and all actual echo inputs, zero hot query-family factories"
    );
}
