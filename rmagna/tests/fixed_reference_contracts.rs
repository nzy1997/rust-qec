//! Initial R09 probes, not the complete25-method reproduction witness yet.
use rmagna::{
    circuit::{ParseOptions, parse_circuit},
    gf2::BitVector,
    instrument::{CompileOptions, compile_unitary},
    pauli::PauliWord,
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::{PauliInsertion, QueryWorkStatistics},
};
fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| value & (1 << j) != 0)).unwrap()
}
fn probability(zero: bool) -> ExactScalar {
    let sign = if zero { 1 } else { -1 };
    ScalarArithmetic::default()
        .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
        .unwrap()
}
fn preparations(s: QueryWorkStatistics) -> (u64, u64, u64) {
    assert!(!s.saturated);
    (
        s.affine_closures_prepared,
        s.phase_maps_prepared,
        s.phase_resources_prepared,
    )
}
#[test]
fn actual_factory_counters_distinguish_warm_evaluation_from_cold_reconstruction() {
    let a = ScalarArithmetic::default();
    let u = compile_unitary(
        &parse_circuit("H 0\nT 0\nH 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let rows = [bits(1, 1)];
    let plan = u
        .prepare_probability_with_fibers(&rows, 256, Default::default(), &a)
        .unwrap();
    let before = u.query_work_statistics();
    assert_eq!(preparations(before), (1, 1, 1));
    for bit in 0..32 {
        assert_eq!(
            plan.evaluate(&u, &bits(1, bit & 1), 256, &a).unwrap().value,
            probability(bit & 1 == 0)
        );
    }
    let warm = u.query_work_statistics();
    assert_eq!(preparations(warm), preparations(before));
    assert_eq!(warm.fixed_evaluations - before.fixed_evaluations, 32);
    assert_eq!(warm.cold_evaluations, before.cold_evaluations);
    for bit in 0..4 {
        let fresh = u.prepare_probability(&rows, &a).unwrap();
        let expected = plan.evaluate(&u, &bits(1, bit & 1), 256, &a).unwrap().value;
        assert_eq!(
            fresh
                .evaluate(&u, &bits(1, bit & 1), 256, &a)
                .unwrap()
                .value,
            expected
        );
    }
    let cold = u.query_work_statistics();
    assert_eq!(
        cold.affine_closures_prepared - warm.affine_closures_prepared,
        4
    );
    assert_eq!(cold.cold_evaluations - warm.cold_evaluations, 4);
    assert_ne!(
        preparations(cold),
        preparations(warm),
        "numerically equal cold reconstruction must fail the warm work contract"
    );
    println!(
        "32warm evaluations:0newclosure/map/resource;4forcedcold:4realclosurefactories and4cold evaluations;exactvalues identical"
    );
}
#[test]
fn views_and_threads_share_only_preparation_and_observe_actual_work_on_one_owner() {
    let a = ScalarArithmetic::default();
    let u = compile_unitary(
        &parse_circuit("H 0\nT 0\nH 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let x = u
        .with_insertions(
            &[PauliInsertion {
                before_gate: 3,
                word: PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap(),
            }],
            &a,
        )
        .unwrap();
    let plan = u
        .prepare_probability_with_fibers(&[bits(1, 1)], 256, Default::default(), &a)
        .unwrap();
    let before = u.query_work_statistics();
    assert_eq!(x.query_work_statistics(), before);
    std::thread::scope(|scope| {
        for thread in 0..8 {
            let state = if thread & 1 == 0 { &u } else { &x };
            let plan = &plan;
            let a = &a;
            scope.spawn(move || {
                for bit in 0..16 {
                    let outcome = bit & 1;
                    assert_eq!(
                        plan.evaluate(state, &bits(1, outcome), 256, a)
                            .unwrap()
                            .value,
                        probability((outcome ^ (thread & 1)) == 0)
                    );
                }
            });
        }
    });
    let after = u.query_work_statistics();
    assert_eq!(preparations(after), preparations(before));
    assert_eq!(after.fixed_evaluations - before.fixed_evaluations, 128);
    assert_eq!(after, x.query_work_statistics());
    assert!(plan.evaluate(&u, &bits(0, 0), 256, &a).is_err());
    assert_eq!(
        after,
        u.query_work_statistics(),
        "invalid RHS must reject before numerical work"
    );
    println!(
        "8threads x16RHS:128evaluations,onepreparedowner,independentnumericworkspaces;wrongRHSrefused beforework"
    );
}

fn requery_history(history: &rmagna::runtime::ResolvedHistory) -> ExactScalar {
    let rows = history
        .conditions()
        .iter()
        .map(|(p, _)| p.clone())
        .collect::<Vec<_>>();
    let outcomes = BitVector::from_indices(
        rows.len(),
        history
            .conditions()
            .iter()
            .enumerate()
            .filter_map(|(j, (_, v))| v.then_some(j)),
    )
    .unwrap();
    history
        .instrument()
        .reported_probability(
            &rows,
            &outcomes,
            history.faults(),
            256,
            &ScalarArithmetic::default(),
        )
        .unwrap()
        .value
}

#[test]
fn completed_exact_histories_reuse_actual_physical_mass_and_preserve_every_callback_and_word() {
    use rmagna::{
        circuit::{BitAtom, BitExpr},
        noise::{RandomSource, SplitMix64},
        records::bernoulli_ratio,
        runtime::{
            CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler, RuntimeOptions,
        },
        unitary::CompilerStrategy,
    };
    use std::sync::{Arc, Mutex};
    struct Counted {
        rng: SplitMix64,
        words: usize,
    }
    impl RandomSource for Counted {
        fn next_u64(&mut self) -> u64 {
            self.words += 1;
            self.rng.next_u64()
        }
    }
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for magic in [false, true] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for compiler in [
                CompilerStrategy::ParityCopies,
                CompilerStrategy::SharedCommutingLayer,
            ] {
                for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                    let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
                    let seen = Arc::clone(&calls);
                    let mut registry = ControllerRegistry::default();
                    registry
                        .register("echo", "1", move |v: &[bool]| {
                            seen.lock().unwrap().push(v.to_vec());
                            Ok(v.to_vec())
                        })
                        .unwrap();
                    let prefix = if magic {
                        "H 0\nT 0\nMX 0\n"
                    } else {
                        "X 0\nM 0\n"
                    };
                    let source = format!(
                        "{prefix}DETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[1]\nZ_ERROR(1) ctrl[1] 2"
                    );
                    let circuit = parse_circuit(
                        &source,
                        &ParseOptions {
                            controllers: registry.names(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    let mut options = RuntimeOptions::default();
                    options.strategy = strategy;
                    options.history_sampler = sampler;
                    options.compile.compiler_strategy = compiler;
                    let program = CausalProgram::new(&circuit, &registry, options, &a).unwrap();
                    let mut observed = std::collections::BTreeSet::new();
                    for seed in 0..16 {
                        calls.lock().unwrap().clear();
                        let mut random = Counted {
                            rng: SplitMix64::new(seed),
                            words: 0,
                        };
                        let history = program
                            .resolve(&[BitExpr::atom(BitAtom::Record(0))], &mut random, &a)
                            .unwrap();
                        let bit = history.requested_values()[0];
                        observed.insert(bit);
                        let expected = if magic {
                            probability(!bit)
                        } else {
                            assert!(bit);
                            ExactScalar::one()
                        };
                        assert_eq!(*history.history_mass(), expected);
                        assert_eq!(*history.history_mass(), requery_history(&history));
                        assert_eq!(
                            history.history_mass_reused(),
                            sampler == HistorySampler::ExactMarginal
                        );
                        assert_eq!(*calls.lock().unwrap(), vec![vec![bit], vec![bit]]);
                        assert_eq!(random.words, usize::from(magic));
                        assert_eq!(history.entropy_words(), (0, random.words));
                        if sampler == HistorySampler::ExactMarginal {
                            assert!(history.history_proposal_mass().is_none());
                            let mut reference = SplitMix64::new(seed);
                            let expected_bit = if magic {
                                !bernoulli_ratio(
                                    &probability(true),
                                    &ExactScalar::one(),
                                    &mut reference,
                                    32,
                                    &a,
                                )
                                .unwrap()
                                .selected
                            } else {
                                true
                            };
                            assert_eq!(bit, expected_bit);
                            assert_eq!(random.rng.state(), reference.state());
                        } else {
                            let expected_q = if magic {
                                a.omega(0, -2).unwrap()
                            } else {
                                ExactScalar::one()
                            };
                            assert_eq!(history.history_proposal_mass(), Some(&expected_q));
                            if magic {
                                assert_ne!(
                                    *history.history_mass(),
                                    expected_q,
                                    "proposal Q must never replace physical P"
                                );
                            }
                        }
                        cases += 1;
                    }
                    assert_eq!(observed.len(), if magic { 2 } else { 1 });
                }
            }
        }
    }
    println!(
        "{cases} completed physical histories:two source prefixes x2strategies x2compilers x2samplers x16seeds;exactmassreuseonly;literalP,independentfullquery,RNGstateand2actualcallbacksagree"
    );
}

#[test]
fn cached_physical_prefix_mass_is_independent_of_future_selection_and_guarded_noise() {
    use rmagna::{
        circuit::{BitAtom, BitExpr},
        noise::SplitMix64,
        runtime::{
            CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler, RuntimeOptions,
        },
    };
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for select in ["POSTSELECT det[-1]", "POSTSELECT !det[-1]"] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", |v: &[bool]| Ok(v.to_vec()))
                .unwrap();
            let source = format!(
                "H 0\nT 0\nMX 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCH ctrl[0] 1\nZ_ERROR(1/3) ctrl[0] 1\nM(1/7) 1\nDETECTOR rec[-1]\n{select}"
            );
            let circuit = parse_circuit(
                &source,
                &ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                },
            )
            .unwrap();
            let mut options = RuntimeOptions::default();
            options.strategy = strategy;
            options.history_sampler = HistorySampler::ExactMarginal;
            let program = CausalProgram::new(&circuit, &registry, options, &a).unwrap();
            for seed in 0..16 {
                let history = program
                    .resolve(
                        &[BitExpr::atom(BitAtom::Record(0))],
                        &mut SplitMix64::new(seed),
                        &a,
                    )
                    .unwrap();
                assert!(history.history_mass_reused());
                let bit = history.requested_values()[0];
                assert_eq!(*history.history_mass(), probability(!bit));
                assert_eq!(*history.history_mass(), requery_history(&history));
                assert_eq!(history.controller_calls().len(), 1);
                cases += 1;
            }
        }
    }
    println!(
        "{cases} future-selection/guarded-noise paths:cachedunconditionedP equals literalTmass and freshfinalinstrumentquery"
    );
}

#[test]
fn prepared_feedback_layers_reuse_actual_factories_and_match_independent_basis_targets() {
    use rmagna::{
        costs::{ExecutionLimits, ExecutionMeter},
        instrument::QueryPreparation,
        noise::{NoiseSampling, SplitMix64},
        runtime::{ControllerRegistry, HistorySampler},
        task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task},
        unitary::CompilerStrategy,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let files = Files(std::env::temp_dir().join(format!(
        "rmagna-fixed-reference-layers-{}",
        std::process::id()
    )));
    std::fs::create_dir(&files.0).unwrap();
    std::fs::write(files.0.join("ideal.magna"), "I 0").unwrap();
    std::fs::write(files.0.join("magic.magna"), "H 0\nT 0").unwrap();
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for layers in [1, 3, 7] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for mode in [
                QueryPreparation::Trace,
                QueryPreparation::FixedFibers(Default::default()),
                QueryPreparation::FixedCatalogue(Default::default()),
            ] {
                for compiler in [
                    CompilerStrategy::ParityCopies,
                    CompilerStrategy::SharedCommutingLayer,
                ] {
                    for magic in [false, true] {
                        let calls = Arc::new(AtomicUsize::new(0));
                        let seen = Arc::clone(&calls);
                        let mut registry = ControllerRegistry::default();
                        registry
                            .register("and", "1", move |v: &[bool]| {
                                seen.fetch_add(1, Ordering::Relaxed);
                                Ok(vec![v[0] & v[1]])
                            })
                            .unwrap();
                        let mut source = String::new();
                        for j in 0..layers {
                            let (u, v) = (2 * j + 1, 2 * j + 2);
                            source += &format!(
                                "H {u} {v}\nM {u} {v}\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[{j}]\nCX ctrl[{j}] 0\n"
                            );
                        }
                        let parse = ParseOptions {
                            controllers: registry.names(),
                            ..Default::default()
                        };
                        let circuit = parse_circuit(&source, &parse).unwrap();
                        let task = format!(
                            "{{\"outputs\":[0],\"ideal_state\":\"{}\"}}",
                            if magic { "magic.magna" } else { "ideal.magna" }
                        );
                        let modules = parse_task(&task, &files.0, &TaskOptions::default())
                            .unwrap()
                            .load_modules(&circuit, &parse)
                            .unwrap();
                        let mut options = EvaluationOptions::default();
                        options.runtime.history_sampler = sampler;
                        options.runtime.compile.compiler_strategy = compiler;
                        options.runtime.compile.probability_preparation = mode;
                        options.runtime.compile.density_preparation = mode;
                        options.runtime.compile.moment_preparation = mode;
                        let prep_meter = ExecutionMeter::new(ExecutionLimits::default());
                        let e = TaskEvaluator::new_with_costs(
                            &circuit,
                            &modules,
                            &registry,
                            options,
                            &NoiseSampling::Physical,
                            &prep_meter,
                        )
                        .unwrap();
                        let prepared = e.prepare_sampling_queries(Default::default(), &prep_meter);
                        assert_eq!(prepared.result, Ok(()));
                        assert_eq!(calls.load(Ordering::Relaxed), 0);
                        let before = e.query_work_statistics();
                        let before_source = before.source.unwrap();
                        let meter = ExecutionMeter::default();
                        let mut rng = SplitMix64::new(21);
                        for _ in 0..8 {
                            let result = e.sample_with_costs(&mut rng, &meter).unwrap();
                            // Independent physical oracle: each layer has two
                            // fair independent records. The final output is XOR
                            // of their ANDs. A magic target overlaps either
                            // computational basis output with probability 1/2.
                            let mass = a.omega(0, -4 * layers as i64).unwrap();
                            assert_eq!(result.history_mass, mass);
                            assert_eq!(result.raw_acceptance, mass);
                            assert_eq!(result.controller_calls.len(), layers);
                            let mut parity = false;
                            for call in &result.controller_calls {
                                assert_eq!(call.outputs, vec![call.inputs[0] & call.inputs[1]]);
                                parity ^= call.outputs[0];
                            }
                            let error = if magic {
                                a.multiply(&mass, &a.omega(0, -2).unwrap()).unwrap()
                            } else if parity {
                                mass.clone()
                            } else {
                                ExactScalar::zero()
                            };
                            assert_eq!(result.raw_error, error);
                            assert_eq!(result.network_compilations, 0);
                        }
                        let after = e.query_work_statistics();
                        let after_source = after.source.unwrap();
                        assert_eq!(preparations(before_source), preparations(after_source));
                        assert_eq!(before.target, after.target);
                        assert_eq!(calls.load(Ordering::Relaxed), layers * 8);
                        assert_eq!(meter.snapshot().compilation.attempted, 0);
                        let actual_queries = if sampler == HistorySampler::ExactMarginal {
                            8 * (3 * layers + 2)
                        } else {
                            24
                        };
                        assert_eq!(meter.snapshot().query.attempted, actual_queries as u64);
                        assert_eq!(
                            after_source.closure_evaluations - before_source.closure_evaluations,
                            actual_queries as u64
                        );
                        let cold = after_source.cold_evaluations - before_source.cold_evaluations;
                        assert_eq!(
                            cold,
                            if matches!(mode, QueryPreparation::Trace) {
                                actual_queries as u64
                            } else {
                                0
                            }
                        );
                        assert_eq!(e.prepared_density_reuses(), Some(8));
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 72);
    println!(
        "72prepared feedback cases/576draws:1,3,7layers x2samplers x3policies x2compilers x2targets;0hotfactories;actualquerycounts and independent joint/basis target masses agree"
    );
}

struct TaskFiles(std::path::PathBuf);
impl TaskFiles {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-fixed-task-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("ideal.magna"), "I 0").unwrap();
        Self(path)
    }
    fn evaluator(
        &self,
        source: &str,
        task: &str,
        registry: &rmagna::runtime::ControllerRegistry,
        options: rmagna::task::EvaluationOptions,
    ) -> rmagna::task::TaskEvaluator {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(source, &parse).unwrap();
        let modules = rmagna::task::parse_task(task, &self.0, &Default::default())
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
        rmagna::task::TaskEvaluator::new(&circuit, &modules, registry, options).unwrap()
    }
}
impl Drop for TaskFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn explicit_controller_elision_removes_cancelled_symbols_before_routing_and_preserves_default_effects()
 {
    use rmagna::{
        costs::ExecutionMeter,
        noise::SplitMix64,
        runtime::ControllerRegistry,
        task::{ControllerExecution, EvaluationOptions},
        unitary::CompilerStrategy,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let files = TaskFiles::new();
    let source = "H 1 2\nM 1 2\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nCONTROL[XOR] ctrl[0] ctrl[1] -> ctrl[2]\nCX ctrl[2] 0";
    let task = r#"{"outputs":[0],"ideal_state":"ideal.magna"}"#;
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let mut registry = ControllerRegistry::default();
        registry
            .register("and", "1", move |v: &[bool]| {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(vec![v[0] & v[1]])
            })
            .unwrap();
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        let declared = files.evaluator(source, task, &registry, options);
        assert_eq!(declared.execution_route(), "causal_fixed_template");
        let draw = declared.sample(&mut SplitMix64::new(9)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let bit = draw.controller_calls[0].inputs[1];
        assert_eq!(
            draw.raw_error,
            if bit {
                draw.history_mass.clone()
            } else {
                ExactScalar::zero()
            }
        );
        options.controller_execution = ControllerExecution::RequiredForTask;
        let required = files.evaluator(source, task, &registry, options);
        assert_eq!(required.execution_route(), "linear_affine");
        let report =
            required.prepare_sampling_queries(Default::default(), &ExecutionMeter::default());
        assert_eq!(report.result, Ok(()));
        assert!(!report.by_kind.contains_key("reported_probability"));
        assert!(!report.by_kind.contains_key("reported_support"));
        for seed in 0..16 {
            let (draw, diagnostics) = required
                .sample_with_diagnostics(&mut SplitMix64::new(seed), &Default::default())
                .unwrap();
            assert!(draw.controller_calls.is_empty());
            assert_eq!(diagnostics.history_entropy_words, 0);
            assert_eq!(diagnostics.protocol_controller_calls, 0);
            assert_eq!(draw.raw_acceptance, ExactScalar::one());
            assert_eq!(draw.history_mass, ExactScalar::one());
            assert_eq!(draw.raw_error, a.omega(0, -2).unwrap());
        }
        assert_eq!(
            calls.load(Ordering::Relaxed),
            1,
            "elision must remove actual callbacks,not just their reported count"
        );
        assert_eq!(
            parse_circuit(
                source,
                &ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                }
            )
            .unwrap()
            .external_calls(),
            1
        );
    }
    println!(
        "2compilers/32integrateddraws:cancelled nonlinear callback absent before route/preparation;exact independent error1/2,nohistoryentropy;default declared effect retained"
    );
}

#[test]
fn required_controller_dependencies_keep_sparse_call_ids_guards_selection_and_recovery_namespaces()
{
    use rmagna::{
        noise::SplitMix64,
        runtime::ControllerRegistry,
        task::{ControllerExecution, EvaluationOptions},
    };
    let files = TaskFiles::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("dead", "1", |_: &[bool]| {
            Err("elided callback executed".into())
        })
        .unwrap();
    registry
        .register("echo", "1", |v: &[bool]| Ok(v.to_vec()))
        .unwrap();
    let options = EvaluationOptions {
        controller_execution: ControllerExecution::RequiredForTask,
        ..Default::default()
    };
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[dead] det[0] -> ctrl[0]\nCONTROL[echo] det[0] -> ctrl[1]\nCONTROL[echo] ctrl[1] -> ctrl[2]\nCX ctrl[2] 1\nCONTROL[dead] ctrl[1] -> ctrl[3]";
    let evaluator = files.evaluator(
        source,
        r#"{"outputs":[1],"sample_records":[0],"ideal_state":"ideal.magna"}"#,
        &registry,
        options,
    );
    let mut cases = 0;
    for seed in 0..16 {
        let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
        assert_eq!(
            draw.controller_calls
                .iter()
                .map(|c| c.call)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(draw.controller_calls[0].inputs, draw.sampled_records);
        assert_eq!(draw.controller_calls[1].inputs, draw.sampled_records);
        assert_eq!(
            draw.raw_error,
            if draw.sampled_records[0] {
                draw.history_mass.clone()
            } else {
                ExactScalar::zero()
            }
        );
        cases += 1;
    }
    for root in [
        "X_ERROR(1/3) ctrl[0] 1",
        "POSTSELECT ctrl[0]",
        "OBSERVABLE_INCLUDE(0) ctrl[0]",
    ] {
        let source =
            format!("I 1\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\n{root}");
        let e = files.evaluator(
            &source,
            r#"{"outputs":[1],"sample_records":[0],"ideal_state":"ideal.magna"}"#,
            &registry,
            options,
        );
        for seed in 0..8 {
            let draw = e.sample(&mut SplitMix64::new(seed)).unwrap();
            assert_eq!(draw.controller_calls.len(), 1, "live root:{root}");
            assert_eq!(draw.controller_calls[0].inputs, draw.sampled_records);
            cases += 1;
        }
        let integrated = files.evaluator(
            &source,
            r#"{"outputs":[1],"ideal_state":"ideal.magna"}"#,
            &registry,
            options,
        );
        let exact = integrated.enumerate_physical(&Default::default()).unwrap();
        let f = rmagna::field::FieldArithmetic::default();
        let rational = |n: i64, d: i64| {
            f.from_rational(num_rational::BigRational::new(n.into(), d.into()))
                .unwrap()
        };
        assert_eq!(
            exact.masses.accepted,
            rational(1, if root.starts_with("POSTSELECT") { 2 } else { 1 })
        );
        assert_eq!(
            exact.masses.error,
            if root.starts_with("X_ERROR") {
                rational(1, 6)
            } else {
                rational(0, 1)
            }
        );
    }
    std::fs::write(
        files.0.join("recovery.magna"),
        "H 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 0",
    )
    .unwrap();
    let e = files.evaluator(
        "I 0\nCONTROL[dead] -> ctrl[0]",
        r#"{"outputs":[0],"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,
        &registry,
        options,
    );
    for seed in 0..8 {
        let (draw, d) = e
            .sample_with_diagnostics(&mut SplitMix64::new(seed), &Default::default())
            .unwrap();
        assert_eq!(draw.controller_calls.len(), 1);
        assert_eq!(draw.controller_calls[0].call, 1);
        assert_eq!(d.protocol_controller_calls, 0);
        assert_eq!(d.recovery_controller_calls, 1);
        assert_eq!(
            draw.raw_error,
            if draw.controller_calls[0].inputs[0] {
                draw.history_mass.clone()
            } else {
                ExactScalar::zero()
            }
        );
        cases += 1;
    }
    assert_eq!(cases, 48);
    println!(
        "48draws:transitive livecallIDs1/2,3guard/selection/observable roots,and recoverycall1 afterdeadprotocolcall0;unneeded callbacks returnerrorsifactuallyexecuted"
    );
}

fn literal_plan(
    width: usize,
    operations: Vec<rmagna::instrument::OriginalOperation>,
    controls: Vec<rmagna::instrument::OriginalControl>,
    limits: rmagna::instrument::OriginalLimits,
) -> rmagna::instrument::OriginalAssignmentPlan {
    use rmagna::{clifford::CliffordCircuit, instrument::*, unitary::CompilerStrategy};
    let a = ScalarArithmetic::default();
    OriginalAssignmentPlan::from_owned(
        OriginalInstrument::new(
            (0..width as i64).collect(),
            (0..width as i64).collect(),
            CliffordCircuit::computational(BitVector::zeros(width).unwrap()).unwrap(),
            operations,
            controls,
            limits,
            &a,
        )
        .unwrap(),
        CompilerStrategy::ParityCopies,
        a,
    )
}
fn original_op(
    id: &str,
    gate: rmagna::instrument::OriginalGate,
) -> rmagna::instrument::OriginalOperation {
    rmagna::instrument::OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn original_z(record: &str, wire: i64) -> rmagna::instrument::OriginalGate {
    rmagna::instrument::OriginalGate::Measure {
        factors: vec![(wire, rmagna::instrument::OriginalPauli::Z)],
        inverted: false,
        record: record.into(),
    }
}

#[test]
fn probability_value_cache_keeps_true_fault_report_and_proposal_inputs_separate() {
    use rmagna::{field::FieldArithmetic, instrument::*};
    let ops = vec![
        original_op("i", OriginalGate::Identity(vec![0])),
        original_op("m", original_z("m", 0)),
    ];
    let plan = literal_plan(1, ops.clone(), vec![], OriginalLimits::default());
    let mut cache = OriginalProbabilityCache::new(2, 1 << 20);
    let mut assignment = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let first = cache.compile(&plan, assignment.clone()).unwrap();
    assert!(!first.cache_hit());
    assert_eq!(*first.certificate().probability(), ExactScalar::one());
    assert!(
        cache
            .compile(&plan, assignment.clone())
            .unwrap()
            .cache_hit()
    );
    assignment.true_values[0].1 = true;
    let zero = cache.compile(&plan, assignment.clone()).unwrap();
    assert!(!zero.cache_hit());
    assert!(zero.certificate().probability().is_zero());
    assert!(
        cache
            .compile(&plan, assignment.clone())
            .unwrap()
            .cache_hit(),
        "zero values are cached too"
    );
    assignment.faults.push(OriginalFault {
        after: "i".into(),
        wire: 0,
        pauli: OriginalPauli::X,
    });
    assignment.report_flips.push("m".into());
    let fault = cache.compile(&plan, assignment.clone()).unwrap();
    assert_eq!(*fault.certificate().probability(), ExactScalar::one());
    assert!(!fault.certificate().branch().records()[0].reported_value);
    assignment.faults[0].pauli = OriginalPauli::Z;
    assert!(
        cache
            .compile(&plan, assignment.clone())
            .unwrap()
            .certificate()
            .probability()
            .is_zero()
    );
    assert_eq!(
        *first.certificate().probability(),
        ExactScalar::one(),
        "old results own their values"
    );
    let before = cache.statistics();
    assert!(cache.compile(&plan, OriginalAssignment::default()).is_err());
    assert_eq!(
        cache.statistics(),
        before,
        "bad capture never enters cache kernel"
    );
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.statistics().serialized_bytes, 0);
    let assignment = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let f = FieldArithmetic::default();
    for (index, denominator) in [3, 4].into_iter().enumerate() {
        let result = cache.compile(&plan, assignment.clone()).unwrap();
        assert_eq!(result.cache_hit(), index != 0);
        let weighted = f
            .scale(
                &f.from_scalar(result.certificate().probability()).unwrap(),
                &num_rational::BigRational::from_integer(denominator.into()),
            )
            .unwrap();
        assert_eq!(
            weighted,
            f.from_rational(num_rational::BigRational::from_integer(denominator.into()))
                .unwrap()
        );
    }
    let mut statistics = rmagna::statistics::ExactStatistics::new(1, Default::default()).unwrap();
    for (true_value, reciprocal_numerator) in [(false, 3), (true, 0), (true, 0)] {
        let input = OriginalAssignment {
            true_values: vec![("m".into(), true_value)],
            ..Default::default()
        };
        let result = cache.compile(&plan, input).unwrap();
        let inverse_q =
            num_rational::BigRational::new(3.into(), if true_value { 2.into() } else { 1.into() });
        let weight = f
            .scale(
                &f.from_scalar(result.certificate().probability()).unwrap(),
                &inverse_q,
            )
            .unwrap();
        assert_eq!(
            weight,
            f.from_rational(num_rational::BigRational::from_integer(
                reciprocal_numerator.into()
            ))
            .unwrap()
        );
        statistics.push(&[weight.clone()], &weight).unwrap();
    }
    let summary = statistics.summarize().unwrap().unwrap();
    assert_eq!(summary.count, 3);
    assert_eq!(summary.means, vec![rmagna::field::ExactField::one()]);
    assert_eq!(
        summary.effective_sample_size,
        rmagna::field::ExactField::one()
    );
    let replaced = literal_plan(
        1,
        vec![
            original_op(
                "i",
                OriginalGate::Pauli {
                    wire: 0,
                    pauli: OriginalPauli::X,
                },
            ),
            original_op("m", original_z("m", 0)),
        ],
        vec![],
        OriginalLimits::default(),
    );
    let changed = cache.compile(&replaced, assignment).unwrap();
    assert!(!changed.cache_hit());
    assert!(changed.certificate().probability().is_zero());
    println!(
        "physical value LRU:true/report/fault changes,owned zero/nonzero values,invalid capture,explicit newsource;proposal1/3and1/4share physical entry andgive weights3and4;cachedzero drawsretain3proposal mean1andESS1"
    );
}

#[test]
fn probability_value_cache_recollects_real_emissions_and_does_not_own_callback_sources() {
    use rmagna::instrument::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let toggle = Arc::new(AtomicBool::new(false));
    let lifetime = Arc::new(());
    let weak = Arc::downgrade(&lifetime);
    let (seen, changed, owned) = (
        Arc::clone(&calls),
        Arc::clone(&toggle),
        Arc::clone(&lifetime),
    );
    let control = OriginalControl {
        operation: "c".into(),
        parents: vec![],
        version: "same".into(),
        callback: Arc::new(move |_| {
            let _ = &owned;
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(vec![if changed.load(Ordering::Relaxed) {
                OriginalGate::Pauli {
                    wire: 0,
                    pauli: OriginalPauli::X,
                }
            } else {
                OriginalGate::Identity(vec![0])
            }])
        }),
    };
    let plan = literal_plan(
        1,
        vec![
            original_op("c", OriginalGate::Control),
            original_op("m", original_z("m", 0)),
        ],
        vec![control],
        OriginalLimits::default(),
    );
    drop(lifetime);
    let mut cache = OriginalProbabilityCache::default();
    let input = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let mut last = None;
    for (index, (emits_x, hit)) in [
        (false, false),
        (false, true),
        (true, false),
        (true, true),
        (false, true),
    ]
    .into_iter()
    .enumerate()
    {
        toggle.store(emits_x, Ordering::Relaxed);
        let result = cache.compile(&plan, input.clone()).unwrap();
        assert_eq!(result.cache_hit(), hit);
        assert_eq!(result.certificate().probability().is_zero(), emits_x);
        assert_eq!(calls.load(Ordering::Relaxed), index + 1);
        last = Some(result);
    }
    assert_eq!(cache.statistics().probability_evaluations, 2);
    drop(plan);
    assert!(
        weak.upgrade().is_none(),
        "neither LRU nor returned certificate retains callback owner"
    );
    assert_eq!(
        *last.unwrap().certificate().probability(),
        ExactScalar::one()
    );
    assert_eq!(cache.len(), 2);
    println!(
        "5actual callbacks,same declared version;2different emission bodies evaluated,3safe value hits;cache/results release source callback owner"
    );
}

#[test]
fn probability_value_cache_evicts_real_entries_by_both_count_and_encoded_bytes() {
    use rmagna::instrument::*;
    let mut ops = vec![];
    for q in 0..3 {
        ops.push(original_op(&format!("h{q}"), OriginalGate::H(q)));
        ops.push(original_op(
            &format!("m{q}"),
            original_z(&format!("m{q}"), q),
        ));
    }
    let plan = literal_plan(3, ops, vec![], OriginalLimits::default());
    let input = |word| OriginalAssignment {
        true_values: (0..3)
            .map(|j| (format!("m{j}"), word & (1 << j) != 0))
            .collect(),
        ..Default::default()
    };
    let mut cache = OriginalProbabilityCache::new(2, 1 << 20);
    for word in 0..8 {
        let result = cache.compile(&plan, input(word)).unwrap();
        assert!(!result.cache_hit());
        assert_eq!(
            *result.certificate().probability(),
            ScalarArithmetic::default().omega(0, -6).unwrap()
        );
        assert_eq!(cache.len(), (word + 1).min(2));
        assert!(cache.statistics().serialized_bytes <= cache.limits().1);
    }
    assert_eq!(cache.statistics().evictions, 6);
    assert!(cache.compile(&plan, input(7)).unwrap().cache_hit());
    assert!(!cache.compile(&plan, input(0)).unwrap().cache_hit());
    let one_entry_bytes = cache.statistics().serialized_bytes / 2;
    cache.set_limits(2, one_entry_bytes);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.statistics().serialized_bytes, one_entry_bytes);
    cache.set_limits(2, one_entry_bytes - 1);
    assert!(cache.is_empty());
    let before = cache.statistics().bypasses;
    assert!(!cache.compile(&plan, input(1)).unwrap().cache_hit());
    assert!(cache.is_empty());
    assert_eq!(cache.statistics().bypasses, before + 1);
    cache.set_limits(0, 1 << 20);
    assert!(!cache.compile(&plan, input(1)).unwrap().cache_hit());
    assert!(cache.is_empty());
    println!(
        "8three-bit physical assignments:2-entry LRU6evictions;recency,byte shrink,value-too-large andzero-entry bypass preserve exact1/8"
    );
}

#[test]
fn a_new_tighter_source_budget_is_not_bypassed_by_a_warm_probability_value() {
    use rmagna::instrument::*;
    let ops = vec![
        original_op("h", OriginalGate::H(0)),
        original_op(
            "t",
            OriginalGate::Phase {
                wires: vec![0],
                turns: 1,
            },
        ),
        original_op(
            "m",
            OriginalGate::Measure {
                factors: vec![(0, OriginalPauli::X)],
                inverted: false,
                record: "m".into(),
            },
        ),
    ];
    let normal = literal_plan(1, ops.clone(), vec![], OriginalLimits::default());
    let input = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let mut cache = OriginalProbabilityCache::default();
    assert_eq!(
        *cache
            .compile(&normal, input.clone())
            .unwrap()
            .certificate()
            .probability(),
        probability(true)
    );
    assert!(cache.compile(&normal, input.clone()).unwrap().cache_hit());
    let mut limits = OriginalLimits::default();
    limits.expansion.max_side_terms = 1;
    let tight = literal_plan(1, ops, vec![], limits);
    let before = cache.statistics();
    let error = cache
        .compile(&tight, input)
        .err()
        .expect("new finite expansion budget must still fail");
    assert!(
        matches!(
            error,
            OriginalError::Unitary(_) | OriginalError::ResourceLimit { .. }
        ),
        "{error:?}"
    );
    assert_eq!(cache.statistics().hits, before.hits);
    assert_eq!(cache.len(), 1, "failed results are never cached");
    println!(
        "warm exact Tmass doesnot bypass newsource max_side_terms1;failed evaluation absentfromcache"
    );
}

#[test]
fn fresh_callback_phase_tables_invalidate_value_entries_without_version_changes() {
    use rmagna::instrument::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let turns = Arc::new(AtomicUsize::new(0));
    let selected = Arc::clone(&turns);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let control = OriginalControl {
        operation: "c".into(),
        parents: vec![],
        version: "unchanged".into(),
        callback: Arc::new(move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
            let value = selected.load(Ordering::Relaxed);
            if value == 99 {
                return Err(OriginalCallbackError::Failed(
                    "actual callback refusal".into(),
                ));
            }
            Ok(vec![OriginalGate::Phase {
                wires: vec![0],
                turns: value as i64,
            }])
        }),
    };
    let plan = literal_plan(
        1,
        vec![
            original_op("h", OriginalGate::H(0)),
            original_op("c", OriginalGate::Control),
            original_op("h2", OriginalGate::H(0)),
            original_op("m", original_z("m", 0)),
        ],
        vec![control],
        OriginalLimits::default(),
    );
    let input = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let a = ScalarArithmetic::default();
    let mut cache = OriginalProbabilityCache::default();
    for (index, t) in [0, 1, 3, 2, 0, 1].into_iter().enumerate() {
        turns.store(t, Ordering::Relaxed);
        let row = cache.compile(&plan, input.clone()).unwrap();
        let expected = match t {
            0 => ExactScalar::one(),
            1 => probability(true),
            3 => probability(false),
            2 => a.omega(0, -2).unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(*row.certificate().probability(), expected);
        assert_eq!(row.cache_hit(), index >= 4);
    }
    assert_eq!(cache.len(), 4);
    assert_eq!(cache.statistics().probability_evaluations, 4);
    let before = cache.statistics();
    turns.store(99, Ordering::Relaxed);
    let error = cache
        .compile(&plan, input)
        .err()
        .expect("actual callback failure must not use an older value");
    assert!(
        matches!(error,OriginalError::Callback{error:OriginalCallbackError::Failed(ref message),..} if message=="actual callback refusal")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 7);
    assert_eq!(cache.statistics(), before);
    println!(
        "4same-version phase tables return literal1,(2+sqrt2)/4,(2-sqrt2)/4,1/2;2safe value hits;seventh actual callback error propagates before lookup"
    );
}
