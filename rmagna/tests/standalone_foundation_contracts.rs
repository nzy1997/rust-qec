// SPDX-License-Identifier: MPL-2.0
// Complete schedules adapted from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/native/foundation.cpp (Mozilla Public License 2.0). Finite-sum and
// bit-by-bit oracles retain raw inputs, independent of production elimination.
#[path = "support/original_gf2.rs"]
#[allow(dead_code)]
mod original_gf2;
use original_gf2::Mt19937;
use rmagna::{
    gf2::{AffineSupport, BitVector, LinearFactorization, transpose_columns, xors},
    phase::ParityPhase,
    quadratic::QuadraticForm,
    scalar::{ExactScalar, ScalarArithmetic},
    stabilizer::{AffineKet, PreparedAffineContraction},
};
use std::collections::BTreeSet;
fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|j| word >> j & 1 != 0),
    )
    .unwrap()
}
fn index(row: &BitVector) -> usize {
    row.ones().fold(0, |v, j| v | (1usize << j))
}
fn scalar_q(word: &BitVector, linear: &[u8], upper: &[BitVector]) -> i64 {
    let mut q = 0;
    for j in 0..linear.len() {
        if !word.get(j).unwrap() {
            continue;
        }
        q += i64::from(linear[j]);
        for k in j + 1..linear.len() {
            if word.get(k).unwrap() && upper[j].get(k).unwrap() {
                q += 2;
            }
        }
    }
    q.rem_euclid(4)
}
#[derive(Clone)]
struct Input {
    width: usize,
    offset: usize,
    columns: Vec<usize>,
    linear: Vec<u8>,
    upper: Vec<BitVector>,
    phase: i64,
    power: i64,
}
impl Input {
    fn ket(&self, a: &ScalarArithmetic) -> AffineKet {
        AffineKet::new(
            bits(self.width, self.offset),
            self.columns.iter().map(|&c| bits(self.width, c)).collect(),
            QuadraticForm::new(0, self.linear.clone(), self.upper.clone()).unwrap(),
            a.omega(self.phase, self.power).unwrap(),
        )
        .unwrap()
    }
    fn dense(&self, a: &ScalarArithmetic) -> Vec<ExactScalar> {
        let mut out = vec![ExactScalar::zero(); 1 << self.width];
        for u in 0..1usize << self.columns.len() {
            let mut label = self.offset;
            let mut phase = self.phase;
            for j in 0..self.columns.len() {
                if u >> j & 1 == 0 {
                    continue;
                }
                label ^= self.columns[j];
                phase += 2 * i64::from(self.linear[j]);
                for k in j + 1..self.columns.len() {
                    if u >> k & 1 != 0 && self.upper[j].get(k).unwrap() {
                        phase += 4;
                    }
                }
            }
            out[label] = a
                .add(&out[label], &a.omega(phase, self.power).unwrap())
                .unwrap();
        }
        out
    }
}
fn amplitudes(ket: Option<&AffineKet>, width: usize, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let prepared = ket.map(|k| k.prepare().unwrap());
    (0..1usize << width)
        .map(|word| {
            prepared.as_ref().map_or(ExactScalar::zero(), |k| {
                k.amplitude(&bits(width, word), a).unwrap()
            })
        })
        .collect()
}
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0 >> 32
    }
}
#[test]
fn check_reference_coordinates() {
    let mut rng = Lcg(928271);
    let mut cases = 0;
    for width in 0..=7 {
        for count in 0..=7 {
            let words = (0..count)
                .map(|_| rng.next() as usize & ((1 << width) - 1))
                .collect::<Vec<_>>();
            let basis = words.iter().map(|&w| bits(width, w)).collect::<Vec<_>>();
            let map = LinearFactorization::new(count, &transpose_columns(width, &basis).unwrap())
                .unwrap();
            // Exhaustive coefficient words in numeric order yield the free-zero
            // low-pivot solution, not a second Gaussian implementation.
            let image = (0..1usize << count)
                .map(|choice| {
                    words
                        .iter()
                        .enumerate()
                        .fold(0, |v, (j, &b)| if choice >> j & 1 != 0 { v ^ b } else { v })
                })
                .collect::<Vec<_>>();
            for word in 0..1usize << width {
                let expected = image.iter().position(|&point| point == word);
                let actual = map.solve(&bits(width, word)).unwrap();
                assert_eq!(actual.is_some(), expected.is_some());
                assert_eq!(actual.as_ref().map(index), expected);
                cases += 1;
            }
            assert!(map.solve(&bits(width + 1, 1 << width)).is_err());
        }
    }
    let mut basis = (0..67)
        .map(|j| BitVector::from_indices(137, [j, 70 + j]).unwrap())
        .collect::<Vec<_>>();
    let mut dependent = basis[1].clone();
    dependent.xor_assign(&basis[65]).unwrap();
    basis.push(dependent);
    basis.push(bits(137, 0));
    let map = LinearFactorization::new(69, &transpose_columns(137, &basis).unwrap()).unwrap();
    for _ in 0..128 {
        let choice = BitVector::from_indices(69, (0..69).filter(|_| rng.next() & 1 != 0)).unwrap();
        let row = xors(137, &basis, &choice).unwrap();
        let mut expected = choice.low(67).unwrap();
        if choice.get(67).unwrap() {
            for j in [1, 65] {
                let value = expected.get(j).unwrap();
                expected.set(j, !value).unwrap();
            }
        }
        let actual = map.solve(&row).unwrap().unwrap();
        assert_eq!(actual.low(67).unwrap(), expected);
        assert!(!actual.get(67).unwrap() && !actual.get(68).unwrap());
        let mut outside = row;
        outside.set(68, true).unwrap();
        assert_eq!(map.solve(&outside).unwrap(), None);
    }
    assert_eq!(cases, 2040);
}
#[test]
fn consuming_and_finite_sum_contraction() {
    let a = ScalarArithmetic::default();
    let mut rng = Mt19937::new(2026100503);
    let mut cases = 0;
    for n in 0..=7 {
        for _ in 0..40 {
            let mut input = Input {
                width: 3,
                offset: rng.next() as usize % 8,
                columns: vec![],
                linear: vec![],
                upper: vec![],
                phase: (rng.next() % 8) as i64,
                power: -(n as i64),
            };
            for j in 0..n {
                input.columns.push(rng.next() as usize % 8);
                input.linear.push((rng.next() % 4) as u8);
                input.upper.push(
                    BitVector::from_indices(n, (j + 1..n).filter(|_| rng.next() & 1 != 0)).unwrap(),
                );
            }
            let expected = input.dense(&a);
            let source = input.ket(&a);
            let copy = source.clone().contract(&a).unwrap();
            let moved = source.contract(&a).unwrap();
            drop(source);
            assert_eq!(copy, moved);
            assert_eq!(amplitudes(moved.as_ref(), 3, &a), expected);
            cases += 1;
        }
    }
    assert_eq!(cases, 320);
    for phase in 0..8 {
        for linear in 0..4 {
            for visible in 0..2 {
                let input = Input {
                    width: 1,
                    offset: 0,
                    columns: vec![visible],
                    linear: vec![linear],
                    upper: vec![bits(1, 0)],
                    phase,
                    power: -1,
                };
                let result = input.ket(&a).contract(&a).unwrap();
                assert_eq!(amplitudes(result.as_ref(), 1, &a), input.dense(&a));
            }
        }
    }
}
#[test]
fn wide_clifford_elimination() {
    let a = ScalarArithmetic::default();
    let mut rng = Mt19937::new(2026100307);
    let mut cases = 0;
    for n in [63, 64, 65, 129, 257] {
        for _ in 0..24 {
            let offset = rng.next() as usize % 8;
            let phase = (rng.next() % 8) as i64;
            let locations = (0..8).map(|j| j * (n - 1) / 7).collect::<Vec<_>>();
            let mut wide = Input {
                width: 3,
                offset,
                columns: vec![0; n],
                linear: vec![0; n],
                upper: vec![bits(n, 0); n],
                phase,
                power: -(n as i64),
            };
            let mut small = Input {
                width: 3,
                offset,
                columns: vec![],
                linear: vec![],
                upper: vec![],
                phase,
                power: -8,
            };
            let mut factor_phase = 0;
            for j in 0..n {
                if !locations.contains(&j) {
                    let positive = rng.next() & 1 != 0;
                    wide.linear[j] = if positive { 1 } else { 3 };
                    factor_phase += if positive { 1 } else { -1 };
                }
            }
            for j in 0..8 {
                let c = if j % 3 == 0 {
                    0
                } else {
                    rng.next() as usize % 8
                };
                let l = (rng.next() % 4) as u8;
                let mut upper = bits(8, 0);
                for k in j + 1..8 {
                    if rng.next() & 1 != 0 {
                        upper.set(k, true).unwrap();
                        wide.upper[locations[j]].set(locations[k], true).unwrap();
                    }
                }
                small.columns.push(c);
                small.linear.push(l);
                small.upper.push(upper);
                wide.columns[locations[j]] = c;
                wide.linear[locations[j]] = l;
            }
            let factor = a.omega(factor_phase, 0).unwrap();
            let expected = small
                .dense(&a)
                .iter()
                .map(|x| a.multiply(x, &factor).unwrap())
                .collect::<Vec<_>>();
            let source = wide.ket(&a);
            let plan = PreparedAffineContraction::new(&source).unwrap();
            let direct = source.contract(&a).unwrap();
            let warm = plan.contract(&source, &a).unwrap();
            for result in [direct.as_ref(), warm.as_ref()] {
                assert_eq!(amplitudes(result, 3, &a), expected);
            }
            assert_eq!(direct.is_none(), warm.is_none());
            // Rust retains global phase in both exact routes. Compare absolute
            // squared amplitudes explicitly; it has no phase-dropping flag.
            for (x, y) in
                amplitudes(direct.as_ref(), 3, &a)
                    .iter()
                    .zip(amplitudes(warm.as_ref(), 3, &a))
            {
                assert_eq!(
                    a.multiply(x, &x.conjugate()).unwrap(),
                    a.multiply(&y, &y.conjugate()).unwrap()
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 120);
}
fn add_even(linear: &[u8], upper: &[BitVector], terms: &[(BitVector, i64)]) -> QuadraticForm {
    let n = linear.len();
    let p = ParityPhase::new(
        n,
        0,
        terms
            .iter()
            .map(|(m, c)| (m.clone(), c.rem_euclid(8) as u8)),
    )
    .unwrap();
    let (constant, delta) = p.clifford().unwrap();
    assert_eq!(constant, 0);
    let changed = linear
        .iter()
        .zip(delta.linear())
        .map(|(a, b)| (a + b) & 3)
        .collect();
    let rows = upper
        .iter()
        .zip(delta.upper())
        .map(|(a, b)| {
            let mut r = a.clone();
            r.xor_assign(b).unwrap();
            r
        })
        .collect();
    QuadraticForm::new(0, changed, rows).unwrap()
}
#[test]
fn check_packed_clifford_updates() {
    for width in [0, 1, 63, 64, 65, 137] {
        let rows = (0..70)
            .map(|j| {
                BitVector::from_indices(width, (0..width).filter(|k| (j * 17 + k * 13) % 7 < 3))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for trial in 0..32 {
            let mask =
                BitVector::from_indices(70, (0..70).filter(|j| (j * 11 + trial * 3) % 13 < 7))
                    .unwrap();
            let expected = BitVector::from_indices(
                width,
                (0..width).filter(|&k| {
                    (0..70)
                        .filter(|&j| mask.get(j).unwrap() && rows[j].get(k).unwrap())
                        .count()
                        % 2
                        != 0
                }),
            )
            .unwrap();
            assert_eq!(xors(width, &rows, &mask).unwrap(), expected);
        }
        assert!(xors(width, &rows, &BitVector::from_indices(71, [70]).unwrap()).is_err());
    }
    for n in [0, 1, 7, 63, 64, 65, 137] {
        let mask = BitVector::from_indices(n, (0..n).filter(|j| j % 3 != 1)).unwrap();
        for cut in [0usize, 1, 63, 64, 65, 140] {
            let mut actual = BitVector::from_indices(n, 0..n).unwrap();
            actual.xor_above_assign(&mask, cut).unwrap();
            let expected =
                BitVector::from_indices(n, (0..n).filter(|&j| j <= cut || !mask.get(j).unwrap()))
                    .unwrap();
            assert_eq!(actual, expected);
        }
        for coefficient in [-6i64, -4, -2, 0, 2, 4, 6, 8, 10] {
            let linear = (0..n).map(|j| (j % 4) as u8).collect::<Vec<_>>();
            let upper = (0..n)
                .map(|j| BitVector::from_indices(n, (j + 2 < n).then_some(j + 2)).unwrap())
                .collect::<Vec<_>>();
            let mut expected_l = linear.clone();
            let mut expected_u = upper.clone();
            let c = coefficient.rem_euclid(8) / 2;
            for j in 0..n {
                if !mask.get(j).unwrap() {
                    continue;
                }
                expected_l[j] = ((i64::from(expected_l[j]) + c) % 4) as u8;
                for k in j + 1..n {
                    if c & 1 != 0 && mask.get(k).unwrap() {
                        let old = expected_u[j].get(k).unwrap();
                        expected_u[j].set(k, !old).unwrap();
                    }
                }
            }
            let result = add_even(&linear, &upper, &[(mask.clone(), coefficient)]);
            assert_eq!(result.linear(), expected_l);
            assert_eq!(result.upper(), expected_u);
        }
    }
}
#[test]
fn check_quadratic_batches() {
    let mut rng = Mt19937::new(20261004);
    let mut transpose_cases = 0;
    let mut update_cases = 0;
    let mut restriction_cases = 0;
    for n in [0, 1, 63, 64, 65, 137] {
        for m in [0, 7, 64, 131] {
            for density in [2, 31] {
                let mut columns = vec![bits(n, 0); m];
                let mut expected = vec![bits(m, 0); n];
                for j in 0..m {
                    for k in 0..n {
                        if rng.next() % density == 0 {
                            columns[j].set(k, true).unwrap();
                            expected[k].set(j, true).unwrap();
                        }
                    }
                }
                assert_eq!(transpose_columns(n, &columns).unwrap(), expected);
                transpose_cases += 1;
            }
        }
    }
    assert!(transpose_columns(65, &[BitVector::from_indices(66, [65]).unwrap()]).is_err());
    for n in [0, 1, 8, 63, 64, 65, 137, 513, 1025] {
        for count in [1, 7, 8, 32] {
            for density in [2, 31] {
                let mut linear = vec![0; n];
                let mut upper = vec![bits(n, 0); n];
                let mut terms = vec![];
                for j in 0..n {
                    linear[j] = (rng.next() % 4) as u8;
                    for k in j + 1..n {
                        if rng.next() % 31 == 0 {
                            upper[j].set(k, true).unwrap();
                        }
                    }
                }
                for _ in 0..count {
                    let mask =
                        BitVector::from_indices(n, (0..n).filter(|_| rng.next() % density == 0))
                            .unwrap();
                    terms.push((mask, 2 * (rng.next() % 9) as i64 - 8));
                }
                let result = add_even(&linear, &upper, &terms);
                for trial in 0..if n <= 8 { 1 << n } else { 32 } {
                    let word = if n <= 8 {
                        bits(n, trial)
                    } else {
                        BitVector::from_indices(n, (0..n).filter(|_| rng.next() & 1 != 0)).unwrap()
                    };
                    let mut expected = 2 * scalar_q(&word, &linear, &upper);
                    for (mask, c) in &terms {
                        let mut parity = false;
                        for j in 0..n {
                            parity ^= word.get(j).unwrap() && mask.get(j).unwrap();
                        }
                        if parity {
                            expected += c;
                        }
                    }
                    assert_eq!(
                        2 * i64::from(result.evaluate(&word).unwrap()),
                        expected.rem_euclid(8)
                    );
                    update_cases += 1;
                }
            }
        }
    }
    for n in [8, 65, 137] {
        let mut linear = vec![0; n];
        let mut upper = vec![bits(n, 0); n];
        let mut basis = vec![bits(n, 0); n + 3];
        let mut shift = bits(n, 0);
        for j in 0..n {
            linear[j] = (rng.next() % 4) as u8;
            if rng.next() & 1 != 0 {
                shift.set(j, true).unwrap();
            }
            for k in j + 1..n {
                if rng.next() % 3 == 0 {
                    upper[j].set(k, true).unwrap();
                }
            }
            for k in 0..n {
                if rng.next() & 1 != 0 {
                    basis[j].set(k, true).unwrap();
                }
            }
        }
        let source = QuadraticForm::new(0, linear.clone(), upper.clone()).unwrap();
        let result = source.restrict(&shift, &basis).unwrap();
        let without = source.restrict_without_constant(&shift, &basis).unwrap();
        assert_eq!(without.constant(), 0);
        assert_eq!(result.linear(), without.linear());
        assert_eq!(result.upper(), without.upper());
        for _ in 0..64 {
            let word =
                BitVector::from_indices(n + 3, (0..n + 3).filter(|_| rng.next() & 1 != 0)).unwrap();
            let mut original = shift.clone();
            for j in word.ones() {
                original.xor_assign(&basis[j]).unwrap();
            }
            assert_eq!(
                i64::from(result.evaluate(&word).unwrap()),
                scalar_q(&original, &linear, &upper)
            );
            restriction_cases += 1;
        }
    }
    assert_eq!(transpose_cases, 48);
    assert_eq!(update_cases, 3608);
    assert_eq!(restriction_cases, 192);
    assert_eq!(rng.draws, 6_228_326);
    assert_eq!(rng.fingerprint, 0x9638_c34e_8c58_49b7);
}
#[test]
fn word_shifts() {
    let mut rng = Mt19937::new(20261003);
    for width in [0usize, 1, 63, 64, 65, 130, 1025] {
        let mask =
            BitVector::from_indices(width, (0..width).filter(|_| rng.next() & 1 != 0)).unwrap();
        for offset in [0usize, 1, 63, 64, 65, 127, 1024, 2049] {
            let left =
                BitVector::from_indices(width + offset, mask.ones().map(|j| j + offset)).unwrap();
            let right = BitVector::from_indices(
                width.saturating_sub(offset),
                mask.ones().filter_map(|j| j.checked_sub(offset)),
            )
            .unwrap();
            assert_eq!(mask.shifted_left(offset).unwrap(), left);
            assert_eq!(mask.shifted_right(offset).unwrap(), right);
            assert_eq!(left.shifted_right(offset).unwrap(), mask);
        }
    }
}
#[test]
fn prepared_clifford() {
    let a = ScalarArithmetic::default();
    let mut rng = Lcg(71819);
    let mut assignments = 0;
    for n in 0..=7 {
        for width in 0..=3 {
            for _ in 0..16 {
                let phase = (rng.next() % 8) as i64;
                let offset = rng.next() as usize & ((1 << width) - 1);
                let mut input = Input {
                    width,
                    offset,
                    columns: vec![],
                    linear: vec![],
                    upper: vec![],
                    phase,
                    power: -(n as i64),
                };
                for j in 0..n {
                    input.columns.push(rng.next() as usize & ((1 << width) - 1));
                    input.linear.push((rng.next() % 4) as u8);
                    let row = rng.next() as usize & ((1 << n) - 1);
                    input.upper.push(
                        BitVector::from_indices(n, (j + 1..n).filter(|k| row >> k & 1 != 0))
                            .unwrap(),
                    );
                }
                let reference = input.ket(&a);
                let plan = PreparedAffineContraction::new(&reference).unwrap();
                let original = input.linear.clone();
                for assignment in 0..1usize << n {
                    input.linear = original
                        .iter()
                        .enumerate()
                        .map(|(j, l)| (l + 2 * ((assignment >> j & 1) as u8)) & 3)
                        .collect();
                    let changed = input.ket(&a);
                    let cold = changed.contract(&a).unwrap();
                    let warm = plan.contract(&changed, &a).unwrap();
                    assert_eq!(cold.is_none(), warm.is_none());
                    assert_eq!(cold, warm);
                    assert_eq!(amplitudes(warm.as_ref(), width, &a), input.dense(&a));
                    assignments += 1;
                }
                let mut tensor_input = input.clone();
                tensor_input.linear = original;
                tensor_input.phase = 0;
                let tensor = PhaseInput {
                    input: tensor_input,
                    terms: vec![],
                };
                let (resource, prepared) = freeze_phase(&tensor, &a);
                assert_eq!(resource.quotient_rank(), 0);
                assert_eq!(prepared.usage().consistent_terms, 1);
                check_transported_phase(&tensor, &resource, &prepared, &a);
            }
        }
    }
    assert_eq!(assignments, 16320);
}
fn affine_words(support: Option<&AffineSupport>) -> BTreeSet<usize> {
    let Some(s) = support else {
        return BTreeSet::new();
    };
    (0..1usize << s.dimension())
        .map(|u| {
            let mut word = index(s.offset());
            for (j, c) in s.columns().iter().enumerate() {
                if u >> j & 1 != 0 {
                    word ^= index(c);
                }
            }
            word
        })
        .collect()
}
#[test]
fn affine_proposal_hull_and_projection() {
    let cols = (0..80)
        .map(|j| BitVector::from_indices(82, [j]).unwrap())
        .collect::<Vec<_>>();
    let first = AffineSupport::new(bits(82, 0), cols.clone()).unwrap();
    let second = AffineSupport::new(BitVector::from_indices(82, [80, 81]).unwrap(), cols).unwrap();
    let hull = AffineSupport::hull(&[first, second]).unwrap().unwrap();
    assert_eq!(hull.dimension(), 81);
    let map =
        LinearFactorization::new(81, &transpose_columns(82, hull.columns()).unwrap()).unwrap();
    assert!(
        map.solve(&BitVector::from_indices(82, [80, 81]).unwrap())
            .unwrap()
            .is_some()
    );
    assert_eq!(
        map.solve(&BitVector::from_indices(82, [80]).unwrap())
            .unwrap(),
        None
    );
    assert!(
        hull.condition(
            &[
                BitVector::from_indices(82, [80]).unwrap(),
                BitVector::from_indices(82, [81]).unwrap()
            ],
            &bits(2, 2)
        )
        .unwrap()
        .is_none()
    );
    let mut rng = Mt19937::new(202609301);
    let mut cases = 0;
    for n in 0..=7 {
        for _ in 0..40 {
            let physical = 1 + rng.next() as usize % 7;
            let offset = rng.next() as usize & ((1 << physical) - 1);
            let columns = (0..n)
                .map(|_| rng.next() as usize & ((1 << physical) - 1))
                .collect::<Vec<_>>();
            let width = 1 + rng.next() as usize % 5;
            let keep = (0..width)
                .map(|_| rng.next() as usize & ((1 << physical) - 1))
                .collect::<Vec<_>>();
            let count = rng.next() as usize % 5;
            let mut events = vec![];
            let mut values = 0;
            for j in 0..count {
                events.push(rng.next() as usize & ((1 << physical) - 1));
                values |= (rng.next() as usize & 1) << j;
            }
            let mut expected = BTreeSet::new();
            for u in 0..1usize << n {
                let point = columns
                    .iter()
                    .enumerate()
                    .fold(offset, |v, (j, &c)| if u >> j & 1 != 0 { v ^ c } else { v });
                if events
                    .iter()
                    .enumerate()
                    .any(|(j, m)| ((point & m).count_ones() as usize & 1) != (values >> j & 1))
                {
                    continue;
                }
                let label = keep.iter().enumerate().fold(0, |v, (j, m)| {
                    v | (((point & m).count_ones() as usize & 1) << j)
                });
                expected.insert(label);
            }
            let source = AffineSupport::new(
                bits(physical, offset),
                columns.iter().map(|&c| bits(physical, c)).collect(),
            )
            .unwrap();
            let conditioned = source
                .condition(
                    &events
                        .iter()
                        .map(|&m| bits(physical, m))
                        .collect::<Vec<_>>(),
                    &bits(count, values),
                )
                .unwrap();
            let projected = conditioned.map(|s| {
                s.linear_image(&keep.iter().map(|&m| bits(physical, m)).collect::<Vec<_>>())
                    .unwrap()
            });
            assert_eq!(affine_words(projected.as_ref()), expected);
            cases += 1;
        }
    }
    assert_eq!(cases, 320);
}
#[test]
fn quadratic_radical_support() {
    let a = ScalarArithmetic::default();
    let mut rng = Mt19937::new(20260930);
    let mut cases = 0;
    for n in 0..=7 {
        for _ in 0..80 {
            let width = 1 + rng.next() as usize % 4;
            let mut linear = vec![];
            let mut upper = vec![];
            for j in 0..n {
                linear.push((rng.next() % 4) as u8);
                upper.push(
                    BitVector::from_indices(n, (j + 1..n).filter(|_| rng.next() & 1 != 0)).unwrap(),
                );
            }
            let mut rows = vec![];
            let mut offset = 0;
            for j in 0..width {
                rows.push(rng.next() as usize & ((1 << n) - 1));
                offset |= (rng.next() as usize & 1) << j;
            }
            let columns = (0..n)
                .map(|j| {
                    rows.iter()
                        .enumerate()
                        .fold(0, |v, (k, r)| v | ((r >> j & 1) << k))
                })
                .collect::<Vec<_>>();
            let input = Input {
                width,
                offset,
                columns,
                linear,
                upper,
                phase: 0,
                power: 0,
            };
            let expected = input.dense(&a);
            let contracted = input.ket(&a).contract(&a).unwrap();
            let support = contracted
                .as_ref()
                .map(|k| AffineSupport::new(k.offset().clone(), k.columns().to_vec()).unwrap());
            let actual = affine_words(support.as_ref());
            for (word, sum) in expected.iter().enumerate() {
                assert_eq!(actual.contains(&word), !sum.is_zero());
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 640);
}
use rmagna::{
    catalogue::PreparedCatalogueBoundary,
    phase::{NativePhaseOptions, PreparedPhaseResource},
};
#[derive(Clone)]
struct PhaseInput {
    input: Input,
    terms: Vec<(usize, i64)>,
}
impl PhaseInput {
    fn phase(&self) -> ParityPhase {
        let n = self.input.columns.len();
        ParityPhase::new(
            n,
            0,
            self.terms
                .iter()
                .map(|&(mask, c)| (bits(n, mask), c.rem_euclid(8) as u8)),
        )
        .unwrap()
    }
    fn dense(&self, a: &ScalarArithmetic) -> Vec<ExactScalar> {
        let i = &self.input;
        let n = i.columns.len();
        let mut out = vec![ExactScalar::zero(); 1 << i.width];
        for word in 0..1usize << n {
            let mut output = i.offset;
            let mut exponent = i.phase;
            for j in 0..n {
                if word >> j & 1 == 0 {
                    continue;
                }
                output ^= i.columns[j];
                exponent += 2 * i64::from(i.linear[j]);
                for k in j + 1..n {
                    if word >> k & 1 != 0 && i.upper[j].get(k).unwrap() {
                        exponent += 4;
                    }
                }
            }
            for &(mask, c) in &self.terms {
                exponent += c * i64::from((mask & word).count_ones() & 1);
            }
            out[output] = a
                .add(&out[output], &a.omega(exponent, i.power).unwrap())
                .unwrap();
        }
        out
    }
}
fn phase_input(
    n: usize,
    rows: &[usize],
    offset: usize,
    linear: Vec<u8>,
    upper: Vec<BitVector>,
    phase: i64,
    power: i64,
    terms: Vec<(usize, i64)>,
) -> PhaseInput {
    let columns = (0..n)
        .map(|j| {
            rows.iter()
                .enumerate()
                .fold(0, |v, (k, r)| v | ((r >> j & 1) << k))
        })
        .collect();
    PhaseInput {
        input: Input {
            width: rows.len(),
            offset,
            columns,
            linear,
            upper,
            phase,
            power,
        },
        terms,
    }
}
fn freeze_phase(
    reference: &PhaseInput,
    a: &ScalarArithmetic,
) -> (PreparedPhaseResource, PreparedCatalogueBoundary) {
    let resource = PreparedPhaseResource::new_native(
        &reference.phase(),
        4096,
        NativePhaseOptions::default(),
        None,
        a,
    )
    .unwrap();
    let boundary = resource
        .native_resource()
        .unwrap()
        .prepare_boundary(
            &reference.phase(),
            &reference.input.ket(a),
            &Default::default(),
            a,
        )
        .unwrap();
    (resource, boundary)
}
fn check_transported_phase(
    input: &PhaseInput,
    resource: &PreparedPhaseResource,
    prepared: &PreparedCatalogueBoundary,
    a: &ScalarArithmetic,
) {
    let before = (
        resource.num_terms(),
        resource.quotient_rank(),
        prepared.usage(),
    );
    let cold = resource
        .expand(&input.phase(), &input.input.ket(a), a)
        .unwrap();
    let warm = prepared
        .expand(&input.phase(), &input.input.ket(a), a)
        .unwrap();
    assert_eq!(warm.states.len(), cold.len());
    let canonical = |ket: &AffineKet| {
        AffineKet::new(
            ket.offset().clone(),
            ket.columns().to_vec(),
            QuadraticForm::new(
                0,
                ket.phase().linear().to_vec(),
                ket.phase().upper().to_vec(),
            )
            .unwrap(),
            a.multiply(
                ket.coefficient(),
                &a.omega(2 * i64::from(ket.phase().constant()), 0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    // Scalar roots and q's constant encode the same global phase. Canonicalize
    // only that representation; all columns and variable coefficients must match.
    assert_eq!(
        warm.states.iter().map(canonical).collect::<Vec<_>>(),
        cold.iter().map(canonical).collect::<Vec<_>>()
    );
    assert_eq!(
        warm.statistics.gaussian_reuses + warm.statistics.gaussian_recomputations,
        prepared.usage().consistent_terms
    );
    let mut actual = vec![ExactScalar::zero(); 1 << input.input.width];
    for term in &warm.states {
        for (total, v) in actual
            .iter_mut()
            .zip(amplitudes(Some(term), input.input.width, a))
        {
            *total = a.add(total, &v).unwrap();
        }
    }
    assert_eq!(actual, input.dense(a));
    assert_eq!(
        (
            resource.num_terms(),
            resource.quotient_rank(),
            prepared.usage()
        ),
        before
    );
}
#[test]
fn character_geometry_and_resource_transport() {
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for n in 2..=6 {
        for width in [0, 1, 3] {
            let rows = (0..width)
                .map(|j| ((j + 1) * 3) & ((1 << n) - 1))
                .collect::<Vec<_>>();
            let base = phase_input(
                n,
                &rows,
                0,
                vec![0; n],
                vec![bits(n, 0); n],
                0,
                -(n as i64),
                vec![(1, 1), (1 << (n - 1), 3)],
            );
            let (resource, prepared) = freeze_phase(&base, &a);
            for sample in 0..16 {
                let mut changed = base.clone();
                changed.input.linear = (0..n).map(|j| ((sample + 3 * j) % 4) as u8).collect();
                changed.input.upper = (0..n)
                    .map(|j| {
                        BitVector::from_indices(
                            n,
                            (j + 1..n).filter(|k| (sample * (j + 1)) >> k & 1 != 0),
                        )
                        .unwrap()
                    })
                    .collect();
                changed.input.phase = (sample % 8) as i64;
                changed.input.power -= (sample % 3) as i64;
                changed.terms.push(((1 << n) - 1, 2 * (sample % 4) as i64));
                changed.input.offset = sample & ((1 << width) - 1);
                check_transported_phase(&changed, &resource, &prepared, &a);
                cases += 1;
            }
            let mut changed_rows = rows;
            changed_rows.push(1 << (n - 1));
            let changed = phase_input(
                n,
                &changed_rows,
                1 << width,
                vec![0; n],
                vec![bits(n, 0); n],
                0,
                -(n as i64),
                base.terms.clone(),
            );
            assert!(
                prepared
                    .expand(&changed.phase(), &changed.input.ket(&a), &a)
                    .is_err()
            );
            let mapped = resource
                .native_resource()
                .unwrap()
                .prepare_boundary(
                    &changed.phase(),
                    &changed.input.ket(&a),
                    &Default::default(),
                    &a,
                )
                .unwrap();
            check_transported_phase(&changed, &resource, &mapped, &a);
        }
    }
    assert_eq!(cases, 240);
    let reference = phase_input(
        2,
        &[1, 2],
        0,
        vec![0, 0],
        vec![bits(2, 0); 2],
        0,
        -2,
        vec![(1, 1), (2, 1)],
    );
    let (resource, prepared) = freeze_phase(&reference, &a);
    let mut actual = phase_input(
        2,
        &[3, 2],
        1,
        vec![1, 2],
        vec![bits(2, 2), bits(2, 0)],
        3,
        -4,
        reference.terms.clone(),
    );
    let mapped = resource
        .native_resource()
        .unwrap()
        .prepare_boundary(
            &actual.phase(),
            &actual.input.ket(&a),
            &Default::default(),
            &a,
        )
        .unwrap();
    check_transported_phase(&actual, &resource, &mapped, &a);
    let dense = actual.dense(&a);
    for word in 0..4 {
        let x = word & 1;
        let y = word >> 1;
        let destination = (x ^ y ^ 1) | (y << 1);
        let exponent = (x + y + 2 * x + 4 * y + 4 * x * y + 3) as i64;
        assert_eq!(dense[destination], a.omega(exponent, -4).unwrap());
    }
    actual.terms.push((3, 1));
    assert!(
        resource
            .expand(&actual.phase(), &actual.input.ket(&a), &a)
            .is_err()
    );
    // Explicit transport quota rejection retains the already selected core.
    let mut tight = rmagna::catalogue::CatalogueBoundaryOptions::default();
    tight.max_geometry_words = 0;
    assert!(
        resource
            .native_resource()
            .unwrap()
            .prepare_boundary(&reference.phase(), &reference.input.ket(&a), &tight, &a)
            .is_err()
    );
    check_transported_phase(&reference, &resource, &prepared, &a);
}
#[test]
fn strict_parity_representation() {
    let a = ScalarArithmetic::default();
    let base = phase_input(
        4,
        &[1, 2, 4, 8],
        0,
        vec![1, 3, 2, 0],
        vec![bits(4, 6), bits(4, 0), bits(4, 0), bits(4, 0)],
        5,
        -4,
        vec![(1, 1), (3, 7), (7, 1)],
    );
    let (resource, prepared) = freeze_phase(&base, &a);
    for increment in [0, 2, 4, 6] {
        for offset in 0..2 {
            let mut changed = phase_input(
                4,
                &[3, 3, 12],
                offset | (1 << 1) | (offset << 2),
                vec![3, 0, 1, 2],
                vec![bits(4, 6), bits(4, 12), bits(4, 0), bits(4, 0)],
                offset as i64,
                -6,
                base.terms.clone(),
            );
            changed.terms[0].1 += increment;
            changed.terms.push((14, increment));
            let mapped = resource
                .native_resource()
                .unwrap()
                .prepare_boundary(
                    &changed.phase(),
                    &changed.input.ket(&a),
                    &Default::default(),
                    &a,
                )
                .unwrap();
            check_transported_phase(&changed, &resource, &mapped, &a);
        }
    }
    let mut alternative = base.clone();
    for mask in 1..16 {
        alternative.terms.push((mask, 1));
    }
    alternative.input.linear[2] = 1;
    alternative.input.upper[0].set(3, true).unwrap();
    // Rust validates the exact Clifford difference and frozen physical columns.
    // It can accept another parity spelling without building a fresh quotient;
    // the immutable plan reports its actual unchanged prepared geometry.
    check_transported_phase(&alternative, &resource, &prepared, &a);
    check_transported_phase(&base, &resource, &prepared, &a);
}
use num_bigint::{BigInt, BigUint};
use num_rational::BigRational;
use rmagna::{
    field::{ExactField, FieldArithmetic},
    file_api::SamplerDecision,
    numeric::{NumericOptions, approximate_real, approximate_scalar_ratio},
    records::bernoulli_ratio_bits,
    statistics::{ExactStatistics, HistoryEfficiency},
};
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
#[test]
fn history_efficiency_and_paired_statistics() {
    let mut efficiency = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    let mut stats = ExactStatistics::new(2, Default::default()).unwrap();
    for error in [0, 1] {
        efficiency
            .push(
                &field(1, 1),
                Some(&field(error, 1)),
                &field(1, 1),
                &field(1, 1),
            )
            .unwrap();
        stats
            .push(&[field(1, 1), field(error, 1)], &field(1, 1))
            .unwrap();
    }
    let e = efficiency.summarize().unwrap();
    assert_eq!(e.kappa, Some(field(1, 1)));
    assert_eq!(e.variance_coefficient, Some(field(1, 4)));
    assert_eq!(
        SamplerDecision::from_pilot_totals(4., 2., 4., 10)
            .unwrap()
            .gamma,
        2.
    );
    let ratio = stats.paired_ratio(1, 0).unwrap().unwrap();
    assert_eq!(ratio.ratio, field(1, 2));
    assert_eq!(ratio.variance, Some(field(1, 4)));
    let mut huge_stats = ExactStatistics::new(1, Default::default()).unwrap();
    let huge = FieldArithmetic::default()
        .from_rational(BigRational::from_integer(BigInt::from(1) << 19999usize))
        .unwrap();
    huge_stats.push(&[huge.clone()], &huge).unwrap();
    huge_stats.push(&[huge.clone()], &huge).unwrap();
    let summary = huge_stats.summarize().unwrap().unwrap();
    assert_eq!(summary.effective_sample_size, field(2, 1));
    let scientific = approximate_real(&summary.mean_weight, NumericOptions::default()).unwrap();
    assert_eq!(
        (
            scientific.scientific.mantissa,
            scientific.scientific.exponent
        ),
        (1., 19999)
    );
}
#[test]
fn wide_exact_scalars_and_caller_rng() {
    let a = ScalarArithmetic::default();
    let huge = a
        .scalar(
            [
                (BigInt::from(1) << 10000usize),
                0.into(),
                0.into(),
                0.into(),
            ],
            0,
        )
        .unwrap();
    let ratio = approximate_scalar_ratio(&huge, &huge, NumericOptions::default()).unwrap();
    assert_eq!((ratio.mantissa, ratio.exponent), (1., 0));
    let equations = [
        BitVector::from_indices(194, [0, 193]).unwrap(),
        BitVector::from_indices(194, [1]).unwrap(),
    ];
    let factor = LinearFactorization::new(194, &equations).unwrap();
    assert_eq!(factor.solve(&bits(2, 1)).unwrap(), Some(bits(194, 1)));
    assert_eq!(factor.nullspace().unwrap().len(), 192);
    let mut calls = 0;
    let tiny = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -2048)
        .unwrap();
    let draw = bernoulli_ratio_bits(
        &tiny,
        &ExactScalar::one(),
        &mut || {
            calls += 1;
            false
        },
        2048,
        &a,
    )
    .unwrap();
    assert!(draw.selected);
    assert_eq!((draw.entropy_bits, calls), (2048, 2048));
}
#[test]
fn boundary_coordinate_rhs() {
    use rmagna::instrument::{
        OriginalBoundaryLimits, OriginalRecordEvent, OriginalRecordLayout, OriginalRecordParity,
    };
    let names = ["a", "b", "c"].map(str::to_string);
    let events = [
        OriginalRecordEvent {
            parity: OriginalRecordParity {
                records: vec!["a".into(), "b".into()],
                inverted: false,
            },
            value: false,
        },
        OriginalRecordEvent {
            parity: OriginalRecordParity {
                records: vec!["b".into(), "a".into()],
                inverted: false,
            },
            value: true,
        },
    ];
    let layout =
        OriginalRecordLayout::new(&names, &events, OriginalBoundaryLimits::default()).unwrap();
    assert!(!layout.compatible());
    assert_eq!(layout.postselection_rank(), 1);
    let with_feedback = LinearFactorization::new(3, &[bits(3, 3), bits(3, 4)]).unwrap();
    assert_eq!(with_feedback.rank() - layout.postselection_rank(), 1);
    assert_eq!(layout.transform().len(), 3);
    for word in 0..8 {
        let input = bits(3, word);
        assert_eq!(
            layout
                .to_records(&layout.to_virtual(&input).unwrap())
                .unwrap(),
            input
        );
    }
}
use rmagna::{
    circuit::{BitExpr, Operation, ParseOptions, parse_circuit},
    noise::{RandomSource, SplitMix64},
    runtime::{ControllerRegistry, HistorySampler},
    task::{EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-standalone-foundation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn evaluator(&self, text: &str, spec: &str, registry: &ControllerRegistry) -> TaskEvaluator {
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let physical = parse_circuit(text, &parse).unwrap();
        let task = parse_task(spec, &self.0, &TaskOptions::default()).unwrap();
        let modules = task.load_modules(&physical, &parse).unwrap();
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::ExactMarginal;
        options.runtime.compile.probability_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.runtime.compile.density_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.runtime.compile.moment_preparation =
            QueryPreparation::FixedFibers(Default::default());
        TaskEvaluator::new(&physical, &modules, registry, options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn independent_proposal_and_exact_range_order() {
    use rmagna::{
        clifford::CliffordCircuit,
        instrument::{
            OriginalAssignmentPlan, OriginalGate, OriginalIndependentRecordProposal,
            OriginalInstrument, OriginalLimits, OriginalOperation, OriginalPauli,
            OriginalSharedFaults,
        },
        unitary::CompilerStrategy,
    };
    let a = ScalarArithmetic::default();
    let input = CliffordCircuit::computational(bits(2, 0)).unwrap();
    let ir = OriginalInstrument::new(
        vec![0, 1],
        vec![0, 1],
        input,
        (0..2)
            .map(|j| OriginalOperation {
                id: format!("m{j}"),
                gate: OriginalGate::Measure {
                    factors: vec![(j as i64, OriginalPauli::Z)],
                    inverted: false,
                    record: ["a", "b"][j].into(),
                },
            })
            .collect(),
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let plan = OriginalAssignmentPlan::new(&ir, CompilerStrategy::SharedCommutingLayer, a);
    let proposal = OriginalIndependentRecordProposal::new(
        &plan,
        &BTreeMap::from([("a".into(), "2/7".parse().unwrap())]),
        Default::default(),
    )
    .unwrap();
    let mut bounds = vec![];
    let mut choose = |n: &BigUint| -> Result<BigUint, std::convert::Infallible> {
        bounds.push(n.clone());
        Ok(0u8.into())
    };
    let draw = proposal
        .draw(&mut choose, OriginalSharedFaults::empty())
        .unwrap();
    assert_eq!(bounds, vec![BigUint::from(7u8), 2u8.into()]);
    assert_eq!(draw.assignment.word(), &bits(2, 3));
    assert_eq!(draw.proposal_probability, rational(1, 7));
    assert_eq!(draw.range_requests, 2);
}
#[test]
fn exact_file_parser() {
    let parse = ParseOptions {
        controllers: ["decoder".into()].into_iter().collect(),
        ..Default::default()
    };
    let circuit=parse_circuit("M(1/30) 0\nCONTROL[XOR](1) rec[-1] rec[-1] -> ctrl[0]\nCONTROL[decoder] ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]\n",&parse).unwrap();
    assert_eq!(circuit.num_records(), 1);
    assert_eq!(circuit.external_calls(), 1);
    assert_eq!(circuit.format_version(), "magna-0.1");
    let Operation::Measure { report_flip, .. } = &circuit.operations()[0].operation else {
        panic!("missing exact measurement")
    };
    assert_eq!(report_flip.value(), &rational(1, 30));
    let Operation::Control { outputs, .. } = &circuit.operations()[1].operation else {
        panic!("missing XOR")
    };
    assert_eq!(
        outputs[0].expression,
        BitExpr {
            constant: true,
            terms: BTreeSet::new()
        }
    );
    assert!(
        matches!(&circuit.operations()[2].operation,Operation::Control{name,..} if name=="decoder")
    );
    assert!(
        parse_circuit(
            "REPEAT 99999999999999999999999999 {\n}\n",
            &ParseOptions {
                max_instructions: 1,
                allow_wide_repeat_counts: true,
                ..Default::default()
            }
        )
        .unwrap()
        .operations()
        .is_empty()
    );
}
const RUNTIME_SOURCE: &str =
    "H 0\nM(1/3) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1\n";
#[test]
fn file_causal_runtime_and_enumeration() {
    let files = Files::new();
    files.put("zero.magna", "I 0\n");
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "foundation-v1", move |values: &[bool]| {
            assert_eq!(values.len(), 1);
            count.fetch_add(1, Ordering::Relaxed);
            Ok(values.to_vec())
        })
        .unwrap();
    let runtime = files.evaluator(
        RUNTIME_SOURCE,
        r#"{"outputs":[1],"sample_records":[0],"ideal_state":"zero.magna","observables":[[0,1]]}"#,
        &registry,
    );
    let a = ScalarArithmetic::default();
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    let prepared = runtime.prepare_sampling_queries(
        Default::default(),
        &rmagna::costs::ExecutionMeter::default(),
    );
    assert!(prepared.result.is_ok() && prepared.all_families_retained());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        runtime
            .query_work_statistics()
            .source
            .unwrap()
            .closure_evaluations,
        0
    );
    let mut seen = [false; 2];
    let mut words = 0;
    struct Counted {
        rng: SplitMix64,
        words: usize,
    }
    impl RandomSource for Counted {
        fn next_u64(&mut self) -> u64 {
            self.words += 1;
            self.rng.next_u64()
        }
    }
    for j in 0..12 {
        let mut rng = Counted {
            rng: SplitMix64::new(j),
            words: 0,
        };
        let draw = runtime.sample(&mut rng).unwrap();
        words += rng.words;
        assert_eq!(draw.sampled_records.len(), 1);
        let report = draw.sampled_records[0];
        seen[usize::from(report)] = true;
        assert_eq!(draw.history_mass, half);
        assert_eq!(
            draw.raw_error,
            if report {
                half.clone()
            } else {
                ExactScalar::zero()
            }
        );
        assert_eq!(
            draw.raw_moments,
            vec![if report { half.negated() } else { half.clone() }]
        );
        assert_eq!(draw.controller_calls.len(), 1);
        assert_eq!(draw.history_queries, 2);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 12);
    assert_eq!(seen, [true, true]);
    assert!(words >= 24);
    let enumeration = files.evaluator(
        RUNTIME_SOURCE,
        r#"{"outputs":[1],"ideal_state":"zero.magna","observables":[[0,1]]}"#,
        &registry,
    );
    let before = calls.load(Ordering::Relaxed);
    let total = enumeration
        .enumerate_physical(&TaskEnumerationOptions::default())
        .unwrap();
    assert_eq!(total.covered_probability.value(), &rational(1, 1));
    assert_eq!((total.patterns, total.histories), (2, 4));
    assert_eq!(total.masses.error, field(1, 2));
    assert_eq!(calls.load(Ordering::Relaxed) - before, 4);
    // Enumeration has no random-source argument; it cannot consume the retained
    // caller stream. Its callback effects still occur for every complete branch.
}
#[test]
fn target_and_recovery_mapping() {
    let files = Files::new();
    files.put("ideal.magna", "H 0\nT 0\n");
    let registry = ControllerRegistry::default();
    let fidelity = files.evaluator(
        "H 5\nT 5\n",
        r#"{"outputs":[5],"ideal_state":"ideal.magna"}"#,
        &registry,
    );
    let draw = fidelity.sample(&mut SplitMix64::new(0)).unwrap();
    assert_eq!(draw.raw_error, ExactScalar::zero());
    assert_eq!(draw.raw_target_success, Some(ExactScalar::one()));
    assert!(draw.sampled_records.is_empty());
    // Rust keeps the target independent, so remapping introduces no synthetic
    // target records into the source's chronological record namespace.
    files.put("zero.magna", "I 0\n");
    files.put("recovery.magna", "H 1\nM 1\nCX rec[-1] 0\n");
    for physical in ["I 5\n", "H 5\nCX 5 1\n"] {
        let recovery = files.evaluator(
            physical,
            r#"{"outputs":[5],"ideal_state":"zero.magna","recovery":"recovery.magna"}"#,
            &registry,
        );
        let result = recovery.sample(&mut SplitMix64::new(0)).unwrap();
        assert!(result.sampled_records.is_empty());
        assert_eq!(result.history_mass, ExactScalar::one());
        assert_eq!(result.error, field(1, 2));
        assert_eq!(result.target_success, Some(field(1, 2)));
    }
}
#[test]
fn entry_dependent_geometry_matches_finite_sums() {
    use rmagna::catalogue::{
        CatalogueLimits, CliffordIndicator, PhaseCatalogue, PhaseDecompositionInput,
        PhaseSelectionRoute,
    };
    let a = ScalarArithmetic::default();
    let reference = phase_input(
        2,
        &[1, 2],
        0,
        vec![0; 2],
        vec![bits(2, 0); 2],
        0,
        -2,
        vec![(1, 1), (2, 1)],
    );
    for quadratic_entries in [false, true] {
        let mut catalogue = PhaseCatalogue::new();
        let terms = (0..4)
            .map(|word| {
                let phase = if quadratic_entries && word == 3 {
                    ParityPhase::new(2, 0, [(bits(2, 1), 2)]).unwrap()
                } else {
                    ParityPhase::new(2, 0, []).unwrap()
                };
                let exponent =
                    (word & 1) + (word >> 1) - if quadratic_entries && word == 3 { 2 } else { 0 };
                CliffordIndicator {
                    coefficient: a.omega(exponent as i64, 0).unwrap(),
                    constraints: (0..2)
                        .map(|j| (bits(2, 1 << j), word >> j & 1 != 0))
                        .collect(),
                    phase,
                }
            })
            .collect();
        catalogue
            .insert(
                PhaseDecompositionInput {
                    dimension: 2,
                    phase_table: vec![0, 1, 1, 2],
                    terms,
                },
                &CatalogueLimits::default(),
                &a,
            )
            .unwrap();
        let resource = PreparedPhaseResource::new_native(
            &reference.phase(),
            4,
            NativePhaseOptions::default(),
            Some(&catalogue),
            &a,
        )
        .unwrap();
        assert_eq!(
            resource.native_resource().unwrap().receipt().route,
            PhaseSelectionRoute::CertifiedCatalogue
        );
        let prepared = resource
            .native_resource()
            .unwrap()
            .prepare_boundary(
                &reference.phase(),
                &reference.input.ket(&a),
                &Default::default(),
                &a,
            )
            .unwrap();
        assert_eq!(prepared.usage().consistent_terms, 4);
        // Rust can prepare these entry-dependent families directly, with a
        // separate contraction per term; it does not assume one common support
        // or common Clifford phase as the Native optional geometry would.
        check_transported_phase(&reference, &resource, &prepared, &a);
    }
}
use rmagna::{
    circuit::Basis,
    noise::{
        NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, OriginalNoiseAction,
        OriginalNoiseOptions, PreparedNoiseSchedule, PreparedNoiseWeights, RawNoiseFactor,
        SparseNoiseAction, SparseNoiseSampler, original_noise_categories,
    },
    probability::Probability,
};
fn probability(r: BigRational) -> Probability {
    Probability::from_rational(r).unwrap()
}
fn model(rows: &[Vec<BigRational>]) -> NoiseModel {
    NoiseModel::new(
        rows.iter()
            .map(|r| r.iter().cloned().map(probability).collect())
            .collect(),
        &NoiseOptions::default(),
    )
    .unwrap()
}
fn complete(errors: impl IntoIterator<Item = BigRational>) -> Vec<BigRational> {
    let errors = errors.into_iter().collect::<Vec<_>>();
    std::iter::once(rational(1, 1) - errors.iter().cloned().sum::<BigRational>())
        .chain(errors)
        .collect()
}
impl RandomSource for Mt19937 {
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}
#[test]
fn scaled_noise_model_and_categorical_law_interning() {
    let source=parse_circuit("DEPOLARIZE2(1/10) 0 1\nCORRELATED_ERROR(1/3) X0\nELSE_CORRELATED_ERROR(1/2) Y0\nM(1/5) 0\n",&ParseOptions::default()).unwrap();
    let categories = original_noise_categories(&source, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 3);
    assert_eq!(
        categories.iter().map(|c| c.channels.len()).sum::<usize>(),
        18
    );
    let physical = NoiseModel::new(
        categories
            .iter()
            .map(|c| {
                std::iter::once(c.identity_probability.clone())
                    .chain(c.channels.iter().map(|a| a.probability.clone()))
                    .collect()
            })
            .collect(),
        &Default::default(),
    )
    .unwrap();
    let proposed = physical
        .scaled_proposal(&rational(1, 2), &BTreeSet::new(), &Default::default())
        .unwrap();
    assert_eq!(proposed.summary().scaled_categories, 3);
    assert_eq!(
        proposed.model().probabilities(1).unwrap()[1].value(),
        &rational(1, 6)
    );
    let empty = NoiseModel::new(vec![], &Default::default()).unwrap();
    for scale in [-1, 0] {
        assert!(
            empty
                .scaled_proposal(&rational(scale, 1), &BTreeSet::new(), &Default::default())
                .is_err()
        );
    }
    let mixed = model(&[complete([rational(1, 10)]), complete([rational(1, 5)])]);
    for scale in [5, 6] {
        assert!(
            mixed
                .scaled_proposal(&rational(scale, 1), &BTreeSet::new(), &Default::default())
                .is_err()
        );
    }
    let repeated = parse_circuit(
        "DEPOLARIZE1(1/8) 0 1\nX_ERROR(1/8) 0\nDEPOLARIZE1(1/8) 2\n",
        &ParseOptions::default(),
    )
    .unwrap();
    let categories = original_noise_categories(&repeated, &Default::default()).unwrap();
    let channels = categories
        .iter()
        .map(|c| c.channels.iter().map(|v| v.probability.clone()).collect())
        .collect::<Vec<Vec<_>>>();
    assert_eq!(channels[0], channels[1]);
    assert_eq!(channels[0], channels[3]);
    assert_ne!(channels[0], channels[2]);
    let actions = categories
        .iter()
        .map(|c| {
            c.channels
                .iter()
                .map(|v| match &v.action {
                    OriginalNoiseAction::Pauli { factors } => SparseNoiseAction::Pauli {
                        operation: format!("op{}", v.source.operation),
                        factors: factors.clone(),
                    },
                    _ => panic!("unexpected report"),
                })
                .collect()
        })
        .collect::<Vec<Vec<_>>>();
    let plan = PreparedNoiseSchedule::unweighted(&channels, &actions, Default::default()).unwrap();
    assert_eq!(plan.statistics().conditional_models, 1);
    assert_eq!(plan.statistics().sites, 4);
    assert_ne!(actions[0], actions[1]);
}
// Literal per-lane comparison against the raw rational law. Unlike the
// production packed kernel, this oracle keeps a separate comparison state per
// site. The source fixture has the fixed conditional integer weights1:2:3.
fn literal_small_range(bound: u64, rng: &mut impl RandomSource) -> u64 {
    let bits = 64 - (bound - 1).leading_zeros();
    if bits == 0 {
        return 0;
    }
    loop {
        let value = rng.next_u64() & ((1u64 << bits) - 1);
        if value < bound {
            return value;
        }
    }
}
fn literal_prepared_choices(
    rows: &[Vec<BigRational>],
    rng: &mut impl RandomSource,
) -> (Vec<Option<usize>>, usize, usize) {
    use num_traits::{ToPrimitive, Zero};
    let mut groups: Vec<(BigRational, Vec<usize>)> = vec![];
    for (site, row) in rows.iter().enumerate() {
        let total = rational(1, 1) - &row[0];
        if let Some((_, sites)) = groups.iter_mut().find(|(law, _)| *law == total) {
            sites.push(site);
        } else {
            groups.push((total, vec![site]));
        }
    }
    let mut selected = vec![false; rows.len()];
    let mut planes = 0;
    let mut tails = 0;
    for (total, sites) in groups {
        if total == rational(0, 1) {
            continue;
        }
        if total == rational(1, 1) {
            for site in sites {
                selected[site] = true;
            }
            continue;
        }
        let scaled = total.numer() << 64usize;
        let cutoff = (&scaled / total.denom()).to_u64().unwrap();
        let residual = BigRational::new(scaled % total.denom(), total.denom().clone());
        for block in sites.chunks(64) {
            let mut equal = vec![true; block.len()];
            for position in 0..64 {
                if !equal.iter().any(|&v| v) {
                    break;
                }
                // A dyadic equality can reject after the last one bit, because
                // all future cutoff bits and the residual are zero.
                if residual.is_zero() && (cutoff << position) == 0 {
                    break;
                }
                let plane = rng.next_u64();
                planes += 1;
                let threshold = cutoff >> (63 - position) & 1;
                for (lane, &site) in block.iter().enumerate() {
                    if !equal[lane] {
                        continue;
                    }
                    let random = plane >> lane & 1;
                    if random != threshold {
                        selected[site] = random < threshold;
                        equal[lane] = false;
                    }
                }
            }
            if !residual.is_zero() {
                let denominator = residual.denom().to_u64().unwrap();
                let numerator = residual.numer().to_u64().unwrap();
                for (lane, &site) in block.iter().enumerate() {
                    if equal[lane] {
                        // The scalar residual uses an identity-first category.
                        selected[site] =
                            literal_small_range(denominator, rng) >= denominator - numerator;
                        tails += 1;
                    }
                }
            }
        }
    }
    let choices = selected
        .into_iter()
        .map(|yes| {
            if yes {
                let value = literal_small_range(6, rng);
                Some(if value < 1 {
                    0
                } else if value < 3 {
                    1
                } else {
                    2
                })
            } else {
                None
            }
        })
        .collect();
    (choices, planes, tails)
}
#[derive(Clone)]
struct PreparedLawWords {
    prefix: std::collections::VecDeque<u64>,
    fallback: Mt19937,
    draws: usize,
    fingerprint: u64,
}
impl RandomSource for PreparedLawWords {
    fn next_u64(&mut self) -> u64 {
        let value = self
            .prefix
            .pop_front()
            .unwrap_or_else(|| self.fallback.next());
        self.draws += 1;
        self.fingerprint = self.fingerprint.rotate_left(7) ^ value;
        value
    }
}
#[test]
fn cached_noise_law_and_weight() {
    let mut channels = vec![
        (1i64, 1000i64, false),
        (1, 1000, false),
        (1, 1000, true),
        (0, 6, false),
        (1, 6, false),
    ];
    channels.extend((2..=36).map(|k| (k, 1000, false)));
    channels.push((1, 1000, false));
    assert_eq!(channels.len(), 41);
    let mut text = "CONTROL[XOR](1) -> ctrl[0]\n".to_string();
    for (j, &(n, d, guard)) in channels.iter().enumerate() {
        text += &format!(
            "PAULI_CHANNEL_1({n}/{d},{}/{d},{}/{d}) {}{}\n",
            2 * n,
            3 * n,
            if guard { "ctrl[0] " } else { "" },
            j % 3
        );
    }
    let source = parse_circuit(&text, &ParseOptions::default()).unwrap();
    let metadata = original_noise_categories(&source, &Default::default()).unwrap();
    assert_eq!(metadata.len(), 41);
    let actual_p = metadata
        .iter()
        .map(|c| {
            std::iter::once(c.identity_probability.value().clone())
                .chain(c.channels.iter().map(|v| v.probability.value().clone()))
                .collect()
        })
        .collect::<Vec<Vec<_>>>();
    let independent_p = channels
        .iter()
        .map(|&(n, d, _)| complete((1..=3).map(|k| rational(k * n, d))))
        .collect::<Vec<_>>();
    assert_eq!(actual_p, independent_p);
    let mut actions = vec![];
    for (j, c) in metadata.iter().enumerate() {
        let mut row = vec![];
        for (k, v) in c.channels.iter().enumerate() {
            assert_eq!(v.source.operation, j + 1);
            assert_eq!(
                v.action,
                OriginalNoiseAction::Pauli {
                    factors: vec![(j % 3, [Basis::X, Basis::Y, Basis::Z][k])]
                }
            );
            row.push(SparseNoiseAction::Pauli {
                operation: format!("op{}", j + 1),
                factors: vec![(j % 3, [Basis::X, Basis::Y, Basis::Z][k])],
            });
        }
        actions.push(row);
    }
    let guarded = metadata
        .iter()
        .enumerate()
        .filter_map(|(j, c)| c.guard.is_some().then_some(j))
        .collect::<BTreeSet<_>>();
    assert_eq!(guarded, BTreeSet::from([2]));
    for scale in [
        None,
        Some(rational(1, 2)),
        Some(rational(1, 1)),
        Some(rational(2, 1)),
    ] {
        let mut expected_q = independent_p.clone();
        let mut factors = vec![];
        let mut tilted = 0;
        for (j, &(n, d, guard)) in channels.iter().enumerate() {
            let eligible = scale.is_some() && !guard && n > 0 && 6 * n < d;
            let ratio = if eligible {
                scale.clone().unwrap()
            } else {
                rational(1, 1)
            };
            expected_q[j] = complete((1..=3).map(|k| rational(k * n, d) * &ratio));
            if eligible && ratio != rational(1, 1) {
                tilted += 1;
            }
            let absent = if independent_p[j][0] == rational(0, 1) {
                rational(1, 1)
            } else {
                &independent_p[j][0] / &expected_q[j][0]
            };
            let present = if n > 0 {
                rational(1, 1) / ratio
            } else {
                rational(1, 1)
            };
            // Endpoint 1 has zero absent mass; its unused ratio is one.
            factors.push((
                RawNoiseFactor::from_ratio(absent.numer().clone(), absent.denom().clone()).unwrap(),
                RawNoiseFactor::from_ratio(present.numer().clone(), present.denom().clone())
                    .unwrap(),
            ));
        }
        let p = model(&actual_p);
        let independent = model(&independent_p);
        let q = if let Some(scale) = &scale {
            let q = p
                .scaled_proposal(scale, &guarded, &Default::default())
                .unwrap();
            assert_eq!(q.summary().scaled_categories, tilted);
            assert_eq!(q.summary().endpoint_categories, 2);
            q.into_model()
        } else {
            p.clone()
        };
        for (j, expected) in expected_q.iter().enumerate() {
            assert_eq!(
                q.probabilities(j)
                    .unwrap()
                    .iter()
                    .map(|p| p.value().clone())
                    .collect::<Vec<_>>(),
                *expected
            );
        }
        let prepared_channels = (0..41)
            .map(|j| q.probabilities(j).unwrap()[1..].to_vec())
            .collect::<Vec<_>>();
        let prepared =
            PreparedNoiseSchedule::new(&prepared_channels, &actions, &factors, Default::default())
                .unwrap();
        let prepared_stats = prepared.statistics();
        assert_eq!(prepared_stats.sites, 41);
        assert_eq!(prepared_stats.conditional_models, 1);
        assert_eq!(prepared_stats.endpoint_groups, 2);
        let totals = expected_q
            .iter()
            .map(|row| rational(1, 1) - &row[0])
            .collect::<BTreeSet<_>>();
        assert_eq!(prepared_stats.occurrence_groups, totals.len());
        let expected = model(&expected_q);
        let full = NoiseSampler::importance(p, q.clone(), Default::default()).unwrap();
        let fresh =
            NoiseSampler::importance(independent, expected.clone(), Default::default()).unwrap();
        let sparse =
            SparseNoiseSampler::new(q, &actions, &NoiseSampling::Physical, Default::default())
                .unwrap();
        let fresh_sparse = SparseNoiseSampler::new(
            expected,
            &actions,
            &NoiseSampling::Physical,
            Default::default(),
        )
        .unwrap();
        let weights = PreparedNoiseWeights::new(&factors, Default::default()).unwrap();
        for seed in 0..34 {
            use num_traits::ToPrimitive;
            let mut prefix = std::collections::VecDeque::new();
            if seed >= 32 {
                // Force U==floor(p*2^64) on every lane in the first shared
                // group, then alternate exact residual accepts/rejects. MT
                // schedules virtually never hit this equality by chance.
                let total = rational(1, 1) - &expected_q[0][0];
                let scaled = total.numer() << 64usize;
                let cutoff = (&scaled / total.denom()).to_u64().unwrap();
                let residual = BigRational::new(scaled % total.denom(), total.denom().clone());
                let denominator = residual.denom().to_u64().unwrap();
                let lanes = expected_q
                    .iter()
                    .filter(|row| rational(1, 1) - &row[0] == total)
                    .count();
                for bit in (0..64).rev() {
                    prefix.push_back(if cutoff >> bit & 1 != 0 { u64::MAX } else { 0 });
                }
                for lane in 0..lanes {
                    prefix.push_back(if (lane + seed as usize) % 2 == 0 {
                        denominator - 1
                    } else {
                        0
                    });
                }
            }
            let mut actual_rng = PreparedLawWords {
                prefix,
                fallback: Mt19937::new(seed),
                draws: 0,
                fingerprint: 0,
            };
            let mut expected_rng = actual_rng.clone();
            let (expected_choices, planes, tails) =
                literal_prepared_choices(&expected_q, &mut expected_rng);
            let draw = prepared.draw(&mut actual_rng).unwrap();
            assert_eq!(draw.choices, expected_choices);
            assert_eq!(
                (draw.occurrence_planes, draw.exact_tail_evaluations),
                (planes, tails)
            );
            assert_eq!(draw.entropy_words, actual_rng.draws);
            assert_eq!(
                (actual_rng.draws, actual_rng.fingerprint),
                (expected_rng.draws, expected_rng.fingerprint)
            );
            assert_eq!(actual_rng.prefix, expected_rng.prefix);
            if seed >= 32 {
                assert!(tails >= 3);
            }
            let mut expected_actions = vec![];
            let mut p_mass = rational(1, 1);
            let mut q_mass = rational(1, 1);
            for (site, choice) in expected_choices.iter().enumerate() {
                let column = choice.map_or(0, |j| j + 1);
                p_mass *= &independent_p[site][column];
                q_mass *= &expected_q[site][column];
                if let Some(alternative) = choice {
                    expected_actions.push(SparseNoiseAction::Pauli {
                        operation: format!("op{}", site + 1),
                        factors: vec![(site % 3, [Basis::X, Basis::Y, Basis::Z][*alternative])],
                    });
                }
            }
            assert_eq!(draw.actions, expected_actions);
            assert_eq!(draw.physical_probability, q_mass);
            let ratio = BigRational::new(
                draw.raw_weight.numerator().clone().into(),
                draw.raw_weight.denominator().clone().into(),
            );
            assert_eq!(&ratio * &q_mass, p_mass);
            assert_eq!(prepared.statistics(), prepared_stats);
            // Packed and ordinary scalar paths intentionally have different
            // streams. Each is compared with its own exact input/RNG oracle.
            if seed >= 32 {
                continue;
            }
            let mut reused_rng = Mt19937::new(seed);
            let mut fresh_rng = reused_rng.clone();
            let observed = full.draw(&mut reused_rng).unwrap();
            let independent = fresh.draw(&mut fresh_rng).unwrap();
            assert_eq!(observed, independent);
            assert_eq!(
                (reused_rng.draws, reused_rng.fingerprint),
                (fresh_rng.draws, fresh_rng.fingerprint)
            );
            let mut sparse_rng = Mt19937::new(seed);
            let sparse_draw = sparse.draw(&mut sparse_rng).unwrap();
            assert_eq!(sparse_draw.noise.faults, observed.faults);
            assert_eq!(
                (sparse_rng.draws, sparse_rng.fingerprint),
                (reused_rng.draws, reused_rng.fingerprint)
            );
            let mut other_rng = Mt19937::new(seed);
            assert_eq!(sparse_draw, fresh_sparse.draw(&mut other_rng).unwrap());
            assert_eq!(
                (other_rng.draws, other_rng.fingerprint),
                (reused_rng.draws, reused_rng.fingerprint)
            );
            let mut selected = vec![false; 41];
            for f in &observed.faults {
                selected[f.site] = true;
            }
            let raw = weights.evaluate(&selected).unwrap();
            let ratio = BigRational::new(
                raw.numerator().clone().into(),
                raw.denominator().clone().into(),
            );
            assert_eq!(ratio, observed.likelihood_ratio);
            let mut expected_weight = rational(1, 1);
            for j in 0..41 {
                let c = observed
                    .faults
                    .iter()
                    .find(|f| f.site == j)
                    .map_or(0, |f| f.alternative + 1);
                expected_weight *= &independent_p[j][c] / &expected_q[j][c];
            }
            assert_eq!(ratio, expected_weight);
        }
    }
}
#[path = "common/mod.rs"]
#[allow(dead_code)]
mod common;
use rmagna::{
    circuit::BitAtom,
    clifford::CliffordGate as C,
    instrument::{
        CompileOptions, CompiledInstrument, FaultSelection, QueryPreparation, compile_affine,
    },
    unitary::CompilerStrategy,
};
fn report(mask: usize) -> BitExpr {
    BitExpr {
        constant: false,
        terms: (0..3)
            .filter(|j| mask >> j & 1 != 0)
            .map(BitAtom::Record)
            .collect(),
    }
}
fn history_dense(magic: bool, faults: &[FaultSelection], a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut state = vec![ExactScalar::zero(); 8];
    state[0] = ExactScalar::one();
    for q in 0..3 {
        common::dense_gate(C::H(q), &mut state, a);
    }
    for (word, v) in state.iter_mut().enumerate() {
        if word & 4 != 0 {
            *v = a
                .multiply(v, &a.omega(if magic { 1 } else { 2 }, 0).unwrap())
                .unwrap();
        }
    }
    common::dense_gate(C::H(2), &mut state, a);
    if faults.iter().any(|f| f.site == 0) {
        common::dense_gate(C::X(0), &mut state, a);
    }
    for (word, v) in state.iter_mut().enumerate() {
        if word & 3 == 3 {
            *v = a.multiply(v, &a.omega(2, 0).unwrap()).unwrap();
        }
    }
    common::dense_gate(C::H(1), &mut state, a);
    if faults.iter().any(|f| f.site == 1) {
        common::dense_gate(C::X(0), &mut state, a);
    }
    if faults.iter().any(|f| f.site == 2) {
        common::dense_gate(C::X(1), &mut state, a);
    }
    state
        .iter()
        .map(|v| a.multiply(v, &v.conjugate()).unwrap())
        .collect()
}
fn prepared_counts(s: rmagna::unitary::QueryWorkStatistics) -> [u64; 10] {
    assert!(!s.saturated);
    [
        s.affine_closures_prepared,
        s.affine_geometry_preparations,
        s.phase_maps_prepared,
        s.phase_resources_prepared,
        s.support_projections_prepared,
        s.support_boundary_maps_prepared,
        s.support_contractions_prepared,
        s.support_phase_maps_prepared,
        s.support_phase_resources_prepared,
        s.support_parents_prepared,
    ]
}
fn history_query(
    base: &CompiledInstrument,
    committed: &CompiledInstrument,
    observed: &[(usize, bool)],
    sector: bool,
    faults: &[FaultSelection],
    keep: &[usize],
    magic: bool,
    a: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let flip = faults.iter().filter(|f| f.site == 0 || f.site == 1).count() % 2 != 0;
    let sector_event = (report(1), sector ^ flip);
    let view = committed
        .with_reported_conditions(&[sector_event.clone()])
        .unwrap();
    let mut predicates = vec![sector_event];
    predicates.extend(observed.iter().map(|&(row, v)| (report(row), v)));
    let fresh = base.with_reported_conditions(&predicates).unwrap();
    let before = view.acceptance(faults, 256, a).unwrap().value;
    let saved = committed.acceptance(faults, 256, a).unwrap().value;
    let dense = history_dense(magic, faults, a);
    let allowed = |word: usize| {
        ((word & 1 != 0) == (sector ^ flip))
            && observed
                .iter()
                .all(|&(row, v)| ((row & word).count_ones() % 2 != 0) == v)
    };
    let keep_rows = keep.iter().map(|&m| report(m)).collect::<Vec<_>>();
    let mut actual = vec![];
    for code in 0..1usize << keep.len() {
        let value = view
            .accepted_probability(&keep_rows, &bits(keep.len(), code), faults, 256, a)
            .unwrap()
            .value;
        let direct = fresh
            .accepted_probability(&keep_rows, &bits(keep.len(), code), faults, 256, a)
            .unwrap()
            .value;
        let expected = (0..8)
            .filter(|&word| {
                allowed(word)
                    && keep.iter().enumerate().all(|(j, &row)| {
                        ((row & word).count_ones() as usize & 1) == (code >> j & 1)
                    })
            })
            .fold(ExactScalar::zero(), |sum, word| {
                a.add(&sum, &dense[word]).unwrap()
            });
        assert_eq!(value, direct);
        assert_eq!(value, expected);
        actual.push(value);
    }
    let counts = prepared_counts(base.query_work_statistics());
    for code in 0..1usize << keep.len() {
        assert_eq!(
            view.accepted_probability(&keep_rows, &bits(keep.len(), code), faults, 256, a)
                .unwrap()
                .value,
            actual[code]
        );
    }
    assert_eq!(
        prepared_counts(base.query_work_statistics()),
        counts,
        "warm disposable marginals prepare no maps or resources"
    );
    assert_eq!(view.acceptance(faults, 256, a).unwrap().value, before);
    assert_eq!(committed.acceptance(faults, 256, a).unwrap().value, saved);
    let total = actual
        .iter()
        .fold(ExactScalar::zero(), |sum, v| a.add(&sum, v).unwrap());
    assert_eq!(before, total);
    assert_eq!(fresh.acceptance(faults, 256, a).unwrap().value, total);
    let raw = base
        .reported_support_terms(&[report(1), report(2), report(4)], faults, 256, a)
        .unwrap();
    let event_rows = std::iter::once(bits(3, 1))
        .chain(observed.iter().map(|&(row, _)| bits(3, row)))
        .collect::<Vec<_>>();
    let rhs = BitVector::from_indices(
        1 + observed.len(),
        std::iter::once(sector ^ flip)
            .chain(observed.iter().map(|&(_, v)| v))
            .enumerate()
            .filter_map(|(j, v)| v.then_some(j)),
    )
    .unwrap();
    let conditioned = raw
        .iter()
        .filter_map(|s| s.condition(&event_rows, &rhs).unwrap())
        .map(|s| {
            s.linear_image(&keep.iter().map(|&m| bits(3, m)).collect::<Vec<_>>())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let support = AffineSupport::hull(&conditioned).unwrap();
    let support_words = affine_words(support.as_ref());
    // Build the independent envelope from explicit original support words,
    // conditioning each word before projection and affine closure.
    let mut points = BTreeSet::new();
    for source in &raw {
        for word in affine_words(Some(source)) {
            if allowed(word) {
                points.insert(keep.iter().enumerate().fold(0, |v, (j, &m)| {
                    v | (((word & m).count_ones() as usize & 1) << j)
                }));
            }
        }
    }
    let expected = if let Some(&offset) = points.first() {
        let differences = points.iter().map(|&p| p ^ offset).collect::<Vec<_>>();
        let mut span = BTreeSet::from([offset]);
        for d in differences {
            let next = span.iter().map(|v| v ^ d).collect::<Vec<_>>();
            span.extend(next);
        }
        span
    } else {
        BTreeSet::new()
    };
    assert_eq!(support_words, expected);
    for (code, mass) in actual.iter().enumerate() {
        if !mass.is_zero() {
            assert!(support_words.contains(&code));
        }
    }
    assert_eq!(view.acceptance(faults, 256, a).unwrap().value, before);
    assert_eq!(committed.acceptance(faults, 256, a).unwrap().value, saved);
    actual
}
#[test]
fn check_incremental_history_session() {
    let a = ScalarArithmetic::default();
    for magic in [false, true] {
        let text = format!(
            "H 0\nH 1\nH 2\n{} 2\nH 2\nX_ERROR(1/2) 0\nCS 0 1\nH 1\nX_ERROR(1/2) 0\nX_ERROR(1/2) 1\nM 0 1 2\n",
            if magic { "T" } else { "S" }
        );
        let options = CompileOptions {
            compiler_strategy: CompilerStrategy::SharedCommutingLayer,
            probability_preparation: QueryPreparation::FixedFibers(Default::default()),
            max_prepared_probabilities: 128,
            ..Default::default()
        };
        let base = compile_affine(
            &parse_circuit(&text, &ParseOptions::default()).unwrap(),
            &options,
            &a,
        )
        .unwrap();
        for sector in [false, true] {
            let mut observed = vec![(1, sector)];
            let mut committed = base
                .with_reported_conditions(&[(report(1), sector)])
                .unwrap();
            let check = |committed: &CompiledInstrument,
                         observed: &[(usize, bool)],
                         faults: &[FaultSelection],
                         keep: &[usize]| {
                history_query(&base, committed, observed, sector, faults, keep, magic, &a)
            };
            check(&committed, &observed, &[], &[2]);
            let second = check(&committed, &observed, &[], &[4]);
            let bias = if magic {
                a.omega(0, -3).unwrap()
            } else {
                ExactScalar::zero()
            };
            let quarter = a.omega(0, -4).unwrap();
            assert_eq!(
                second,
                [
                    a.add(
                        &quarter,
                        &a.multiply(&bias, &a.omega(0, -2).unwrap()).unwrap()
                    )
                    .unwrap(),
                    a.subtract(
                        &quarter,
                        &a.multiply(&bias, &a.omega(0, -2).unwrap()).unwrap()
                    )
                    .unwrap()
                ]
            );
            check(&committed, &observed, &[], &[3, 4]);
            for event in [(3, sector), (2, false)] {
                observed.push(event);
                committed = committed
                    .with_reported_conditions(&[(report(event.0), event.1)])
                    .unwrap();
            }
            let original = check(&committed, &observed, &[], &[4, 2]);
            let before = [FaultSelection {
                site: 0,
                alternative: 0,
            }];
            let zero = check(&committed, &observed, &before, &[2, 4]);
            assert!(zero.iter().all(ExactScalar::is_zero));
            let restore = [
                FaultSelection {
                    site: 0,
                    alternative: 0,
                },
                FaultSelection {
                    site: 1,
                    alternative: 0,
                },
            ];
            check(&committed, &observed, &restore, &[2]);
            let changed = check(&committed, &observed, &restore, &[4, 3]);
            assert_ne!(
                original
                    .iter()
                    .fold(ExactScalar::zero(), |s, v| a.add(&s, v).unwrap()),
                changed
                    .iter()
                    .fold(ExactScalar::zero(), |s, v| a.add(&s, v).unwrap()),
                "the same prepared owner transports an actual changed conditional law"
            );
            let report_flip = [
                FaultSelection {
                    site: 0,
                    alternative: 0,
                },
                FaultSelection {
                    site: 1,
                    alternative: 0,
                },
                FaultSelection {
                    site: 2,
                    alternative: 0,
                },
            ];
            check(&committed, &observed, &report_flip, &[4, 2]);
            check(&committed, &observed, &[], &[2, 4]);
            assert_eq!(observed.len(), 3);
        }
        assert!(base.query_work_statistics().support_evaluations > 0);
        assert_eq!(
            base.query_work_statistics()
                .support_changed_boundary_fallbacks,
            0
        );
    }
}
#[test]
fn check_reference_boundary() {
    let a = ScalarArithmetic::default();
    let generators = [bits(2, 3), bits(2, 2)];
    let coordinates =
        LinearFactorization::new(2, &transpose_columns(2, &generators).unwrap()).unwrap();
    let physical_generators = [bits(2, 3), bits(2, 1)];
    let mapped = |mask: usize| {
        xors(
            2,
            &physical_generators,
            &coordinates.solve(&bits(2, mask)).unwrap().unwrap(),
        )
        .unwrap()
    };
    for a in 0..4 {
        for b in 0..4 {
            let labels = [mapped(a), mapped(b), mapped(a ^ b)];
            for (label, word) in labels.iter().zip([a, b, a ^ b]) {
                assert_eq!(*label, bits(2, ((word & 1) << 1) | (word >> 1)));
            }
            let mut sum = labels[0].clone();
            sum.xor_assign(&labels[1]).unwrap();
            assert_eq!(sum, labels[2]);
        }
    }
    let repeated = (0..70).map(|j| bits(2, 1 << (j % 2))).collect::<Vec<_>>();
    assert_eq!(
        xors(2, &repeated, &BitVector::from_indices(70, [0, 64]).unwrap()).unwrap(),
        bits(2, 0)
    );
    assert_eq!(
        xors(
            2,
            &repeated,
            &BitVector::from_indices(70, [1, 65, 69]).unwrap()
        )
        .unwrap(),
        bits(2, 2)
    );
    assert!(
        xors(
            2,
            &[bits(2, 2), bits(2, 1)],
            &BitVector::from_indices(71, [70]).unwrap()
        )
        .is_err()
    );
    assert!(BitVector::from_indices(2, [2]).is_err());
    let closed =
        LinearFactorization::new(1, &transpose_columns(2, &[bits(2, 1)]).unwrap()).unwrap();
    for sector in 0..2 {
        for flip in 0..4 {
            let values = [1, 0, 1].iter().enumerate().fold(0, |v, (j, &m)| {
                let c = closed.solve(&bits(2, m)).unwrap().unwrap();
                v | ((usize::from(c.dot(&bits(1, sector)).unwrap())
                    ^ ((m & flip).count_ones() as usize & 1))
                    << j)
            });
            assert_eq!(values, if (sector ^ flip) & 1 != 0 { 5 } else { 0 });
            assert_eq!(closed.solve(&bits(2, 3)).unwrap(), None);
            for mask in 0..4 {
                let row = mapped(mask);
                let c = closed.solve(&row).unwrap();
                let value =
                    c.map(|c| c.dot(&bits(1, sector)).unwrap() ^ row.dot(&bits(2, flip)).unwrap());
                let physical = ((mask & 1) << 1) | (mask >> 1);
                let expected = (physical & 2 == 0).then_some(
                    ((physical & 1) & sector) ^ ((physical & flip).count_ones() as usize & 1) != 0,
                );
                assert_eq!(value, expected);
            }
            let family = AffineSupport::new(bits(2, sector ^ flip), vec![bits(2, 2)]).unwrap();
            assert!(
                family
                    .condition(&[bits(2, 1), bits(2, 1)], &bits(2, 2))
                    .unwrap()
                    .is_none()
            );
        }
    }
    let observed = [3usize, 2, 1, 0, 3];
    let open_rows = observed
        .iter()
        .map(|m| bits(1, (m >> 1) & 1))
        .collect::<Vec<_>>();
    for sector in 0..2 {
        for outcomes in 0..32 {
            for direct_closed in [false, true] {
                let factor = LinearFactorization::new(1, &open_rows).unwrap();
                let geometry = (
                    factor.rank(),
                    factor.pivot_columns().to_vec(),
                    factor.row_space_basis().to_vec(),
                );
                for flip in [0usize, 1, 3, 2, 0, 0, 3] {
                    let rhs = BitVector::from_indices(
                        5,
                        observed.iter().enumerate().filter_map(|(j, &mask)| {
                            let physical = bits(2, mask);
                            let closed_value = if direct_closed {
                                (mask & 1) & sector
                            } else {
                                usize::from(physical.low(1).unwrap().dot(&bits(1, sector)).unwrap())
                            };
                            let value = (outcomes >> j & 1)
                                ^ closed_value
                                ^ usize::from(physical.dot(&bits(2, flip)).unwrap());
                            (value != 0).then_some(j)
                        }),
                    )
                    .unwrap();
                    let solution = factor.solve(&rhs).unwrap();
                    let allowed = solution.map(|point| {
                        AffineSupport::new(point, factor.nullspace().unwrap()).unwrap()
                    });
                    let fresh = AffineSupport::new(bits(2, sector ^ (flip & 1)), vec![bits(2, 2)])
                        .unwrap()
                        .condition(
                            &observed.iter().map(|&m| bits(2, m)).collect::<Vec<_>>(),
                            &bits(5, outcomes),
                        )
                        .unwrap();
                    let fresh_open = fresh.map(|s| {
                        s.linear_image(&[bits(2, 2)])
                            .unwrap()
                            .translated(&bits(1, flip >> 1))
                            .unwrap()
                    });
                    for word in 0..2 {
                        let physical = (sector ^ (flip & 1)) | ((word ^ (flip >> 1)) << 1);
                        let expected = observed.iter().enumerate().all(|(j, m)| {
                            ((physical & m).count_ones() as usize & 1) == (outcomes >> j & 1)
                        });
                        assert_eq!(affine_words(allowed.as_ref()).contains(&word), expected);
                        assert_eq!(affine_words(fresh_open.as_ref()).contains(&word), expected);
                    }
                    assert_eq!(
                        factor.solve(&rhs).unwrap(),
                        allowed.as_ref().map(|s| s.offset().clone())
                    );
                    assert_eq!(
                        (
                            factor.rank(),
                            factor.pivot_columns().to_vec(),
                            factor.row_space_basis().to_vec()
                        ),
                        geometry
                    );
                }
            }
        }
    }
    let circuit = parse_circuit(
        "X 0\nH 1\nT 1\nH 1\nX_ERROR(1/2) 0\nX_ERROR(1/2) 1\nM 0 1\n",
        &ParseOptions::default(),
    )
    .unwrap();
    let base = compile_affine(
        &circuit,
        &CompileOptions {
            probability_preparation: QueryPreparation::FixedFibers(Default::default()),
            max_prepared_probabilities: 128,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    for sector in 0..2 {
        for flip in 0..4 {
            for event in 0..5 {
                for bit in 0..2 {
                    let faults = (0..2)
                        .filter(|j| flip >> j & 1 != 0)
                        .map(|site| FaultSelection {
                            site,
                            alternative: 0,
                        })
                        .collect::<Vec<_>>();
                    let mut conditions = vec![(report(1), (sector ^ (flip & 1)) != 0)];
                    if event < 4 {
                        conditions.push((report(event), bit != 0));
                    }
                    let view = base.with_reported_conditions(&conditions).unwrap();
                    let before = view.acceptance(&faults, 256, &a).unwrap().value;
                    let mut expected = vec![ExactScalar::zero(); 4];
                    if sector == 1 {
                        for second in 0..2 {
                            let word = (1 | (second << 1)) ^ flip;
                            if event < 4 && ((event & word).count_ones() as usize & 1) != bit {
                                continue;
                            }
                            let label = ((word & 1) ^ ((word >> 1) & 1)) | ((word & 1) << 1);
                            let half = a.omega(0, -2).unwrap();
                            let bias = a.omega(0, -3).unwrap();
                            expected[label] = if second == 0 {
                                a.add(&half, &bias).unwrap()
                            } else {
                                a.subtract(&half, &bias).unwrap()
                            };
                        }
                    }
                    for word in 0..4 {
                        assert_eq!(
                            view.accepted_probability(
                                &[report(3), report(1)],
                                &bits(2, word),
                                &faults,
                                256,
                                &a
                            )
                            .unwrap()
                            .value,
                            expected[word]
                        );
                    }
                    assert_eq!(view.acceptance(&faults, 256, &a).unwrap().value, before);
                }
            }
        }
    }
    // The prepared record map owns real source payloads. Moving/cloning its
    // owner and dropping every original input must leave the results valid.
    let u = rmagna::instrument::compile_unitary(
        &parse_circuit("X 0\nH 1\nT 1\nH 1\n", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let terms = u.stabilizer_expansion(Default::default(), &a).unwrap();
    let spec = rmagna::records::CompletedRecordSpec::new(
        2,
        &[bits(2, 3), bits(2, 1)],
        &[],
        &bits(0, 0),
        Default::default(),
    )
    .unwrap();
    let original = Arc::new(
        spec.prepare(terms.entries(), Default::default(), &a)
            .unwrap(),
    );
    let copied = original.clone();
    let moved = copied;
    let weak = Arc::downgrade(&original);
    drop(original);
    drop(terms);
    drop(u);
    assert_eq!(Arc::strong_count(&moved), 1);
    let law = moved.probability(&bits(0, 0), &a).unwrap().distribution;
    assert_eq!(
        law.value(&bits(2, 3), &a).unwrap(),
        a.add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
            .unwrap()
    );
    assert_eq!(
        law.value(&bits(2, 2), &a).unwrap(),
        a.subtract(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
            .unwrap()
    );
    drop(moved);
    assert!(weak.upgrade().is_none());
}
use rmagna::instrument::{
    ControlExpression, ControlGate, ControlGateKind, ControlRule, OriginalAssignment,
    OriginalControl, OriginalDeclaredControl, OriginalFault, OriginalGate as G, OriginalInstrument,
    OriginalLimits, OriginalOperation, OriginalOrigin, OriginalPauli, OriginalRecordEvent,
    OriginalRecordParity,
};
fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn measure(id: &str, record: &str, x: bool) -> OriginalOperation {
    op(
        id,
        G::Measure {
            factors: vec![(
                0,
                if x {
                    OriginalPauli::X
                } else {
                    OriginalPauli::Z
                },
            )],
            inverted: false,
            record: record.into(),
        },
    )
}
#[test]
fn literal_branch_order_and_declared_controls() {
    let a = ScalarArithmetic::default();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let operations = vec![
        measure("m", "m", false),
        op("c", G::Control),
        measure("n", "n", true),
    ];
    let controls = vec![OriginalControl {
        operation: "c".into(),
        parents: vec!["m".into()],
        version: "v1".into(),
        callback: Arc::new(move |parents| {
            calls.fetch_add(1, Ordering::Relaxed);
            assert_eq!(parents.entries(), &[("m".into(), false)]);
            Ok(vec![G::Pauli {
                wire: 0,
                pauli: OriginalPauli::X,
            }])
        }),
    }];
    let input = rmagna::clifford::CliffordCircuit::computational(bits(1, 0)).unwrap();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        input,
        operations,
        controls,
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let assignment = OriginalAssignment {
        true_values: vec![("m".into(), true), ("n".into(), false)],
        report_flips: vec!["m".into()],
        faults: vec![
            OriginalFault {
                after: "c".into(),
                wire: 0,
                pauli: OriginalPauli::I,
            },
            OriginalFault {
                after: "m".into(),
                wire: 0,
                pauli: OriginalPauli::Z,
            },
            OriginalFault {
                after: "c".into(),
                wire: 0,
                pauli: OriginalPauli::Y,
            },
        ],
    };
    let branch = ir
        .compile(&assignment, CompilerStrategy::ParityCopies, &a)
        .unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(branch.decisions().len(), 1);
    assert!(branch.records().iter().all(|r| !r.reported_value));
    assert_eq!(
        branch.origins(),
        &[
            OriginalOrigin::Operation("m".into()),
            OriginalOrigin::Fault {
                after: "m".into(),
                ordinal: 1
            },
            OriginalOrigin::Operation("c".into()),
            OriginalOrigin::ControlEmission {
                control: "c".into(),
                ordinal: 0
            },
            OriginalOrigin::Fault {
                after: "c".into(),
                ordinal: 0
            },
            OriginalOrigin::Fault {
                after: "c".into(),
                ordinal: 2
            },
            OriginalOrigin::Operation("n".into())
        ]
    );
    let guard = ControlExpression::record("m".into(), Default::default()).unwrap();
    let gate =
        ControlGate::new(ControlGateKind::X, vec![0], None, guard, Default::default()).unwrap();
    let declaration = OriginalDeclaredControl::new(
        "c".into(),
        vec!["m".into()],
        "v1".into(),
        ControlRule::new(vec![gate], Default::default()).unwrap(),
    )
    .unwrap();
    let input = Input {
        width: 1,
        offset: 0,
        columns: vec![1],
        linear: vec![0],
        upper: vec![bits(1, 0)],
        phase: 0,
        power: -1,
    }
    .ket(&a);
    let declared = OriginalInstrument::new_with_declared_controls(
        vec![0],
        vec![0],
        input,
        vec![
            measure("m", "m", false),
            op("c", G::Control),
            measure("n", "n", false),
        ],
        vec![declaration],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let assignment = OriginalAssignment {
        true_values: vec![("m".into(), true), ("n".into(), false)],
        ..Default::default()
    };
    let literal = declared
        .compile(&assignment, CompilerStrategy::SharedCommutingLayer, &a)
        .unwrap();
    assert!(declared.control_rule("c").is_some());
    assert_eq!(literal.probability(&a).unwrap(), a.omega(0, -2).unwrap());
    let trace = declared
        .compile_trace(CompilerStrategy::SharedCommutingLayer, &a)
        .unwrap();
    let view = trace.with_assignment(&Default::default(), &a).unwrap();
    let events = [("m", true), ("n", false)].map(|(r, value)| OriginalRecordEvent {
        parity: OriginalRecordParity {
            records: vec![r.into()],
            inverted: false,
        },
        value,
    });
    assert_eq!(
        view.probability(&[], &events, &Default::default(), &a)
            .unwrap()
            .distribution
            .total(&a)
            .unwrap(),
        a.omega(0, -2).unwrap()
    );
    let reports = OriginalAssignment {
        report_flips: vec!["m".into(), "m".into()],
        ..assignment.clone()
    };
    let branch = declared
        .compile(&reports, CompilerStrategy::ParityCopies, &a)
        .unwrap();
    assert!(branch.records()[0].true_value && branch.records()[0].reported_value);
}
#[test]
fn operation_sampling_reported_input_and_terminal_mass() {
    let files = Files::new();
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register(
            "decoder",
            "foundation-operation-v1",
            move |parents: &[bool]| {
                counter.fetch_add(1, Ordering::Relaxed);
                assert_eq!(parents, &[true]);
                Ok(vec![true])
            },
        )
        .unwrap();
    let evaluator=files.evaluator("M(1) 0\nDETECTOR rec[-1]\nCONTROL[decoder] det[-1] -> ctrl[0]\nCX ctrl[0] 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT !det[-1]\n",r#"{"outputs":[0],"observables":[[0,1]]}"#,&registry);
    struct NoEntropy;
    impl RandomSource for NoEntropy {
        fn next_u64(&mut self) -> u64 {
            panic!("deterministic original operation fixture consumed entropy")
        }
    }
    let draw = evaluator.sample(&mut NoEntropy).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(draw.history_mass, ExactScalar::one());
    assert_eq!(draw.raw_acceptance, ExactScalar::one());
    assert_eq!(
        draw.raw_moments,
        vec![ScalarArithmetic::default().integer(-1).unwrap()]
    );
    assert_eq!(draw.raw_target_success, None);
}
#[test]
fn true_reported_records_and_coincident_fault_order() {
    use rmagna::instrument::OriginalAssignmentPlan;
    let a = ScalarArithmetic::default();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        rmagna::clifford::CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![measure("m", "a", false), op("c", G::Control)],
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["a".into()],
            version: "1".into(),
            callback: Arc::new(move |p| {
                c.fetch_add(1, Ordering::Relaxed);
                assert!(p.get("a").unwrap());
                Ok(vec![])
            }),
        }],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let plan = OriginalAssignmentPlan::new(&ir, CompilerStrategy::ParityCopies, a);
    let faults = vec![
        OriginalFault {
            after: "c".into(),
            wire: 0,
            pauli: OriginalPauli::X,
        },
        OriginalFault {
            after: "m".into(),
            wire: 0,
            pauli: OriginalPauli::Z,
        },
        OriginalFault {
            after: "c".into(),
            wire: 0,
            pauli: OriginalPauli::Y,
        },
    ];
    let group = plan
        .shared_faults(faults, vec!["a".into(), "a".into()])
        .unwrap();
    assert!(group.report_flips().is_empty());
    let assignment = plan.compact(bits(1, 1), group).unwrap();
    assert_eq!(assignment.word(), &bits(1, 1));
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    let certificate = plan.compile(assignment).unwrap();
    let branch = certificate.branch();
    assert!(branch.records()[0].true_value && branch.records()[0].reported_value);
    assert_eq!(
        branch
            .origins()
            .iter()
            .filter_map(
                |origin| if let OriginalOrigin::Fault { ordinal, .. } = origin {
                    Some(*ordinal)
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        [1, 0, 2]
    );
}
#[test]
fn malformed_history_task_is_rejected_before_execution() {
    use rmagna::records::CompletedRecordSpec;
    assert!(
        CompletedRecordSpec::new(1, &[], &[bits(1, 1)], &bits(0, 0), Default::default()).is_err()
    );
    assert!(BitVector::from_words(1, vec![2]).is_err());
    let files = Files::new();
    for invalid in [
        r#"{"selection":[["m0"]]}"#,
        r#"{"selection":[["m0"]],"outcomes":[2]}"#,
    ] {
        assert!(parse_task(invalid, &files.0, &TaskOptions::default()).is_err());
    }
}
#[test]
fn literal_callback_certificate_owns_signed_wire_result() {
    use rmagna::instrument::{OriginalAssignmentExecutionPolicy, OriginalAssignmentPlan};
    let a = ScalarArithmetic::default();
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = counter.clone();
    let input = Input {
        width: 1,
        offset: 0,
        columns: vec![1],
        linear: vec![0],
        upper: vec![bits(1, 0)],
        phase: 0,
        power: -1,
    }
    .ket(&a);
    let operations = vec![
        op(
            "m",
            G::Measure {
                factors: vec![(-2, OriginalPauli::Z)],
                inverted: false,
                record: "r".into(),
            },
        ),
        op("c", G::Control),
        op(
            "n",
            G::Measure {
                factors: vec![(-2, OriginalPauli::X)],
                inverted: false,
                record: "s".into(),
            },
        ),
    ];
    let ir = OriginalInstrument::new(
        vec![-2],
        vec![-2],
        input,
        operations,
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["r".into()],
            version: "v1".into(),
            callback: Arc::new(move |p| {
                calls.fetch_add(1, Ordering::Relaxed);
                assert!(p.get("r").unwrap());
                Ok(vec![G::H(-2)])
            }),
        }],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let plan = OriginalAssignmentPlan::new(&ir, CompilerStrategy::SharedCommutingLayer, a);
    assert_eq!(
        plan.execution_policy(),
        OriginalAssignmentExecutionPolicy::LiteralPerAssignment
    );
    let certificate = plan
        .compile(OriginalAssignment {
            true_values: vec![("r".into(), true), ("s".into(), false)],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(certificate.probability().is_zero());
    assert_eq!(certificate.branch().outputs(), &[-2]);
    drop(plan);
    drop(ir);
    assert!(certificate.branch().probability(&a).unwrap().is_zero());
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}
#[test]
fn completed_pair_sampling_and_branch_limit() {
    use rmagna::records::{
        CompletedRecordSpec, RecordEnumerationOptions, RecordQuery, SamplingError,
        enumerate_reported_conditioned,
    };
    let a = ScalarArithmetic::default();
    let plus = Input {
        width: 1,
        offset: 0,
        columns: vec![1],
        linear: vec![0],
        upper: vec![bits(1, 0)],
        phase: 0,
        power: -1,
    }
    .ket(&a);
    let spec =
        CompletedRecordSpec::new(1, &[bits(1, 1)], &[], &bits(0, 0), Default::default()).unwrap();
    let law = spec.probability(&[plus], &a).unwrap();
    for word in 0..2 {
        assert_eq!(
            law.value(&bits(1, word), &a).unwrap(),
            a.omega(0, -2).unwrap()
        );
    }
    let ir = compile_affine(
        &parse_circuit("H 0\nM 0\n", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let parities = [report(1)];
    let query = RecordQuery {
        parities: &parities,
        given: &[],
    };
    let enumeration =
        enumerate_reported_conditioned(&ir, &query, &[], &RecordEnumerationOptions::default(), &a)
            .unwrap();
    assert_eq!(
        enumeration
            .branches
            .iter()
            .map(|b| b.values.clone())
            .collect::<Vec<_>>(),
        [vec![false], vec![true]]
    );
    assert!(matches!(
        enumerate_reported_conditioned(
            &ir,
            &query,
            &[],
            &RecordEnumerationOptions {
                max_branches: 1,
                ..Default::default()
            },
            &a
        ),
        Err(SamplingError::EnumerationLimit {
            option: "max_branches",
            limit: 1,
            ..
        })
    ));
    let mut ones = 0;
    for seed in 0..1000 {
        let mut rng = Mt19937::new(seed);
        let draw = law.sample(&mut rng, &Default::default(), &a).unwrap();
        ones += usize::from(draw.values.get(0).unwrap());
    }
    assert!(ones > 400 && ones < 600);
}
#[test]
fn sparse_noise_stream_and_weight_sectors() {
    use rmagna::enumeration::{EnumerationOptions, enumerate_noise};
    let p = model(&[vec![rational(1, 2), rational(1, 4), rational(1, 4)]]);
    let full = NoiseSampler::physical(p.clone(), Default::default());
    let actions = vec![vec![
        SparseNoiseAction::Pauli {
            operation: "a".into(),
            factors: vec![(0, Basis::X)],
        },
        SparseNoiseAction::Pauli {
            operation: "a".into(),
            factors: vec![(0, Basis::Z)],
        },
    ]];
    let sparse =
        SparseNoiseSampler::new(p, &actions, &NoiseSampling::Physical, Default::default()).unwrap();
    for seed in 0..100 {
        let mut a = Mt19937::new(seed);
        let mut b = a.clone();
        assert_eq!(
            full.draw(&mut a).unwrap(),
            sparse.draw(&mut b).unwrap().noise
        );
        assert_eq!((a.draws, a.fingerprint), (b.draws, b.fingerprint));
    }
    let binary = model(&[
        vec![rational(3, 4), rational(1, 4)],
        vec![rational(3, 4), rational(1, 4)],
    ]);
    let enumeration = enumerate_noise(
        &binary,
        &EnumerationOptions {
            max_weight: 1,
            max_patterns: 3,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(enumeration.patterns().len(), 3);
    assert_eq!(
        enumeration.patterns()[0].probability.value(),
        &rational(9, 16)
    );
    assert_eq!(
        enumeration.covered_probability().value() * BigInt::from(16),
        rational(15, 1)
    );
    let error = enumeration
        .patterns()
        .iter()
        .filter(|p| p.weight() != 0)
        .map(|p| p.probability.value().clone())
        .sum::<BigRational>();
    assert_eq!(error * BigInt::from(16), rational(6, 1));
}
#[test]
fn coherent_expansion_and_target_amplitude() {
    use rmagna::records::CompletedRecordSpec;
    let a = ScalarArithmetic::default();
    let plus = Input {
        width: 1,
        offset: 0,
        columns: vec![1],
        linear: vec![0],
        upper: vec![bits(1, 0)],
        phase: 0,
        power: -1,
    }
    .ket(&a);
    let zero = Input {
        width: 1,
        offset: 0,
        columns: vec![],
        linear: vec![],
        upper: vec![],
        phase: 0,
        power: 0,
    }
    .ket(&a);
    let imaginary = AffineKet::new(
        bits(1, 0),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        a.omega(2, 0).unwrap(),
    )
    .unwrap();
    let spec = CompletedRecordSpec::new(1, &[], &[], &bits(0, 0), Default::default()).unwrap();
    let norm = spec
        .probability(&[plus.clone(), imaginary], &a)
        .unwrap()
        .total(&a)
        .unwrap();
    assert_eq!(norm, a.integer(2).unwrap());
    assert_eq!(plus.overlap(&zero, &a).unwrap(), a.omega(0, -1).unwrap());
}
#[test]
fn virtual_noise_actions() {
    use rmagna::noise::CompactNoiseActions;
    let mut actions = CompactNoiseActions::new();
    actions
        .push(SparseNoiseAction::Pauli {
            operation: "manual".into(),
            factors: vec![(2, Basis::Z), (2, Basis::X), (2, Basis::Z)],
        })
        .unwrap();
    actions.add_two("pair".into(), 7, 3).unwrap();
    actions
        .push(SparseNoiseAction::Pauli {
            operation: "empty".into(),
            factors: vec![],
        })
        .unwrap();
    actions.add_one("single".into(), 9).unwrap();
    let saved = actions.clone();
    drop(actions);
    let mut index = 1;
    for left in [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)] {
        for right in [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)] {
            if left.is_none() && right.is_none() {
                continue;
            }
            let factors = left
                .map(|b| (7, b))
                .into_iter()
                .chain(right.map(|b| (3, b)))
                .collect();
            assert_eq!(
                saved.get(index),
                Some(SparseNoiseAction::Pauli {
                    operation: "pair".into(),
                    factors
                })
            );
            index += 1;
        }
    }
    assert_eq!(
        saved.get(0),
        Some(SparseNoiseAction::Pauli {
            operation: "manual".into(),
            factors: vec![(2, Basis::Z), (2, Basis::X), (2, Basis::Z)]
        })
    );
    assert_eq!(
        saved.get(16),
        Some(SparseNoiseAction::Pauli {
            operation: "empty".into(),
            factors: vec![]
        })
    );
    assert_eq!(
        saved.get(19),
        Some(SparseNoiseAction::Pauli {
            operation: "single".into(),
            factors: vec![(9, Basis::Z)]
        })
    );
    assert_eq!(saved.get(20), None);
    assert_eq!(saved.template_sites(), 2);
    assert_eq!(saved.len(), 20);
}
#[test]
fn literal_layout_avoids_inline_record_collisions() {
    use rmagna::instrument::OriginalLiteralProgramLayout;
    let operations = vec![measure("x", "__inline_1", false), measure("x", "", true)];
    let layout =
        OriginalLiteralProgramLayout::new(operations.clone(), &[0, 1], OriginalLimits::default())
            .unwrap();
    assert_eq!(layout.operations()[1].id, "__literal_operation_1");
    let G::Measure { record, .. } = &layout.operations()[1].gate else {
        panic!("literal measurement lost")
    };
    assert_eq!(record, "__inline_1_");
    assert_eq!(
        layout.truth(),
        &[("__inline_1".into(), false), ("__inline_1_".into(), true)]
    );
    assert!(
        OriginalLiteralProgramLayout::new(operations.clone(), &[0], OriginalLimits::default())
            .is_err()
    );
    assert!(
        OriginalLiteralProgramLayout::new(operations, &[0, 2], OriginalLimits::default()).is_err()
    );
}
#[test]
fn signed_affine_grouping() {
    use rmagna::records::CompletedProbability;
    let a = ScalarArithmetic::default();
    let source = Input {
        width: 1,
        offset: 0,
        columns: vec![1],
        linear: vec![0],
        upper: vec![bits(1, 0)],
        phase: 0,
        power: -1,
    }
    .ket(&a);
    let scaled = |c: i64| {
        AffineKet::new(
            source.offset().clone(),
            source.columns().to_vec(),
            source.phase().clone(),
            a.multiply(source.coefficient(), &a.integer(c).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let grouped = CompletedProbability::from_boundary_terms(
        1,
        vec![scaled(-1), scaled(2)],
        Default::default(),
    )
    .unwrap();
    let mixture = grouped.affine_mixture(&a).unwrap().unwrap();
    assert_eq!(mixture.components().len(), 1);
    assert_eq!(mixture.total_mass(), &a.omega(0, 1).unwrap());
    assert_eq!(
        mixture.components()[0].mass_per_label(),
        source.coefficient()
    );
    let terms = [
        (bits(3, 0), vec![bits(3, 1), bits(3, 2)], -1),
        (bits(3, 3), vec![bits(3, 3), bits(3, 1)], 2),
    ]
    .into_iter()
    .map(|(offset, cols, c)| {
        AffineKet::new(
            offset,
            cols,
            QuadraticForm::new(0, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
            a.integer(c).unwrap(),
        )
        .unwrap()
    })
    .collect();
    let same_set = CompletedProbability::from_boundary_terms(3, terms, Default::default()).unwrap();
    let mixture = same_set.affine_mixture(&a).unwrap().unwrap();
    assert_eq!(mixture.components().len(), 1);
    assert_eq!(mixture.total_mass(), &a.integer(4).unwrap());
    for word in 0..8 {
        assert_eq!(
            same_set.value(&bits(3, word), &a).unwrap(),
            a.integer(i64::from(word < 4)).unwrap()
        );
    }
    let negative =
        CompletedProbability::from_boundary_terms(1, vec![scaled(-1)], Default::default()).unwrap();
    assert!(negative.affine_mixture(&a).unwrap().is_none());
}
#[test]
fn dimension_changing_clifford_transport() {
    let a = ScalarArithmetic::default();
    let scalar_reference = ParityPhase::new(0, 3, []).unwrap();
    let scalar = PreparedPhaseResource::new_native(
        &scalar_reference,
        1,
        NativePhaseOptions::default(),
        None,
        &a,
    )
    .unwrap();
    assert_eq!(scalar.quotient_rank(), 0);
    for dimension in [2, 3] {
        let mut linear = vec![1, 3];
        let mut upper = vec![bits(dimension, 2), bits(dimension, 0)];
        if dimension == 3 {
            linear.push(2);
            upper.push(bits(3, 0));
            upper[0].set(2, true).unwrap();
        }
        let mut expanded = phase_input(
            dimension,
            &[3, 1 << (dimension - 1)],
            1,
            linear,
            upper,
            7,
            -4,
            vec![],
        );
        for even in [false, true] {
            if even {
                expanded.terms = vec![(3, 2), (1 << (dimension - 1), 6)];
            }
            let before = (scalar.num_terms(), scalar.quotient_rank());
            for constant in [0, 1, 3, 7] {
                let phase =
                    ParityPhase::new(dimension, constant, expanded.phase().terms().to_vec())
                        .unwrap();
                let states = scalar
                    .expand_clifford(&phase, &expanded.input.ket(&a), &a)
                    .unwrap();
                let mut actual = vec![ExactScalar::zero(); 4];
                for term in &states {
                    for (sum, v) in actual.iter_mut().zip(amplitudes(Some(term), 2, &a)) {
                        *sum = a.add(sum, &v).unwrap();
                    }
                }
                let global = a.omega(i64::from(constant), 0).unwrap();
                let expected = expanded
                    .dense(&a)
                    .iter()
                    .map(|value| a.multiply(value, &global).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
            }
            assert_eq!((scalar.num_terms(), scalar.quotient_rank()), before);
            let (fresh, prepared) = freeze_phase(&expanded, &a);
            check_transported_phase(&expanded, &fresh, &prepared, &a);
        }
        let reference_value = scalar
            .evaluate(
                &scalar_reference,
                &QuadraticForm::new(0, vec![], vec![]).unwrap(),
                &ExactScalar::one(),
                &a,
            )
            .unwrap();
        assert_eq!(
            a.multiply(&a.omega(7, 0).unwrap(), &reference_value.conjugate())
                .unwrap(),
            a.omega(4, 0).unwrap()
        );
        expanded.terms.push((1, 1));
        assert!(
            scalar
                .expand_clifford(&expanded.phase(), &expanded.input.ket(&a), &a)
                .is_err()
        );
    }
    let reference = phase_input(
        2,
        &[1, 2],
        0,
        vec![0; 2],
        vec![bits(2, 0); 2],
        0,
        -2,
        vec![(1, 1), (2, 1)],
    );
    let (nonclifford, _) = freeze_phase(&reference, &a);
    let changed = phase_input(
        3,
        &[1, 2],
        0,
        vec![0; 3],
        vec![bits(3, 0); 3],
        0,
        -3,
        vec![(1, 1), (2, 1)],
    );
    assert!(
        nonclifford
            .expand(&changed.phase(), &changed.input.ket(&a), &a)
            .is_err()
    );
    assert!(
        nonclifford
            .expand_clifford(&changed.phase(), &changed.input.ket(&a), &a)
            .is_err()
    );
}
use rmagna::unitary::{UnitaryCircuit, UnitaryGate as U};
fn literal_clifford(gate: U) -> Option<C> {
    match gate {
        U::Clifford(c) => Some(c),
        U::T { qubit, turns } if turns & 1 == 0 => Some(C::S {
            qubit,
            turns: turns / 2,
        }),
        U::Cs { a, b, turns: 2 } => Some(C::Cz { a, b }),
        _ => None,
    }
}
fn raw_dense_unitary(
    width: usize,
    initial: usize,
    gates: &[U],
    a: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut state = vec![ExactScalar::zero(); 1 << width];
    state[initial] = ExactScalar::one();
    for &gate in gates {
        if let Some(c) = literal_clifford(gate) {
            common::dense_gate(c, &mut state, a);
            continue;
        }
        for (word, v) in state.iter_mut().enumerate() {
            let exponent = match gate {
                U::T { qubit, turns } => i64::from(turns) * ((word >> qubit) & 1) as i64,
                U::Cs { a, b, turns } => {
                    2 * i64::from(turns) * ((word >> a) & (word >> b) & 1) as i64
                }
                U::Ccz { a, b, c } => 4 * ((word >> a) & (word >> b) & (word >> c) & 1) as i64,
                _ => unreachable!(),
            };
            *v = a.multiply(v, &a.omega(exponent, 0).unwrap()).unwrap();
        }
    }
    state
}
fn literal_transport(mut x: usize, mut z: usize, gates: &[U]) -> (usize, usize) {
    for &gate in gates {
        let Some(c) = literal_clifford(gate) else {
            continue;
        };
        match c {
            C::H(q) => {
                let different = ((x >> q) ^ (z >> q)) & 1;
                x ^= different << q;
                z ^= different << q;
            }
            C::S { qubit, turns } => {
                if turns & 1 != 0 {
                    z ^= ((x >> qubit) & 1) << qubit;
                }
            }
            C::Cx { control, target } => {
                x ^= ((x >> control) & 1) << target;
                z ^= ((z >> target) & 1) << control;
            }
            C::Cz { a, b } => {
                z ^= ((x >> a) & 1) << b;
                z ^= ((x >> b) & 1) << a;
            }
            C::Swap { a, b } => {
                let d = ((x >> a) ^ (x >> b)) & 1;
                x ^= (d << a) | (d << b);
                let d = ((z >> a) ^ (z >> b)) & 1;
                z ^= (d << a) | (d << b);
            }
            C::X(_) | C::Z(_) => {}
        }
    }
    (x, z)
}
fn full_axis_sector(
    circuit: &UnitaryCircuit,
    rows: &[BitVector],
) -> (bool, Vec<BitVector>, usize, usize) {
    let gates = circuit.gates();
    let mut axes = vec![];
    for (position, &gate) in gates.iter().enumerate() {
        if literal_clifford(gate).is_some() {
            continue;
        }
        let masks = match gate {
            U::T { qubit, turns } if turns & 1 != 0 => vec![1 << qubit],
            U::Cs { a, b, turns } if turns & 1 != 0 => vec![1 << a, 1 << b, (1 << a) | (1 << b)],
            U::Ccz { a, b, c } => (1..8)
                .map(|m| ((m & 1) << a) | (((m >> 1) & 1) << b) | (((m >> 2) & 1) << c))
                .collect(),
            _ => vec![],
        };
        for mask in masks {
            axes.push(literal_transport(0, mask, &gates[position + 1..]));
        }
    }
    let mut span = BTreeSet::from([0usize]);
    for &(x, z) in &axes {
        let p = x | (z << circuit.width());
        let added = span.iter().map(|v| v ^ p).collect::<Vec<_>>();
        span.extend(added);
    }
    let rank = span.len().trailing_zeros() as usize;
    for &(x, z) in &axes {
        for &(u, v) in &axes {
            if (((x & v).count_ones() ^ (z & u).count_ones()) & 1) != 0 {
                return (false, vec![], axes.len(), rank);
            }
        }
    }
    let mut basis = vec![];
    let mut covered = BTreeSet::from([0usize]);
    for mask in 1..1usize << rows.len() {
        let physical = rows.iter().enumerate().fold(0, |v, (j, row)| {
            if mask >> j & 1 != 0 {
                v ^ index(row)
            } else {
                v
            }
        });
        if axes
            .iter()
            .any(|(x, _)| (physical & x).count_ones() & 1 != 0)
            || covered.contains(&mask)
        {
            continue;
        }
        basis.push(bits(rows.len(), mask));
        let next = covered.iter().map(|v| v ^ mask).collect::<Vec<_>>();
        covered.extend(next);
    }
    (true, basis, axes.len(), rank)
}
fn prefix_gates(noncommuting: bool, changed: bool) -> Vec<U> {
    let mut gates = vec![
        U::Clifford(if changed { C::X(0) } else { C::H(0) }),
        U::Ccz { a: 0, b: 1, c: 2 },
        U::Ccz { a: 0, b: 1, c: 2 },
        U::Clifford(C::Cx {
            control: 0,
            target: 2,
        }),
        U::T { qubit: 2, turns: 3 },
        U::Clifford(C::Cx {
            control: 0,
            target: 2,
        }),
    ];
    if noncommuting {
        gates.extend([U::Clifford(C::H(0)), U::T { qubit: 0, turns: 1 }]);
    }
    gates
}
#[test]
fn check_axis_prefix() {
    let a = ScalarArithmetic::default();
    let original_rows = [1, 2, 4, 3, 0].map(|m| bits(3, m));
    let physical_rows = [bits(5, 4), bits(5, 1), bits(5, 2), bits(5, 5), bits(5, 0)];
    for noncommuting in [false, true] {
        let circuit = UnitaryCircuit::new(3, prefix_gates(noncommuting, false)).unwrap();
        let expected = full_axis_sector(&circuit, &original_rows);
        let actual = circuit
            .commuting_axis_sector(&original_rows, Default::default())
            .unwrap();
        assert_eq!(
            (
                actual.common,
                actual.rows,
                actual.rotation_slots,
                actual.independent_axes
            ),
            expected
        );
        assert_eq!(actual.common, !noncommuting);
        assert!(actual.independent_axes < actual.rotation_slots);
        for initial in [0, 7] {
            let base = circuit
                .compile_with_strategy(
                    &bits(3, initial),
                    &a,
                    CompilerStrategy::SharedCommutingLayer,
                )
                .unwrap();
            assert!(
                base.compilation().shared_observations_before_reduction
                    < base.compilation().reference_layer_slots
            );
            for variation in 0..3 {
                // Typed slots 0,1,2 replace source wire names 10,11,12; slot3
                // is wire70. Swap with a fresh zero slot4 defers RESET_PLUS.
                let mut suffix = vec![
                    U::Clifford(C::H(3)),
                    U::Clifford(C::Cx {
                        control: 3,
                        target: 1,
                    }),
                    U::Clifford(C::Swap { a: 0, b: 4 }),
                    U::Clifford(C::H(0)),
                    U::Clifford(C::H(1)),
                ];
                if variation == 1 {
                    suffix.push(U::Ccz { a: 3, b: 1, c: 2 });
                }
                let suffix = UnitaryCircuit::new(5, suffix).unwrap();
                let changed =
                    UnitaryCircuit::new(3, prefix_gates(noncommuting, variation == 2)).unwrap();
                let owner = (variation == 2).then(|| {
                    changed
                        .compile_with_strategy(
                            &bits(3, initial),
                            &a,
                            CompilerStrategy::SharedCommutingLayer,
                        )
                        .unwrap()
                });
                let source = owner.as_ref().unwrap_or(&base);
                let plan = source
                    .prepare_continuation(&suffix, Default::default(), &a)
                    .unwrap();
                if variation == 2 {
                    let old = base
                        .prepare_continuation(&suffix, Default::default(), &a)
                        .unwrap();
                    let before = old.statistics();
                    assert!(old.apply(source, &a).is_err());
                    assert_eq!(old.statistics(), before);
                }
                let mut full = changed.gates().to_vec();
                full.extend_from_slice(suffix.gates());
                let full_circuit = UnitaryCircuit::new(5, full.clone()).unwrap();
                let independent = full_axis_sector(&full_circuit, &physical_rows);
                let sector = full_circuit
                    .commuting_axis_sector(&physical_rows, Default::default())
                    .unwrap();
                assert_eq!(
                    (
                        sector.common,
                        sector.rows,
                        sector.rotation_slots,
                        sector.independent_axes
                    ),
                    independent
                );
                let fresh = full_circuit
                    .compile_with_strategy(
                        &bits(5, initial),
                        &a,
                        CompilerStrategy::SharedCommutingLayer,
                    )
                    .unwrap();
                let cold = fresh
                    .prepare_probability_with_fibers(&physical_rows, 256, Default::default(), &a)
                    .unwrap();
                let warm = plan
                    .prepare_probability_with_fibers(&physical_rows, 256, Default::default(), &a)
                    .unwrap();
                let view = plan.apply(source, &a).unwrap();
                let dense = raw_dense_unitary(5, initial, &full, &a);
                let mut expected = vec![ExactScalar::zero(); 32];
                for (word, amp) in dense.iter().enumerate() {
                    let label = physical_rows.iter().enumerate().fold(0, |v, (j, row)| {
                        v | (((word & index(row)).count_ones() as usize & 1) << j)
                    });
                    expected[label] = a
                        .add(
                            &expected[label],
                            &a.multiply(amp, &amp.conjugate()).unwrap(),
                        )
                        .unwrap();
                }
                let before = prepared_counts(plan.query_work_statistics());
                for code in 0..32 {
                    let rhs = bits(5, code);
                    assert_eq!(
                        view.evaluate_probability(&warm, &rhs, 256, &a)
                            .unwrap()
                            .value,
                        expected[code]
                    );
                    assert_eq!(
                        cold.evaluate(&fresh, &rhs, 256, &a).unwrap().value,
                        expected[code]
                    );
                }
                assert_eq!(prepared_counts(plan.query_work_statistics()), before);
                let stats = plan.statistics();
                assert_eq!(
                    (
                        stats.suffix_compilations,
                        stats.transport_tables_prepared,
                        stats.frame_applications
                    ),
                    (1, 1, 1)
                );
                assert_eq!(stats.query_original_positions, 0);
            }
        }
    }
}
use rmagna::pauli::PauliWord;
fn ch_gates(control: usize, target: usize) -> Vec<U> {
    vec![
        U::Clifford(C::S {
            qubit: target,
            turns: 3,
        }),
        U::Clifford(C::H(target)),
        U::T {
            qubit: target,
            turns: 7,
        },
        U::Clifford(C::H(target)),
        U::Clifford(C::Cz {
            a: control,
            b: target,
        }),
        U::Clifford(C::H(target)),
        U::T {
            qubit: target,
            turns: 1,
        },
        U::Clifford(C::H(target)),
        U::Clifford(C::S {
            qubit: target,
            turns: 1,
        }),
    ]
}
#[test]
fn trace_purpose_preserves_dilation_axes_and_every_fault_location() {
    let a = ScalarArithmetic::default();
    let mut gates = vec![
        U::Clifford(C::H(0)),
        U::Clifford(C::Cx {
            control: 0,
            target: 2,
        }),
    ];
    gates.extend(ch_gates(2, 1));
    assert_eq!(gates.len(), 11);
    let circuit = UnitaryCircuit::new(3, gates.clone()).unwrap();
    let amplitude = circuit.compile(&bits(3, 0), &a).unwrap();
    let trace = circuit.compile(&bits(3, 0), &a).unwrap();
    assert_eq!(amplitude.compilation(), trace.compilation());
    assert_eq!(trace.compilation().phase_ancillas, 2);
    assert_eq!(prepared_counts(trace.query_work_statistics()), [0; 10]);
    let word = PauliWord::new(bits(3, 0), bits(3, 2), 0).unwrap();
    let mut native = NativePhaseOptions::default();
    native.transport.mode = rmagna::phase::NativeTransportMode::Prepared;
    let probe = trace.amplitude_ownership_probe();
    let runtime = trace
        .into_trace_runtime(
            &[rmagna::unitary::TraceQuery {
                word,
                event_rows: vec![],
            }],
            rmagna::unitary::TraceOptions {
                max_terms: 256,
                phase: native,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert!(probe.is_released());
    assert!(runtime.amplitude_ownership_probe().is_released());
    let prepared = prepared_counts(runtime.query_work_statistics());
    let mut cases = 0;
    for position in 1..=gates.len() {
        for wire in 0..2 {
            for pauli in 1..=3 {
                let x = if pauli == 2 { 0 } else { 1 << wire };
                let z = if pauli == 1 { 0 } else { 1 << wire };
                let phase = if pauli == 3 { 2 } else { 0 };
                let insertion = rmagna::unitary::PauliInsertion {
                    before_gate: position,
                    word: PauliWord::new(bits(3, x), bits(3, z), phase).unwrap(),
                };
                let direct = amplitude.with_insertions(&[insertion.clone()], &a).unwrap();
                let view = runtime.assignment(&[insertion]).unwrap();
                let p = direct.assignment_frame().unwrap();
                let q = view.transported_frame();
                assert_eq!((p.x(), p.z(), p.phase()), (q.x(), q.z(), q.phase()));
                let mut literal = gates[..position].to_vec();
                literal.extend(match pauli {
                    1 => vec![U::Clifford(C::X(wire))],
                    2 => vec![U::Clifford(C::Z(wire))],
                    _ => vec![
                        U::Clifford(C::S {
                            qubit: wire,
                            turns: 3,
                        }),
                        U::Clifford(C::X(wire)),
                        U::Clifford(C::S {
                            qubit: wire,
                            turns: 1,
                        }),
                    ],
                });
                literal.extend_from_slice(&gates[position..]);
                let expected = raw_dense_unitary(3, 0, &literal, &a);
                let expansion = direct.stabilizer_expansion(Default::default(), &a).unwrap();
                let mut actual = vec![ExactScalar::zero(); 8];
                for term in expansion.entries() {
                    for (sum, v) in actual.iter_mut().zip(amplitudes(Some(term), 3, &a)) {
                        *sum = a.add(sum, &v).unwrap();
                    }
                }
                assert_eq!(actual, expected);
                let moment =
                    expected
                        .iter()
                        .enumerate()
                        .fold(ExactScalar::zero(), |sum, (word, v)| {
                            let mass = a.multiply(v, &v.conjugate()).unwrap();
                            if word & 2 == 0 {
                                a.add(&sum, &mass).unwrap()
                            } else {
                                a.subtract(&sum, &mass).unwrap()
                            }
                        });
                assert_eq!(
                    runtime.evaluate(0, &view, &bits(0, 0), &a).unwrap().value,
                    moment
                );
                assert_eq!(prepared_counts(runtime.query_work_statistics()), prepared);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 66);
    assert_eq!(runtime.query_work_statistics().fixed_evaluations, 66);
    assert!(probe.is_released());
    let retained = runtime.storage_statistics();
    assert_eq!(retained.phase_variables, 2);
    assert_eq!(retained.event_phase_widths, [4]);
    assert_eq!(retained.original_positions, 11);
}
#[test]
fn coherent_file_pipeline_and_prepared_trace_runtime() {
    use rmagna::instrument::{InstrumentQueryLayout as Q, QueryPreparationOutcome as O};
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nM 0\nCH rec[-1] 1\n", &ParseOptions::default()).unwrap();
    assert!(!source.needs_history_sampling());
    let mut native = NativePhaseOptions::default();
    native.transport.mode = rmagna::phase::NativeTransportMode::Prepared;
    let ir = compile_affine(
        &source,
        &CompileOptions {
            probability_preparation: QueryPreparation::Native(native),
            moment_preparation: QueryPreparation::Native(native),
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    assert_eq!(ir.num_records(), 1);
    assert_eq!(ir.noise_sites().len(), 1);
    assert_eq!(ir.noise_sites()[0].source_operations(), &[1]);
    assert_eq!(ir.compilation().phase_ancillas, 2);
    let z = PauliWord::new(bits(2, 0), bits(2, 3), 0).unwrap();
    let query = Q::PauliMoment {
        outputs: &[0, 1],
        word: &z,
    };
    assert_eq!(ir.prepare_query(query, 256, &a).unwrap(), O::Created);
    assert_eq!(ir.prepare_query(query, 256, &a).unwrap(), O::Reused);
    let statistics = ir.prepared_moment_statistics().unwrap();
    assert_eq!(statistics.cached_queries, 1);
    assert_eq!(statistics.native_transports.prepared, 1);
    let before = prepared_counts(ir.query_work_statistics());
    let half = a.omega(0, -2).unwrap();
    for report_flip in [false, true] {
        let faults = if report_flip {
            vec![FaultSelection {
                site: 0,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        assert_eq!(
            ir.pauli_moment(&[0, 1], &z, &faults, 256, &a)
                .unwrap()
                .value,
            if report_flip {
                half.negated()
            } else {
                half.clone()
            }
        );
        assert_eq!(ir.prepared_moment_statistics().unwrap(), statistics);
        assert_eq!(prepared_counts(ir.query_work_statistics()), before);
    }
    let z1 = PauliWord::new(bits(2, 0), bits(2, 2), 0).unwrap();
    assert_eq!(
        ir.pauli_moment(&[0, 1], &z1, &[], 256, &a).unwrap().value,
        half
    );
    struct Zeros;
    impl RandomSource for Zeros {
        fn next_u64(&mut self) -> u64 {
            0
        }
    }
    let sampled = rmagna::records::sample_reported(
        &ir,
        &[report(1)],
        &[],
        &mut Zeros,
        &Default::default(),
        &a,
    )
    .unwrap();
    assert_eq!(sampled.values, [false]);
    assert_eq!(sampled.mass, half);
    assert_eq!(sampled.condition_mass, ExactScalar::one());
    // Seal the actual parsed/lowered instrument, including its already warm
    // ordinary caches. Every original amplitude owner must then expire.
    use rmagna::instrument::InstrumentTraceQuery as TQ;
    let probe = ir.amplitude_ownership_probe();
    assert!(!probe.is_released());
    let trace = ir
        .into_trace_runtime(
            &[
                TQ {
                    outputs: vec![0, 1],
                    word: z,
                    parities: vec![],
                    accepted: true,
                },
                TQ {
                    outputs: vec![0, 1],
                    word: z1,
                    parities: vec![],
                    accepted: true,
                },
                TQ {
                    outputs: vec![],
                    word: PauliWord::identity(0).unwrap(),
                    parities: vec![report(1)],
                    accepted: false,
                },
                TQ {
                    outputs: vec![],
                    word: PauliWord::identity(0).unwrap(),
                    parities: vec![],
                    accepted: false,
                },
            ],
            rmagna::unitary::TraceOptions {
                max_terms: 256,
                phase: native,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert!(probe.is_released());
    let before = prepared_counts(trace.runtime().query_work_statistics());
    let retained = trace.runtime().storage_statistics();
    for flipped in [false, true] {
        let faults = if flipped {
            vec![FaultSelection {
                site: 0,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        let assignment = trace.assignment(&faults).unwrap();
        assert_eq!(
            trace
                .evaluate(0, &assignment, &bits(0, 0), &a)
                .unwrap()
                .value,
            if flipped {
                half.negated()
            } else {
                half.clone()
            }
        );
        assert_eq!(
            trace
                .evaluate(1, &assignment, &bits(0, 0), &a)
                .unwrap()
                .value,
            half
        );
        for bit in 0..2 {
            assert_eq!(
                trace
                    .evaluate(2, &assignment, &bits(1, bit), &a)
                    .unwrap()
                    .value,
                half
            );
        }
        let mut entropy = 0;
        let draw = trace
            .runtime()
            .sample_event_chain(
                &[3, 2],
                &assignment,
                &bits(0, 0),
                &mut || {
                    entropy += 1;
                    false
                },
                64,
                &a,
            )
            .unwrap();
        assert_eq!(draw.values, bits(1, 0));
        assert_eq!(draw.mass, half);
        assert_eq!(draw.condition_mass, ExactScalar::one());
        assert_eq!((draw.entropy_bits, entropy), (1, 1));
        assert_eq!(
            prepared_counts(trace.runtime().query_work_statistics()),
            before
        );
        assert_eq!(trace.runtime().storage_statistics(), retained);
        assert!(probe.is_released());
    }
}
#[test]
fn fixed_virtual_template_and_selection_offset() {
    let files = Files::new();
    let decision = Arc::new(AtomicUsize::new(0));
    let value = decision.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register(
            "decoder",
            "foundation-template-v1",
            move |input: &[bool]| {
                assert_eq!(input, &[false]);
                count.fetch_add(1, Ordering::Relaxed);
                Ok(vec![value.load(Ordering::Relaxed) != 0])
            },
        )
        .unwrap();
    let source = "M 0\nDETECTOR rec[-1]\nCONTROL[decoder] det[-1] -> ctrl[0]\nCH ctrl[0] 1\nCONTROL[NOT] ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]\n";
    let evaluator = files.evaluator(
        source,
        r#"{"outputs":[1],"observables":[[0,1]]}"#,
        &registry,
    );
    let prepared = evaluator.prepare_sampling_queries(
        Default::default(),
        &rmagna::costs::ExecutionMeter::default(),
    );
    assert!(prepared.all_families_retained());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let storage = evaluator.reference_storage_statistics().unwrap();
    // Two named wires, one record slot, and two versioned control slots.
    assert_eq!(storage.source.unwrap().physical_width, 5);
    assert_eq!(storage.source.unwrap().raw_phase_variables, 2);
    struct NoEntropy;
    impl RandomSource for NoEntropy {
        fn next_u64(&mut self) -> u64 {
            panic!("deterministic template requested entropy")
        }
    }
    for bit in 0..2 {
        decision.store(bit, Ordering::Relaxed);
        let draw = evaluator.sample(&mut NoEntropy).unwrap();
        assert_eq!(
            draw.raw_acceptance,
            ScalarArithmetic::default().integer(bit as i64).unwrap()
        );
        assert_eq!(draw.raw_moments, vec![ExactScalar::zero()]);
        assert_eq!(draw.controller_calls[0].outputs, vec![bit != 0]);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(evaluator.reference_storage_statistics().unwrap(), storage);
}
const FRAME_PROTOCOL: &str = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[bits] det[-1] -> ctrl[0] ctrl[1]\nCONTROL[XOR] ctrl[0] ctrl[1] -> ctrl[2]\nCX ctrl[0] 1\nCS ctrl[2] 1\nCY ctrl[1] 2\nDETECTOR rec[-1]\nCONTROL[bits] det[-1] -> ctrl[0] ctrl[1]\nCZ ctrl[0] 1\nCONTROL[NOT] ctrl[1] -> ctrl[3]\nPOSTSELECT ctrl[3]\n";
const FRAME_RECOVERY: &str = "H 2\nM 2\nDETECTOR rec[-1]\nCONTROL[bits] det[-1] -> ctrl[0] ctrl[1]\nCY ctrl[0] 0\nCH ctrl[1] 1\n";
fn append_parity(gates: &mut Vec<U>, wires: &[usize], turns: u8) {
    let (&pivot, rest) = wires.split_last().unwrap();
    for &q in rest {
        gates.push(U::Clifford(C::Cx {
            control: q,
            target: pivot,
        }));
    }
    gates.push(U::T {
        qubit: pivot,
        turns,
    });
    for &q in rest.iter().rev() {
        gates.push(U::Clifford(C::Cx {
            control: q,
            target: pivot,
        }));
    }
}
fn cy_gates(control: usize, target: usize) -> [U; 3] {
    [
        U::Clifford(C::S {
            qubit: target,
            turns: 3,
        }),
        U::Clifford(C::Cx { control, target }),
        U::Clifford(C::S {
            qubit: target,
            turns: 1,
        }),
    ]
}
#[test]
fn check_composed_feedback_frames() {
    let a = ScalarArithmetic::default();
    let parse = ParseOptions {
        controllers: ["bits".into()].into_iter().collect(),
        ..Default::default()
    };
    let protocol = parse_circuit(FRAME_PROTOCOL, &parse).unwrap();
    let recovery = parse_circuit(FRAME_RECOVERY, &parse).unwrap();
    let decisions = |c: &rmagna::circuit::Circuit| {
        c.operations()
            .iter()
            .filter_map(|op| match &op.operation {
                Operation::Control { name, outputs, .. } if name == "bits" => Some(outputs.len()),
                _ => None,
            })
            .sum::<usize>()
    };
    assert_eq!((decisions(&protocol), decisions(&recovery)), (4, 2));
    // Recovery's local outputs 0,1 map to protocol wires1,2. Its wire2 is
    // a fresh ancilla3. Record slots4,5 and six independent versioned decision
    // slots6..11 give one typed dilation, without depending on Native wire IDs.
    let mut gates = vec![
        U::Clifford(C::H(0)),
        U::Clifford(C::Cx {
            control: 0,
            target: 4,
        }),
        U::Clifford(C::Cx {
            control: 6,
            target: 1,
        }),
    ];
    append_parity(&mut gates, &[6, 7], 1);
    append_parity(&mut gates, &[1], 1);
    append_parity(&mut gates, &[6, 7, 1], 7);
    gates.extend(cy_gates(7, 2));
    gates.push(U::Clifford(C::Cz { a: 8, b: 1 }));
    gates.extend([
        U::Clifford(C::H(3)),
        U::Clifford(C::Cx {
            control: 3,
            target: 5,
        }),
    ]);
    gates.extend(cy_gates(10, 1));
    gates.extend(ch_gates(11, 2));
    let source = UnitaryCircuit::new(12, gates)
        .unwrap()
        .compile(&bits(12, 0), &a)
        .unwrap();
    let mut catalog = (6..12)
        .map(|q| rmagna::unitary::PauliInsertion {
            before_gate: 0,
            word: PauliWord::new(BitVector::from_indices(12, [q]).unwrap(), bits(12, 0), 0)
                .unwrap(),
        })
        .collect::<Vec<_>>();
    catalog.push(rmagna::unitary::PauliInsertion {
        before_gate: 0,
        word: PauliWord::new(bits(12, 1 << 9), bits(12, 0), 0).unwrap(),
    });
    let prepared = source
        .prepare_insertions(&catalog, Default::default())
        .unwrap();
    let mut checks = 0;
    for choice in 0..729 {
        let mut digits = choice;
        let mut selected = vec![6];
        let mut incremental = prepared.transported_frame(&selected).unwrap();
        for j in 0..6 {
            let state = digits % 3;
            digits /= 3;
            if state == 2 {
                selected.push(j);
                incremental = prepared
                    .transported_frame(&[j])
                    .unwrap()
                    .multiply(&incremental)
                    .unwrap();
            }
            let insertions = selected
                .iter()
                .map(|&j| catalog[j].clone())
                .collect::<Vec<_>>();
            let direct = source.with_insertions(&insertions, &a).unwrap();
            let frame = direct.assignment_frame().unwrap();
            assert_eq!((incremental.x(), incremental.z()), (frame.x(), frame.z()));
            let complete = prepared.transported_frame(&selected).unwrap();
            assert_eq!(
                (complete.x(), complete.z(), complete.phase()),
                (frame.x(), frame.z(), frame.phase())
            );
            checks += 1;
        }
    }
    assert_eq!(checks, 4374);
    // Distinct host names adapt Native's stage/index-aware hook to Rust's
    // bit-only registry. Gate order, shadowed registers and each six-bit
    // decision assignment remain the complete source fixture.
    let files = Files::new();
    let mut physical = FRAME_PROTOCOL.replacen("CONTROL[bits]", "CONTROL[p0]", 1);
    physical = physical.replacen("CONTROL[bits]", "CONTROL[p1]", 1);
    files.put(
        "recovery.magna",
        &FRAME_RECOVERY.replace("CONTROL[bits]", "CONTROL[r0]"),
    );
    let choice = Arc::new(AtomicUsize::new(0));
    let count = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    for (name, shift) in [("p0", 0), ("p1", 2), ("r0", 4)] {
        let bits = choice.clone();
        let calls = count.clone();
        registry
            .register(name, "foundation-composed-v1", move |input: &[bool]| {
                assert_eq!(input.len(), 1);
                calls.fetch_add(1, Ordering::Relaxed);
                let word = bits.load(Ordering::Relaxed);
                Ok(vec![
                    (word >> shift) & 1 != 0,
                    (word >> (shift + 1)) & 1 != 0,
                ])
            })
            .unwrap();
    }
    // The Rust task facade requires a target for recovery. Its independent
    // zero target is auxiliary here; only source acceptance and moments are compared.
    files.put("ideal2.magna", "I 0 1\n");
    let evaluator=files.evaluator(&physical,r#"{"outputs":[1,2],"ideal_state":"ideal2.magna","recovery":"recovery.magna","observables":[[0,1],[0,2],[0,3]]}"#,&registry);
    for word in 0..64 {
        choice.store(word, Ordering::Relaxed);
        let before = count.load(Ordering::Relaxed);
        let result = evaluator.sample(&mut SplitMix64::new(0)).unwrap();
        assert_eq!(count.load(Ordering::Relaxed) - before, 3);
        let accepted = word & 8 != 0;
        assert_eq!(result.accepted, field(i64::from(accepted), 1));
        let first = (word & 1 != 0) ^ (word & 16 != 0);
        let second = word & 2 != 0;
        let z1 = if first { -1 } else { 1 };
        let z2 = if word & 32 != 0 {
            0
        } else if second {
            -1
        } else {
            1
        };
        assert_eq!(
            result.moments,
            if accepted {
                vec![field(z1, 1), field(z2, 1), field(z1 * z2, 1)]
            } else {
                vec![field(0, 1); 3]
            }
        );
    }
}
#[test]
fn checked_primitives_preserve_padding_shapes_and_refusal() {
    use rmagna::{circuit::ParseResource, gf2::Gf2Error, records::CompletedProbability};
    let a = ScalarArithmetic::default();
    let value = BitVector::from_indices(65, [0, 63, 64]).unwrap();
    assert_eq!(
        value.shifted_right(1).unwrap().words(),
        &[(1u64 << 62) | (1u64 << 63)]
    );
    assert_eq!(value.shifted_right(usize::MAX).unwrap(), bits(0, 0));
    assert_eq!(value.shifted_left(0).unwrap(), value);
    assert_eq!(
        value.shifted_left(usize::MAX),
        Err(Gf2Error::AllocationFailed)
    );
    let mut destination = value.clone();
    assert!(
        destination
            .xor_above_assign(&bits(64, 0), usize::MAX)
            .is_err()
    );
    assert_eq!(destination, value);
    destination.xor_above_assign(&value, usize::MAX).unwrap();
    assert_eq!(destination, value);
    assert_eq!(AffineSupport::hull(&[]).unwrap(), None);
    assert!(
        AffineSupport::hull(&[
            AffineSupport::new(bits(1, 0), vec![]).unwrap(),
            AffineSupport::new(bits(2, 0), vec![]).unwrap()
        ])
        .is_err()
    );
    let wide = ParseOptions {
        allow_wide_repeat_counts: true,
        max_instructions: 1,
        ..Default::default()
    };
    let count = "99999999999999999999999999";
    let error = parse_circuit(&format!("REPEAT {count} {{\nH 0\n}}"), &wide).unwrap_err();
    let stop = error.resource_limit.unwrap();
    assert_eq!(stop.option, ParseResource::Instructions);
    assert_eq!(stop.needed, count.parse::<BigUint>().unwrap());
    assert!(
        parse_circuit(
            &format!("REPEAT 0 {{\nREPEAT {count} {{\nH 0\n}}\n}}"),
            &wide
        )
        .unwrap()
        .operations()
        .is_empty()
    );
    assert!(parse_circuit("REPEAT -1 {\n}\n", &wide).is_err());
    let q = QuadraticForm::new(0, vec![], vec![]).unwrap();
    let zero = AffineKet::new(bits(0, 0), vec![], q, ExactScalar::zero()).unwrap();
    let scalar = PreparedPhaseResource::new_native(
        &ParityPhase::new(0, 0, []).unwrap(),
        1,
        NativePhaseOptions::default(),
        None,
        &a,
    )
    .unwrap();
    assert!(
        scalar
            .expand_clifford(&ParityPhase::new(1, 0, []).unwrap(), &zero, &a)
            .is_err()
    );
    let term = |coefficient: ExactScalar| {
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
            coefficient,
        )
        .unwrap()
    };
    let cancelled = CompletedProbability::from_boundary_terms(
        1,
        vec![term(a.integer(-1).unwrap()), term(ExactScalar::one())],
        Default::default(),
    )
    .unwrap()
    .affine_mixture(&a)
    .unwrap()
    .unwrap();
    assert!(cancelled.components().is_empty());
    assert!(cancelled.total_mass().is_zero());
    let complex = CompletedProbability::from_boundary_terms(
        1,
        vec![
            term(a.omega(2, 0).unwrap()),
            term(
                a.scalar([1.into(), 0.into(), (-1).into(), 0.into()], 0)
                    .unwrap(),
            ),
        ],
        Default::default(),
    )
    .unwrap();
    let mixture = complex.affine_mixture(&a).unwrap().unwrap();
    assert_eq!(mixture.components().len(), 1);
    assert_eq!(mixture.total_mass(), &a.integer(2).unwrap());
    let variable = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        QuadraticForm::new(0, vec![1], vec![bits(1, 0)]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    assert!(
        CompletedProbability::from_boundary_terms(1, vec![variable], Default::default())
            .unwrap()
            .affine_mixture(&a)
            .unwrap()
            .is_none()
    );
    let no_axes = UnitaryCircuit::new(0, vec![]).unwrap();
    assert_eq!(
        no_axes
            .commuting_axis_sector(&[bits(0, 0)], Default::default())
            .unwrap()
            .rows,
        vec![bits(1, 1)]
    );
    assert!(
        no_axes
            .commuting_axis_sector(&[bits(1, 0)], Default::default())
            .is_err()
    );
    assert!(
        no_axes
            .commuting_axis_sector(
                &[],
                rmagna::unitary::AxisSectorOptions {
                    max_geometry_words: 0
                }
            )
            .is_err()
    );
    let mut actions = rmagna::noise::CompactNoiseActions::new();
    assert!(actions.is_empty());
    actions
        .push(SparseNoiseAction::ReportFlip { record: "r".into() })
        .unwrap();
    let mut copy = actions.get(0).unwrap();
    if let SparseNoiseAction::ReportFlip { record } = &mut copy {
        record.clear();
    }
    assert_eq!(
        actions.get(0),
        Some(SparseNoiseAction::ReportFlip { record: "r".into() })
    );
    assert_eq!(actions.get(usize::MAX), None);
    assert!(
        rmagna::instrument::OriginalLiteralProgramLayout::new(
            vec![measure("x", "", false)],
            &[0],
            OriginalLimits {
                max_metadata_bytes: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn target_projection_keeps_seven_protocol_records_and_independent_owner() {
    let files = Files::new();
    files.put("ideal.magna", "H 0\nT 0\n");
    let registry = ControllerRegistry::default();
    let evaluator = files.evaluator(
        "H 5\nT 5\nM 0 0 0 0 0 0 0\n",
        r#"{"outputs":[5],"sample_records":[0,1,2,3,4,5,6],"ideal_state":"ideal.magna"}"#,
        &registry,
    );
    let storage = evaluator.reference_storage_statistics().unwrap();
    // Sparse physical labels {0, 5} occupy two slots, plus seven records.
    assert_eq!(storage.source.unwrap().physical_width, 9);
    assert_eq!(storage.target.unwrap().physical_width, 1);
    let draw = evaluator.sample(&mut SplitMix64::new(0)).unwrap();
    assert_eq!(draw.sampled_records, [false; 7]);
    assert_eq!(draw.raw_target_success, Some(ExactScalar::one()));
    assert!(draw.raw_error.is_zero());
}

#[test]
fn exact_categorical_zero_channel_uses_one_caller_word() {
    let law = model(&[vec![rational(0, 1), rational(1, 3), rational(2, 3)]]);
    let sampler = NoiseSampler::physical(law, Default::default());
    for word in 0..3 {
        struct OneWord(Option<u64>);
        impl RandomSource for OneWord {
            fn next_u64(&mut self) -> u64 {
                self.0.take().expect("zero channel requested extra entropy")
            }
        }
        let mut random = OneWord(Some(word));
        let draw = sampler.draw(&mut random).unwrap();
        assert_eq!(random.0, None);
        assert_eq!(draw.entropy_words, 1);
        assert_eq!(
            draw.faults,
            [FaultSelection {
                site: 0,
                alternative: usize::from(word != 0)
            }]
        );
        assert_eq!(
            draw.physical_probability,
            if word == 0 {
                rational(1, 3)
            } else {
                rational(2, 3)
            }
        );
    }
}

fn literal_projected_moment(
    dense: &[ExactScalar],
    p: &PauliWord,
    rows: &[BitVector],
    outcome: usize,
    a: &ScalarArithmetic,
) -> ExactScalar {
    let allowed = |v: usize| {
        rows.iter()
            .enumerate()
            .all(|(j, row)| ((v & index(row)).count_ones() as usize & 1) == (outcome >> j & 1))
    };
    let mut sum = ExactScalar::zero();
    for (u, value) in dense.iter().enumerate() {
        let v = u ^ index(p.x());
        if !allowed(u) || !allowed(v) {
            continue;
        }
        let phase = i64::from(p.phase()) + 4 * i64::from((u & index(p.z())).count_ones() & 1);
        let product = a.multiply(&dense[v].conjugate(), value).unwrap();
        sum = a
            .add(
                &sum,
                &a.multiply(&product, &a.omega(phase, 0).unwrap()).unwrap(),
            )
            .unwrap();
    }
    sum
}
#[test]
fn sealed_trace_both_compilers_preserve_complex_moments_dependent_events_and_frames() {
    use rmagna::unitary::{TraceOptions, TraceQuery};
    let a = ScalarArithmetic::default();
    let gates = vec![
        U::Clifford(C::H(0)),
        U::Clifford(C::H(1)),
        U::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
        U::T { qubit: 0, turns: 1 },
        U::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        U::Clifford(C::H(0)),
        U::T { qubit: 1, turns: 3 },
        U::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
        U::Clifford(C::H(2)),
    ];
    let circuit = UnitaryCircuit::new(3, gates.clone()).unwrap();
    let mut queries = vec![
        TraceQuery {
            word: PauliWord::identity(3).unwrap(),
            event_rows: vec![bits(3, 1), bits(3, 2), bits(3, 3)],
        },
        TraceQuery {
            word: PauliWord::new(bits(3, 1), bits(3, 3), 2).unwrap(),
            event_rows: vec![bits(3, 4)],
        },
        TraceQuery {
            word: PauliWord::new(bits(3, 1), bits(3, 0), 2).unwrap(),
            event_rows: vec![bits(3, 4)],
        },
        TraceQuery {
            word: PauliWord::new(bits(3, 1), bits(3, 0), 0).unwrap(),
            event_rows: vec![bits(3, 1)],
        },
    ];
    queries.push(TraceQuery {
        word: PauliWord::new(bits(3, 0), bits(3, 4), 2).unwrap(),
        event_rows: vec![bits(3, 4)],
    });
    // All three-wire Pauli words expose coherent off-diagonal signs, rather
    // than relying on observables whose exact value might vanish accidentally.
    for x in 0..8 {
        for z in 0..8usize {
            queries.push(TraceQuery {
                word: PauliWord::new(bits(3, x), bits(3, z), 2 * (x & z).count_ones() as u8)
                    .unwrap(),
                event_rows: vec![],
            });
        }
    }
    let mut cases = 0;
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for initial in [0, 5] {
            let source = circuit
                .compile_with_strategy(&bits(3, initial), &a, strategy)
                .unwrap();
            let baseline = circuit
                .compile_with_strategy(&bits(3, initial), &a, strategy)
                .unwrap();
            let diagnostic = source.compilation().clone();
            let trace = source
                .into_trace_runtime(
                    &queries,
                    TraceOptions {
                        max_terms: 4096,
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
            assert!(trace.amplitude_ownership_probe().is_released());
            assert_eq!(trace.compilation(), &diagnostic);
            let prepared = prepared_counts(trace.query_work_statistics());
            for position in 0..=gates.len() {
                for letter in 0..4 {
                    let wire = position % 3;
                    let x = if letter == 0 || letter == 2 {
                        1 << wire
                    } else {
                        0
                    };
                    let z = if letter == 1 || letter == 2 {
                        1 << wire
                    } else {
                        0
                    };
                    let insertion = rmagna::unitary::PauliInsertion {
                        before_gate: position,
                        word: PauliWord::new(
                            bits(3, x),
                            bits(3, z),
                            if letter == 2 {
                                2
                            } else if letter == 3 {
                                3
                            } else {
                                0
                            },
                        )
                        .unwrap(),
                    };
                    let assigned = trace.assignment(&[insertion.clone()]).unwrap();
                    let ordinary = baseline.with_insertions(&[insertion], &a).unwrap();
                    assert_eq!(
                        assigned.transported_frame(),
                        ordinary.assignment_frame().unwrap()
                    );
                    let mut literal = gates[..position].to_vec();
                    if z != 0 {
                        literal.push(U::Clifford(C::Z(wire)));
                    }
                    if x != 0 {
                        literal.push(U::Clifford(C::X(wire)));
                    }
                    literal.extend_from_slice(&gates[position..]);
                    let dense = raw_dense_unitary(3, initial, &literal, &a);
                    for (i, q) in queries.iter().enumerate() {
                        for outcome in 0..1usize << q.event_rows.len() {
                            let expected = literal_projected_moment(
                                &dense,
                                &q.word,
                                &q.event_rows,
                                outcome,
                                &a,
                            );
                            let actual = trace
                                .evaluate(i, &assigned, &bits(q.event_rows.len(), outcome), &a)
                                .unwrap();
                            assert_eq!(
                                actual.value, expected,
                                "{strategy:?} gate{position} letter{letter} query{i} outcome{outcome}"
                            );
                        }
                    }
                    assert_eq!(prepared_counts(trace.query_work_statistics()), prepared);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 160);
}
#[test]
fn trace_release_is_observed_even_with_external_ordinary_plans_and_invalid_requests() {
    use rmagna::unitary::{TraceOptions, TraceQuery, UnitaryError};
    let a = ScalarArithmetic::default();
    let circuit =
        UnitaryCircuit::new(1, vec![U::Clifford(C::H(0)), U::T { qubit: 0, turns: 1 }]).unwrap();
    let q = TraceQuery {
        word: PauliWord::identity(1).unwrap(),
        event_rows: vec![],
    };
    assert!(matches!(
        circuit
            .compile(&bits(1, 0), &a)
            .unwrap()
            .into_trace_runtime(
                &[q.clone()],
                TraceOptions {
                    max_path_bits: 0,
                    ..Default::default()
                },
                &a
            ),
        Err(UnitaryError::RawPathLimit { .. })
    ));
    let source = circuit.compile(&bits(1, 0), &a).unwrap();
    let held = source.prepare_probability(&[], &a).unwrap();
    let probe = source.amplitude_ownership_probe();
    assert!(!probe.is_released());
    let trace = source
        .into_trace_runtime(&[q.clone()], TraceOptions::default(), &a)
        .unwrap();
    assert!(
        !probe.is_released(),
        "the independent ordinary family still owns its amplitude input"
    );
    drop(held);
    assert!(probe.is_released());
    let assigned = trace.assignment(&[]).unwrap();
    assert_eq!(
        trace.evaluate(0, &assigned, &bits(0, 0), &a).unwrap().value,
        ExactScalar::one()
    );
    let foreign = circuit
        .compile_trace(
            &bits(1, 0),
            CompilerStrategy::SharedCommutingLayer,
            &[q.clone()],
            TraceOptions::default(),
            &a,
        )
        .unwrap();
    let other = foreign.assignment(&[]).unwrap();
    assert!(matches!(
        trace.evaluate(0, &other, &bits(0, 0), &a),
        Err(UnitaryError::IncompatibleProgram)
    ));
    assert!(matches!(
        trace.evaluate(1, &assigned, &bits(0, 0), &a),
        Err(UnitaryError::InvalidTraceQuery { .. })
    ));
    assert!(matches!(
        trace.evaluate(0, &assigned, &bits(1, 0), &a),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(
        circuit
            .compile_trace(
                &bits(1, 0),
                CompilerStrategy::ParityCopies,
                &[q.clone()],
                TraceOptions {
                    max_queries: 0,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert!(
        circuit
            .compile_trace(
                &bits(1, 0),
                CompilerStrategy::ParityCopies,
                &[q.clone()],
                TraceOptions {
                    max_geometry_words: 0,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert!(
        circuit
            .compile_trace(
                &bits(1, 0),
                CompilerStrategy::ParityCopies,
                &[q.clone()],
                TraceOptions {
                    max_terms: 0,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert!(
        circuit
            .compile_trace(
                &bits(1, 0),
                CompilerStrategy::ParityCopies,
                &[TraceQuery {
                    word: PauliWord::identity(2).unwrap(),
                    event_rows: vec![]
                }],
                TraceOptions::default(),
                &a
            )
            .is_err()
    );
    let source = circuit.compile(&bits(1, 0), &a).unwrap();
    let changed = source
        .with_insertions(
            &[rmagna::unitary::PauliInsertion {
                before_gate: 0,
                word: PauliWord::identity(1).unwrap(),
            }],
            &a,
        )
        .unwrap();
    assert!(matches!(
        changed.into_trace_runtime(&[q], TraceOptions::default(), &a),
        Err(UnitaryError::AssignedTraceReference)
    ));
    let mut bits_called = 0;
    let bad = trace.sample_event_chain(
        &[],
        &assigned,
        &bits(0, 0),
        &mut || {
            bits_called += 1;
            false
        },
        64,
        &a,
    );
    assert!(matches!(
        bad,
        Err(rmagna::unitary::TraceSamplingError::InvalidChain)
    ));
    assert_eq!(bits_called, 0);
    drop(trace);
    assert!(probe.is_released());
}
#[test]
fn trace_cross_word_frames_and_prefix_sampling_keep_exact_conditional_mass() {
    use rmagna::unitary::{TraceOptions, TraceQuery, TraceSamplingError};
    let a = ScalarArithmetic::default();
    let gates = vec![
        U::Clifford(C::H(64)),
        U::Clifford(C::Cx {
            control: 64,
            target: 0,
        }),
        U::T {
            qubit: 64,
            turns: 1,
        },
        U::Clifford(C::H(0)),
        U::T { qubit: 0, turns: 3 },
    ];
    let small = vec![
        U::Clifford(C::H(1)),
        U::Clifford(C::Cx {
            control: 1,
            target: 0,
        }),
        U::T { qubit: 1, turns: 1 },
        U::Clifford(C::H(0)),
        U::T { qubit: 0, turns: 3 },
    ];
    let high = BitVector::from_indices(65, [64]).unwrap();
    let word = PauliWord::new(
        bits(65, 1),
        BitVector::from_indices(65, [0, 64]).unwrap(),
        2,
    )
    .unwrap();
    let queries = vec![
        TraceQuery {
            word: word.clone(),
            event_rows: vec![high.clone()],
        },
        TraceQuery {
            word: PauliWord::identity(65).unwrap(),
            event_rows: vec![],
        },
        TraceQuery {
            word: PauliWord::identity(65).unwrap(),
            event_rows: vec![high.clone()],
        },
        TraceQuery {
            word: PauliWord::identity(65).unwrap(),
            event_rows: vec![high.clone(), high.clone()],
        },
        TraceQuery {
            word: PauliWord::identity(65).unwrap(),
            event_rows: vec![bits(65, 0)],
        },
    ];
    let mut cases = 0;
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let trace = UnitaryCircuit::new(65, gates.clone())
            .unwrap()
            .compile_trace(
                &bits(65, 0),
                strategy,
                &queries,
                TraceOptions {
                    max_terms: 1024,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
        assert!(trace.amplitude_ownership_probe().is_released());
        let work = prepared_counts(trace.query_work_statistics());
        for position in 0..=gates.len() {
            let insertion = rmagna::unitary::PauliInsertion {
                before_gate: position,
                word: PauliWord::new(high.clone(), high.clone(), 2).unwrap(),
            };
            let assigned = trace.assignment(&[insertion]).unwrap();
            let mut literal = small[..position].to_vec();
            literal.extend([U::Clifford(C::Z(1)), U::Clifford(C::X(1))]);
            literal.extend_from_slice(&small[position..]);
            let dense = raw_dense_unitary(2, 0, &literal, &a);
            let expected_word = PauliWord::new(bits(2, 1), bits(2, 3), 2).unwrap();
            for outcome in 0..2 {
                assert_eq!(
                    trace
                        .evaluate(0, &assigned, &bits(1, outcome), &a)
                        .unwrap()
                        .value,
                    literal_projected_moment(&dense, &expected_word, &[bits(2, 2)], outcome, &a)
                );
            }
            let mut n = 0;
            let draw = trace
                .sample_event_chain(
                    &[1, 2, 3],
                    &assigned,
                    &bits(0, 0),
                    &mut || {
                        n += 1;
                        false
                    },
                    64,
                    &a,
                )
                .unwrap();
            assert_eq!(draw.values, bits(2, 0));
            assert_eq!(draw.mass, a.omega(0, -2).unwrap());
            assert_eq!(draw.condition_mass, ExactScalar::one());
            assert_eq!((draw.entropy_bits, n), (1, 1));
            let mut forbidden = || panic!("invalid trace chain requested entropy");
            assert!(matches!(
                trace.sample_event_chain(&[1, 0], &assigned, &bits(0, 0), &mut forbidden, 64, &a),
                Err(TraceSamplingError::InvalidChain)
            ));
            assert!(
                trace
                    .sample_event_chain(&[4], &assigned, &bits(1, 1), &mut forbidden, 64, &a)
                    .is_err()
            );
            assert!(
                trace
                    .sample_event_chain(&[1, 2], &assigned, &bits(0, 0), &mut forbidden, 0, &a)
                    .is_err()
            );
            assert_eq!(prepared_counts(trace.query_work_statistics()), work);
            cases += 1;
        }
    }
    assert_eq!(cases, 12);
}

#[test]
fn sealed_file_trace_keeps_selection_parity_constants_and_fault_validation() {
    use rmagna::instrument::{InstrumentError, InstrumentTraceQuery as TQ};
    use rmagna::unitary::TraceOptions;
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nM(1/4) 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n",
        &Default::default(),
    )
    .unwrap();
    let ir = compile_affine(&source, &Default::default(), &a).unwrap();
    let flipped = BitExpr {
        constant: true,
        terms: [BitAtom::Record(0)].into_iter().collect(),
    };
    let q = |accepted| TQ {
        outputs: vec![],
        word: PauliWord::identity(0).unwrap(),
        parities: vec![flipped.clone()],
        accepted,
    };
    for options in [
        TraceOptions {
            max_queries: 0,
            ..Default::default()
        },
        TraceOptions {
            max_geometry_words: 0,
            ..Default::default()
        },
        // Admission of instrument metadata alone must not waive the unitary
        // closed-kernel cost in the same budget.
        TraceOptions {
            max_geometry_words: 128,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            ir.clone().into_trace_runtime(&[q(true)], options, &a),
            Err(InstrumentError::Unitary(
                rmagna::unitary::UnitaryError::QueryCacheLimit { .. }
            ))
        ));
    }
    let probe = ir.amplitude_ownership_probe();
    let other = ir.clone();
    let trace = ir
        .into_trace_runtime(&[q(false), q(true)], TraceOptions::default(), &a)
        .unwrap();
    assert!(!probe.is_released());
    for fault in [false, true] {
        let faults = if fault {
            vec![FaultSelection {
                site: 0,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        let assigned = trace.assignment(&faults).unwrap();
        for outcome in 0..2 {
            assert_eq!(
                trace
                    .evaluate(0, &assigned, &bits(1, outcome), &a)
                    .unwrap()
                    .value,
                other
                    .reported_probability(&[flipped.clone()], &bits(1, outcome), &faults, 256, &a)
                    .unwrap()
                    .value
            );
            assert_eq!(
                trace
                    .evaluate(1, &assigned, &bits(1, outcome), &a)
                    .unwrap()
                    .value,
                if outcome == 1 {
                    a.omega(0, -2).unwrap()
                } else {
                    ExactScalar::zero()
                }
            );
        }
    }
    drop(other);
    assert!(probe.is_released());
    assert!(matches!(
        trace.assignment(&[FaultSelection {
            site: 1,
            alternative: 0
        }]),
        Err(InstrumentError::InvalidFault { .. })
    ));
    assert!(matches!(
        trace.assignment(&[FaultSelection {
            site: 0,
            alternative: 1
        }]),
        Err(InstrumentError::InvalidFault { .. })
    ));
    assert!(matches!(
        trace.assignment(
            &[FaultSelection {
                site: 0,
                alternative: 0
            }; 2]
        ),
        Err(InstrumentError::RepeatedFault { .. })
    ));
}

#[test]
fn explicit_trace_compilation_omits_the_open_query_projection() {
    use rmagna::unitary::{TraceOptions, TraceQuery};
    let a = ScalarArithmetic::default();
    // A fixed Z observation has rankzero on |0>. The ordinary compiler builds
    // a separate reduced query boundary; trace purpose deliberately omits it.
    let circuit = UnitaryCircuit::new(1, vec![U::T { qubit: 0, turns: 1 }]).unwrap();
    let open = circuit
        .compile_with_strategy(&bits(1, 0), &a, CompilerStrategy::SharedCommutingLayer)
        .unwrap();
    let open_probe = open.amplitude_ownership_probe();
    assert!(open_probe.has_distinct_query_boundary());
    assert_eq!(open.phase_legs(), 0);
    let trace = circuit
        .compile_trace(
            &bits(1, 0),
            CompilerStrategy::SharedCommutingLayer,
            &[TraceQuery {
                word: PauliWord::identity(1).unwrap(),
                event_rows: vec![],
            }],
            TraceOptions::default(),
            &a,
        )
        .unwrap();
    let probe = trace.amplitude_ownership_probe();
    assert!(!probe.has_distinct_query_boundary());
    assert!(probe.is_released());
    assert_eq!(trace.reference_phase().num_variables(), 1);
    assert_eq!(trace.reference_phase().terms().len(), 1);
    assert_eq!(
        trace.compilation().reference_coherent_width,
        open.compilation().reference_coherent_width
    );
    let assignment = trace.assignment(&[]).unwrap();
    assert_eq!(
        trace
            .evaluate(0, &assignment, &bits(0, 0), &a)
            .unwrap()
            .value,
        ExactScalar::one()
    );
    // Amplitude behavior is separately retained and includes exact globalphase.
    assert_eq!(
        open.amplitude(&bits(1, 0), 256, &a).unwrap().value,
        ExactScalar::one()
    );
    assert_eq!(
        open.amplitude(&bits(1, 1), 256, &a).unwrap().value,
        ExactScalar::zero()
    );
}
