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
