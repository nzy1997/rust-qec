// SPDX-License-Identifier: MPL-2.0
// Adapted from itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/native/causal.cpp and foundation.cpp's online-classical-knowledge group.
// Fixed-width vectors replace trimmed Bits. The dense-span fixture reserves
// width+65 coordinates for the original unseen-high-pivot probe.

#[path = "support/original_gf2.rs"]
#[allow(dead_code)]
mod original_gf2;

use original_gf2::Mt19937;
use rmagna::causal::{CausalError, CausalKnowledge};
use rmagna::gf2::BitVector;

fn mask(width: usize, indices: impl IntoIterator<Item = usize>) -> BitVector {
    BitVector::from_indices(width, indices).unwrap()
}

fn sum(a: &BitVector, b: &BitVector) -> BitVector {
    let mut result = a.clone();
    result.xor_assign(b).unwrap();
    result
}

fn scalar_parity(mask: &BitVector, world: &BitVector, width: usize) -> bool {
    (0..width).fold(false, |value, index| {
        value ^ (mask.get(index).unwrap() && world.get(index).unwrap())
    })
}

#[test]
fn check_pending_semantics() {
    for width in [2, 63, 64, 65, 129] {
        let a = mask(width, [0]);
        let b = mask(width, [width - 1]);
        let expressions = vec![
            (b.clone(), false),
            (a.clone(), true),
            (sum(&a, &b), false),
            (b.clone(), true),
            (mask(width, []), true),
        ];
        let mut known = CausalKnowledge::new(width);
        let request = known.request(&expressions).unwrap();
        assert_eq!(request.pending(), &[b.clone(), a.clone(), sum(&a, &b)]);
        assert_eq!(
            request.outputs(),
            &[
                (Some(0), false),
                (Some(1), true),
                (Some(2), false),
                (Some(0), true),
                (None, true)
            ]
        );
        assert_eq!(request, known.request(&expressions).unwrap());
        assert!(known.rows().is_empty() && known.values().is_empty());
        let result = known.observe(&request, &mask(3, [0, 2])).unwrap();
        assert_eq!(result, [true, true, true, false, true]);
        assert_eq!(known.rows(), &[b.clone(), a.clone()]);
        assert_eq!(known.values(), &[true, false]);
        let after = known.request(&expressions).unwrap();
        assert!(after.pending().is_empty());
        assert_eq!(known.observe(&after, &mask(0, [])).unwrap(), result);
        let mut inconsistent = CausalKnowledge::new(width);
        assert_eq!(
            inconsistent.observe(&request, &mask(1, [0])),
            Err(CausalError::ContradictoryObservation { pending: 2 })
        );
        // The source retains preceding independent rows on contradiction.
        assert_eq!(inconsistent.rows(), &[b, a]);
        assert_eq!(inconsistent.values(), &[true, false]);
        let mut outside = CausalKnowledge::new(width);
        assert!(outside.observe(&request, &mask(4, [3])).is_err());
        assert!(outside.rows().is_empty());
    }
    println!("standalone causal/check_pending_semantics: 10 assertions, 5 width cases");
}

#[test]
fn check_finite_assignments() {
    let mut random = Mt19937::new(202610041);
    let mut batches = 0;
    for width in [0usize, 1, 7, 63, 64, 65, 137] {
        let mut positions = Vec::new();
        for index in [0, 1, 2, 62, 63, 64, 65, width.saturating_sub(1)] {
            if index < width && !positions.contains(&index) {
                positions.push(index);
            }
        }
        for _trial in 0..24 {
            let mut possible = (0..1usize << positions.len())
                .map(|word| {
                    mask(
                        width,
                        positions
                            .iter()
                            .enumerate()
                            .filter_map(|(index, position)| {
                                ((word >> index) & 1 != 0).then_some(*position)
                            }),
                    )
                })
                .collect::<Vec<_>>();
            let truth = possible[random.next() as usize % possible.len()].clone();
            let mut known = CausalKnowledge::new(width);
            for batch in 0..12 {
                let mut expressions = Vec::new();
                for _ in 0..1 + batch % 7 {
                    let selected = positions
                        .iter()
                        .copied()
                        .filter(|_| random.next() & 1 != 0)
                        .collect::<Vec<_>>();
                    expressions.push((mask(width, selected), random.next() & 1 != 0));
                }
                if batch % 3 == 0 {
                    expressions.push(expressions[0].clone());
                }
                let request = known.request(&expressions).unwrap();
                assert_eq!(request, known.request(&expressions).unwrap());
                for (index, (mask, offset)) in expressions.iter().enumerate() {
                    let first = scalar_parity(mask, &possible[0], width) ^ offset;
                    let variable = possible
                        .iter()
                        .any(|world| scalar_parity(mask, world, width) ^ offset != first);
                    assert_eq!(request.outputs()[index].0.is_some(), variable);
                }
                for world in &possible {
                    let pending = request
                        .pending()
                        .iter()
                        .map(|mask| scalar_parity(mask, world, width))
                        .collect::<Vec<_>>();
                    for (index, (mask, offset)) in expressions.iter().enumerate() {
                        let (label, constant) = request.outputs()[index];
                        assert_eq!(
                            constant ^ label.is_some_and(|j| pending[j]),
                            scalar_parity(mask, world, width) ^ offset
                        );
                    }
                }
                let mut expected_rows = known.rows().to_vec();
                let mut expected_values = known.values().to_vec();
                let mut observed = BitVector::zeros(request.pending().len()).unwrap();
                for (index, pending) in request.pending().iter().enumerate() {
                    let value = scalar_parity(pending, &truth, width);
                    observed.set(index, value).unwrap();
                    if possible
                        .iter()
                        .any(|world| scalar_parity(pending, world, width) != value)
                    {
                        expected_rows.push(pending.clone());
                        expected_values.push(value);
                    }
                    possible.retain(|world| scalar_parity(pending, world, width) == value);
                }
                let output = known.observe(&request, &observed).unwrap();
                assert_eq!(known.rows(), expected_rows);
                assert_eq!(known.values(), expected_values);
                for (index, (mask, offset)) in expressions.iter().enumerate() {
                    assert_eq!(output[index], scalar_parity(mask, &truth, width) ^ offset);
                }
                batches += 1;
            }
        }
    }
    assert_eq!(batches, 2016);
    println!(
        "standalone causal/check_finite_assignments: 5 assertions, {batches} batches, rng draws {} fingerprint {:016x}",
        random.draws, random.fingerprint
    );
}

#[test]
fn check_large_request_order() {
    for width in [63, 64, 65, 257] {
        for duplicates in [false, true] {
            let mut expressions = Vec::new();
            let mut pending = Vec::new();
            let mut outputs = Vec::new();
            let mut values = Vec::new();
            let mut expected = Vec::new();
            let mut word = BitVector::zeros(width).unwrap();
            for index in (0..width).rev() {
                let label = width - 1 - index;
                let offset = index & 1 != 0;
                let value = index % 5 < 2;
                let row = mask(width, [index]);
                pending.push(row.clone());
                values.push(value);
                expressions.push((row.clone(), offset));
                outputs.push((Some(label), offset));
                expected.push(value ^ offset);
                word.set(label, value).unwrap();
                if duplicates && index % 3 == 0 {
                    expressions.push((row, !offset));
                    outputs.push((Some(label), !offset));
                    expected.push(value ^ !offset);
                }
            }
            if duplicates {
                expressions.push((mask(width, []), true));
                outputs.push((None, true));
                expected.push(true);
            }
            let mut known = CausalKnowledge::new(width);
            let request = known.request(&expressions).unwrap();
            assert_eq!(request.pending(), pending);
            assert_eq!(request.outputs(), outputs);
            assert_eq!(request, known.request(&expressions).unwrap());
            assert_eq!(known.observe(&request, &word).unwrap(), expected);
            assert_eq!(known.rows(), pending);
            assert_eq!(known.values(), values);
            let after = known.request(&expressions).unwrap();
            assert!(after.pending().is_empty());
            assert_eq!(known.observe(&after, &mask(0, [])).unwrap(), expected);
        }
    }
    println!("standalone causal/check_large_request_order: 6 assertions, 8 width/duplicate cases");
}

#[test]
fn check_dense_spans() {
    let mut random = Mt19937::new(202610042);
    let mut spans = 0;
    for width in [0, 1, 63, 64, 65, 137] {
        let capacity = width + 65;
        let mut known = CausalKnowledge::new(capacity);
        let mut independent = Vec::new();
        let truth = mask(
            capacity,
            (0..width)
                .filter(|_| random.next() & 1 != 0)
                .collect::<Vec<_>>(),
        );
        for _round in 0..9 {
            let next = mask(
                capacity,
                (0..width)
                    .filter(|_| random.next() & 1 != 0)
                    .collect::<Vec<_>>(),
            );
            if known
                .insert(&next, scalar_parity(&next, &truth, width))
                .unwrap()
                .0
            {
                independent.push(next);
            }
            for choice in 0..1usize << independent.len() {
                let row = mask(
                    capacity,
                    (0..width).filter(|coordinate| {
                        independent
                            .iter()
                            .enumerate()
                            .fold(false, |parity, (index, row)| {
                                parity
                                    ^ ((choice >> index) & 1 != 0 && row.get(*coordinate).unwrap())
                            })
                    }),
                );
                let expected = scalar_parity(&row, &truth, width);
                let reduced = known.reduce(&row, false).unwrap();
                assert!(reduced.0.is_zero());
                assert_eq!(reduced.1, expected);
                assert_eq!(known.insert(&row, expected).unwrap(), (false, false));
                assert_eq!(known.insert(&row, !expected).unwrap(), (false, true));
                spans += 1;
            }
        }
        let outside = mask(capacity, [width + 64]);
        assert_eq!(known.reduce(&outside, false).unwrap(), (outside, false));
    }
    println!(
        "standalone causal/check_dense_spans: 4 assertions, {spans} span cases, rng draws {} fingerprint {:016x}",
        random.draws, random.fingerprint
    );
}

#[test]
fn foundation_online_classical_knowledge() {
    let mut causal = CausalKnowledge::new(2);
    let pending = causal
        .request(&[
            (mask(2, [0, 1]), true),
            (mask(2, [0, 1]), false),
            (mask(2, [0]), false),
        ])
        .unwrap();
    assert_eq!(pending.pending(), &[mask(2, [0, 1]), mask(2, [0])]);
    assert_eq!(
        causal.observe(&pending, &mask(1, [0])).unwrap(),
        [false, true, false]
    );
    let known = causal.request(&[(mask(2, [1]), false)]).unwrap();
    assert!(known.pending().is_empty());
    assert_eq!(causal.observe(&known, &mask(0, [])).unwrap(), [true]);
    println!("standalone foundation/main.online-classical-knowledge: 3 assertions");
}

#[test]
fn unknown_high_pivots_stop_reduction_and_word_admission_is_atomic() {
    let mut known = CausalKnowledge::new(65);
    known.insert(&mask(65, [0]), true).unwrap();
    let unknown = mask(65, [0, 64]);
    assert_eq!(known.reduce(&unknown, false).unwrap(), (unknown, false));
    assert!(known.request(&[(mask(64, [0]), false)]).is_err());
    let request = known.request(&[(mask(65, [1]), false)]).unwrap();
    assert!(known.observe(&request, &mask(257, [128])).is_err());
    assert!(known.rows().is_empty());
    assert_eq!(known.observe(&request, &mask(257, [])).unwrap(), [false]);
}
