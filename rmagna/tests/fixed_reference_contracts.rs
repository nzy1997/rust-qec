// SPDX-License-Identifier: MPL-2.0
//! Fixed-reference core contracts; complete source qualification continues in R09.
#[path = "support/continuation_cases.rs"]
mod continuation_cases;
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
fn preparations(s: QueryWorkStatistics) -> [u64; 10] {
    assert!(!s.saturated);
    [
        s.affine_closures_prepared,
        s.affine_geometry_preparations,
        s.phase_maps_prepared,
        s.phase_resources_prepared,
        s.support_projections_prepared,
        s.support_boundary_maps_prepared,
        s.support_contractions_prepared,
        s.support_phase_maps_prepared,
        s.support_phase_resources_prepared,
        s.support_parents_prepared,
    ]
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
    assert_eq!(preparations(before), [1, 1, 1, 1, 0, 0, 0, 0, 0, 0]);
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
                    let mut options = RuntimeOptions {
                        strategy,
                        history_sampler: sampler,
                        ..Default::default()
                    };
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
            let options = RuntimeOptions {
                strategy,
                history_sampler: HistorySampler::ExactMarginal,
                ..Default::default()
            };
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
                QueryPreparation::Native(Default::default()),
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
                        let storage_before = e.reference_storage_statistics().unwrap();
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
                        let storage_after = e.reference_storage_statistics().unwrap();
                        assert_eq!(
                            storage_after, storage_before,
                            "all retained program positions, Gaussian widths, weak slots and allocated slot capacity stay fixed"
                        );
                        let probability = e.prepared_probability_statistics().unwrap().unwrap();
                        let density = e.prepared_density_statistics().unwrap().unwrap();
                        let moments = e.prepared_moment_statistics().unwrap().unwrap();
                        let entries = probability.cached_queries
                            + density.cached_queries
                            + moments.cached_queries;
                        assert!(
                            probability.cached_queries
                                <= options.runtime.compile.max_prepared_probabilities
                        );
                        assert!(
                            density.cached_queries
                                <= options.runtime.compile.max_prepared_densities
                        );
                        assert!(
                            moments.cached_queries <= options.runtime.compile.max_prepared_moments
                        );
                        for family in [probability, density, moments] {
                            assert!(family.fixed_resources <= family.cached_queries);
                        }
                        let retained = storage_after.source.unwrap();
                        assert!(retained.unique_reference_boundaries <= 2);
                        assert!(retained.live_geometries <= entries);
                        assert!(retained.geometry_weak_slots <= entries + 1);
                        if sampler == HistorySampler::ExactMarginal
                            && matches!(mode, QueryPreparation::Native(_))
                            && !magic
                        {
                            println!(
                                "layers={layers} quiescent source={retained:?}; P/D/M={}/{}/{} fixed={}/{}/{} support={:?}; target={:?}",
                                probability.cached_queries,
                                density.cached_queries,
                                moments.cached_queries,
                                probability.fixed_resources,
                                density.fixed_resources,
                                moments.fixed_resources,
                                e.prepared_support_statistics().unwrap(),
                                storage_after.target
                            );
                        }
                        let after_source = after.source.unwrap();
                        if sampler == HistorySampler::AffineSupport
                            && matches!(mode, QueryPreparation::Trace)
                        {
                            // Trace explicitly defers the labelled support resource.
                            // Its one real first-use construction is not a warm claim.
                            assert_eq!(prepared.by_kind["reported_support"].cold, 1);
                            assert_eq!(
                                &preparations(before_source)[..4],
                                &preparations(after_source)[..4]
                            );
                            assert_eq!(
                                after_source.support_parents_prepared
                                    - before_source.support_parents_prepared,
                                1
                            );
                            assert_eq!(
                                after_source.support_phase_maps_prepared
                                    - before_source.support_phase_maps_prepared,
                                1
                            );
                            assert_eq!(
                                after_source.support_phase_resources_prepared
                                    - before_source.support_phase_resources_prepared,
                                1
                            );
                        } else {
                            assert_eq!(preparations(before_source), preparations(after_source));
                            if sampler == HistorySampler::AffineSupport {
                                assert_eq!(prepared.by_kind["reported_support"].cold, 0);
                                assert_eq!(before_source.support_parents_prepared, 1);
                            }
                        }
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
    assert_eq!(cases, 96);
    println!(
        "96prepared feedback cases/768draws:1,3,7layers x2samplers x4policies x2compilers x2targets;fixedpolicies0hotfactories,Traceexplicitlydefers1supportparent;actualquerycounts and independent joint/basis target masses agree"
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
        statistics
            .push(std::slice::from_ref(&weight), &weight)
            .unwrap();
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
    let mut single = OriginalProbabilityCache::new(1, 1 << 20);
    let input = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let first = single.compile(&plan, input.clone()).unwrap();
    let mut editable_export = first.certificate().to_json();
    editable_export["probability"]["coefficients"][0] = serde_json::json!("999");
    assert_eq!(*first.certificate().probability(), ExactScalar::one());
    let again = single.compile(&plan, input).unwrap();
    assert!(again.cache_hit());
    assert_eq!(*again.certificate().probability(), ExactScalar::one());
    for _ in 0..2 {
        let zero = single
            .compile(
                &plan,
                OriginalAssignment {
                    true_values: vec![("m".into(), true)],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(zero.certificate().probability().is_zero());
        assert_eq!(single.len(), 1);
    }
    assert_eq!(single.statistics().evictions, 1);
    let before = single.statistics();
    assert!(
        single
            .compile(&plan, OriginalAssignment::default())
            .is_err()
    );
    assert!(
        single
            .compile(
                &plan,
                OriginalRecordAssignment {
                    records: vec![("m".into(), serde_json::json!(2))],
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(single.statistics(), before);
    assert!(
        OriginalIndependentRecordProposal::new(
            &plan,
            &std::collections::BTreeMap::from([("m".into(), "0".parse().unwrap())]),
            Default::default()
        )
        .is_err()
    );
    println!(
        "physical value LRU:true/report/fault changes,owned zero/nonzero values,invalid capture,explicit newsource;proposal1/3and1/4share physical entry andgive weights3and4;cachedzero drawsretain3proposal mean1andESS1;one-entry evictions,editable export isolation,missing/nonbinary record and zero-full-support proposal refusals"
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

fn dense_three_wire_reference() -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut state = vec![ExactScalar::zero(); 8];
    // H on all three wires, then T0, S1, CCZ012 and CX02.
    for v in 0usize..8 {
        let turns = (v & 1) as i64 + 2 * ((v >> 1) & 1) as i64 + if v == 7 { 4 } else { 0 };
        let out = v ^ if v & 1 != 0 { 4 } else { 0 };
        state[out] = a.omega(turns, -3).unwrap();
    }
    // H1 followed by T1; matrix arithmetic is independent of compiled maps.
    let mut out = vec![ExactScalar::zero(); 8];
    for v in (0usize..8).filter(|v| v & 2 == 0) {
        out[v] = a
            .multiply(
                &a.add(&state[v], &state[v | 2]).unwrap(),
                &a.omega(0, -1).unwrap(),
            )
            .unwrap();
        out[v | 2] = a
            .multiply(
                &a.subtract(&state[v], &state[v | 2]).unwrap(),
                &a.omega(1, -1).unwrap(),
            )
            .unwrap();
    }
    out
}
fn dense_pauli(state: &[ExactScalar], x: usize, z: usize) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut out = vec![ExactScalar::zero(); 8];
    for v in 0usize..8 {
        let phase = 2 * i64::from((x & z).count_ones()) + 4 * i64::from((v & z).count_ones() & 1);
        out[v ^ x] = a.multiply(&state[v], &a.omega(phase, 0).unwrap()).unwrap();
    }
    out
}
fn dense_projected_moment(
    state: &[ExactScalar],
    x: usize,
    z: usize,
    outcome: usize,
) -> ExactScalar {
    let a = ScalarArithmetic::default();
    let mut value = ExactScalar::zero();
    for v in 0usize..8 {
        if (v >> 2) & 1 != outcome || ((v ^ x) >> 2) & 1 != outcome {
            continue;
        }
        let phase = 2 * i64::from((x & z).count_ones()) + 4 * i64::from((v & z).count_ones() & 1);
        let term = a
            .multiply(
                &a.multiply(&state[v ^ x].conjugate(), &state[v]).unwrap(),
                &a.omega(phase, 0).unwrap(),
            )
            .unwrap();
        value = a.add(&value, &term).unwrap();
    }
    value
}
#[test]
fn shared_symbolic_geometry_preserves_all_pauli_rhs_fault_and_zero_pairing_values() {
    use rmagna::{
        clifford::CliffordGate as C,
        unitary::{CompilerStrategy, UnitaryCircuit, UnitaryGate as G},
    };
    let a = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        3,
        vec![
            G::Clifford(C::H(0)),
            G::Clifford(C::H(1)),
            G::Clifford(C::H(2)),
            G::T { qubit: 0, turns: 1 },
            G::Clifford(C::S { qubit: 1, turns: 1 }),
            G::Ccz { a: 0, b: 1, c: 2 },
            G::Clifford(C::Cx {
                control: 0,
                target: 2,
            }),
            G::Clifford(C::H(1)),
            G::T { qubit: 1, turns: 1 },
        ],
    )
    .unwrap();
    let dense = dense_three_wire_reference();
    let row = [bits(3, 4)];
    let (mut cases, mut zero, mut nonzero) = (0, 0, 0);
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let u = circuit
            .compile_with_strategy(&bits(3, 0), &a, compiler)
            .unwrap();
        // Independent dense amplitudes validate the entire reference state.
        for (v, expected) in dense.iter().enumerate() {
            assert_eq!(u.amplitude(&bits(3, v), 256, &a).unwrap().value, *expected);
        }
        let probability = u
            .prepare_probability_with_fibers(&row, 256, Default::default(), &a)
            .unwrap();
        let mut queries = Vec::new();
        for x in 0usize..8 {
            for z in 0usize..8 {
                let word = PauliWord::new(bits(3, x), bits(3, z), (2 * (x & z).count_ones()) as u8)
                    .unwrap();
                queries.push((
                    x,
                    z,
                    u.prepare_pauli_moment_with_fibers(&word, &row, 256, Default::default(), &a)
                        .unwrap(),
                ));
            }
        }
        let prepared = u.query_work_statistics();
        assert_eq!(prepared.affine_closures_prepared, 33);
        assert_eq!(prepared.affine_geometry_preparations, 1);
        assert_eq!(prepared.affine_geometry_reuses, 32);
        assert_eq!(prepared.live_affine_geometries, Some(1));
        for (fx, fz) in [(0, 0), (1, 0), (2, 2), (0, 4)] {
            let fault =
                PauliWord::new(bits(3, fx), bits(3, fz), (2 * (fx & fz).count_ones()) as u8)
                    .unwrap();
            let view = u
                .with_insertions(
                    &[PauliInsertion {
                        before_gate: circuit.gates().len(),
                        word: fault,
                    }],
                    &a,
                )
                .unwrap();
            let expected = dense_pauli(&dense, fx, fz);
            for outcome in 0..2 {
                assert_eq!(
                    probability
                        .evaluate(&view, &bits(1, outcome), 256, &a)
                        .unwrap()
                        .value,
                    dense_projected_moment(&expected, 0, 0, outcome)
                );
                for (x, z, query) in &queries {
                    let actual = query
                        .evaluate(&view, &bits(1, outcome), 256, &a)
                        .unwrap()
                        .value;
                    assert_eq!(
                        actual,
                        dense_projected_moment(&expected, *x, *z, outcome),
                        "{compiler:?} fault{fx}/{fz},Pauli{x}/{z},outcome{outcome}"
                    );
                    if actual.is_zero() {
                        zero += 1;
                    } else {
                        nonzero += 1;
                    }
                    cases += 1;
                }
            }
        }
        let warm = u.query_work_statistics();
        assert_eq!(preparations(prepared), preparations(warm));
        assert_eq!(
            warm.affine_geometry_preparations,
            prepared.affine_geometry_preparations
        );
        assert_eq!(warm.cold_evaluations, 0);
        drop(queries);
        drop(probability);
        assert_eq!(
            u.query_work_statistics().live_affine_geometries,
            Some(0),
            "weak pool must release unused maps"
        );
        let recreated = u.prepare_probability(&row, &a).unwrap();
        assert_eq!(u.query_work_statistics().affine_geometry_preparations, 2);
        assert_eq!(
            recreated.evaluate(&u, &bits(1, 0), 256, &a).unwrap().value,
            dense_projected_moment(&dense, 0, 0, 0)
        );
    }
    assert_eq!(cases, 1024);
    assert!(zero > 0 && nonzero > 0);
    println!(
        "{cases}exact dense Pauli/RHS/fault cells across2compilers;{zero}zero/{nonzero}nonzero;33closures share1actualsymbolicgeometry perowner,0hotreconstruction;weak lifetime releases maps"
    );
}

#[test]
fn concurrent_scalar_preparations_share_one_live_geometry_and_keep_event_structures_distinct() {
    use std::sync::{Arc, Barrier};
    let a = ScalarArithmetic::default();
    let u = Arc::new(
        compile_unitary(
            &parse_circuit("H 0\nT 0\nH 0\nCX 0 1\nX 1", &ParseOptions::default()).unwrap(),
            &CompileOptions::default(),
            &a,
        )
        .unwrap(),
    );
    let start = Arc::new(Barrier::new(8));
    let plans = std::thread::scope(|scope| {
        let mut jobs = vec![];
        for _ in 0..8 {
            let (u, start) = (Arc::clone(&u), Arc::clone(&start));
            jobs.push(scope.spawn(move || {
                start.wait();
                u.prepare_probability(&[bits(2, 1)], &a).unwrap()
            }));
        }
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    let work = u.query_work_statistics();
    assert_eq!(work.affine_closures_prepared, 8);
    assert!((1..=8).contains(&work.affine_geometry_preparations));
    assert_eq!(work.affine_geometry_reuses, 7);
    assert_eq!(work.live_affine_geometries, Some(1));
    for plan in &plans {
        for bit in 0..2 {
            assert_eq!(
                plan.evaluate(&u, &bits(1, bit), 256, &a).unwrap().value,
                probability(bit == 0)
            );
        }
    }
    let another = u.prepare_probability(&[bits(2, 2)], &a).unwrap();
    assert_eq!(
        another.evaluate(&u, &bits(1, 1), 256, &a).unwrap().value,
        probability(true)
    );
    assert_eq!(u.query_work_statistics().live_affine_geometries, Some(2));
    let before = u.query_work_statistics();
    assert!(u.prepare_probability(&[bits(3, 1)], &a).is_err());
    assert_eq!(u.query_work_statistics(), before);
    drop(plans);
    assert_eq!(u.query_work_statistics().live_affine_geometries, Some(1));
    drop(another);
    assert_eq!(u.query_work_statistics().live_affine_geometries, Some(0));
    println!(
        "8simultaneouspreparations:oneactualretainedgeometry,7actualreuses,{}realconstructions(includingracingmisses);differentrowsretainsecondmap;malformedrowsdo no work;weak release0",
        work.affine_geometry_preparations
    );
}

#[test]
fn explicit_scalar_plan_lru_caps_forty_event_structures_at_thirty_two_and_validates_warm_calls() {
    use rmagna::unitary::{ScalarQuery, ScalarQueryCache, ScalarQueryCacheOptions, UnitaryError};
    let a = ScalarArithmetic::default();
    let circuit = parse_circuit("H 0", &ParseOptions::default()).unwrap();
    let u = compile_unitary(&circuit, &CompileOptions::default(), &a).unwrap();
    let identity = PauliWord::identity(1).unwrap();
    let mut cache = ScalarQueryCache::new(&u, Default::default(), None);
    let mut query = |n| {
        let rows = vec![bits(1, 0); n];
        let rhs = bits(n, 0);
        cache
            .evaluate(
                &u,
                ScalarQuery {
                    events: &rows,
                    outcomes: &rhs,
                    pauli: &identity,
                },
                &a,
            )
            .unwrap()
            .value
    };
    for n in 0..40 {
        assert_eq!(query(n), ExactScalar::one());
    }
    assert_eq!(cache.len(), 32);
    assert_eq!(cache.statistics().preparations, 40);
    assert_eq!(cache.statistics().evictions, 8);
    for (n, expected_hit) in [(8, true), (0, false), (8, true), (9, false)] {
        let rows = vec![bits(1, 0); n];
        let rhs = bits(n, 0);
        let before = cache.statistics();
        assert_eq!(
            cache
                .evaluate(
                    &u,
                    ScalarQuery {
                        events: &rows,
                        outcomes: &rhs,
                        pauli: &identity
                    },
                    &a
                )
                .unwrap()
                .value,
            ExactScalar::one()
        );
        assert_eq!(
            cache.statistics().hits - before.hits,
            usize::from(expected_hit)
        );
        assert_eq!(cache.len(), 32);
    }
    let row = [bits(1, 1)];
    let rhs = bits(1, 0);
    let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
    cache
        .evaluate(
            &u,
            ScalarQuery {
                events: &row,
                outcomes: &rhs,
                pauli: &z,
            },
            &a,
        )
        .unwrap();
    let before = cache.statistics();
    let work = u.query_work_statistics();
    let wrong_rhs = bits(0, 0);
    let wrong_rows = [bits(2, 1)];
    let x = PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap();
    let skew = PauliWord::new(bits(1, 1), bits(1, 1), 0).unwrap();
    for (q, expected) in [
        (
            ScalarQuery {
                events: &row,
                outcomes: &wrong_rhs,
                pauli: &z,
            },
            UnitaryError::DimensionMismatch,
        ),
        (
            ScalarQuery {
                events: &wrong_rows,
                outcomes: &rhs,
                pauli: &z,
            },
            UnitaryError::DimensionMismatch,
        ),
        (
            ScalarQuery {
                events: &row,
                outcomes: &rhs,
                pauli: &x,
            },
            UnitaryError::NonCommutingScalarQuery,
        ),
        (
            ScalarQuery {
                events: &[],
                outcomes: &wrong_rhs,
                pauli: &skew,
            },
            UnitaryError::NonHermitianScalarQuery,
        ),
    ] {
        assert_eq!(cache.evaluate(&u, q, &a).unwrap_err(), expected);
    }
    let foreign = compile_unitary(&circuit, &CompileOptions::default(), &a).unwrap();
    assert_eq!(
        cache.evaluate_many(&foreign, &[], &a).unwrap_err(),
        UnitaryError::IncompatibleProgram
    );
    assert_eq!(cache.statistics(), before);
    assert_eq!(u.query_work_statistics(), work);
    cache.set_max_plans(1);
    assert_eq!(cache.len(), 1);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(u.query_work_statistics().live_affine_geometries, Some(0));
    let options = ScalarQueryCacheOptions {
        max_plans: 0,
        ..Default::default()
    };
    let mut disabled = ScalarQueryCache::new(&u, options, None);
    for _ in 0..2 {
        assert_eq!(
            disabled
                .evaluate(
                    &u,
                    ScalarQuery {
                        events: &[],
                        outcomes: &wrong_rhs,
                        pauli: &identity
                    },
                    &a
                )
                .unwrap()
                .value,
            ExactScalar::one()
        );
    }
    assert!(disabled.is_empty());
    assert_eq!(disabled.statistics().preparations, 2);
    println!(
        "40event structures retain exactly32plans,8initialevictions;LRUtouch8protects8while9evicts;warmmalformed/noncommuting/nonHermitian/foreigncalls do no work;zero capacity prepares real ephemeral plans"
    );
}

#[test]
fn scalar_plan_batch_admission_is_atomic_and_failed_resource_preparation_is_not_cached() {
    use rmagna::unitary::{ScalarQuery, ScalarQueryCache, ScalarQueryCacheOptions, UnitaryError};
    let a = ScalarArithmetic::default();
    let u = compile_unitary(
        &parse_circuit("H 0\nT 0\nH 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let i = PauliWord::identity(1).unwrap();
    let rows = [bits(1, 1)];
    let rhs = bits(1, 0);
    let empty = bits(0, 0);
    let valid = ScalarQuery {
        events: &rows,
        outcomes: &rhs,
        pauli: &i,
    };
    let invalid = ScalarQuery {
        events: &rows,
        outcomes: &empty,
        pauli: &i,
    };
    let mut cache = ScalarQueryCache::new(&u, Default::default(), None);
    let before = u.query_work_statistics();
    assert_eq!(
        cache.evaluate_many(&u, &[valid, invalid], &a).unwrap_err(),
        UnitaryError::DimensionMismatch
    );
    assert_eq!(cache.statistics(), Default::default());
    assert_eq!(u.query_work_statistics(), before);
    for options in [
        ScalarQueryCacheOptions {
            max_geometry_words: 0,
            ..Default::default()
        },
        ScalarQueryCacheOptions {
            max_event_rows: 0,
            ..Default::default()
        },
        ScalarQueryCacheOptions {
            max_batch_queries: 0,
            ..Default::default()
        },
    ] {
        let mut refused = ScalarQueryCache::new(&u, options, None);
        assert!(matches!(
            refused.evaluate_many(&u, &[valid], &a),
            Err(UnitaryError::QueryCacheLimit { .. })
        ));
        assert_eq!(refused.statistics(), Default::default());
        assert_eq!(u.query_work_statistics(), before);
    }
    let mut failed = ScalarQueryCache::new(
        &u,
        ScalarQueryCacheOptions {
            max_terms: 0,
            ..Default::default()
        },
        None,
    );
    assert!(failed.evaluate(&u, valid, &a).is_err());
    assert!(failed.is_empty());
    assert_eq!(failed.statistics().preparation_attempts, 1);
    assert_eq!(failed.statistics().preparations, 0);
    assert_eq!(failed.statistics().evaluations, 0);
    assert_eq!(
        cache.evaluate_many(&u, &[valid], &a).unwrap()[0].value,
        probability(true)
    );
    let opposite = bits(1, 1);
    assert_eq!(
        cache
            .evaluate(
                &u,
                ScalarQuery {
                    outcomes: &opposite,
                    ..valid
                },
                &a
            )
            .unwrap()
            .value,
        probability(false)
    );
    assert_eq!(cache.statistics().preparations, 1);
    assert_eq!(cache.statistics().hits, 1);
    println!(
        "batchshape/row/geometry/request caps refuse before any factory;max_terms0 retains failed attempt without entry;both literalTbranches share1plan with changingRHS"
    );
}

#[test]
fn frozen_lru_plans_preserve_dense_faulted_batches_and_live_outside_eviction_for_threads() {
    use rmagna::unitary::{ScalarQuery, ScalarQueryCache, ScalarQueryCacheOptions};
    use std::sync::Arc;
    let a = ScalarArithmetic::default();
    let circuit = parse_circuit(
        "H 0 1 2\nT 0\nS 1\nCCZ 0 1 2\nCX 0 2\nH 1\nT 1",
        &ParseOptions::default(),
    )
    .unwrap();
    let u = Arc::new(compile_unitary(&circuit, &CompileOptions::default(), &a).unwrap());
    let dense = dense_three_wire_reference();
    let empty = bits(0, 0);
    let words = (0usize..8)
        .flat_map(|x| {
            (0usize..8).map(move |z| {
                (
                    x,
                    z,
                    PauliWord::new(bits(3, x), bits(3, z), (2 * (x & z).count_ones()) as u8)
                        .unwrap(),
                )
            })
        })
        .collect::<Vec<_>>();
    let requests = words
        .iter()
        .map(|(_, _, word)| ScalarQuery {
            events: &[],
            outcomes: &empty,
            pauli: word,
        })
        .collect::<Vec<_>>();
    let mut cache = ScalarQueryCache::new(
        &u,
        ScalarQueryCacheOptions {
            max_plans: 64,
            ..Default::default()
        },
        None,
    );
    let mut owned = vec![];
    for request in &requests {
        owned.push(cache.prepare_query(&u, *request, &a).unwrap());
    }
    let work = u.query_work_statistics();
    assert_eq!(cache.statistics().preparations, 64);
    assert_eq!(cache.statistics().evaluations, 0);
    let mut cases = 0;
    for (fx, fz) in [(0, 0), (1, 0), (2, 2), (0, 4)] {
        // The parsed text expands the initial H instruction to three gates.
        let terminal = circuit
            .operations()
            .iter()
            .filter(|op| matches!(op.operation, rmagna::circuit::Operation::Unitary { .. }))
            .count();
        let fault =
            PauliWord::new(bits(3, fx), bits(3, fz), (2 * (fx & fz).count_ones()) as u8).unwrap();
        let view = u
            .with_insertions(
                &[PauliInsertion {
                    before_gate: terminal,
                    word: fault,
                }],
                &a,
            )
            .unwrap();
        let expected = dense_pauli(&dense, fx, fz);
        let values = cache.evaluate_many(&view, &requests, &a).unwrap();
        for ((x, z, _), value) in words.iter().zip(values) {
            let shifted = dense_pauli(&expected, *x, *z);
            let mut sum = ExactScalar::zero();
            for v in 0..8 {
                sum = a
                    .add(
                        &sum,
                        &a.multiply(&expected[v].conjugate(), &shifted[v]).unwrap(),
                    )
                    .unwrap();
            }
            assert_eq!(value.value, sum);
            cases += 1;
        }
    }
    assert_eq!(preparations(u.query_work_statistics()), preparations(work));
    assert_eq!(cache.statistics().preparations, 64);
    assert_eq!(cache.statistics().evaluations, 256);
    cache.clear();
    assert!(cache.is_empty());
    assert!(
        u.query_work_statistics().live_affine_geometries.unwrap() > 0,
        "external prepared owners survive eviction"
    );
    std::thread::scope(|scope| {
        let mut jobs = vec![];
        for _ in 0..8 {
            let (u, plans, words) = (Arc::clone(&u), &owned, &words);
            jobs.push(scope.spawn(move || {
                for ((_, _, word), plan) in words.iter().zip(plans) {
                    let expected = u
                        .pauli_moment(word, &[], &bits(0, 0), 256, &a)
                        .unwrap()
                        .value;
                    assert_eq!(
                        plan.evaluate(&u, &bits(0, 0), 256, &a).unwrap().value,
                        expected
                    );
                }
            }));
        }
        for job in jobs {
            job.join().unwrap();
        }
    });
    drop(owned);
    assert_eq!(u.query_work_statistics().live_affine_geometries, Some(0));
    assert_eq!(cases, 256);
    println!(
        "64frozen Pauli plans:256independent dense faultedbatch values,0warmfactories;8threads x64numericqueries onownedplans aftercacheclear,independentworkspaces;lastownerrelease0livegeometry"
    );
}

#[test]
fn resolved_instrument_views_share_lru_preparation_but_not_decisions_values_or_new_quotas() {
    use rmagna::{
        circuit::{BitAtom, BitExpr},
        instrument::InstrumentScalarQuery,
        runtime::{CausalProgram, ControllerRegistry, RuntimeOptions},
        unitary::UnitaryError,
    };
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |v: &[bool]| Ok(v.to_vec()))
        .unwrap();
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\nCZ rec[-1] 1\nZ_ERROR(1/4) 1";
    let circuit = parse_circuit(
        source,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let program = CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &a).unwrap();
    let histories = [false, true].map(|value| {
        program
            .resolve_prescribed_history(&[], &[], &[vec![value]], &a)
            .unwrap()
    });
    let mut cache = histories[0]
        .instrument()
        .scalar_query_cache(Default::default(), None);
    let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
    let i = PauliWord::identity(0).unwrap();
    let none = bits(0, 0);
    let row = [BitExpr::atom(BitAtom::Record(0))];
    for (value, h) in [false, true].into_iter().zip(&histories) {
        let actual = h
            .instrument()
            .reported_moment_cached(
                &mut cache,
                InstrumentScalarQuery {
                    outputs: &[1],
                    pauli: &z,
                    parities: &[],
                    outcomes: &none,
                },
                h.faults(),
                &a,
            )
            .unwrap()
            .value;
        assert_eq!(
            actual,
            if value {
                ExactScalar::one().negated()
            } else {
                ExactScalar::one()
            }
        );
        for bit in 0..2 {
            let rhs = bits(1, bit);
            assert_eq!(
                h.instrument()
                    .reported_moment_cached(
                        &mut cache,
                        InstrumentScalarQuery {
                            outputs: &[],
                            pauli: &i,
                            parities: &row,
                            outcomes: &rhs
                        },
                        h.faults(),
                        &a
                    )
                    .unwrap()
                    .value,
                a.omega(0, -2).unwrap()
            );
        }
    }
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.statistics().preparations, 2);
    assert_eq!(cache.statistics().hits, 4);
    assert_eq!(program.reference_compilations(), 1);
    let mut tight = RuntimeOptions::default();
    tight.compile.max_path_bits = 0;
    let error = CausalProgram::new(&circuit, &registry, tight, &a)
        .err()
        .expect("new raw path quota must be admitted");
    assert!(
        matches!(
            error,
            rmagna::runtime::RuntimeError::Instrument(
                rmagna::instrument::InstrumentError::Unitary(UnitaryError::RawPathLimit {
                    limit: 0,
                    ..
                })
            )
        ),
        "{error:?}"
    );
    let mut limited = histories[0].instrument().scalar_query_cache(
        rmagna::unitary::ScalarQueryCacheOptions {
            max_geometry_words: 0,
            ..Default::default()
        },
        None,
    );
    let before_work = histories[0].instrument().query_work_statistics();
    assert!(matches!(
        histories[0].instrument().reported_moment_cached(
            &mut limited,
            InstrumentScalarQuery {
                outputs: &[],
                pauli: &i,
                parities: &row,
                outcomes: &bits(1, 0)
            },
            &[],
            &a
        ),
        Err(rmagna::instrument::InstrumentError::Unitary(
            UnitaryError::QueryCacheLimit {
                option: "scalar_query_cache.max_geometry_words",
                ..
            }
        ))
    ));
    assert_eq!(
        histories[0].instrument().query_work_statistics(),
        before_work
    );
    assert_eq!(limited.statistics(), Default::default());
    let foreign = CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &a)
        .unwrap()
        .resolve_prescribed_history(&[], &[], &[vec![false]], &a)
        .unwrap();
    let before = cache.statistics();
    assert!(matches!(
        foreign.instrument().reported_moment_cached(
            &mut cache,
            InstrumentScalarQuery {
                outputs: &[],
                pauli: &i,
                parities: &row,
                outcomes: &bits(1, 0)
            },
            foreign.faults(),
            &a
        ),
        Err(rmagna::instrument::InstrumentError::Unitary(
            UnitaryError::IncompatibleProgram
        ))
    ));
    assert_eq!(cache.statistics(), before);
    println!(
        "2resolved decision views/6scalar cells share2realLRUplans on1program;unconditionedZ=+/-1,recordmass1/2;newsource/newrawpathquota cannot bypass ownership/admission"
    );
}

#[test]
fn instrument_lru_eviction_keeps_all_sixty_actual_callbacks_guarded_noise_masses_and_rng_words() {
    use rmagna::{
        instrument::InstrumentScalarQuery,
        noise::SplitMix64,
        runtime::{CausalProgram, ControllerRegistry, RuntimeOptions},
        unitary::ScalarQueryCacheOptions,
    };
    use std::sync::{Arc, Mutex};
    let a = ScalarArithmetic::default();
    let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
    let seen = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |v: &[bool]| {
            seen.lock().unwrap().push(v.to_vec());
            Ok(v.to_vec())
        })
        .unwrap();
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\nX_ERROR(1/2) ctrl[0] 1\nM 1";
    let circuit = parse_circuit(
        source,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let program = CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &a).unwrap();
    let target = compile_unitary(
        &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let words = [
        PauliWord::identity(1).unwrap(),
        PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap(),
    ];
    let mut results = vec![];
    for capacity in [4, 1] {
        let calls_before = calls.lock().unwrap().len();
        let mut random = SplitMix64::new(5);
        let mut cache = None;
        let mut transcript = vec![];
        let mut observed_outputs = [false; 2];
        for index in 0..30 {
            let h = program.resolve(&[], &mut random, &a).unwrap();
            let cache = cache.get_or_insert_with(|| {
                h.instrument().scalar_query_cache(
                    ScalarQueryCacheOptions {
                        max_plans: capacity,
                        ..Default::default()
                    },
                    None,
                )
            });
            let parities = h
                .conditions()
                .iter()
                .map(|(p, _)| p.clone())
                .collect::<Vec<_>>();
            let rhs = BitVector::from_indices(
                parities.len(),
                h.conditions()
                    .iter()
                    .enumerate()
                    .filter_map(|(j, (_, v))| v.then_some(j)),
            )
            .unwrap();
            let word = &words[index % 2];
            let actual = h
                .instrument()
                .reported_moment_cached(
                    cache,
                    InstrumentScalarQuery {
                        outputs: &[1],
                        pauli: word,
                        parities: &parities,
                        outcomes: &rhs,
                    },
                    h.faults(),
                    &a,
                )
                .unwrap()
                .value;
            let cold = h
                .instrument()
                .pauli_moment(&[1], word, h.faults(), 256, &a)
                .unwrap()
                .value;
            assert_eq!(actual, cold);
            let masses = h
                .instrument()
                .masses(&[1], &target, h.faults(), 256, &a)
                .unwrap();
            let bit = h.controller_calls()[0].inputs[0];
            let output = bit ^ !h.faults().is_empty();
            observed_outputs[usize::from(output)] = true;
            assert_eq!(*h.history_mass(), a.omega(0, -2).unwrap());
            assert_eq!(masses.accepted.value, *h.history_mass());
            assert_eq!(
                masses.error,
                if output {
                    h.history_mass().clone()
                } else {
                    ExactScalar::zero()
                }
            );
            assert_eq!(h.network_compilations(), 0);
            assert!(cache.len() <= capacity);
            transcript.push((
                h.history_mass().clone(),
                h.noise_probability().clone(),
                h.noise_likelihood_ratio().clone(),
                h.controller_calls().to_vec(),
                h.entropy_words(),
                random.state(),
                actual,
                masses.error,
            ));
        }
        assert_eq!(calls.lock().unwrap().len() - calls_before, 30);
        assert_eq!(observed_outputs, [true, true]);
        let statistics = cache.unwrap().statistics();
        assert_eq!(statistics.preparations, if capacity == 1 { 30 } else { 2 });
        assert_eq!(statistics.evictions, if capacity == 1 { 29 } else { 0 });
        results.push(transcript);
    }
    assert_eq!(results[0], results[1]);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 60);
    assert!(calls.iter().any(|v| v == &[false]));
    assert!(calls.iter().any(|v| v == &[true]));
    println!(
        "30draws each atLRU4/LRU1:60actualcallbacks;2vs30realpreparations/0vs29evictions;entirephysicalmass/noise/entropy/RNG/controller/error transcriptidentical;literal conditional output oracle agrees"
    );
}

#[test]
fn affine_probe_refuses_biased_t_readout_without_building_a_phase_resource() {
    use rmagna::{
        clifford::CliffordCircuit,
        instrument::*,
        phase::{NativePhaseOptions, PhaseError},
        unitary::{CompilerStrategy, UnitaryError},
    };
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for kind in ["uniform", "biased", "deterministic"] {
            let mut ops = vec![];
            if kind != "deterministic" {
                ops.push(original_op("h", OriginalGate::H(0)));
            }
            if kind == "biased" {
                ops.extend([
                    original_op(
                        "t",
                        OriginalGate::Phase {
                            wires: vec![0],
                            turns: 1,
                        },
                    ),
                    original_op("h2", OriginalGate::H(0)),
                ]);
            }
            ops.extend([
                original_op("anchor", OriginalGate::Identity(vec![0])),
                original_op("m", original_z("r", 0)),
            ]);
            let instrument = OriginalInstrument::new(
                vec![0],
                vec![0],
                CliffordCircuit::computational(bits(1, 0)).unwrap(),
                ops,
                vec![],
                OriginalLimits::default(),
                &a,
            )
            .unwrap();
            let trace = instrument.compile_trace(compiler, &a).unwrap();
            let mut native = NativePhaseOptions::default();
            native.selection.max_quotient_rank = 0;
            let options = OriginalNetworkOptions {
                resource: OriginalNetworkResource::Native(native),
                max_terms: 256,
                ..Default::default()
            };
            for fault in [false, true] {
                for report in [false, true] {
                    for inverted in [false, true] {
                        let view = trace
                            .with_assignment(
                                &OriginalTraceAssignment {
                                    faults: if fault {
                                        vec![OriginalFault {
                                            after: "anchor".into(),
                                            wire: 0,
                                            pauli: OriginalPauli::X,
                                        }]
                                    } else {
                                        vec![]
                                    },
                                    report_flips: if report { vec!["r".into()] } else { vec![] },
                                },
                                &a,
                            )
                            .unwrap();
                        let keep = [OriginalRecordParity {
                            records: vec!["r".into()],
                            inverted,
                        }];
                        let before = trace.query_work_statistics();
                        let result = view
                            .probe_affine_records(&keep, &options, 64 << 20, &a)
                            .unwrap();
                        let after = trace.query_work_statistics();
                        assert_eq!(
                            after.phase_resources_prepared,
                            before.phase_resources_prepared
                        );
                        assert_eq!(
                            after.support_phase_resources_prepared,
                            before.support_phase_resources_prepared
                        );
                        assert_eq!(
                            after.support_parents_prepared,
                            before.support_parents_prepared
                        );
                        assert_eq!(
                            after.support_projections_prepared,
                            before.support_projections_prepared + 1
                        );
                        assert_eq!(after.live_affine_geometries, Some(0));
                        if kind == "biased" {
                            assert!(result.is_none());
                            assert!(
                                view.prepare_record_probabilities(&keep, &options, &a)
                                    .is_err()
                            );
                        } else {
                            let result = result.expect("direct Clifford probability certificate");
                            assert_eq!(result.total_mass(), &ExactScalar::one());
                            if kind == "uniform" {
                                assert_eq!(result.support().dimension(), 1);
                                assert_eq!(result.mass_per_label(), &a.omega(0, -2).unwrap());
                            } else {
                                assert_eq!(result.support().dimension(), 0);
                                assert_eq!(
                                    result.support().offset(),
                                    &bits(1, usize::from(fault ^ report ^ inverted))
                                );
                                assert_eq!(result.mass_per_label(), &ExactScalar::one());
                            }
                        }
                        let before = trace.query_work_statistics();
                        let mut bounded = options;
                        bounded.limits.max_geometry_words = 0;
                        assert!(matches!(
                            view.probe_affine_records(&keep, &bounded, 64 << 20, &a),
                            Err(OriginalError::ResourceLimit {
                                option: "max_network_geometry_words",
                                ..
                            })
                        ));
                        let unknown = [OriginalRecordParity {
                            records: vec!["unknown".into()],
                            inverted: false,
                        }];
                        assert!(
                            view.probe_affine_records(&unknown, &options, 64 << 20, &a)
                                .is_err()
                        );
                        assert_eq!(
                            preparations(trace.query_work_statistics()),
                            preparations(before)
                        );
                        if kind == "biased" {
                            assert!(matches!(
                                view.probe_affine_records(&keep, &options, 0, &a),
                                Err(OriginalError::Unitary(UnitaryError::Phase(
                                    PhaseError::ResourceLimit {
                                        option: "affine_probe.max_signature_work",
                                        ..
                                    }
                                )))
                            ));
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 48);
    println!(
        "48 affine probes: exact source HTH readout refuses certification and ordinary rank-zero resource query; Clifford laws preserve actual faults/report offsets; zero resource constructions and explicit geometry/signature admission"
    );
}

#[path = "support/fixed_reference_replay.rs"]
mod replay;

#[path = "support/fixed_reference_networks.rs"]
mod networks;

#[path = "support/fixed_reference_tasks.rs"]
mod source_tasks;

#[test]
fn prepared_continuations_reuse_frozen_source_frames_and_phase_maps() {
    continuation_cases::warm_templates();
}

#[test]
fn full_original_continuation_generators_match_fresh_compilation_and_output_orders() {
    continuation_cases::all_source_generators();
}

#[test]
fn continuations_keep_mixed_compiler_magic_axes_growth_and_global_phases() {
    continuation_cases::mixed_magic_compilers();
}

#[test]
fn complete_incremental_history_stream_matches_preoptimization_baselines_and_real_work_budgets() {
    replay::complete_incremental_history_stream_matches_preoptimization_baselines_and_real_work_budgets();
}

#[test]
fn full_shared_effect_event_and_fault_fixture_matches_every_independent_fixed_record_sum() {
    networks::full_shared_effect_event_and_fault_fixture_matches_every_independent_fixed_record_sum(
    );
}

#[test]
fn original_ccz_destructive_trace_reuses_all_fault_rhs_effects_and_threaded_batches() {
    networks::original_ccz_destructive_trace_reuses_all_fault_rhs_effects_and_threaded_batches();
}

#[test]
fn exact_source_nonzero_constrained_domains_reuse_fixed_factories_and_preserve_biased_weights() {
    source_tasks::exact_source_nonzero_constrained_domains_reuse_fixed_factories_and_preserve_biased_weights();
}

#[test]
fn original_three_bit_selection_has_literal_joint_masses_and_exact_three_quarter_one_quarter_totals()
 {
    source_tasks::original_three_bit_selection_has_literal_joint_masses_and_exact_three_quarter_one_quarter_totals();
}

#[test]
fn twelve_redundant_decoded_labels_enumerate_two_histories_under_one_independent_bit() {
    source_tasks::twelve_redundant_decoded_labels_enumerate_two_histories_under_one_independent_bit(
    );
}

#[test]
fn original_affine_tasks_have_only_terminal_roles_and_batch_target_reuse_preserves_the_word_stream()
{
    source_tasks::original_affine_tasks_have_only_terminal_roles_and_batch_target_reuse_preserves_the_word_stream();
}

#[test]
fn completed_source_assignment_feeds_acceptance_target_and_all_paulis_once() {
    source_tasks::completed_source_assignment_feeds_acceptance_target_and_all_paulis_once();
}

#[test]
fn original_eager_lazy_fault_report_event_and_pauli_cells() {
    source_tasks::original_eager_lazy_fault_report_event_and_pauli_cells();
}

#[test]
fn original_protocol_recovery_prepares_before_callbacks_and_captures_preparation_failure() {
    source_tasks::original_protocol_recovery_prepares_before_callbacks_and_captures_preparation_failure();
}

#[test]
fn automatic_lru_bounds_expired_geometry_slots_and_capacity_over_distinct_layouts() {
    use rmagna::{
        circuit::{BitAtom, BitExpr},
        instrument::{QueryCachePolicy, QueryPreparation, compile_affine},
    };
    let a = ScalarArithmetic::default();
    let circuit =
        parse_circuit("H 0 1 2 3 4 5 6 7\nM 0 1 2 3 4 5 6 7", &Default::default()).unwrap();
    for compiler in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        let instrument = compile_affine(
            &circuit,
            &CompileOptions {
                query_cache_policy: QueryCachePolicy::Lru,
                max_prepared_probabilities: 1,
                probability_preparation: QueryPreparation::Native(Default::default()),
                compiler_strategy: compiler,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        for code in 1..=1024 {
            let mask = 1 + (code - 1) % 255;
            let parity = BitExpr {
                constant: false,
                terms: (0..8)
                    .filter(|q| mask & (1 << q) != 0)
                    .map(BitAtom::Record)
                    .collect(),
            };
            let result = instrument
                .reported_probability(&[parity], &bits(1, code & 1), &[], 256, &a)
                .unwrap();
            assert_eq!(result.value, a.omega(0, -2).unwrap());
            let plans = instrument.prepared_probability_statistics().unwrap();
            assert_eq!(plans.cached_queries, 1);
            assert_eq!(plans.fixed_resources, 1);
            assert_eq!(plans.cache_evictions, code - 1);
            let retained = instrument.reference_storage_statistics().unwrap();
            assert_eq!(retained.live_geometries, 1);
            assert!(retained.geometry_weak_slots <= 2, "{retained:?}");
            assert!(retained.geometry_slot_capacity <= 4, "{retained:?}");
        }
    }
    println!(
        "2048 true LRU layout changes:255 distinct event parities, every exact mass1/2,one retained resource,at most2 weak slots and4 allocated slot capacity; both compilers"
    );
}

#[test]
fn original_prepared_record_entries_transport_both_irrational_event_masses() {
    use rmagna::{clifford::CliffordCircuit, instrument::*};
    let a = ScalarArithmetic::default();
    let instrument = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![
            original_op("h0", OriginalGate::H(0)),
            original_op(
                "t",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            original_op("h1", OriginalGate::H(0)),
            original_op("m", original_z("m", 0)),
        ],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let rows = [OriginalRecordParity {
        records: vec!["m".into()],
        inverted: false,
    }];
    let options = OriginalNetworkOptions {
        max_terms: 256,
        ..Default::default()
    };
    let mut cases = 0;
    for compiler in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        let trace = instrument.compile_trace(compiler, &a).unwrap();
        let event = trace
            .prepare_network_probability(&rows, &options, &a)
            .unwrap();
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        let entries = view
            .prepare_record_probabilities(&rows, &options, &a)
            .unwrap();
        assert!(event.resource().num_terms() > 0);
        assert!(entries.phase_resource().num_terms() > 0);
        let before = trace.query_work_statistics();
        for _ in 0..8 {
            let tensor = entries.evaluate(&a).unwrap();
            for bit in 0..2 {
                let value = tensor
                    .terms()
                    .iter()
                    .try_fold(ExactScalar::zero(), |sum, term| {
                        a.add(
                            &sum,
                            &term
                                .prepare()
                                .unwrap()
                                .amplitude(&bits(1, bit), &a)
                                .unwrap(),
                        )
                    })
                    .unwrap();
                assert_eq!(value, probability(bit == 0));
            }
            for flip in [false, true] {
                let view = trace
                    .with_assignment(
                        &OriginalTraceAssignment {
                            report_flips: if flip { vec!["m".into()] } else { vec![] },
                            ..Default::default()
                        },
                        &a,
                    )
                    .unwrap();
                for bit in 0..2 {
                    assert_eq!(
                        event.evaluate(&view, &bits(1, bit), &a).unwrap().value,
                        probability((bit != 0) == flip)
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(
            preparations(trace.query_work_statistics()),
            preparations(before)
        );
    }
    assert_eq!(cases, 64);
    println!(
        "64 original H,T,H,M event/RHS/report-flip queries plus16retained tensor evaluations: exact(2+/-sqrt2)/4 and no new scalar/labelled maps/resources"
    );
}

#[test]
fn original_parity_feedback_uses_all_eight_manual_report_offsets_without_reconstructing_query_families()
 {
    source_tasks::original_parity_feedback_uses_all_eight_manual_report_offsets_without_reconstructing_query_families();
}
