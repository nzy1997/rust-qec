use rmagna::{
    clifford::CliffordGate as C,
    gf2::BitVector,
    pauli::PauliWord,
    scalar::{ExactScalar as E, ScalarArithmetic as A},
    unitary::{CompilerStrategy, DensityLayout, PauliInsertion, UnitaryCircuit, UnitaryGate as G},
};
fn bits(n: usize, m: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| m >> j & 1 != 0)).unwrap()
}
#[test]
fn native_zero_certificates_need_no_phase_plan_and_program_changes_are_errors() {
    let a = A::default();
    let circuit = UnitaryCircuit::new(3, []).unwrap();
    let reference = circuit.compile(&bits(3, 0), &a).unwrap();
    let other = circuit.compile(&bits(3, 0), &a).unwrap();
    let row = [bits(3, 1)];
    let word = PauliWord::new(bits(3, 1), bits(3, 0), 1).unwrap();
    let mut options = rmagna::phase::NativePhaseOptions::default();
    options.selection.max_core_table_bytes = 0;
    let plan = reference
        .prepare_pauli_moment(&word, &row, &a)
        .unwrap()
        .with_native_phase_resource(0, options, None, &a)
        .unwrap();
    assert!(plan.has_zero_certificate());
    assert!(plan.fixed_phase_resource().is_none());
    assert_eq!(
        plan.evaluate(&reference, &bits(1, 1), 0, &a).unwrap().value,
        E::zero()
    );
    assert!(matches!(
        plan.evaluate(&other, &bits(1, 0), 0, &a),
        Err(rmagna::unitary::UnitaryError::IncompatibleProgram)
    ));
}
fn add(a: &A, x: &E, y: &E) -> E {
    a.add(x, y).unwrap()
}
fn mul(a: &A, x: &E, y: &E) -> E {
    a.multiply(x, y).unwrap()
}
fn parity(x: usize, m: usize) -> usize {
    ((x & m).count_ones() & 1) as usize
}
fn selected(x: usize, rows: &[usize], out: usize) -> bool {
    rows.iter()
        .enumerate()
        .all(|(j, &m)| parity(x, m) == out >> j & 1)
}
fn pauli(a: &A, v: &[E], x: usize, z: usize, p: u8) -> Vec<E> {
    let mut r = vec![E::zero(); v.len()];
    for (j, c) in v.iter().enumerate() {
        r[j ^ x] = mul(
            a,
            c,
            &a.omega(i64::from(p) + 4 * parity(j, z) as i64, 0).unwrap(),
        );
    }
    r
}
fn literal(a: &A, gates: &[G], fault: Option<(usize, usize, usize, u8)>) -> Vec<E> {
    let mut v = vec![E::zero(); 8];
    v[0] = E::one();
    for t in 0..=gates.len() {
        if let Some((time, x, z, p)) = fault
            && time == t
        {
            v = pauli(a, &v, x, z, p);
        }
        if t == gates.len() {
            break;
        }
        match gates[t] {
            G::Clifford(C::H(q)) => {
                let old = v.clone();
                let h = a.omega(0, -1).unwrap();
                for j in 0..8 {
                    let l = &old[j & !(1 << q)];
                    let r = &old[j | (1 << q)];
                    v[j] = mul(
                        a,
                        &add(
                            a,
                            l,
                            &if j >> q & 1 == 0 {
                                r.clone()
                            } else {
                                r.negated()
                            },
                        ),
                        &h,
                    );
                }
            }
            G::Clifford(C::Cx { control, target }) => {
                let old = v.clone();
                for j in 0..8 {
                    v[j ^ if j >> control & 1 != 0 {
                        1 << target
                    } else {
                        0
                    }] = old[j].clone();
                }
            }
            G::T { qubit, turns } => {
                for (j, c) in v.iter_mut().enumerate() {
                    if j >> qubit & 1 != 0 {
                        *c = mul(a, c, &a.omega(i64::from(turns), 0).unwrap());
                    }
                }
            }
            G::Cs { a: q, b: r, turns } => {
                for (j, c) in v.iter_mut().enumerate() {
                    if j >> q & 1 != 0 && j >> r & 1 != 0 {
                        *c = mul(a, c, &a.omega(2 * i64::from(turns), 0).unwrap());
                    }
                }
            }
            G::Ccz { a: q, b: r, c: s } => {
                for (j, c) in v.iter_mut().enumerate() {
                    if j >> q & 1 != 0 && j >> r & 1 != 0 && j >> s & 1 != 0 {
                        *c = c.negated();
                    }
                }
            }
            _ => panic!("unmodelled literal gate"),
        }
    }
    v
}
fn probability(a: &A, v: &[E], rows: &[usize], out: usize) -> E {
    v.iter()
        .enumerate()
        .filter(|(x, _)| selected(*x, rows, out))
        .fold(E::zero(), |s, (_, c)| {
            add(a, &s, &mul(a, c, &c.conjugate()))
        })
}
fn moment(a: &A, v: &[E], rows: &[usize], out: usize, x: usize, z: usize, p: u8) -> E {
    let filtered: Vec<_> = v
        .iter()
        .enumerate()
        .map(|(j, c)| {
            if selected(j, rows, out) {
                c.clone()
            } else {
                E::zero()
            }
        })
        .collect();
    let right = pauli(a, &filtered, x, z, p);
    filtered.iter().zip(right).fold(E::zero(), |s, (l, r)| {
        add(a, &s, &mul(a, &l.conjugate(), &r))
    })
}
fn packed(x: usize, outputs: &[usize]) -> usize {
    outputs
        .iter()
        .enumerate()
        .map(|(j, q)| ((x >> q) & 1) << j)
        .sum()
}
fn density(a: &A, v: &[E], outputs: &[usize], rows: &[usize], out: usize) -> Vec<E> {
    let d = 1 << outputs.len();
    let outside: Vec<_> = (0..3).filter(|q| !outputs.contains(q)).collect();
    let mut r = vec![E::zero(); d * d];
    for x in 0..8 {
        for y in 0..8 {
            if selected(x, rows, out)
                && selected(y, rows, out)
                && packed(x, &outside) == packed(y, &outside)
            {
                let j = packed(x, outputs) * d + packed(y, outputs);
                r[j] = add(a, &r[j], &mul(a, &v[x], &v[y].conjugate()));
            }
        }
    }
    r
}
#[test]
fn native_query_families_match_literal_faulted_state_vectors() {
    let a = A::new(512).unwrap();
    let mut counts = [0_usize; 5];
    let circuits = vec![
        vec![
            G::Clifford(C::H(0)),
            G::T { qubit: 0, turns: 1 },
            G::Clifford(C::H(0)),
            G::Clifford(C::Cx {
                control: 0,
                target: 1,
            }),
        ],
        vec![
            G::Clifford(C::H(0)),
            G::Clifford(C::H(1)),
            G::Clifford(C::H(2)),
            G::T { qubit: 0, turns: 1 },
            G::T { qubit: 1, turns: 1 },
            G::Clifford(C::H(0)),
        ],
        vec![
            G::Clifford(C::H(0)),
            G::Clifford(C::H(1)),
            G::Clifford(C::H(2)),
            G::Ccz { a: 0, b: 1, c: 2 },
            G::Cs {
                a: 0,
                b: 1,
                turns: 1,
            },
            G::Clifford(C::H(1)),
        ],
    ];
    let tg = vec![
        G::Clifford(C::H(0)),
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::Clifford(C::H(2)),
    ];
    let tv = literal(&a, &tg, None);
    for injection in [
        None,
        Some(rmagna::catalogue::PhaseInjectionOptions::default()),
    ] {
        let mut native_options = rmagna::phase::NativePhaseOptions {
            injection,
            ..Default::default()
        };
        native_options.selection.max_quotient_rank = 12;
        native_options.selection.preparation.max_terms = 65536;
        let db = Some(rmagna::catalogue::builtin_phase_catalogue().unwrap());
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let target = UnitaryCircuit::new(3, tg.clone())
                .unwrap()
                .compile_with_strategy(&bits(3, 0), &a, strategy)
                .unwrap();
            for gates in &circuits {
                let reference = UnitaryCircuit::new(3, gates.clone())
                    .unwrap()
                    .compile_with_strategy(&bits(3, 0), &a, strategy)
                    .unwrap();
                let label_rows = vec![bits(3, 1), bits(3, 3), bits(3, 2)];
                let support = reference
                    .prepare_probability_support_with_native(
                        &label_rows,
                        65536,
                        native_options,
                        db,
                        &a,
                    )
                    .unwrap();
                for fault in [
                    None,
                    Some((1, 1, 0, 0)),
                    Some((gates.len(), 2, 1, 3)),
                    Some((2, 4, 4, 2)),
                ] {
                    let state = if let Some((before_gate, x, z, p)) = fault {
                        reference
                            .with_insertions(
                                &[PauliInsertion {
                                    before_gate,
                                    word: PauliWord::new(bits(3, x), bits(3, z), p).unwrap(),
                                }],
                                &a,
                            )
                            .unwrap()
                    } else {
                        reference.with_insertions(&[], &a).unwrap()
                    };
                    let v = literal(&a, gates, fault);
                    for (x, expected) in v.iter().enumerate() {
                        assert_eq!(
                            state.amplitude(&bits(3, x), 65536, &a).unwrap().value,
                            *expected
                        );
                        counts[0] += 1;
                    }
                    for rm in [vec![], vec![1, 3], vec![1, 1]] {
                        let rows: Vec<_> = rm.iter().map(|&m| bits(3, m)).collect();
                        let plan = reference
                            .prepare_probability(&rows, &a)
                            .unwrap()
                            .with_native_phase_resource(65536, native_options, db, &a)
                            .unwrap();
                        for out in 0..1 << rm.len() {
                            let expected = probability(&a, &v, &rm, out);
                            assert_eq!(
                                state
                                    .probability_z(&rows, &bits(rm.len(), out), 65536, &a)
                                    .unwrap()
                                    .value,
                                expected
                            );
                            assert_eq!(
                                plan.evaluate(&state, &bits(rm.len(), out), 65536, &a)
                                    .unwrap()
                                    .value,
                                expected
                            );
                            counts[1] += 1;
                        }
                    }
                    for (x, z, p) in [(0, 0, 0), (1, 0, 1), (1, 1, 2), (0, 2, 3)] {
                        let rows = [bits(3, 1)];
                        let word = PauliWord::new(bits(3, x), bits(3, z), p).unwrap();
                        let plan = reference
                            .prepare_pauli_moment(&word, &rows, &a)
                            .unwrap()
                            .with_native_phase_resource(65536, native_options, db, &a)
                            .unwrap();
                        for out in 0..2 {
                            let expected = moment(&a, &v, &[1], out, x, z, p);
                            assert_eq!(
                                state
                                    .pauli_moment(&word, &rows, &bits(1, out), 65536, &a)
                                    .unwrap()
                                    .value,
                                expected
                            );
                            assert_eq!(
                                plan.evaluate(&state, &bits(1, out), 65536, &a)
                                    .unwrap()
                                    .value,
                                expected
                            );
                            counts[2] += 1;
                        }
                    }
                    let lr = [bits(3, 1)];
                    let rr = [bits(3, 2)];
                    let plan = reference
                        .prepare_density_overlap(
                            DensityLayout {
                                outputs: &[0, 2],
                                event_rows: &lr,
                            },
                            &target,
                            DensityLayout {
                                outputs: &[2, 0],
                                event_rows: &rr,
                            },
                            &a,
                        )
                        .unwrap()
                        .with_native_phase_resource(65536, native_options, db, &a)
                        .unwrap();
                    for lo in 0..2 {
                        for ro in 0..2 {
                            let left = density(&a, &v, &[0, 2], &[1], lo);
                            let right = density(&a, &tv, &[2, 0], &[2], ro);
                            let mut expected = E::zero();
                            for x in 0..4 {
                                for y in 0..4 {
                                    expected = add(
                                        &a,
                                        &expected,
                                        &mul(&a, &left[x * 4 + y], &right[y * 4 + x]),
                                    );
                                }
                            }
                            assert_eq!(
                                plan.evaluate(
                                    &state,
                                    &target,
                                    &bits(1, lo),
                                    &bits(1, ro),
                                    65536,
                                    &a
                                )
                                .unwrap()
                                .value,
                                expected
                            );
                            counts[3] += 1;
                        }
                    }
                    let terms = support.evaluate(&state, 65536, &a).unwrap();
                    for label in 0..8 {
                        let expected = v
                            .iter()
                            .enumerate()
                            .filter(|(x, _)| {
                                parity(*x, 1) | (parity(*x, 3) << 1) | (parity(*x, 2) << 2) == label
                            })
                            .fold(E::zero(), |s, (_, c)| {
                                add(&a, &s, &mul(&a, c, &c.conjugate()))
                            });
                        let got = terms.iter().fold(E::zero(), |s, t| {
                            add(
                                &a,
                                &s,
                                &t.prepare().unwrap().amplitude(&bits(3, label), &a).unwrap(),
                            )
                        });
                        assert_eq!(got, expected);
                        counts[4] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(counts, [384, 432, 384, 192, 384]);
}
