// SPDX-License-Identifier: MPL-2.0
// Complete UnifiedTests.test_random_noncommuting_networks, itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_unified_simulator.py.
#[allow(dead_code)]
mod common;
use common::{bits, dense_gate};
use rmagna::clifford::CliffordGate as C;
use rmagna::instrument::{
    OriginalAssignment, OriginalBranchCertificateOptions, OriginalGate as G, OriginalInstrument,
    OriginalLimits, OriginalOperation, OriginalOrigin, OriginalPauli as P,
};
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;

// Full physical matrices retain retired wires as independent tensor legs.
// No stabilizer/path/phase-compiler routine is used in this expected state.
fn pauli(values: &[ExactScalar], wire: usize, kind: P, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut out = vec![ExactScalar::zero(); values.len()];
    for (word, value) in values.iter().enumerate() {
        let flip = matches!(kind, P::X | P::Y);
        let sign = if matches!(kind, P::Z | P::Y) {
            4 * ((word >> wire) & 1) as i64
        } else {
            0
        };
        let phase = sign + if kind == P::Y { 2 } else { 0 };
        out[if flip { word ^ (1 << wire) } else { word }] =
            a.multiply(value, &a.omega(phase, 0).unwrap()).unwrap();
    }
    out
}
fn sum(states: &[AffineKet], word: usize, width: usize, a: &ScalarArithmetic) -> ExactScalar {
    states.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(
            &v,
            &k.prepare()
                .unwrap()
                .amplitude(&bits(width, word), a)
                .unwrap(),
        )
        .unwrap()
    })
}
#[test]
fn original_sixty_noncommuting_networks_match_literal_complex_states_and_reset_order() {
    let a = ScalarArithmetic::default();
    let mut rng = SplitMix64::new(617104);
    let mut count = 0;
    let mut phases = [false; 8];
    let mut kinds = [0usize; 8];
    let mut bases = [0usize; 3];
    let mut zero = 0;
    let mut nonzero = 0;
    let mut multiple_resets = 0;
    for n in 1..4 {
        for trial in 0..20 {
            let mask = (1 << n) - 1;
            let base = rng.next_u64() as usize & mask;
            let linear = (0..n)
                .map(|_| (rng.next_u64() % 4) as u8)
                .collect::<Vec<_>>();
            let upper = (0..n)
                .map(|j| rng.next_u64() as usize & mask & !((1 << (j + 1)) - 1))
                .collect::<Vec<_>>();
            let global = (rng.next_u64() % 8) as usize;
            phases[global] = true;
            let input = AffineKet::new(
                bits(n, base),
                (0..n).map(|j| bits(n, 1 << j)).collect(),
                QuadraticForm::new(
                    0,
                    linear.clone(),
                    upper.iter().map(|&x| bits(n, x)).collect(),
                )
                .unwrap(),
                a.omega(global as i64, -(n as i64)).unwrap(),
            )
            .unwrap();
            let mut values = vec![ExactScalar::zero(); 1 << n];
            for path in 0usize..1 << n {
                let mut q = 0i64;
                for j in 0..n {
                    if path >> j & 1 != 0 {
                        q += i64::from(linear[j]) + 2 * (upper[j] & path).count_ones() as i64;
                    }
                }
                values[base ^ path] = a.omega(global as i64 + 2 * q, -(n as i64)).unwrap();
            }
            let mut live = (0..n).collect::<Vec<_>>();
            let mut retired = vec![];
            let mut operations = vec![];
            let mut records = vec![];
            for j in 0..8 {
                let kind = (rng.next_u64() % if n == 1 { 6 } else { 8 }) as usize;
                kinds[kind] += 1;
                let wire = (rng.next_u64() % n as u64) as usize;
                let slot = live[wire];
                let id = j.to_string();
                let gate = match kind {
                    0 => {
                        dense_gate(C::H(slot), &mut values, &a);
                        G::H(wire as i64)
                    }
                    1 | 2 => {
                        let turns = if rng.next_u64() & 1 == 0 { 1 } else { -1 }
                            * if kind == 1 { 2 } else { 1 };
                        for (word, value) in values.iter_mut().enumerate() {
                            if word >> slot & 1 != 0 {
                                *value = a.multiply(value, &a.omega(turns, 0).unwrap()).unwrap();
                            }
                        }
                        G::Phase {
                            wires: vec![wire as i64],
                            turns,
                        }
                    }
                    3 => {
                        let b = (rng.next_u64() % 3) as usize;
                        bases[b] += 1;
                        let body = [P::X, P::Y, P::Z][b];
                        let outcome = rng.next_u64() & 1 != 0;
                        let transformed = pauli(&values, slot, body, &a);
                        for (value, p) in values.iter_mut().zip(transformed) {
                            *value = a
                                .multiply(
                                    &a.add(value, &if outcome { p.negated() } else { p })
                                        .unwrap(),
                                    &a.omega(0, -2).unwrap(),
                                )
                                .unwrap();
                        }
                        let name = format!("m{j}");
                        records.push((name.clone(), outcome));
                        G::Measure {
                            factors: vec![(wire as i64, body)],
                            inverted: false,
                            record: name,
                        }
                    }
                    4 => {
                        let old_width = values.len().trailing_zeros() as usize;
                        let len = values.len();
                        values.resize(2 * len, ExactScalar::zero());
                        retired.push((wire, slot, id.clone()));
                        live[wire] = old_width;
                        G::Reset {
                            wire: wire as i64,
                            plus: false,
                        }
                    }
                    5 => {
                        values = pauli(&values, slot, P::Y, &a);
                        G::Pauli {
                            wire: wire as i64,
                            pauli: P::Y,
                        }
                    }
                    6 | 7 => {
                        let other = (wire + 1 + (rng.next_u64() % (n - 1) as u64) as usize) % n;
                        let gate = if kind == 6 {
                            C::Cx {
                                control: slot,
                                target: live[other],
                            }
                        } else {
                            C::Cz {
                                a: slot,
                                b: live[other],
                            }
                        };
                        dense_gate(gate, &mut values, &a);
                        if kind == 6 {
                            G::Cx(wire as i64, other as i64)
                        } else {
                            G::Cz(wire as i64, other as i64)
                        }
                    }
                    _ => unreachable!(),
                };
                operations.push(OriginalOperation { id, gate });
            }
            multiple_resets += usize::from(retired.len() > 1);
            let order = live
                .iter()
                .copied()
                .chain(retired.iter().map(|x| x.1))
                .collect::<Vec<_>>();
            let mut expected = vec![ExactScalar::zero(); values.len()];
            for (word, value) in values.into_iter().enumerate() {
                let output = order
                    .iter()
                    .enumerate()
                    .map(|(j, &q)| ((word >> q) & 1) << j)
                    .sum::<usize>();
                expected[output] = value;
            }
            let norm = expected.iter().fold(ExactScalar::zero(), |v, k| {
                a.add(&v, &a.multiply(&k.conjugate(), k).unwrap()).unwrap()
            });
            if norm.is_zero() {
                zero += 1
            } else {
                nonzero += 1
            }
            let ir = OriginalInstrument::new(
                (0..n as i64).collect(),
                (0..n as i64).collect(),
                input,
                operations,
                vec![],
                OriginalLimits::default(),
                &a,
            )
            .unwrap();
            let assigned = OriginalAssignment {
                true_values: records.clone(),
                ..Default::default()
            };
            for compiler in [
                CompilerStrategy::ParityCopies,
                CompilerStrategy::SharedCommutingLayer,
            ] {
                let branch = ir.compile(&assigned, compiler, &a).unwrap();
                assert_eq!(branch.width(), n + retired.len());
                assert_eq!(
                    branch
                        .records()
                        .iter()
                        .map(|r| (r.name.clone(), r.true_value))
                        .collect::<Vec<_>>(),
                    records
                );
                assert_eq!(
                    branch
                        .environment()
                        .iter()
                        .map(|e| (e.wire, e.retired_by.clone()))
                        .collect::<Vec<_>>(),
                    retired
                        .iter()
                        .map(|(wire, _, id)| (*wire as i64, OriginalOrigin::Operation(id.clone())))
                        .collect::<Vec<_>>()
                );
                let ordinary = branch.expansion(&a).unwrap();
                let options = OriginalBranchCertificateOptions {
                    catalogue: Some(rmagna::catalogue::builtin_phase_catalogue().unwrap()),
                    ..Default::default()
                };
                let cert = branch.compile_after_absorption(&options, &a).unwrap();
                for (word, value) in expected.iter().enumerate() {
                    assert_eq!(
                        sum(&ordinary, word, branch.width(), &a),
                        *value,
                        "ordinary n={n} trial={trial} word={word}"
                    );
                    assert_eq!(
                        branch
                            .amplitude(&bits(branch.width(), word), 4096, &a)
                            .unwrap(),
                        *value,
                        "coherent n={n} trial={trial} word={word}"
                    );
                    assert_eq!(
                        sum(cert.states(), word, branch.width(), &a),
                        *value,
                        "absorbed n={n} trial={trial} word={word}"
                    );
                }
                assert_eq!(branch.probability(&a).unwrap(), norm);
                assert_eq!(cert.probability(), &norm);
            }
            count += 1;
        }
    }
    assert_eq!(count, 60);
    assert_eq!(phases, [true; 8]);
    assert!(kinds.iter().all(|&n| n > 0));
    assert!(bases.iter().all(|&n| n > 0));
    assert!(zero > 0 && nonzero > 0 && multiple_resets > 0);
}
