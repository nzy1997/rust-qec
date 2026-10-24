use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentError, QueryPreparation, compile_affine,
};
use rmagna::pauli::PauliWord;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, TaskRunOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{path::Path, sync::Arc};
fn word(width: usize, x: usize, z: usize, phase: u8) -> PauliWord {
    let bits =
        |m: usize| BitVector::from_indices(width, (0..width).filter(|j| m >> j & 1 != 0)).unwrap();
    PauliWord::new(bits(x), bits(z), phase).unwrap()
}
#[test]
fn cached_projected_complex_moments_keep_phases_faults_and_output_maps() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nT 0\nH 0\nCX 0 1\nM(1/7) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nX_ERROR(1/4) 0",
        &ParseOptions::default(),
    )
    .unwrap();
    let accepted = a
        .add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
        .unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for mode in [
            QueryPreparation::Cold,
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(Default::default()),
        ] {
            let p = compile_affine(
                &source,
                &CompileOptions {
                    compiler_strategy: strategy,
                    moment_preparation: mode,
                    max_prepared_moments: 8,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            let site = |denominator: i64| {
                p.noise_sites()
                    .iter()
                    .position(|s| {
                        s.alternatives().iter().any(|alt| {
                            alt.probability().value()
                                == &num_rational::BigRational::new(1.into(), denominator.into())
                        })
                    })
                    .unwrap()
            };
            let report = site(7_i64);
            let physical = site(4_i64);
            assert_ne!(report, physical);
            let fault = |site| FaultSelection {
                site,
                alternative: 0,
            };
            for _ in 0..2 {
                for faults in [
                    vec![],
                    vec![fault(physical)],
                    vec![fault(report)],
                    vec![fault(physical), fault(report)],
                ] {
                    for (x, z, phase) in [
                        (0, 1, 0),
                        (0, 1, 4),
                        (0, 0, 0),
                        (1, 0, 0),
                        (1, 1, 2),
                        (0, 1, 1),
                    ] {
                        let report_flipped = faults.iter().any(|f| f.site == report);
                        let physical_flipped = faults.iter().any(|f| f.site == physical);
                        let mass = if report_flipped {
                            a.subtract(&ExactScalar::one(), &accepted).unwrap()
                        } else {
                            accepted.clone()
                        };
                        let mut expected = if x != 0 {
                            ExactScalar::zero()
                        } else {
                            a.multiply(&mass, &a.omega(i64::from(phase), 0).unwrap())
                                .unwrap()
                        };
                        if z != 0 && (report_flipped ^ physical_flipped) {
                            expected = expected.negated();
                        }
                        assert_eq!(
                            p.pauli_moment(&[0], &word(1, x, z, phase), &faults, 256, &a)
                                .unwrap()
                                .value,
                            expected,
                            "strategy={strategy:?} mode={mode:?} X{x}Z{z}phase{phase}faults={faults:?}"
                        );
                    }
                }
            }
            let stats = p.prepared_moment_statistics().unwrap();
            assert_eq!(
                stats.cached_queries,
                if matches!(mode, QueryPreparation::Cold) {
                    0
                } else {
                    6
                }
            );
            assert_eq!(
                stats.fixed_resources,
                if matches!(mode, QueryPreparation::FixedFibers(_)) {
                    6
                } else {
                    0
                }
            );
        }
    }
    let p = compile_affine(
        &parse_circuit("H 0\nT 0\nH 0\nI 1", &ParseOptions::default()).unwrap(),
        &CompileOptions {
            moment_preparation: QueryPreparation::Trace,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let z = word(2, 0, 1, 0);
    assert_eq!(
        p.pauli_moment(&[0, 1], &z, &[], 256, &a).unwrap().value,
        a.omega(0, -1).unwrap()
    );
    assert_eq!(
        p.pauli_moment(&[1, 0], &z, &[], 256, &a).unwrap().value,
        ExactScalar::one()
    );
    assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 2);
    assert!(matches!(
        p.pauli_moment(&[0, 0], &z, &[], 256, &a),
        Err(InstrumentError::Unitary(_))
    ));
    assert!(matches!(
        p.pauli_moment(&[2], &word(1, 0, 1, 0), &[], 256, &a),
        Err(InstrumentError::QubitOutOfBounds { .. })
    ));
    assert!(matches!(
        p.pauli_moment(&[0], &z, &[], 256, &a),
        Err(InstrumentError::Unitary(_))
    ));
    assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 2);
}
#[test]
fn cache_caps_term_failures_and_retries_never_fallback_or_retain_failed_plans() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nT 0\nH 0", &ParseOptions::default()).unwrap();
    let options = CompileOptions {
        moment_preparation: QueryPreparation::FixedFibers(Default::default()),
        max_prepared_moments: 1,
        ..Default::default()
    };
    let p = compile_affine(&source, &options, &a).unwrap();
    let z = word(1, 0, 1, 0);
    assert!(p.pauli_moment(&[0], &z, &[], 0, &a).is_err());
    assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 0);
    assert_eq!(
        p.pauli_moment(&[0], &z, &[], 256, &a).unwrap().value,
        a.omega(0, -1).unwrap()
    );
    assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 1);
    assert!(p.pauli_moment(&[0], &z, &[], 0, &a).is_err());
    assert!(matches!(
        p.pauli_moment(&[0], &word(1, 0, 0, 0), &[], 256, &a),
        Err(InstrumentError::ResourceLimit {
            option: "max_prepared_moments",
            limit: 1,
            required: 2
        })
    ));
    assert_eq!(
        p.pauli_moment(&[0], &z, &[], 256, &a).unwrap().value,
        a.omega(0, -1).unwrap()
    );
    assert_eq!(p.prepared_moment_statistics().unwrap().fixed_resources, 1);
    assert!(matches!(
        compile_affine(
            &source,
            &CompileOptions {
                max_prepared_moments: 0,
                ..options
            },
            &a
        ),
        Err(InstrumentError::ResourceLimit {
            option: "max_prepared_moments",
            limit: 0,
            required: 1
        })
    ));
    let cold = compile_affine(
        &source,
        &CompileOptions {
            moment_preparation: QueryPreparation::Cold,
            max_prepared_moments: 0,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    assert_eq!(
        cold.pauli_moment(&[0], &z, &[], 256, &a).unwrap().value,
        a.omega(0, -1).unwrap()
    );
    assert_eq!(cold.prepared_moment_statistics().unwrap().cached_queries, 0);
}
#[test]
fn concurrent_clones_share_one_immutable_family_and_survive_source_drop() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nT 0\nH 0", &ParseOptions::default()).unwrap();
    let p = Arc::new(
        compile_affine(
            &source,
            &CompileOptions {
                moment_preparation: QueryPreparation::FixedFibers(Default::default()),
                max_prepared_moments: 1,
                ..Default::default()
            },
            &a,
        )
        .unwrap(),
    );
    let handles = (0..8)
        .map(|_| {
            let p = p.clone();
            std::thread::spawn(move || {
                p.pauli_moment(
                    &[0],
                    &word(1, 0, 1, 0),
                    &[],
                    256,
                    &ScalarArithmetic::default(),
                )
                .unwrap()
                .value
            })
        })
        .collect::<Vec<_>>();
    for h in handles {
        assert_eq!(h.join().unwrap(), a.omega(0, -1).unwrap());
    }
    assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 1);
    let retained = (*p).clone();
    drop(p);
    drop(source);
    assert_eq!(
        retained
            .pauli_moment(&[0], &word(1, 0, 1, 0), &[], 256, &a)
            .unwrap()
            .value,
        a.omega(0, -1).unwrap()
    );
    assert_eq!(
        retained
            .prepared_moment_statistics()
            .unwrap()
            .fixed_resources,
        1
    );
}
#[test]
fn causal_conditioned_moments_reuse_fixed_reference_and_keep_seeded_statistics() {
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let source = parse_circuit(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1",
        &parse,
    )
    .unwrap();
    let modules = parse_task(
        r#"{"outputs":[1],"sample_records":[0],"observables":[[0,1]]}"#,
        Path::new("/tmp"),
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&source, &parse)
    .unwrap();
    let run = TaskRunOptions {
        shots: 24,
        seed: 91,
        ..Default::default()
    };
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let mut baseline = None;
        for mode in [
            QueryPreparation::Cold,
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(Default::default()),
        ] {
            let mut options = EvaluationOptions::default();
            options.runtime.strategy = strategy;
            options.runtime.compile.moment_preparation = mode;
            options.runtime.compile.max_prepared_moments = 1;
            let evaluator = TaskEvaluator::new(&source, &modules, &registry, options).unwrap();
            let result = evaluator.run_batch(&run).unwrap();
            let records = result.sampled_records.as_ref().unwrap();
            assert_eq!(records.len(), 24);
            let ones = records.iter().filter(|bits| bits[0]).count() as i64;
            let expected = rmagna::field::FieldArithmetic::default()
                .from_rational(num_rational::BigRational::new(
                    (24 - 2 * ones).into(),
                    24.into(),
                ))
                .unwrap();
            assert_eq!(result.statistics.as_ref().unwrap().moments, [expected]);
            assert!(result.statistics.as_ref().unwrap().error.is_none());
            if let Some((stats, records)) = &baseline {
                assert_eq!(&result.statistics, stats);
                assert_eq!(&result.sampled_records, records);
            } else {
                baseline = Some((result.statistics.clone(), result.sampled_records.clone()));
            }
            let stats = evaluator.prepared_moment_statistics().unwrap();
            if strategy == CausalStrategy::ColdPrefix {
                assert!(stats.is_none());
            } else {
                let stats = stats.unwrap();
                assert_eq!(
                    stats.cached_queries,
                    usize::from(!matches!(mode, QueryPreparation::Cold))
                );
                assert_eq!(
                    stats.fixed_resources,
                    usize::from(matches!(mode, QueryPreparation::FixedFibers(_)))
                );
            }
        }
    }
}
