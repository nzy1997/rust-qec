// SPDX-License-Identifier: MPL-2.0
// Complete itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f scenario,
// tests/core/test_boundary_sampling.py::BoundarySamplingTests::test_all_affine_cliffords_keep_exact_acceptance_and_effects.
// Independent physical-basis projectors and exact matrices; no private
// Python RNG/class ABI or paper performance claim.
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::*;
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar as V, ScalarArithmetic};
use rmagna::unitary::CompilerStrategy;
fn bits(n: usize, value: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|j| value >> j & 1 != 0),
    )
    .unwrap()
}
fn word(n: usize, x: usize, z: usize) -> PauliWord {
    PauliWord::new(bits(n, x), bits(n, z), (2 * (x & z).count_ones() % 8) as u8).unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn initial(n: usize, plus: &[usize], a: &ScalarArithmetic) -> rmagna::stabilizer::AffineKet {
    CliffordCircuit::new(n, plus.iter().map(|&q| CliffordGate::H(q)))
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(n, 0)).unwrap(), a)
        .unwrap()
        .unwrap()
}
fn op(id: &str, gate: OriginalGate) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn measure(
    id: &str,
    name: &str,
    factors: &[(i64, OriginalPauli)],
    inverted: bool,
) -> OriginalOperation {
    op(
        id,
        OriginalGate::Measure {
            factors: factors.to_vec(),
            inverted,
            record: name.into(),
        },
    )
}
fn r(name: &str) -> ControlExpression {
    ControlExpression::record(name.into(), Default::default()).unwrap()
}
fn e(kind: ControlOperator, args: Vec<ControlExpression>) -> ControlExpression {
    ControlExpression::compose(kind, args, Default::default()).unwrap()
}
fn cg(
    kind: ControlGateKind,
    wires: &[i64],
    exponent: Option<ControlExpression>,
    when: ControlExpression,
) -> ControlGate {
    ControlGate::new(kind, wires.to_vec(), exponent, when, Default::default()).unwrap()
}
fn declaration(id: &str, parents: &[&str], gates: Vec<ControlGate>) -> OriginalDeclaredControl {
    OriginalDeclaredControl::new(
        id.into(),
        parents.iter().map(|s| s.to_string()).collect(),
        "literal-v1".into(),
        ControlRule::new(gates, Default::default()).unwrap(),
    )
    .unwrap()
}
fn fault(after: &str, wire: i64, pauli: OriginalPauli) -> OriginalFault {
    OriginalFault {
        after: after.into(),
        wire,
        pauli,
    }
}
fn event(records: &[&str], value: bool) -> OriginalRecordEvent {
    OriginalRecordEvent {
        parity: OriginalRecordParity {
            records: records.iter().map(|s| s.to_string()).collect(),
            inverted: false,
        },
        value,
    }
}
fn h(v: &mut [V], q: usize, a: &ScalarArithmetic) {
    let c = a
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    for i in 0..v.len() {
        if i >> q & 1 == 0 {
            let j = i ^ (1 << q);
            let x = v[i].clone();
            let y = v[j].clone();
            v[i] = a.multiply(&c, &a.add(&x, &y).unwrap()).unwrap();
            v[j] = a.multiply(&c, &a.subtract(&x, &y).unwrap()).unwrap();
        }
    }
}
fn phase(v: &mut [V], mask: usize, turns: u8, a: &ScalarArithmetic) {
    let c = a.omega(turns.into(), 0).unwrap();
    for (i, x) in v.iter_mut().enumerate() {
        if (i & mask).count_ones() % 2 == 1 {
            *x = a.multiply(x, &c).unwrap();
        }
    }
}
fn pauli(v: &[V], x: usize, z: usize, a: &ScalarArithmetic) -> Vec<V> {
    let c = a.omega(i64::from(2 * (x & z).count_ones() % 8), 0).unwrap();
    let mut out = vec![V::zero(); v.len()];
    for (i, t) in v.iter().enumerate() {
        let t = a.multiply(t, &c).unwrap();
        out[i ^ x] = if (i & z).count_ones().is_multiple_of(2) {
            t
        } else {
            t.negated()
        };
    }
    out
}
fn apply(v: &mut Vec<V>, x: usize, z: usize, a: &ScalarArithmetic) {
    *v = pauli(v, x, z, a);
}
fn project(
    v: &[V],
    x: usize,
    z: usize,
    negative: bool,
    outcome: usize,
    a: &ScalarArithmetic,
) -> Vec<V> {
    let pv = pauli(v, x, z, a);
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    v.iter()
        .zip(pv)
        .map(|(u, p)| {
            a.multiply(
                &half,
                &if (outcome != 0) ^ negative {
                    a.subtract(u, &p).unwrap()
                } else {
                    a.add(u, &p).unwrap()
                },
            )
            .unwrap()
        })
        .collect()
}
fn moment(v: &[V], x: usize, z: usize, a: &ScalarArithmetic) -> V {
    v.iter()
        .zip(pauli(v, x, z, a))
        .try_fold(V::zero(), |s, (u, p)| {
            a.add(&s, &a.multiply(&u.conjugate(), &p)?)
        })
        .unwrap()
}
fn cx(v: &mut [V], control: usize, target: usize) {
    for i in 0..v.len() {
        if i >> control & 1 != 0 && i >> target & 1 == 0 {
            v.swap(i, i ^ (1 << target));
        }
    }
}
fn cz(v: &mut [V], control: usize, target: usize) {
    for (i, t) in v.iter_mut().enumerate() {
        if i >> control & 1 != 0 && i >> target & 1 != 0 {
            *t = t.negated();
        }
    }
}
fn original_direct_instrument(kind: usize, a: &ScalarArithmetic) -> OriginalInstrument {
    let (kind, wires, exponent) = match kind {
        0 => (ControlGateKind::X, vec![1], None),
        1 => (ControlGateKind::Y, vec![1], None),
        2 => (ControlGateKind::S, vec![1], None),
        3 => (ControlGateKind::H, vec![1], None),
        4 => (ControlGateKind::Cx, vec![1, 2], None),
        5 => (ControlGateKind::Cz, vec![1, 2], None),
        6 => (
            ControlGateKind::Phase,
            vec![1, 2],
            Some(ControlExpression::constant(2)),
        ),
        _ => unreachable!(),
    };
    OriginalInstrument::new_with_declared_controls(
        vec![0, 1, 2],
        vec![1, 2, 0],
        initial(3, &[1, 2], a),
        vec![
            op("h", OriginalGate::H(0)),
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![1],
                    turns: 1,
                },
            ),
            measure("m", "a", &[(0, OriginalPauli::Z)], false),
            op("c", OriginalGate::Control),
            measure("n", "b", &[(2, OriginalPauli::X)], false),
            op(
                "r",
                OriginalGate::Reset {
                    wire: 0,
                    plus: false,
                },
            ),
        ],
        vec![declaration(
            "c",
            &["a"],
            vec![cg(
                kind,
                &wires,
                exponent,
                e(ControlOperator::Not, vec![r("a")]),
            )],
        )],
        Default::default(),
        a,
    )
    .unwrap()
}
// Literal physical three-wire vectors. Post-MZ q0 is a known basis value,
// so resetting it to zero preserves every branch norm and output-q1 moment.
// No production simulator, branch compiler, marginal or trace constructs these
// expected values. Observable bit0 means physical q1 in output order [1,2,0].
fn direct_literal(kind: usize, flip: usize, a: &ScalarArithmetic) -> (V, V, Vec<V>) {
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    let mut mass = V::zero();
    let mut acceptance = V::zero();
    let mut moments = vec![V::zero(); 3];
    for av in 0..2 {
        for bv in 0..2 {
            let mut v = (0..8)
                .map(|i| if i & 1 == 0 { half.clone() } else { V::zero() })
                .collect::<Vec<_>>();
            h(&mut v, 0, a);
            phase(&mut v, 2, 1, a);
            v = project(&v, 0, 1, false, av, a);
            if av ^ (flip & 1) == 0 {
                match kind {
                    0 => apply(&mut v, 2, 0, a),
                    1 => apply(&mut v, 2, 2, a),
                    2 => phase(&mut v, 2, 2, a),
                    3 => h(&mut v, 1, a),
                    4 => cx(&mut v, 1, 2),
                    5 => cz(&mut v, 1, 2),
                    6 => phase(&mut v, 6, 2, a),
                    _ => unreachable!(),
                }
            }
            apply(&mut v, 2, 2, a); // Actual Y fault executes after c even if its gate is inactive.
            v = project(&v, 4, 0, false, bv, a);
            if av != 0 {
                apply(&mut v, 1, 0, a);
            }
            let p = moment(&v, 0, 0, a);
            mass = a.add(&mass, &p).unwrap();
            if (av ^ (flip & 1)) ^ (bv ^ ((flip >> 1) & 1)) == 0 {
                acceptance = a.add(&acceptance, &p).unwrap();
                for (j, (x, z)) in [(2, 0), (2, 2), (0, 2)].into_iter().enumerate() {
                    moments[j] = a.add(&moments[j], &moment(&v, x, z, a)).unwrap();
                }
            }
        }
    }
    assert_eq!(mass, V::one());
    (mass, acceptance, moments)
}
struct DirectRng {
    state: u64,
    calls: usize,
}
impl rmagna::noise::RandomSource for DirectRng {
    fn next_u64(&mut self) -> u64 {
        self.calls += 1;
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        self.state
    }
}
#[test]
fn original_seven_affine_clifford_boundary_tasks_keep_exact_acceptance_and_effects() {
    let a = ScalarArithmetic::default();
    let mut checked = 0;
    let task = OriginalBoundaryTask {
        postselection: vec![event(&["a", "b"], false)],
        observables: vec![word(3, 1, 0), word(3, 1, 1), word(3, 0, 1)],
    };
    for kind in 0..7 {
        for compiler in strategies() {
            let ir = original_direct_instrument(kind, &a);
            let trace = ir.compile_trace(compiler, &a).unwrap();
            let options = OriginalBoundaryOptions {
                network: OriginalNetworkOptions {
                    resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
                    catalogue: Some(rmagna::catalogue::builtin_phase_catalogue().unwrap()),
                    max_terms: 256,
                    ..Default::default()
                },
                ..Default::default()
            };
            let sampler = trace.prepare_boundary_task(&task, &options, &a).unwrap();
            assert!(sampler.required_reports().is_empty());
            assert_eq!(
                (
                    sampler.layout().postselection_rank(),
                    sampler.layout().feedback_rank(),
                    sampler.layout().trace_rank()
                ),
                (1, 0, 1)
            );
            for (flips, mask) in [
                (vec![], 0),
                (vec!["a".into()], 1),
                (vec!["a".into(), "a".into(), "b".into()], 2),
            ] {
                let assignment = OriginalTraceAssignment {
                    faults: vec![fault("c", 1, OriginalPauli::Y)],
                    report_flips: flips,
                };
                let expected = direct_literal(kind, mask, &a);
                let rational = |v: i64, power| {
                    a.scalar([v.into(), 0.into(), 0.into(), 0.into()], power)
                        .unwrap()
                };
                let expected_acceptance = if kind < 5 {
                    rational(1, -1)
                } else if mask == 2 {
                    rational(3, -2)
                } else {
                    rational(1, -2)
                };
                assert_eq!(expected.1, expected_acceptance);
                let sqrt2_quarter = a
                    .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -2)
                    .unwrap();
                if kind == 3 && mask == 0 {
                    assert_eq!(
                        expected.2,
                        vec![V::zero(), sqrt2_quarter.negated(), sqrt2_quarter.negated()]
                    );
                }
                if kind == 5 && mask == 2 {
                    assert_eq!(
                        expected.2,
                        vec![
                            sqrt2_quarter.negated(),
                            sqrt2_quarter.clone(),
                            rational(1, -2)
                        ]
                    );
                }
                for seed in [0, 19, u64::MAX] {
                    let mut rng = DirectRng {
                        state: seed,
                        calls: 0,
                    };
                    let view = trace.with_assignment(&assignment, &a).unwrap();
                    let draw = sampler.sample(&view, &mut rng, &a).unwrap();
                    assert!(draw.reports.is_empty());
                    assert_eq!(
                        (&draw.mass, &draw.acceptance, &draw.moments),
                        (&expected.0, &expected.1, &expected.2),
                        "kind={kind} compiler={compiler:?} flip={mask}"
                    );
                    assert_eq!((rng.state, rng.calls), (seed, 0));
                    assert_eq!(draw.final_resources.len(), 5);
                    assert!(
                        draw.final_resources
                            .iter()
                            .all(|x| x.constructive_terms > 0 && x.constructive_terms <= 256)
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 126);
}

// Supplemental deterministic branch breaks the original scenario's flip symmetry.
#[test]
fn duplicate_report_flips_cancel_on_deterministic_boundary_postselection() {
    let a = ScalarArithmetic::default();
    for compiler in strategies() {
        let ir = OriginalInstrument::new(
            vec![0],
            vec![0],
            initial(1, &[], &a),
            vec![measure("m", "a", &[(0, OriginalPauli::Z)], false)],
            vec![],
            Default::default(),
            &a,
        )
        .unwrap();
        let trace = ir.compile_trace(compiler, &a).unwrap();
        let task = OriginalBoundaryTask {
            postselection: vec![event(&["a"], false)],
            observables: vec![word(1, 0, 1)],
        };
        let options = OriginalBoundaryOptions {
            network: OriginalNetworkOptions {
                resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        };
        let sampler = trace.prepare_boundary_task(&task, &options, &a).unwrap();
        for (flips, expected) in [
            (vec![], V::one()),
            (vec!["a".into(), "a".into()], V::one()),
            (vec!["a".into()], V::zero()),
            (vec!["a".into(), "a".into(), "a".into()], V::zero()),
        ] {
            let assignment = OriginalTraceAssignment {
                faults: vec![],
                report_flips: flips,
            };
            let mut rng = DirectRng {
                state: 19,
                calls: 0,
            };
            let view = trace.with_assignment(&assignment, &a).unwrap();
            let draw = sampler.sample(&view, &mut rng, &a).unwrap();
            assert_eq!(draw.mass, V::one());
            assert_eq!(
                draw.acceptance, expected,
                "compiler={compiler:?} assignment={assignment:?}"
            );
            assert_eq!(draw.moments, vec![expected]);
            assert!(draw.reports.is_empty());
            assert_eq!((rng.state, rng.calls), (19, 0));
            assert_eq!(draw.final_resources.len(), 3);
        }
    }
}
