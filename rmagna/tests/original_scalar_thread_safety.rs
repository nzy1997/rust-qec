// SPDX-License-Identifier: MPL-2.0
// Complete RuntimeOwnershipTests.test_scalar_totals_are_thread_safe_and_reject_bad_outcomes,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_runtime_ownership.py.
// Typed packed outcomes translate native argument validation; independent Rust
// scalar families preserve shared ownership without private capsule identity.
use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::fibers::FiberOptions;
use rmagna::gf2::{BitVector, Gf2Error};
use rmagna::instrument::{
    CompileOptions, FaultSelection, InsertionPreparation, InstrumentError, ProbabilityPreparation,
    compile_affine,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, UnitaryError};
use std::sync::Arc;
fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|i| mask >> i & 1 != 0)).unwrap()
}
fn half() -> ExactScalar {
    ScalarArithmetic::default()
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap()
}
#[test]
fn original_scalar_totals_are_thread_safe_and_reject_bad_outcomes() {
    let a = ScalarArithmetic::default();
    let mut configurations = 0;
    let mut checked = 0;
    for (source, width) in [("H 0\nM 0", 1), ("H 0\nCX 0 1\nCX 1 2\nM(1/4) 0 1 2", 2)] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for mode in [
                ProbabilityPreparation::Cold,
                ProbabilityPreparation::Trace,
                ProbabilityPreparation::FixedFibers(FiberOptions::default()),
            ] {
                for insertion in [
                    InsertionPreparation::Direct,
                    InsertionPreparation::Prepared(Default::default()),
                ] {
                    for warmed in [false, true] {
                        let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
                        let instrument = Arc::new(
                            compile_affine(
                                &circuit,
                                &CompileOptions {
                                    compiler_strategy: compiler,
                                    probability_preparation: mode,
                                    insertion_preparation: insertion,
                                    ..Default::default()
                                },
                                &a,
                            )
                            .unwrap(),
                        );
                        let rows = Arc::new(
                            (0..width)
                                .map(|i| BitExpr::atom(BitAtom::Record(i)))
                                .collect::<Vec<_>>(),
                        );
                        if warmed {
                            for label in 0..2 {
                                let actual = instrument
                                    .reported_probability(&rows, &bits(width, label), &[], 256, &a)
                                    .unwrap();
                                let expected = if width == 1 || label == 0 {
                                    half()
                                } else {
                                    ExactScalar::zero()
                                };
                                assert_eq!(actual.value, expected);
                            }
                        }
                        let outputs = std::thread::scope(|scope| {
                            let mut handles = vec![];
                            for worker in 0..3 {
                                let p = instrument.clone();
                                let events = rows.clone();
                                handles.push(scope.spawn(move || {
                                    let mut checked = vec![];
                                    for index in (worker..24).step_by(3) {
                                        let label = index % (1 << width);
                                        let flip = width == 2 && (index / 4) % 2 != 0;
                                        let faults = if flip {
                                            vec![FaultSelection {
                                                site: 0,
                                                alternative: 0,
                                            }]
                                        } else {
                                            vec![]
                                        };
                                        let actual = p
                                            .reported_probability(
                                                &events,
                                                &bits(width, label),
                                                &faults,
                                                256,
                                                &a,
                                            )
                                            .unwrap()
                                            .value;
                                        let expected = if width == 1
                                            || (((label & 1) == ((label >> 1) & 1)) != flip)
                                        {
                                            half()
                                        } else {
                                            ExactScalar::zero()
                                        };
                                        assert_eq!(
                                            actual, expected,
                                            "worker={worker} label={label} flip={flip}"
                                        );
                                        checked.push(index);
                                    }
                                    checked
                                }));
                            }
                            handles
                                .into_iter()
                                .flat_map(|h| h.join().unwrap())
                                .collect::<Vec<_>>()
                        });
                        let mut outputs = outputs;
                        outputs.sort_unstable();
                        assert_eq!(outputs, (0..24).collect::<Vec<_>>());
                        checked += outputs.len();
                        let stats = instrument.prepared_probability_statistics().unwrap();
                        assert_eq!(
                            stats.cached_queries,
                            if matches!(mode, ProbabilityPreparation::Cold) {
                                0
                            } else {
                                1
                            }
                        );
                        assert_eq!(stats.fixed_resources, usize::from(mode.is_fixed()));
                        for wrong in [0, width + 1] {
                            let error = instrument
                                .reported_probability(&rows, &bits(wrong, 0), &[], 256, &a)
                                .unwrap_err();
                            assert!(matches!(
                                error,
                                InstrumentError::Unitary(UnitaryError::DimensionMismatch)
                            ));
                        }
                        assert!(matches!(
                            BitVector::from_indices(1, [1]),
                            Err(Gf2Error::IndexOutOfBounds { index: 1, width: 1 })
                        ));
                        assert!(matches!(
                            BitVector::from_words(1, vec![2]),
                            Err(Gf2Error::NonzeroPadding)
                        ));
                        assert_eq!(instrument.prepared_probability_statistics().unwrap(), stats);
                    }
                    configurations += 1;
                }
            }
        }
    }
    assert_eq!(configurations, 24);
    assert_eq!(checked, 1152);
}
