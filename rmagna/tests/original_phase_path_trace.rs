// SPDX-License-Identifier: MPL-2.0
// Original finite scenarios adapted from itpyi/magna, commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_global_sampling.py: trace_before_decomposition_dense_with_output_effect
// and many_record_phase_resources_cancel_without_decomposition.
// License: fixtures/original_many_record_input.LICENSE. Additional raw-path oracles are independent.
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::phase::{
    ParityPhase, PathTensorError, PathTraceBoundary, PathTraceOptions, PhaseError, PhasePathTensor,
    PreparedPhaseResource,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use rmagna::stabilizer::{AffineKet, StabilizerError};
use sha2::{Digest, Sha256};
#[allow(dead_code)]
mod common;
use common::bits;

#[test]
fn one_sided_events_limits_errors_and_zero_tensors_preserve_input_and_recovery() {
    let a = ScalarArithmetic::default();
    let q = QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap();
    let ket = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        q.clone(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let tensor = PhasePathTensor::new(ket.clone(), ParityPhase::new(1, 0, []).unwrap()).unwrap();
    let before = (tensor.boundary().clone(), tensor.phase().clone());
    let word = PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap();
    let events = [bits(1, 1)];
    let outcomes = bits(1, 0);
    let query = PathTraceBoundary {
        retained_rows: &[],
        event_rows: &events,
        outcomes: &outcomes,
        word: &word,
    };
    let value = tensor
        .trace(query, Default::default(), &a)
        .unwrap()
        .contract_clifford(&a)
        .unwrap()
        .unwrap();
    // <+|Pi0 X|+> = 1/2, while Pi0 X Pi0 is zero.
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    assert_eq!(
        value.prepare().unwrap().amplitude(&bits(0, 0), &a).unwrap(),
        half
    );
    for (options, name) in [
        (
            PathTraceOptions {
                max_physical_bits: 0,
                ..Default::default()
            },
            "path_trace.max_physical_bits",
        ),
        (
            PathTraceOptions {
                max_event_rows: 0,
                ..Default::default()
            },
            "path_trace.max_event_rows",
        ),
        (
            PathTraceOptions {
                max_doubled_path_bits: 1,
                ..Default::default()
            },
            "path_trace.max_doubled_path_bits",
        ),
    ] {
        assert!(
            matches!(tensor.trace(query, options, &a), Err(PathTensorError::Algebra(PhaseError::ResourceLimit { option, .. })) if option == name)
        );
    }
    let error = tensor
        .trace(
            query,
            PathTraceOptions {
                max_geometry_words: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap_err();
    let PathTensorError::Algebra(PhaseError::ResourceLimit {
        option: "path_trace.max_geometry_words",
        required,
        ..
    }) = error
    else {
        panic!("typed geometry admission required")
    };
    let required = usize::try_from(required).unwrap();
    assert!(
        tensor
            .trace(
                query,
                PathTraceOptions {
                    max_geometry_words: required - 1,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert!(
        tensor
            .trace(
                query,
                PathTraceOptions {
                    max_geometry_words: required,
                    ..Default::default()
                },
                &a
            )
            .is_ok()
    );
    let retained = [bits(1, 1)];
    assert!(
        tensor
            .trace(
                PathTraceBoundary {
                    retained_rows: &retained,
                    ..query
                },
                PathTraceOptions {
                    max_retained_rows: 0,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    let wrong_rows = [bits(2, 1)];
    let wrong_word = PauliWord::new(bits(2, 0), bits(2, 0), 0).unwrap();
    let wrong_outcomes = bits(0, 0);
    for bad in [
        PathTraceBoundary {
            retained_rows: &wrong_rows,
            ..query
        },
        PathTraceBoundary {
            word: &wrong_word,
            ..query
        },
        PathTraceBoundary {
            outcomes: &wrong_outcomes,
            ..query
        },
    ] {
        assert!(matches!(
            tensor.trace(bad, Default::default(), &a),
            Err(PathTensorError::Algebra(PhaseError::DimensionMismatch))
        ));
    }
    assert!(PhasePathTensor::new(ket, ParityPhase::new(0, 0, []).unwrap()).is_err());
    let impossible_rows = [bits(1, 0)];
    let impossible_outcomes = bits(1, 1);
    let zero = tensor
        .trace(
            PathTraceBoundary {
                retained_rows: &retained,
                event_rows: &impossible_rows,
                outcomes: &impossible_outcomes,
                ..query
            },
            Default::default(),
            &a,
        )
        .unwrap();
    assert_eq!(zero.boundary().width(), 1);
    assert!(zero.boundary().coefficient().is_zero());
    assert!(zero.contract_clifford(&a).unwrap().is_none());
    let odd = PhasePathTensor::new(
        tensor.boundary().clone(),
        ParityPhase::new(1, 0, [(bits(1, 1), 1)]).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        odd.contract_clifford(&a),
        Err(PathTensorError::Algebra(PhaseError::NotClifford))
    ));
    let large = PhasePathTensor::new(
        AffineKet::new(bits(1, 0), vec![bits(1, 1)], q, a.integer(3).unwrap()).unwrap(),
        ParityPhase::new(1, 0, []).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        large.trace(
            query,
            Default::default(),
            &ScalarArithmetic::new(1).unwrap()
        ),
        Err(PathTensorError::Algebra(PhaseError::Scalar(_)))
    ));
    let overflow = PhasePathTensor::new(
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
            a.scalar([1.into(), 0.into(), 0.into(), 0.into()], i64::MAX)
                .unwrap(),
        )
        .unwrap(),
        ParityPhase::new(1, 0, []).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        overflow.trace(query, Default::default(), &a),
        Err(PathTensorError::Algebra(PhaseError::Stabilizer(
            StabilizerError::Scalar(ScalarError::ExponentOverflow)
        )))
    ));
    assert!(matches!(
        PreparedPhaseResource::new(odd.phase(), 0, Default::default(), None, &a),
        Err(PhaseError::FiberTermLimit { .. })
    ));
    assert_eq!(before, (tensor.boundary().clone(), tensor.phase().clone()));
    assert_eq!(
        tensor
            .trace(query, Default::default(), &a)
            .unwrap()
            .contract_clifford(&a)
            .unwrap()
            .unwrap(),
        value
    );
}
fn trace(
    k: &AffineKet,
    phase: &ParityPhase,
    keep: &[BitVector],
    events: &[BitVector],
    outcomes: &BitVector,
    p: &PauliWord,
    a: &ScalarArithmetic,
) -> Option<(AffineKet, ParityPhase)> {
    let tensor = PhasePathTensor::new(k.clone(), phase.clone()).unwrap();
    let result = tensor
        .trace(
            PathTraceBoundary {
                retained_rows: keep,
                event_rows: events,
                outcomes,
                word: p,
            },
            Default::default(),
            a,
        )
        .unwrap();
    Some((result.boundary().clone(), result.phase().clone()))
}
#[test]
fn original_noninjective_trace_matches_all_retained_pauli_and_bra_event_cases() {
    let a = ScalarArithmetic::default();
    let n = 4;
    let boundary = [(1usize, true), (6, false), (8, true)];
    let columns = (0..n)
        .map(|j| {
            BitVector::from_indices(
                3,
                boundary
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (row, _))| ((row >> j) & 1 != 0).then_some(i)),
            )
            .unwrap()
        })
        .collect();
    let k = AffineKet::new(
        bits(3, 5),
        columns,
        QuadraticForm::new(0, vec![0; n], vec![BitVector::zeros(n).unwrap(); n]).unwrap(),
        a.omega(3, -4).unwrap(),
    )
    .unwrap();
    let phase = ParityPhase::new(
        n,
        0,
        [
            (bits(n, 3), 1),
            (bits(n, 4), 7),
            (bits(n, 12), 2),
            (bits(n, 8), 4),
        ],
    )
    .unwrap();
    let mut dense = vec![ExactScalar::zero(); 8];
    for u in 0..16usize {
        let word = boundary.iter().enumerate().fold(0, |w, (j, (row, b))| {
            w | ((((row & u).count_ones() as usize & 1) ^ usize::from(*b)) << j)
        });
        let power = 3 + [(3usize, 1), (4, 7), (12, 2), (8, 4)]
            .iter()
            .map(|(r, c)| c * ((r & u).count_ones() as i64 & 1))
            .sum::<i64>();
        dense[word] = a.add(&dense[word], &a.omega(power, -4).unwrap()).unwrap();
    }
    let mut cases = 0;
    let mut labels_checked = 0;
    let mut max_rank = 0;
    let mut max_paths = 0;
    for keep in [vec![], vec![1, 2], vec![3]] {
        for (x, z) in [(0usize, 0usize), (4, 0), (4, 4), (0, 6)] {
            for bit in 0..2 {
                let rows = keep.iter().map(|&r| bits(3, r)).collect::<Vec<_>>();
                let word =
                    PauliWord::new(bits(3, x), bits(3, z), 2 * (x & z).count_ones() as u8).unwrap();
                let result = trace(&k, &phase, &rows, &[bits(3, 1)], &bits(1, bit), &word, &a);
                let mut actual = vec![ExactScalar::zero(); 1 << keep.len()];
                if let Some((tensor, p)) = result {
                    max_paths = max_paths.max(tensor.columns().len());
                    let resource =
                        PreparedPhaseResource::new(&p, 64, Default::default(), None, &a).unwrap();
                    max_rank = max_rank.max(resource.quotient_rank());
                    for term in resource.expand(&p, &tensor, &a).unwrap() {
                        let prepared = term.prepare().unwrap();
                        for (j, v) in actual.iter_mut().enumerate() {
                            *v = a
                                .add(v, &prepared.amplitude(&bits(keep.len(), j), &a).unwrap())
                                .unwrap();
                        }
                    }
                }
                let mut expected = vec![ExactScalar::zero(); 1 << keep.len()];
                for (ket, amp) in dense.iter().enumerate() {
                    let bra = ket ^ x;
                    if bra & 1 != bit {
                        continue;
                    }
                    let label = keep.iter().enumerate().fold(0, |v, (j, &row)| {
                        v | (((row & bra).count_ones() as usize & 1) << j)
                    });
                    let factor = a
                        .omega(
                            2 * i64::from((x & z).count_ones())
                                + 4 * i64::from((z & ket).count_ones() & 1),
                            0,
                        )
                        .unwrap();
                    let v = a
                        .multiply(&a.multiply(&dense[bra].conjugate(), amp).unwrap(), &factor)
                        .unwrap();
                    expected[label] = a.add(&expected[label], &v).unwrap();
                }
                assert_eq!(actual, expected, "keep={keep:?},x={x},z={z},bit={bit}");
                labels_checked += actual.len();
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 24);
    assert_eq!(labels_checked, 56);
    println!(
        "All {cases} original generic noninjective NormalForm trace cases exact; max retained paths={max_paths}, quotient rank={max_rank}. Symbolic trace itself uses only pairing/GF2/phase pullback; explicit evaluation resource separate."
    );
}

fn compress_clifford(k: AffineKet, p: ParityPhase, a: &ScalarArithmetic) -> Option<AffineKet> {
    PhasePathTensor::new(k, p)
        .unwrap()
        .contract_clifford(a)
        .unwrap()
}
#[test]
fn original_wide_path_cancellation_requires_no_phase_resource() {
    let a = ScalarArithmetic::default();
    let n = 129;
    let k = AffineKet::new(
        BitVector::zeros(n).unwrap(),
        original_boundary_columns(),
        QuadraticForm::new(0, vec![0; n], vec![BitVector::zeros(n).unwrap(); n]).unwrap(),
        a.omega(0, -129).unwrap(),
    )
    .unwrap();
    let p = ParityPhase::new(
        n,
        0,
        (0..n).map(|j| (BitVector::from_indices(n, [j]).unwrap(), 1)),
    )
    .unwrap();
    let word = PauliWord::new(
        BitVector::zeros(n).unwrap(),
        BitVector::zeros(n).unwrap(),
        0,
    )
    .unwrap();
    let (tensor, phase) = trace(
        &k,
        &p,
        &[BitVector::from_indices(n, [1, 2]).unwrap()],
        &[],
        &bits(0, 0),
        &word,
        &a,
    )
    .unwrap();
    assert!(phase.terms().is_empty());
    let reduced = compress_clifford(tensor, phase, &a).unwrap();
    assert_eq!(reduced.width(), 1);
    assert_eq!(reduced.columns().len(), 1);
    assert!(reduced.phase().linear().iter().all(|c| *c == 0));
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    for j in 0..2 {
        assert_eq!(
            reduced
                .prepare()
                .unwrap()
                .amplitude(&bits(1, j), &a)
                .unwrap(),
            half
        );
    }
    for y in [false, true] {
        let word = PauliWord::new(
            BitVector::from_indices(n, [0]).unwrap(),
            BitVector::from_indices(n, y.then_some(0)).unwrap(),
            if y { 2 } else { 0 },
        )
        .unwrap();
        let (k, p) = trace(&k, &p, &[], &[], &bits(0, 0), &word, &a).unwrap();
        let value = compress_clifford(k, p, &a).unwrap();
        assert_eq!(
            value.coefficient(),
            &a.scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
                .unwrap()
        );
        assert_eq!(value.width(), 0);
        assert!(value.columns().is_empty());
    }
    println!(
        "Original generic129-path normal form also traces/compresses to1uniform label and exactX/Ysqrt2/2 using only algebraic Clifford reduction; NO phase-resource construction for many-record case."
    );
}

fn rnd(s: &mut u64) -> usize {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s as usize
}
fn literal(k: &AffineKet, p: &ParityPhase, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 1 << k.width()];
    for u in 0..(1usize << k.columns().len()) {
        let mut label = k.offset().clone();
        for (j, col) in k.columns().iter().enumerate() {
            if u >> j & 1 != 0 {
                label.xor_assign(col).unwrap();
            }
        }
        let index = label.ones().fold(0, |m, j| m | (1 << j));
        let q = k.phase();
        let mut turn = i64::from(q.constant());
        for (j, c) in q.linear().iter().enumerate() {
            turn += i64::from(*c) * ((u >> j & 1) as i64);
        }
        for (j, row) in q.upper().iter().enumerate() {
            for t in row.ones() {
                turn += 2 * ((u >> j & u >> t & 1) as i64);
            }
        }
        let mut exponent = i64::from(p.constant());
        for (axis, c) in p.terms() {
            let mask = axis.ones().fold(0, |m, j| m | (1 << j));
            exponent += i64::from(*c) * ((u & mask).count_ones() as i64 & 1);
        }
        let v = a
            .multiply(k.coefficient(), &a.omega(2 * turn + exponent, 0).unwrap())
            .unwrap();
        values[index] = a.add(&values[index], &v).unwrap();
    }
    values
}
#[test]
fn generic_quadratic_and_complex_tensors_match_independent_raw_path_definitions() {
    let a = ScalarArithmetic::default();
    let mut seed = 2026102201;
    for case in 0..120 {
        let m = case % 5;
        let n = (case / 5) % 5;
        let mut upper = vec![bits(n, 0); n];
        for (j, row) in upper.iter_mut().enumerate() {
            for t in j + 1..n {
                row.set(t, rnd(&mut seed) & 1 != 0).unwrap();
            }
        }
        let q = QuadraticForm::new(
            (rnd(&mut seed) & 3) as u8,
            (0..n).map(|_| (rnd(&mut seed) & 3) as u8).collect(),
            upper,
        )
        .unwrap();
        let coefficient = if case % 17 == 0 {
            ExactScalar::zero()
        } else {
            a.omega((rnd(&mut seed) & 7) as i64, -3).unwrap()
        };
        let k = AffineKet::new(
            bits(m, rnd(&mut seed)),
            (0..n).map(|_| bits(m, rnd(&mut seed))).collect(),
            q,
            coefficient,
        )
        .unwrap();
        let p = ParityPhase::new(
            n,
            (rnd(&mut seed) & 7) as u8,
            (0..4).map(|_| (bits(n, rnd(&mut seed)), (rnd(&mut seed) & 7) as u8)),
        )
        .unwrap();
        let keep = (0..case % 4)
            .map(|_| bits(m, rnd(&mut seed)))
            .collect::<Vec<_>>();
        let event = bits(m, rnd(&mut seed));
        let outcome = case & 1;
        let x = rnd(&mut seed) & ((1 << m) - 1);
        let z = rnd(&mut seed) & ((1 << m) - 1);
        let ph = (rnd(&mut seed) & 7) as u8;
        let word = PauliWord::new(bits(m, x), bits(m, z), ph).unwrap();
        let result = trace(
            &k,
            &p,
            &keep,
            std::slice::from_ref(&event),
            &bits(1, outcome),
            &word,
            &a,
        );
        let mut actual = vec![ExactScalar::zero(); 1 << keep.len()];
        if let Some((tensor, p)) = result {
            let r = PreparedPhaseResource::new(&p, 256, Default::default(), None, &a).unwrap();
            for term in r.expand(&p, &tensor, &a).unwrap() {
                let prep = term.prepare().unwrap();
                for (j, v) in actual.iter_mut().enumerate() {
                    *v = a
                        .add(v, &prep.amplitude(&bits(keep.len(), j), &a).unwrap())
                        .unwrap();
                }
            }
        }
        let dense = literal(&k, &p, &a);
        let mut expected = vec![ExactScalar::zero(); 1 << keep.len()];
        for (ket, amp) in dense.iter().enumerate() {
            let bra = ket ^ x;
            let bv = bits(m, bra);
            if usize::from(event.dot(&bv).unwrap()) != outcome {
                continue;
            }
            let label = keep.iter().enumerate().fold(0, |v, (j, row)| {
                v | (usize::from(row.dot(&bv).unwrap()) << j)
            });
            let sign = a
                .omega(i64::from(ph) + 4 * i64::from((z & ket).count_ones() & 1), 0)
                .unwrap();
            let v = a
                .multiply(&a.multiply(&dense[bra].conjugate(), amp).unwrap(), &sign)
                .unwrap();
            expected[label] = a.add(&expected[label], &v).unwrap();
        }
        assert_eq!(actual, expected, "randomcase={case},physical={m},paths={n}");
    }
    println!(
        "120 additional generic symbolic traces equal independent exact rawpath/physical definitions incl nonzeroQ,globalcomplexphase,zero/empty/dependent paths,arbitraryPauli phase and one-sided noncommuting braevents."
    );
}
#[test]
fn wide_physical_labels_match_sparse_logical_path_definitions() {
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for m in [63usize, 64, 65, 127, 128, 129, 193] {
        let b = |v: &[usize]| BitVector::from_indices(m, v.iter().copied()).unwrap();
        let c0 = b(&[0, m - 1]);
        let c1 = b(&[m / 2]);
        let q = QuadraticForm::new(
            3,
            vec![1, 2, 3, 0],
            vec![bits(4, 8), bits(4, 4), bits(4, 0), bits(4, 0)],
        )
        .unwrap();
        let coefficient = a
            .scalar([3.into(), (-1).into(), 1.into(), 2.into()], -3)
            .unwrap();
        let k = AffineKet::new(
            b(&[0, m / 2]),
            vec![c0.clone(), c1, c0, BitVector::zeros(m).unwrap()],
            q,
            coefficient,
        )
        .unwrap();
        let p = ParityPhase::new(
            4,
            7,
            [
                (bits(4, 3), 1),
                (bits(4, 4), 7),
                (bits(4, 12), 2),
                (bits(4, 8), 4),
            ],
        )
        .unwrap();
        let mut dense = std::collections::HashMap::<BitVector, ExactScalar>::new();
        for u in 0..16usize {
            let mut label = k.offset().clone();
            for (j, col) in k.columns().iter().enumerate() {
                if u >> j & 1 != 0 {
                    label.xor_assign(col).unwrap();
                }
            }
            let exponent = 2
                * (3 + (u & 1)
                    + 2 * (u >> 1 & 1)
                    + 3 * (u >> 2 & 1)
                    + 2 * ((u & 1) * (u >> 3 & 1) + (u >> 1 & 1) * (u >> 2 & 1)))
                    as i64
                + 7
                + [(3usize, 1), (4, 7), (12, 2), (8, 4)]
                    .iter()
                    .map(|(r, c)| c * ((r & u).count_ones() as i64 & 1))
                    .sum::<i64>();
            let amp = a
                .multiply(k.coefficient(), &a.omega(exponent, 0).unwrap())
                .unwrap();
            let previous = dense.remove(&label).unwrap_or_else(ExactScalar::zero);
            dense.insert(label, a.add(&previous, &amp).unwrap());
        }
        for y in [false, true] {
            for bit in 0..2 {
                let x = b(&[0, m - 1]);
                let z = if y {
                    b(&[m / 2, m - 1])
                } else {
                    BitVector::zeros(m).unwrap()
                };
                let ph = if y { 2 } else { 0 };
                let word = PauliWord::new(x.clone(), z.clone(), ph).unwrap();
                let keep = [b(&[m - 1]), b(&[m / 2, m - 1]), b(&[m - 1])];
                let event = b(&[m / 2, m - 1]);
                let result = trace(
                    &k,
                    &p,
                    &keep,
                    std::slice::from_ref(&event),
                    &bits(1, bit),
                    &word,
                    &a,
                );
                let mut actual = vec![ExactScalar::zero(); 8];
                if let Some((tensor, p)) = result {
                    let r =
                        PreparedPhaseResource::new(&p, 256, Default::default(), None, &a).unwrap();
                    for term in r.expand(&p, &tensor, &a).unwrap() {
                        let prep = term.prepare().unwrap();
                        for (j, v) in actual.iter_mut().enumerate() {
                            *v = a.add(v, &prep.amplitude(&bits(3, j), &a).unwrap()).unwrap();
                        }
                    }
                }
                let mut expected = vec![ExactScalar::zero(); 8];
                for (ket, amp) in &dense {
                    let mut bra = ket.clone();
                    bra.xor_assign(&x).unwrap();
                    if usize::from(event.dot(&bra).unwrap()) != bit {
                        continue;
                    }
                    let Some(left) = dense.get(&bra) else {
                        continue;
                    };
                    let label = keep
                        .iter()
                        .enumerate()
                        .fold(0, |v, (j, r)| v | (usize::from(r.dot(&bra).unwrap()) << j));
                    let phase = a
                        .omega(i64::from(ph) + 4 * i64::from(z.dot(ket).unwrap()), 0)
                        .unwrap();
                    let value = a
                        .multiply(&a.multiply(&left.conjugate(), amp).unwrap(), &phase)
                        .unwrap();
                    expected[label] = a.add(&expected[label], &value).unwrap();
                }
                assert_eq!(actual, expected, "wide physical={m},Y={y},outcome={bit}");
                cases += 1;
            }
        }
    }
    println!(
        "{cases} additional wide physical boundary cases exact at63/64/65/127/128/129/193bits via independent sparse16-path definitions, no physical-state enumeration."
    );
}

fn original_boundary_columns() -> Vec<BitVector> {
    let input_source = include_str!("fixtures/original_many_record_input.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(input_source.as_bytes())),
        "0b165ce97bd2c21601090d61eaa5bd8940d77011dc39ca1ebf9dfb31f8ca8837"
    );
    let input: serde_json::Value = serde_json::from_str(input_source).unwrap();
    let rows = input["physical_boundary_hex"].as_array().unwrap();
    let order = input["native_boundary_order"].as_array().unwrap();
    assert_eq!(rows.len(), 129);
    assert_eq!(order.len(), 129);
    (0..129)
        .map(|j| {
            BitVector::from_indices(
                129,
                order.iter().enumerate().filter_map(|(i, r)| {
                    let s = rows[r.as_u64().unwrap() as usize]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x");
                    s.chars()
                        .rev()
                        .nth(j / 4)
                        .is_some_and(|c| c.to_digit(16).unwrap() >> (j % 4) & 1 != 0)
                        .then_some(i)
                }),
            )
            .unwrap()
        })
        .collect()
}
