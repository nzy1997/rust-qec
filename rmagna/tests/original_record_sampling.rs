// SPDX-License-Identifier: MPL-2.0
// Complete joint-parity scenario from itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_boundary_sampling.py::test_commuting_kernel_includes_joint_parities.
// Literal exact masses and Boolean-stream controls are independent.
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalGate as G, OriginalInstrument, OriginalLimits, OriginalNetworkOptions,
    OriginalNetworkResource, OriginalOperation, OriginalRecordDrawOptions, OriginalRecordParity,
    OriginalRecordProbabilities, OriginalRecordSamplingError as Error,
    OriginalRecordSamplingLimits as Limits, OriginalTraceAssignment, ReadoutBasis,
};
use rmagna::phase::NativePhaseOptions;
use rmagna::quadratic::QuadraticForm;
use rmagna::records::BitSamplingError;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;

fn bits(width: usize, word: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|&j| word >> j & 1 != 0),
    )
    .unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn instrument(plus: bool, separate: bool, turns: i64, a: &ScalarArithmetic) -> OriginalInstrument {
    let input = AffineKet::new(
        bits(2, 0),
        if plus {
            vec![bits(2, 1), bits(2, 2)]
        } else {
            vec![]
        },
        QuadraticForm::new(
            0,
            vec![0; if plus { 2 } else { 0 }],
            vec![bits(if plus { 2 } else { 0 }, 0); if plus { 2 } else { 0 }],
        )
        .unwrap(),
        if plus {
            a.omega(0, -2).unwrap()
        } else {
            ExactScalar::one()
        },
    )
    .unwrap();
    let mut ops = if separate {
        (0..2)
            .map(|q| OriginalOperation {
                id: format!("t{q}"),
                gate: G::Phase {
                    wires: vec![q],
                    turns,
                },
            })
            .collect::<Vec<_>>()
    } else {
        vec![OriginalOperation {
            id: "t".into(),
            gate: G::Phase {
                wires: vec![0, 1],
                turns,
            },
        }]
    };
    for (wire, record) in [(0, "a"), (1, "b")] {
        ops.push(OriginalOperation {
            id: record.into(),
            gate: G::Destructive {
                wire,
                basis: ReadoutBasis::X,
                record: record.into(),
            },
        });
    }
    OriginalInstrument::new(
        vec![0, 1],
        vec![],
        input,
        ops,
        vec![],
        OriginalLimits::default(),
        a,
    )
    .unwrap()
}
fn rows() -> Vec<OriginalRecordParity> {
    ["a", "b"]
        .map(|r| OriginalRecordParity {
            records: vec![r.into()],
            inverted: false,
        })
        .to_vec()
}
fn law_using(
    ir: &OriginalInstrument,
    strategy: CompilerStrategy,
    keep: &[OriginalRecordParity],
    flips: Vec<String>,
    native: bool,
    a: &ScalarArithmetic,
) -> OriginalRecordProbabilities {
    let source = ir.compile_trace(strategy, a).unwrap();
    let view = source
        .with_assignment(
            &OriginalTraceAssignment {
                report_flips: flips,
                ..Default::default()
            },
            a,
        )
        .unwrap();
    let mut options = OriginalNetworkOptions::default();
    if native {
        let mut resource = NativePhaseOptions::default();
        resource.selection.max_quotient_rank = 4;
        options.resource = OriginalNetworkResource::Native(resource);
        options.max_terms = 16;
    }
    view.prepare_record_probabilities(keep, &options, a)
        .unwrap()
        .evaluate(a)
        .unwrap()
}
fn law(
    ir: &OriginalInstrument,
    strategy: CompilerStrategy,
    keep: &[OriginalRecordParity],
    flips: Vec<String>,
    a: &ScalarArithmetic,
) -> OriginalRecordProbabilities {
    law_using(ir, strategy, keep, flips, true, a)
}
fn literal_masses(a: &ScalarArithmetic) -> [ExactScalar; 4] {
    [
        a.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
            .unwrap(),
        ExactScalar::zero(),
        ExactScalar::zero(),
        a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
            .unwrap(),
    ]
}
fn value(
    tensor: &OriginalRecordProbabilities,
    label: &BitVector,
    a: &ScalarArithmetic,
) -> ExactScalar {
    tensor
        .terms()
        .iter()
        .fold(ExactScalar::zero(), |sum, term| {
            a.add(&sum, &term.prepare().unwrap().amplitude(label, a).unwrap())
                .unwrap()
        })
}

#[test]
fn original_commuting_kernel_joint_parities_sample_exact_retained_masses() {
    let a = ScalarArithmetic::default();
    let expected = literal_masses(&a);
    for strategy in strategies() {
        for (native, term_count) in [(true, 4), (false, 2)] {
            let tensor = law_using(
                &instrument(true, false, 1, &a),
                strategy,
                &rows(),
                vec![],
                native,
                &a,
            );
            let before = tensor.terms().to_vec();
            assert_eq!(before.len(), term_count);
            assert!(tensor.uniform_affine(&a).unwrap().is_none());
            for (word, mass) in expected.iter().enumerate() {
                assert_eq!(&value(&tensor, &bits(2, word), &a), mass);
            }
            let plan = tensor
                .prepare_affine_sampling(Limits::default(), &a)
                .unwrap();
            assert_eq!(plan.support_bound().offset(), &bits(2, 0));
            assert_eq!(plan.support_bound().columns(), &[bits(2, 3)]);
            assert_eq!(plan.support_bound().dimension(), 1);
            assert_eq!(plan.total_mass(), &ExactScalar::one());
            assert_eq!(plan.statistics().source_terms, term_count);
            let mut observed = [false; 4];
            for prefix in 0..16 {
                let mut calls = 0;
                let mut stream = || {
                    let bit = calls < 4 && prefix >> calls & 1 != 0;
                    calls += 1;
                    bit
                };
                let draw = plan
                    .sample_bits(OriginalRecordDrawOptions::default(), &mut stream, &a)
                    .unwrap();
                let word = usize::from(draw.label.get(0).unwrap())
                    | (usize::from(draw.label.get(1).unwrap()) << 1);
                assert!(word == 0 || word == 3);
                observed[word] = true;
                assert_eq!(draw.label_mass, expected[word]);
                assert_eq!(draw.total_mass, ExactScalar::one());
                assert_eq!((draw.support_dimension, draw.conditional_queries), (1, 1));
                assert_eq!(draw.entropy_bits, calls);
                assert!(calls > 0);
            }
            assert_eq!(observed, [true, false, false, true]);
            assert_eq!(tensor.terms(), before);
        }
    }
}

#[test]
fn report_flips_and_129_dependent_rows_use_one_support_coordinate() {
    let a = ScalarArithmetic::default();
    let keep = (0..129)
        .map(|j| OriginalRecordParity {
            records: vec![if j % 2 == 0 { "a" } else { "b" }.into()],
            inverted: j % 3 == 1,
        })
        .collect::<Vec<_>>();
    for strategy in strategies() {
        let tensor = law(
            &instrument(true, false, 1, &a),
            strategy,
            &keep,
            vec!["a".into()],
            &a,
        );
        let before = tensor.terms().to_vec();
        let plan = tensor
            .prepare_affine_sampling(
                Limits {
                    max_record_rows: 129,
                    max_support_dimension: 1,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
        assert_eq!(
            (
                plan.support_bound().width(),
                plan.support_bound().dimension()
            ),
            (129, 1)
        );
        assert_eq!(plan.support_bound().columns()[0].ones().count(), 129);
        for high in [false, true] {
            let mut calls = 0;
            let mut bit = || {
                calls += 1;
                high
            };
            let draw = plan
                .sample_bits(
                    OriginalRecordDrawOptions {
                        max_conditional_queries: 1,
                        ..Default::default()
                    },
                    &mut bit,
                    &a,
                )
                .unwrap();
            let truth = draw.label.get(0).unwrap() ^ true;
            for j in 0..129 {
                assert_eq!(
                    draw.label.get(j).unwrap(),
                    truth ^ (j % 2 == 0) ^ (j % 3 == 1)
                );
            }
            assert_eq!(
                draw.label_mass,
                literal_masses(&a)[if truth { 3 } else { 0 }]
            );
            assert_eq!(value(&tensor, &draw.label, &a), draw.label_mass);
            assert_eq!(
                (
                    draw.support_dimension,
                    draw.conditional_queries,
                    draw.entropy_bits
                ),
                (1, 1, calls)
            );
        }
        assert!(matches!(
            tensor.prepare_affine_sampling(
                Limits {
                    max_record_rows: 128,
                    ..Default::default()
                },
                &a
            ),
            Err(Error::ResourceLimit {
                option: "max_record_rows",
                required: 129,
                ..
            })
        ));
        assert_eq!(tensor.terms(), before);
    }
}

#[test]
fn preparation_and_draw_refusals_preserve_the_source_and_allow_retry() {
    let a = ScalarArithmetic::default();
    for strategy in strategies() {
        let tensor = law(
            &instrument(true, false, 1, &a),
            strategy,
            &rows(),
            vec![],
            &a,
        );
        let before = tensor.terms().to_vec();
        for limits in [
            Limits {
                max_record_rows: 1,
                ..Default::default()
            },
            Limits {
                max_terms: 3,
                ..Default::default()
            },
            Limits {
                max_support_dimension: 0,
                ..Default::default()
            },
            Limits {
                max_geometry_words: 0,
                ..Default::default()
            },
            Limits {
                max_work: 0,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                tensor.prepare_affine_sampling(limits, &a),
                Err(Error::ResourceLimit { .. })
            ));
        }
        let plan = tensor
            .prepare_affine_sampling(Limits::default(), &a)
            .unwrap();
        for options in [
            OriginalRecordDrawOptions {
                max_conditional_queries: 0,
                ..Default::default()
            },
            OriginalRecordDrawOptions {
                max_geometry_words: 0,
                ..Default::default()
            },
            OriginalRecordDrawOptions {
                max_work: 0,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                plan.sample_bits(
                    options,
                    &mut || panic!("quota refusal consumed entropy"),
                    &a
                ),
                Err(Error::ResourceLimit { .. })
            ));
        }
        let mut calls = 0;
        let error = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_entropy_bits: 0,
                    ..Default::default()
                },
                &mut || {
                    calls += 1;
                    false
                },
                &a,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            Error::Bits(BitSamplingError::EntropyLimit { limit: 0 })
        ));
        assert_eq!(calls, 0);
        let draw = plan
            .sample_bits(
                OriginalRecordDrawOptions::default(),
                &mut || {
                    calls += 1;
                    false
                },
                &a,
            )
            .unwrap();
        assert_eq!((draw.label, draw.entropy_bits, calls), (bits(2, 3), 3, 3));
        assert_eq!(draw.label_mass, literal_masses(&a)[3]);
        assert_eq!(tensor.terms(), before);
    }
}

#[test]
fn entropy_budget_counts_the_actual_prefix_across_multiple_coordinates() {
    let a = ScalarArithmetic::default();
    for strategy in strategies() {
        let tensor = law(
            &instrument(true, true, 1, &a),
            strategy,
            &rows(),
            vec![],
            &a,
        );
        let plan = tensor
            .prepare_affine_sampling(Limits::default(), &a)
            .unwrap();
        assert_eq!(plan.support_bound().dimension(), 2);
        let mut calls = 0;
        let error = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_entropy_bits: 4,
                    ..Default::default()
                },
                &mut || {
                    calls += 1;
                    false
                },
                &a,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            Error::Bits(BitSamplingError::EntropyLimit { limit: 1 })
        ));
        assert_eq!(calls, 4);
        calls = 0;
        let draw = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_entropy_bits: 6,
                    ..Default::default()
                },
                &mut || {
                    calls += 1;
                    false
                },
                &a,
            )
            .unwrap();
        assert_eq!(
            (
                draw.label,
                draw.entropy_bits,
                calls,
                draw.conditional_queries
            ),
            (bits(2, 3), 6, 6, 2)
        );
        let rare = &literal_masses(&a)[3];
        assert_eq!(draw.label_mass, a.multiply(rare, rare).unwrap());
    }
}

#[test]
fn uniform_deterministic_and_empty_label_laws_keep_existing_certificates() {
    let a = ScalarArithmetic::default();
    for strategy in strategies() {
        let deterministic = law(
            &instrument(true, false, 0, &a),
            strategy,
            &rows(),
            vec![],
            &a,
        );
        assert!(deterministic.uniform_affine(&a).unwrap().is_some());
        let plan = deterministic
            .prepare_affine_sampling(Limits::default(), &a)
            .unwrap();
        let draw = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_conditional_queries: 0,
                    max_entropy_bits: 0,
                    ..Default::default()
                },
                &mut || panic!("deterministic law consumed entropy"),
                &a,
            )
            .unwrap();
        assert_eq!(
            (
                draw.label,
                draw.label_mass,
                draw.support_dimension,
                draw.conditional_queries,
                draw.entropy_bits
            ),
            (bits(2, 0), ExactScalar::one(), 0, 0, 0)
        );
        let uniform = law(
            &instrument(false, false, 0, &a),
            strategy,
            &rows(),
            vec![],
            &a,
        );
        let certificate = uniform.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(certificate.support().dimension(), 2);
        let plan = uniform
            .prepare_affine_sampling(Limits::default(), &a)
            .unwrap();
        let mut calls = 0;
        let draw = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_entropy_bits: 2,
                    ..Default::default()
                },
                &mut || {
                    calls += 1;
                    false
                },
                &a,
            )
            .unwrap();
        assert_eq!(
            (calls, draw.entropy_bits, draw.conditional_queries),
            (2, 2, 2)
        );
        assert_eq!(&draw.label_mass, certificate.mass_per_label());
        assert_eq!(&draw.total_mass, certificate.total_mass());
        let empty = law(&instrument(true, false, 1, &a), strategy, &[], vec![], &a);
        let plan = empty
            .prepare_affine_sampling(Limits::default(), &a)
            .unwrap();
        let draw = plan
            .sample_bits(
                OriginalRecordDrawOptions {
                    max_conditional_queries: 0,
                    max_entropy_bits: 0,
                    ..Default::default()
                },
                &mut || panic!("empty label law consumed entropy"),
                &a,
            )
            .unwrap();
        assert_eq!(
            (
                draw.label.width(),
                draw.label_mass,
                draw.total_mass,
                draw.entropy_bits
            ),
            (0, ExactScalar::one(), ExactScalar::one(), 0)
        );
    }
}
