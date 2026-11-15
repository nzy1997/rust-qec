// SPDX-License-Identifier: MPL-2.0
// Source-bound cases from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
// See fixtures/original_optimization_inputs.NOTICE for complete attribution.
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::{BitVector, LinearFactorization};
use rmagna::instrument::*;
use rmagna::noise::RandomSource;
use rmagna::phase::NativePhaseOptions;
use rmagna::quadratic::QuadraticForm;
use rmagna::records::{
    CompletedProbability, CompletedRecordError as Error, CompletedRecordLimits as Limits,
    CompletedSamplingOptions, SamplingError, sum_boundary_terms,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn bits(width: usize, value: usize) -> BitVector {
    let words = if width == 0 {
        vec![]
    } else {
        vec![value as u64]
    };
    BitVector::from_words(width, words).unwrap()
}
fn mask(bits: &BitVector) -> usize {
    bits.ones().fold(0, |v, j| v | (1 << j))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ket {
    width: usize,
    base: usize,
    columns: Vec<usize>,
    linear: Vec<u8>,
    upper: Vec<usize>,
    power: i64,
    phase: i64,
}
impl Ket {
    fn form(&self) -> QuadraticForm {
        form(&self.linear, &self.upper)
    }
    fn affine(&self, a: &ScalarArithmetic) -> AffineKet {
        AffineKet::new(
            bits(self.width, self.base),
            self.columns.iter().map(|&v| bits(self.width, v)).collect(),
            self.form(),
            a.omega(self.phase, self.power).unwrap(),
        )
        .unwrap()
    }
    // Exhaustive path sum from the source's tensor oracle. No GF(2), phase
    // restriction, Gaussian sum, prepared query or contraction implementation.
    fn dense(&self, a: &ScalarArithmetic) -> Vec<ExactScalar> {
        let mut out = vec![ExactScalar::zero(); 1 << self.width];
        for x in 0..1 << self.columns.len() {
            let y = self
                .columns
                .iter()
                .enumerate()
                .fold(
                    self.base,
                    |y, (j, col)| if x >> j & 1 != 0 { y ^ col } else { y },
                );
            let v = a
                .omega(
                    self.phase + 2 * i64::from(quadratic(x, &self.linear, &self.upper)),
                    self.power,
                )
                .unwrap();
            out[y] = a.add(&out[y], &v).unwrap();
        }
        out
    }
}
fn form(linear: &[u8], upper: &[usize]) -> QuadraticForm {
    QuadraticForm::new(
        0,
        linear.to_vec(),
        upper.iter().map(|&v| bits(linear.len(), v)).collect(),
    )
    .unwrap()
}
fn quadratic(x: usize, linear: &[u8], upper: &[usize]) -> u8 {
    let mut q = 0;
    for (j, &l) in linear.iter().enumerate() {
        if x >> j & 1 != 0 {
            q += usize::from(l) + 2 * (upper[j] & x).count_ones() as usize;
        }
    }
    (q % 4) as u8
}
fn fixture() -> serde_json::Value {
    let raw = include_bytes!("fixtures/original_optimization_inputs.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "173eb98b0e7089d0484fa2305c6c5fc700505562044042563d160cee1edbdc9b"
    );
    let value: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        value["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        value["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_optimization_inputs.py"
            ))
        )
    );
    assert_eq!(
        value["source_sha256"]["tests/core/test_simulator_optimization.py"],
        "2391f35f00a33244677f0a903bbdb34a0ab4448da7cc454850e2b27e33596d3d"
    );
    value
}

#[test]
fn original_batched_inverse_keeps_free_coordinates_and_rejects_dependent_rows() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        width: usize,
        rows: Vec<usize>,
        expected: Option<Vec<usize>>,
    }
    let data = fixture();
    let cases: Vec<Case> = serde_json::from_value(data["inverse"].clone()).unwrap();
    assert_eq!(cases.len(), 13 * 12 + 1);
    for (index, case) in cases.into_iter().enumerate() {
        if index < 156 {
            assert_eq!(case.width, index / 12);
        } else {
            assert_eq!(
                (case.width, case.rows.as_slice(), &case.expected),
                (1, &[1, 1][..], &None)
            );
        }
        let rows = case
            .rows
            .iter()
            .map(|&v| bits(case.width, v))
            .collect::<Vec<_>>();
        let factor = LinearFactorization::new(case.width, &rows).unwrap();
        let found = factor.right_inverse().unwrap();
        assert_eq!(
            found
                .as_ref()
                .map(|v| v.iter().map(mask).collect::<Vec<_>>()),
            case.expected
        );
        if let Some(columns) = found {
            assert_eq!(columns.len(), rows.len());
            for (j, column) in columns.iter().enumerate() {
                assert_eq!(
                    Some(column),
                    factor.solve(&bits(rows.len(), 1 << j)).unwrap().as_ref()
                );
                for (k, row) in rows.iter().enumerate() {
                    assert_eq!(row.dot(column).unwrap(), j == k);
                }
                for free in 0..case.width {
                    if !factor.pivot_columns().contains(&free) {
                        assert!(!column.get(free).unwrap());
                    }
                }
            }
        }
    }
}

#[test]
fn original_common_support_overlap_retains_all_shifts_and_global_phases() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        left: Ket,
        right: Ket,
    }
    let data = fixture();
    let cases: Vec<Case> = serde_json::from_value(data["overlap"].clone()).unwrap();
    assert_eq!(cases.len(), 7 * 20);
    let a = ScalarArithmetic::default();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(
            (case.left.width, case.right.width),
            (index / 20 + 1, index / 20 + 1)
        );
        assert_eq!(case.left.columns, case.right.columns);
        let left = case.left.dense(&a);
        let right = case.right.dense(&a);
        let expected = left
            .iter()
            .zip(&right)
            .fold(ExactScalar::zero(), |s, (x, y)| {
                a.add(&s, &a.multiply(&x.conjugate(), y).unwrap()).unwrap()
            });
        assert_eq!(
            case.left
                .affine(&a)
                .overlap(&case.right.affine(&a), &a)
                .unwrap(),
            expected,
            "case {index}"
        );
    }
}

#[test]
fn original_sparse_contraction_matches_every_dependent_boundary_path_sum() {
    let data = fixture();
    let cases: Vec<Ket> = serde_json::from_value(data["contract"].clone()).unwrap();
    assert_eq!(cases.len(), 10 * 24);
    let a = ScalarArithmetic::default();
    let mut zeros = 0;
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(case.columns.len(), index / 24);
        let expected = case.dense(&a);
        let contracted = case.affine(&a).contract(&a).unwrap();
        let found = if let Some(ket) = contracted {
            let prepared = ket.prepare().unwrap();
            (0..1 << case.width)
                .map(|y| prepared.amplitude(&bits(case.width, y), &a).unwrap())
                .collect()
        } else {
            zeros += 1;
            vec![ExactScalar::zero(); 1 << case.width]
        };
        assert_eq!(found, expected, "case {index}");
    }
    assert!(
        zeros > 0,
        "the source schedule must retain destructive interference"
    );
}

#[test]
fn original_packed_pullback_matches_all_32_assignments_of_all_108_maps() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        linear: Vec<u8>,
        upper: Vec<usize>,
        shift: usize,
        basis: Vec<usize>,
    }
    let data = fixture();
    let cases: Vec<Case> = serde_json::from_value(data["pullback"].clone()).unwrap();
    assert_eq!(cases.len(), 9 * 12);
    for (index, case) in cases.into_iter().enumerate() {
        let n = index / 12;
        assert_eq!(case.linear.len(), n);
        assert_eq!(case.basis.len(), 5);
        let q = form(&case.linear, &case.upper);
        let restricted = q
            .restrict(
                &bits(n, case.shift),
                &case.basis.iter().map(|&b| bits(n, b)).collect::<Vec<_>>(),
            )
            .unwrap();
        for u in 0..32 {
            let x = case.basis.iter().enumerate().fold(case.shift, |x, (j, b)| {
                if u >> j & 1 != 0 { x ^ b } else { x }
            });
            assert_eq!(
                restricted.evaluate(&bits(5, u)).unwrap(),
                quadratic(x, &case.linear, &case.upper),
                "map {index}, word {u}"
            );
        }
    }
}

fn point(width: usize, value: usize, coefficient: ExactScalar) -> AffineKet {
    AffineKet::new(bits(width, value), vec![], form(&[], &[]), coefficient).unwrap()
}
#[derive(Default)]
struct Counted {
    calls: usize,
}
impl RandomSource for Counted {
    fn next_u64(&mut self) -> u64 {
        self.calls += 1;
        0
    }
}
#[test]
fn original_raw_boundary_sampling_rejects_invalid_distribution_and_shape() {
    let a = ScalarArithmetic::default();
    let mut rng = Counted::default();
    let empty = CompletedProbability::from_boundary_terms(1, vec![], Limits::default()).unwrap();
    assert!(matches!(
        empty.sample(&mut rng, &CompletedSamplingOptions::default(), &a),
        Err(Error::Sampling(SamplingError::ImpossibleCondition))
    ));
    assert!(matches!(
        CompletedProbability::from_boundary_terms(
            1,
            vec![point(2, 0, ExactScalar::one())],
            Limits::default()
        ),
        Err(Error::DimensionMismatch)
    ));
    let two = a.add(&ExactScalar::one(), &ExactScalar::one()).unwrap();
    let signed = CompletedProbability::from_boundary_terms(
        1,
        vec![point(1, 0, two), point(1, 1, ExactScalar::one().negated())],
        Limits::default(),
    )
    .unwrap();
    assert_eq!(signed.total(&a).unwrap(), ExactScalar::one());
    assert!(matches!(
        signed.sample(&mut rng, &CompletedSamplingOptions::default(), &a),
        Err(Error::Sampling(SamplingError::InvalidMass))
    ));
    // Rust accepts u64 seed values, so the native negative-seed failure is an
    // input conversion error rather than an invalid value inside the sampler.
    assert!("-1".parse::<u64>().is_err());
    assert_eq!(rng.calls, 0);
}

#[test]
fn original_boundary_sum_validates_every_term_width_and_duplicate_rhs() {
    let a = ScalarArithmetic::default();
    let terms = [
        point(2, 0, ExactScalar::one()),
        point(1, 0, ExactScalar::one()),
    ];
    let query = |terms: &[AffineKet], rows: &[BitVector], rhs: &BitVector| {
        sum_boundary_terms(terms, rows, rhs, Limits::default(), &a)
    };
    assert_eq!(
        query(&terms, &[bits(1, 1), bits(1, 1)], &bits(2, 0)).unwrap(),
        a.add(&ExactScalar::one(), &ExactScalar::one()).unwrap()
    );
    assert_eq!(
        query(&terms, &[bits(1, 1), bits(1, 1)], &bits(2, 2)).unwrap(),
        ExactScalar::zero()
    );
    assert!(matches!(
        query(&terms, &[bits(2, 2)], &bits(1, 0)),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        query(&terms, &[bits(1, 1)], &bits(0, 0)),
        Err(Error::DimensionMismatch)
    ));
    // A one-bit packed RHS cannot encode the native invalid binary value 2.
    assert!(BitVector::from_words(1, vec![2]).is_err());
    // Validation also inspects zero terms and remains independent of order.
    let zero = [
        point(2, 0, ExactScalar::one()),
        point(1, 0, ExactScalar::zero()),
    ];
    assert!(matches!(
        query(&zero, &[bits(2, 2)], &bits(1, 0)),
        Err(Error::DimensionMismatch)
    ));
    assert!(
        query(
            &[terms[1].clone(), terms[0].clone()],
            &[bits(2, 2)],
            &bits(1, 0)
        )
        .is_err()
    );
}

#[test]
fn original_control_callbacks_execute_fresh_with_unchanged_version_and_replacement() {
    let a = ScalarArithmetic::default();
    let emitted_x = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let build = |emitted_x: Arc<AtomicBool>, calls: Arc<AtomicUsize>| {
        OriginalInstrument::new(
            vec![0],
            vec![0],
            CliffordCircuit::computational(bits(1, 0)).unwrap(),
            vec![
                OriginalOperation {
                    id: "c".into(),
                    gate: OriginalGate::Control,
                },
                OriginalOperation {
                    id: "m".into(),
                    gate: OriginalGate::Measure {
                        factors: vec![(0, OriginalPauli::Z)],
                        inverted: false,
                        record: "m".into(),
                    },
                },
            ],
            vec![OriginalControl {
                operation: "c".into(),
                parents: vec![],
                version: "unchanged-version".into(),
                callback: Arc::new(move |records| {
                    assert!(records.entries().is_empty());
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(vec![if emitted_x.load(Ordering::SeqCst) {
                        OriginalGate::Pauli {
                            wire: 0,
                            pauli: OriginalPauli::X,
                        }
                    } else {
                        OriginalGate::Identity(vec![0])
                    }])
                }),
            }],
            Default::default(),
            &a,
        )
        .unwrap()
    };
    let ir = build(emitted_x.clone(), calls.clone());
    let assignment = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    let probability = |ir: &OriginalInstrument| {
        ir.compile(&assignment, CompilerStrategy::ParityCopies, &a)
            .unwrap()
            .probability(&a)
            .unwrap()
    };
    assert_eq!(probability(&ir), ExactScalar::one());
    assert_eq!(probability(&ir), ExactScalar::one());
    emitted_x.store(true, Ordering::SeqCst);
    assert_eq!(probability(&ir), ExactScalar::zero());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    // Instruments own their callback table: replacing a callback is a new
    // immutable instrument, retaining the original unchanged-version label.
    let replacement = build(Arc::new(AtomicBool::new(false)), calls.clone());
    assert_eq!(probability(&replacement), ExactScalar::one());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn imported_boundary_terms_sum_linearly_and_obey_resource_limits() {
    let a = ScalarArithmetic::default();
    let i = a.omega(2, 0).unwrap();
    let terms = vec![
        point(1, 0, a.add(&ExactScalar::one(), &i).unwrap()),
        point(1, 0, i.negated()),
    ];
    let distribution =
        CompletedProbability::from_boundary_terms(1, terms.clone(), Limits::default()).unwrap();
    assert_eq!(distribution.total(&a).unwrap(), ExactScalar::one());
    assert_eq!(distribution.statistics().pair_work, 0);
    let mut rng = Counted::default();
    let draw = distribution
        .sample(&mut rng, &CompletedSamplingOptions::default(), &a)
        .unwrap();
    assert_eq!(draw.values, bits(1, 0));
    assert_eq!(draw.mass, ExactScalar::one());
    assert_eq!(rng.calls, 0);
    assert_eq!(
        sum_boundary_terms(&terms, &[], &bits(0, 0), Limits::default(), &a).unwrap(),
        ExactScalar::one()
    );
    for limits in [
        Limits {
            max_source_entries: 1,
            ..Default::default()
        },
        Limits {
            max_record_rows: 0,
            ..Default::default()
        },
        Limits {
            max_geometry_words: 0,
            ..Default::default()
        },
        Limits {
            max_construction_work: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            CompletedProbability::from_boundary_terms(1, terms.clone(), limits),
            Err(Error::ResourceLimit { .. })
        ));
        assert!(matches!(
            sum_boundary_terms(&terms, &[], &bits(0, 0), limits, &a),
            Err(Error::ResourceLimit { .. })
        ));
    }
}

#[test]
fn original_immutable_native_plan_has_independent_concurrent_workspaces() {
    let a = ScalarArithmetic::default();
    let record = ControlExpression::record("m".into(), Default::default()).unwrap();
    let exponent = ControlExpression::compose(
        ControlOperator::Add,
        vec![
            ControlExpression::constant(1),
            ControlExpression::compose(
                ControlOperator::Multiply,
                vec![ControlExpression::constant(6), record],
                Default::default(),
            )
            .unwrap(),
        ],
        Default::default(),
    )
    .unwrap();
    let rule = ControlRule::new(
        vec![
            ControlGate::new(
                ControlGateKind::T,
                vec![0],
                Some(exponent),
                ControlExpression::constant(1),
                Default::default(),
            )
            .unwrap(),
        ],
        Default::default(),
    )
    .unwrap();
    let measure = |name: &str, pauli| OriginalOperation {
        id: name.into(),
        gate: OriginalGate::Measure {
            factors: vec![(0, pauli)],
            inverted: false,
            record: name.into(),
        },
    };
    let ir = OriginalInstrument::new_with_declared_controls(
        vec![0],
        vec![0],
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            form(&[0], &[0]),
            a.omega(0, -1).unwrap(),
        )
        .unwrap(),
        vec![
            measure("m", OriginalPauli::Z),
            OriginalOperation {
                id: "c".into(),
                gate: OriginalGate::Control,
            },
            measure("n", OriginalPauli::X),
        ],
        vec![
            OriginalDeclaredControl::new(
                "c".into(),
                vec!["m".into()],
                "owned-rule-v1".into(),
                rule,
            )
            .unwrap(),
        ],
        Default::default(),
        &a,
    )
    .unwrap();
    let trace = ir
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap();
    let rows = ["m", "n"].map(|s| OriginalRecordParity {
        records: vec![s.into()],
        inverted: false,
    });
    let query = trace
        .prepare_network_probability(
            &rows,
            &OriginalNetworkOptions {
                resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert!(query.resource().native_resource().is_some());
    let evaluate = |index: usize| {
        let i = index / 4;
        let p = [
            OriginalPauli::I,
            OriginalPauli::X,
            OriginalPauli::Y,
            OriginalPauli::Z,
        ][index % 4];
        let assignment = OriginalTraceAssignment {
            faults: vec![OriginalFault {
                after: "c".into(),
                wire: 0,
                pauli: p,
            }],
            ..Default::default()
        };
        let view = trace.with_assignment(&assignment, &a).unwrap();
        query.evaluate(&view, &bits(2, i), &a).unwrap().value
    };
    let expected = (0..16).map(evaluate).collect::<Vec<_>>();
    let quarter = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -2)
        .unwrap();
    assert!(expected.iter().all(|v| *v == quarter));
    let mut found = vec![ExactScalar::zero(); 16 * 8];
    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|worker| {
                let evaluate = &evaluate;
                scope.spawn(move || {
                    (worker..128)
                        .step_by(4)
                        .map(|index| (index, evaluate(index % 16)))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            for (index, value) in handle.join().unwrap() {
                found[index] = value;
            }
        }
    });
    assert_eq!(
        found,
        (0..8)
            .flat_map(|_| expected.iter().cloned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn original_wire_lifetime_is_checked_on_native_assignment_view() {
    let a = ScalarArithmetic::default();
    let initial = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        form(&[0], &[0]),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        initial,
        vec![
            OriginalOperation {
                id: "t".into(),
                gate: OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            },
            OriginalOperation {
                id: "m".into(),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::X)],
                    inverted: false,
                    record: "m".into(),
                },
            },
        ],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    let trace = ir
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap();
    for wire in [9, -1] {
        let assignment = OriginalTraceAssignment {
            faults: vec![OriginalFault {
                after: "t".into(),
                wire,
                pauli: OriginalPauli::X,
            }],
            ..Default::default()
        };
        assert!(matches!(trace.with_assignment(&assignment, &a),
            Err(OriginalError::Lifetime { wire: bad, expected_live: true }) if bad == wire));
    }
    let view = trace
        .with_assignment(
            &OriginalTraceAssignment {
                faults: vec![OriginalFault {
                    after: "t".into(),
                    wire: 0,
                    pauli: OriginalPauli::X,
                }],
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    let query = trace
        .prepare_network_probability(
            &[OriginalRecordParity {
                records: vec!["m".into()],
                inverted: false,
            }],
            &OriginalNetworkOptions {
                resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    let p0 = query.evaluate(&view, &bits(1, 0), &a).unwrap().value;
    let p1 = query.evaluate(&view, &bits(1, 1), &a).unwrap().value;
    assert_eq!(a.add(&p0, &p1).unwrap(), ExactScalar::one());
}

#[test]
fn original_warm_prepared_query_validates_rhs_and_all_boundary_masks() {
    use rmagna::pauli::PauliWord;
    use rmagna::records::CompletedRecordSpec;
    let a = ScalarArithmetic::default();
    let op = |id: &str, gate| OriginalOperation {
        id: id.into(),
        gate,
    };
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![
            op("h", OriginalGate::H(0)),
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op("h2", OriginalGate::H(0)),
            op(
                "m",
                OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: "m0".into(),
                },
            ),
        ],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    let trace = ir
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap();
    let view = trace.with_assignment(&Default::default(), &a).unwrap();
    let side = view.expansion(&Default::default(), &a).unwrap();
    // The single coherent record is appended after original output wire 0.
    assert_eq!(trace.physical_width(), 2);
    let event = bits(2, 2);
    let spec = CompletedRecordSpec::new(
        2,
        &[],
        std::slice::from_ref(&event),
        &bits(1, 0),
        Limits::default(),
    )
    .unwrap();
    let prepared = spec
        .prepare(side.entries(), Default::default(), &a)
        .unwrap();
    let warm = prepared
        .probability(&bits(1, 0), &a)
        .unwrap()
        .distribution
        .total(&a)
        .unwrap();
    assert!(warm.real_sign().unwrap().is_gt());
    for outcomes in [bits(0, 0), bits(2, 2)] {
        assert!(matches!(
            prepared.probability(&outcomes, &a),
            Err(Error::DimensionMismatch)
        ));
    }
    // Typed packed RHS conversion rejects exactly the original -1 and 2.
    for value in [u64::MAX, 2] {
        assert!(BitVector::from_words(1, vec![value]).is_err());
    }
    let outside = BitVector::from_indices(101, [100]).unwrap();
    assert!(matches!(
        CompletedRecordSpec::new(
            2,
            std::slice::from_ref(&outside),
            &[],
            &bits(0, 0),
            Limits::default()
        ),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        CompletedRecordSpec::new(
            2,
            &[],
            std::slice::from_ref(&outside),
            &bits(1, 0),
            Limits::default()
        ),
        Err(Error::DimensionMismatch)
    ));
    for (x, z) in [
        (outside.clone(), BitVector::zeros(101).unwrap()),
        (BitVector::zeros(101).unwrap(), outside),
    ] {
        let word = PauliWord::new(x, z, 0).unwrap();
        assert!(matches!(
            prepared.moment(&bits(1, 0), &word, &a),
            Err(Error::DimensionMismatch)
        ));
    }
    let noncommuting = PauliWord::new(event, bits(2, 0), 0).unwrap();
    assert!(matches!(
        prepared.moment(&bits(1, 0), &noncommuting, &a),
        Err(Error::NonCommutingPauli {
            kind: "event",
            index: 0
        })
    ));
    assert_eq!(
        prepared
            .probability(&bits(1, 0), &a)
            .unwrap()
            .distribution
            .total(&a)
            .unwrap(),
        warm
    );
}

#[test]
fn imported_boundary_queries_match_exhaustive_signed_complex_path_sums() {
    let data = fixture();
    let cases: Vec<Ket> = serde_json::from_value(data["contract"].clone()).unwrap();
    let a = ScalarArithmetic::default();
    for (index, case) in cases.into_iter().enumerate() {
        let dense = case.dense(&a);
        let ket = case.affine(&a);
        let imported = CompletedProbability::from_boundary_terms(
            case.width,
            vec![ket.clone()],
            Limits::default(),
        )
        .unwrap();
        let masks = if case.width == 0 {
            [0, 0, 0]
        } else {
            [1, 1 << (case.width - 1), 1]
        };
        let rows = masks.map(|v| bits(case.width, v));
        for rhs in 0..8 {
            let expected = dense
                .iter()
                .enumerate()
                .filter(|(y, _)| {
                    masks
                        .iter()
                        .enumerate()
                        .all(|(j, m)| ((y & m).count_ones() as usize & 1) == (rhs >> j & 1))
                })
                .fold(ExactScalar::zero(), |s, (_, v)| a.add(&s, v).unwrap());
            assert_eq!(
                imported.mass(&rows, &bits(3, rhs), &a).unwrap(),
                expected,
                "imported case {index}, rhs {rhs}"
            );
            assert_eq!(
                sum_boundary_terms(
                    std::slice::from_ref(&ket),
                    &rows,
                    &bits(3, rhs),
                    Limits::default(),
                    &a
                )
                .unwrap(),
                expected,
                "sum case {index}, rhs {rhs}"
            );
        }
    }
}

#[test]
fn original_floating_summaries_reject_all_four_nonfinite_and_shape_rows() {
    use rmagna::field::{ExactField, FieldArithmetic};
    use rmagna::statistics::StatisticsOptions;
    use rmagna::task::{FloatingTaskDraw, TaskRunStatistics};
    let row = |acceptance, error, moments| FloatingTaskDraw {
        acceptance,
        error,
        moments,
    };
    for rows in [
        vec![row(f64::NAN, None, vec![])],
        vec![row(1.0, None, vec![]), row(1.0, Some(0.0), vec![])],
        vec![row(1.0, None, vec![]), row(1.0, None, vec![0.0])],
        vec![row(1.0, None, vec![f64::INFINITY])],
    ] {
        let error =
            TaskRunStatistics::from_floating_draws(rows, StatisticsOptions::default()).unwrap_err();
        assert!(error.field.starts_with("floating_draws."));
        assert!(
            error.message.contains("finite") || error.message.contains("match the first row"),
            "{error}"
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for row in [
            row(value, None, vec![]),
            row(1.0, Some(value), vec![]),
            row(1.0, None, vec![value]),
        ] {
            assert!(
                TaskRunStatistics::from_floating_draws([row], StatisticsOptions::default())
                    .unwrap_err()
                    .message
                    .contains("finite")
            );
        }
    }
    assert!(
        TaskRunStatistics::from_floating_draws([], StatisticsOptions::default())
            .unwrap()
            .is_none()
    );
    let summary = TaskRunStatistics::from_floating_draws(
        [
            row(1.0, Some(0.0), vec![-1.0, 0.5]),
            row(1.0, Some(1.0), vec![1.0, 0.5]),
        ],
        StatisticsOptions::default(),
    )
    .unwrap()
    .unwrap();
    let a = FieldArithmetic::default();
    let half = a.from_rational("1/2".parse().unwrap()).unwrap();
    let quarter = a.from_rational("1/4".parse().unwrap()).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(summary.accepted, ExactField::one());
    assert_eq!(summary.error, Some(half.clone()));
    assert_eq!(summary.conditional_error, Some(half.clone()));
    assert_eq!(summary.moments, [ExactField::zero(), half]);
    assert_eq!(summary.variances.accepted, Some(ExactField::zero()));
    assert_eq!(summary.variances.error, Some(quarter.clone()));
    assert_eq!(summary.variances.conditional_error, Some(quarter));
    assert_eq!(
        summary.variances.moments,
        [Some(ExactField::one()), Some(ExactField::zero())]
    );
    assert_eq!(summary.mean_weight, ExactField::one());
    assert_eq!(
        summary.effective_sample_size,
        a.from_rational("2".parse().unwrap()).unwrap()
    );
    let limited = StatisticsOptions {
        max_dimensions: 2,
        ..Default::default()
    };
    assert!(
        TaskRunStatistics::from_floating_draws([row(1.0, None, vec![0.0])], limited)
            .unwrap_err()
            .message
            .contains("limit")
    );
    let limited = StatisticsOptions {
        arithmetic: FieldArithmetic::new(8),
        ..Default::default()
    };
    assert!(
        TaskRunStatistics::from_floating_draws([row(f64::from_bits(1), None, vec![])], limited)
            .unwrap_err()
            .message
            .contains("bits")
    );
}
