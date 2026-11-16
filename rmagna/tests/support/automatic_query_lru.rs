//! Automatic runtime caches: true retention, validation and transactional misses.
use super::*;
use rmagna::instrument::QueryCachePolicy;

fn stats(
    p: &rmagna::instrument::CompiledInstrument,
) -> [rmagna::instrument::PreparedQueryStatistics; 3] {
    [
        p.prepared_probability_statistics().unwrap(),
        p.prepared_density_statistics().unwrap(),
        p.prepared_moment_statistics().unwrap(),
    ]
}
#[test]
fn automatic_lru_evicts_each_real_family_and_failed_construction_preserves_the_good_entry() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for mode in [
            QueryPreparation::FixedFibers(Default::default()),
            QueryPreparation::FixedCatalogue(Default::default()),
            QueryPreparation::Native(Default::default()),
        ] {
            let mut config = options(mode);
            config.compiler_strategy = compiler;
            config.query_cache_policy = QueryCachePolicy::Lru;
            config.max_prepared_probabilities = 1;
            config.max_prepared_densities = 1;
            config.max_prepared_moments = 1;
            let p = compile_affine(&source(), &config, &a).unwrap();
            let target = compile_unitary(
                &parse_circuit("I 0 1", &ParseOptions::default()).unwrap(),
                &config,
                &a,
            )
            .unwrap();
            let record = [BitExpr::atom(BitAtom::Record(0))];
            let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
            let x = PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap();
            let target_layout = DensityLayout {
                outputs: &[0],
                event_rows: &[],
            };
            let first = [
                Q::ReportedProbability { parities: &[] },
                Q::TargetDensity {
                    outputs: &[0],
                    target: &target,
                    target_layout,
                },
                Q::PauliMoment {
                    outputs: &[0],
                    word: &z,
                },
            ];
            let second = [
                Q::ReportedProbability { parities: &record },
                Q::TargetDensity {
                    outputs: &[1],
                    target: &target,
                    target_layout,
                },
                Q::PauliMoment {
                    outputs: &[0],
                    word: &x,
                },
            ];
            for q in first {
                assert_eq!(p.prepare_query(q, 256, &a).unwrap(), O::Created);
            }
            let before = stats(&p);
            for q in second {
                assert!(p.prepare_query(q, 0, &a).is_err());
            }
            assert_eq!(
                stats(&p),
                before,
                "a failed factory must not evict a usable plan"
            );
            for q in first {
                assert_eq!(p.prepare_query(q, 256, &a).unwrap(), O::Reused);
            }
            for q in second {
                assert_eq!(p.prepare_query(q, 256, &a).unwrap(), O::Created);
            }
            for s in stats(&p) {
                assert_eq!((s.cached_queries, s.cache_evictions), (1, 1));
            }
            // Reconstruct evicted layouts rather than returning another layout's value.
            for q in first {
                assert_eq!(p.prepare_query(q, 256, &a).unwrap(), O::Created);
            }
            for s in stats(&p) {
                assert_eq!((s.cached_queries, s.cache_evictions), (1, 2));
            }
            assert_eq!(
                p.reported_probability(&[], &bits(0, 0), &[], 256, &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
        }
    }
}
#[test]
fn malformed_warm_rhs_cannot_change_probability_lru_recency() {
    let a = ScalarArithmetic::default();
    for mode in [
        QueryPreparation::Trace,
        QueryPreparation::FixedFibers(Default::default()),
    ] {
        let mut config = options(mode);
        config.query_cache_policy = QueryCachePolicy::Lru;
        config.max_prepared_probabilities = 2;
        let p = compile_affine(&source(), &config, &a).unwrap();
        let r = BitExpr::atom(BitAtom::Record(0));
        let b = [r.clone()];
        let c = [r.clone(), r];
        for rows in [&[][..], &b] {
            p.prepare_query(Q::ReportedProbability { parities: rows }, 256, &a)
                .unwrap();
        }
        p.reported_probability(&[], &bits(0, 0), &[], 256, &a)
            .unwrap();
        let before = p.prepared_probability_statistics().unwrap();
        for malformed in [bits(0, 0), bits(2, 0)] {
            assert!(
                p.reported_probability(&b, &malformed, &[], 256, &a)
                    .is_err()
            );
        }
        assert_eq!(p.prepared_probability_statistics().unwrap(), before);
        assert_eq!(
            p.prepare_query(Q::ReportedProbability { parities: &c }, 256, &a)
                .unwrap(),
            O::Created
        );
        assert_eq!(
            p.prepare_query(Q::ReportedProbability { parities: &[] }, 256, &a)
                .unwrap(),
            O::Reused
        );
        assert_eq!(
            p.prepare_query(Q::ReportedProbability { parities: &b }, 256, &a)
                .unwrap(),
            O::Created
        );
        assert_eq!(
            p.prepared_probability_statistics().unwrap().cache_evictions,
            2
        );
    }
}
#[test]
fn concurrent_automatic_eviction_keeps_returned_plans_valid_and_disabled_families_stay_cold() {
    let a = ScalarArithmetic::default();
    let mut config = options(QueryPreparation::FixedFibers(Default::default()));
    config.query_cache_policy = QueryCachePolicy::Lru;
    config.max_prepared_probabilities = 1;
    let p = compile_affine(&source(), &config, &a).unwrap();
    std::thread::scope(|scope| {
        let mut jobs = vec![];
        for j in 0..8 {
            let p = &p;
            let a = &a;
            jobs.push(scope.spawn(move || {
                for width in 0..8 {
                    let r = vec![BitExpr::atom(BitAtom::Record(0)); width];
                    let bit = j % 2;
                    let value = p
                        .reported_probability(
                            &r,
                            &BitVector::from_indices(width, (0..width).filter(|_| bit == 1))
                                .unwrap(),
                            &[],
                            256,
                            a,
                        )
                        .unwrap()
                        .value;
                    if width == 0 {
                        assert_eq!(value, ExactScalar::one());
                    } else {
                        // Literal H-T-H readout law, unchanged by duplicate rows.
                        let p0 = a
                            .add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
                            .unwrap();
                        assert_eq!(
                            value,
                            if bit == 0 {
                                p0
                            } else {
                                a.subtract(&ExactScalar::one(), &p0).unwrap()
                            }
                        );
                    }
                }
            }));
        }
        for job in jobs {
            job.join().unwrap();
        }
    });
    let s = p.prepared_probability_statistics().unwrap();
    assert_eq!(s.cached_queries, 1);
    assert!(s.cache_evictions >= 7);
    for policy in [QueryCachePolicy::Quota, QueryCachePolicy::Lru] {
        let mut disabled = CompileOptions {
            query_cache_policy: policy,
            max_prepared_probabilities: 0,
            max_prepared_densities: 0,
            max_prepared_moments: 0,
            ..Default::default()
        };
        assert!(compile_affine(&source(), &disabled, &a).is_ok());
        for family in 0..3 {
            disabled.probability_preparation = QueryPreparation::Cold;
            disabled.density_preparation = QueryPreparation::Cold;
            disabled.moment_preparation = QueryPreparation::Cold;
            match family {
                0 => disabled.probability_preparation = config.probability_preparation,
                1 => disabled.density_preparation = config.density_preparation,
                _ => disabled.moment_preparation = config.moment_preparation,
            };
            assert!(compile_affine(&source(), &disabled, &a).is_err());
        }
    }
}
