// SPDX-License-Identifier: MPL-2.0
// Complete factor/quadratic cases adapted from itpyi/magna tests/native/gf2_kernel.cpp,
// revision 56847478c840395735146ef1e1ccdc3d8aedc10f (Mozilla Public License 2.0).
// Fixtures contain only the actual C++ inputs; all expectations below use bytes.
use rmagna::gf2::{BitVector, Gf2Error, LinearFactorization};
use rmagna::quadratic::{QuadraticError, QuadraticForm};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[path = "support/original_gf2.rs"]
mod original_gf2;
use original_gf2::{Mt19937, bytes, equals_bytes, pack};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    upstream_commit: String,
    seed: u64,
    packed_draws: usize,
    factor_cases: Vec<FactorCase>,
    quadratic_cases: Vec<QuadraticCase>,
    total_draws: usize,
    fnv64: u64,
    byte_fnv64: u64,
    original_checks: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactorCase {
    n: usize,
    rows: Vec<String>,
    trials: Vec<Trial>,
    next_word: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trial {
    rhs: String,
    seed: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuadraticCase {
    n: usize,
    k: usize,
    density: u64,
    linear: Vec<u8>,
    upper: Vec<String>,
    basis: Vec<String>,
    shift: String,
    samples: Vec<String>,
    next_word: u64,
}
type Matrix = Vec<Vec<u8>>;
fn unpack(value: &BitVector) -> Vec<u8> {
    (0..value.width())
        .map(|i| u8::from(value.get(i).unwrap()))
        .collect()
}
fn integer_row(width: usize, value: usize) -> Vec<u8> {
    (0..width).map(|j| ((value >> j) & 1) as u8).collect()
}
fn dot(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).fold(0, |p, (a, b)| p ^ (a * b))
}
fn product(a: &Matrix, x: &[u8]) -> Vec<u8> {
    a.iter().map(|row| dot(row, x)).collect()
}
fn combination(a: &Matrix, mask: &[u8], width: usize) -> Vec<u8> {
    let mut result = vec![0; width];
    for (row, &bit) in a.iter().zip(mask) {
        if bit != 0 {
            for (out, &b) in result.iter_mut().zip(row) {
                *out ^= b;
            }
        }
    }
    result
}
// Independent byte elimination, with explicit column order; never production GF2.
fn reduce(mut a: Matrix, width: usize, high: bool) -> (Matrix, Vec<usize>) {
    let mut pivots = Vec::new();
    for step in 0..width {
        let column = if high { width - 1 - step } else { step };
        let rank = pivots.len();
        let Some(found) = (rank..a.len()).find(|&i| a[i][column] != 0) else {
            continue;
        };
        a.swap(rank, found);
        let pivot = a[rank].clone();
        for (i, row) in a.iter_mut().enumerate() {
            if i != rank && row[column] != 0 {
                for (out, &bit) in row.iter_mut().zip(&pivot) {
                    *out ^= bit;
                }
            }
        }
        pivots.push(column);
    }
    (a, pivots)
}
fn rank(a: &Matrix, width: usize) -> usize {
    reduce(a.clone(), width, false).1.len()
}
fn oracle_solve(a: &Matrix, rhs: &[u8], width: usize, seed: &[u8]) -> Option<Vec<u8>> {
    let augmented = a
        .iter()
        .zip(rhs)
        .map(|(row, &b)| {
            let mut row = row.clone();
            row.push(b);
            row
        })
        .collect();
    let (rref, pivots) = reduce(augmented, width, true);
    if rref[pivots.len()..].iter().any(|row| row[width] != 0) {
        return None;
    }
    let mut x = seed.to_vec();
    for (row, &p) in rref.iter().zip(&pivots) {
        x[p] = row[width] ^ dot(&row[..width], seed) ^ seed[p];
    }
    Some(x)
}
// Reversal conjugates native highest-coordinate pivots into Rust's existing
// lowest-column convention. Apply this same reversible map to every A/x/seed;
// equation indices (and therefore RHS/certificate coordinates) are unchanged.
fn reverse(row: &[u8]) -> Vec<u8> {
    row.iter().rev().copied().collect()
}
fn check_factor(case: &FactorCase, a: Matrix, rng: &mut Mt19937) {
    let n = case.n;
    let m = a.len();
    assert_eq!(
        a,
        case.rows
            .iter()
            .map(|row| bytes(row, n))
            .collect::<Matrix>()
    );
    let input = a.iter().map(|row| pack(&reverse(row))).collect::<Vec<_>>();
    let snapshot = input.clone();
    let words = m * m.div_ceil(64);
    let factor = LinearFactorization::new_with_certificates(n, &input, words).unwrap();
    if words != 0 {
        assert!(matches!(
            LinearFactorization::new_with_certificates(n, &input, words - 1),
            Err(Gf2Error::CertificateWordLimit { required, limit }) if required == words && limit == words - 1
        ));
    }
    let saved = format!("{factor:?}");
    let r = rank(&a, n);
    assert_eq!(factor.rank(), r);
    let native_pivots = factor
        .pivot_columns()
        .iter()
        .map(|&p| n - 1 - p)
        .collect::<Vec<_>>();
    assert_eq!(native_pivots, reduce(a.clone(), n, true).1);
    let kernel = factor
        .nullspace()
        .unwrap()
        .iter()
        .map(|v| reverse(&unpack(v)))
        .collect::<Matrix>();
    assert_eq!(kernel.len(), n - r);
    assert_eq!(rank(&kernel, n), n - r);
    for v in &kernel {
        assert_eq!(product(&a, v), vec![0; m]);
    }
    let certs = factor.row_certificates().unwrap();
    assert_eq!(certs.len(), m);
    for (i, certificate) in certs.iter().enumerate() {
        let certificate = unpack(certificate);
        assert_eq!(certificate.len(), m);
        let value = combination(&a, &certificate, n);
        if i < r {
            let actual = &factor.row_space_basis()[i];
            equals_bytes(actual, &reverse(&value));
            // Native Bits.size() == high_pivot + 1 becomes explicit support.
            assert_eq!(value.iter().rposition(|&b| b != 0), Some(native_pivots[i]));
        } else {
            assert_eq!(value, vec![0; n]);
        }
    }
    let left = factor
        .left_kernel()
        .unwrap()
        .iter()
        .map(unpack)
        .collect::<Matrix>();
    assert_eq!(left.len(), m - r);
    assert_eq!(rank(&left, m), m - r);
    for v in &left {
        assert_eq!(combination(&a, v, n), vec![0; n]);
    }
    let trials = if m <= 5 { 1 << m } else { 16 };
    assert_eq!(case.trials.len(), trials);
    for (trial, literal) in case.trials.iter().enumerate() {
        let mut rhs = if m <= 5 {
            integer_row(m, trial)
        } else {
            rng.row(m, 2)
        };
        if trial % 2 != 0 && m > 5 {
            rhs = product(&a, &rng.row(n, 2));
        }
        assert_eq!(bytes(&literal.rhs, m), rhs);
        let expected = oracle_solve(&a, &rhs, n, &vec![0; n]);
        let packed_rhs = pack(&rhs);
        let solution = factor.solve(&packed_rhs).unwrap();
        assert_eq!(solution.is_some(), expected.is_some());
        if let Some(expected) = expected {
            let actual = reverse(&unpack(&solution.unwrap()));
            assert_eq!(actual, expected);
            assert_eq!(product(&a, &actual), rhs);
            let initial = rng.row(n, 2);
            assert_eq!(bytes(literal.seed.as_ref().unwrap(), n), initial);
            let seed = pack(&reverse(&initial));
            let seeded = factor.solve_seeded(&packed_rhs, &seed).unwrap().unwrap();
            let seeded = reverse(&unpack(&seeded));
            assert_eq!(product(&a, &seeded), rhs);
            assert_eq!(Some(seeded.clone()), oracle_solve(&a, &rhs, n, &initial));
            for j in 0..n {
                if !native_pivots.contains(&j) {
                    assert_eq!(seeded[j], initial[j]);
                }
            }
            equals_bytes(&seed, &reverse(&initial));
            if n <= 5 {
                let expected_solutions = (0..1 << n)
                    .map(|i| integer_row(n, i))
                    .filter(|x| product(&a, x) == rhs)
                    .collect::<BTreeSet<_>>();
                let generated = (0..1 << kernel.len())
                    .map(|i| {
                        let mut row = combination(&kernel, &integer_row(kernel.len(), i), n);
                        for (b, &p) in row.iter_mut().zip(&actual) {
                            *b ^= p;
                        }
                        row
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(generated, expected_solutions);
            }
        } else {
            assert!(literal.seed.is_none());
            assert!(
                factor
                    .solve_seeded(&packed_rhs, &BitVector::zeros(n).unwrap())
                    .unwrap()
                    .is_none()
            );
        }
        equals_bytes(&packed_rhs, &rhs);
    }
    assert_eq!(rng.clone().next(), case.next_word);
    assert_eq!(input, snapshot);
    assert_eq!(format!("{factor:?}"), saved);
}
fn polynomial(x: &[u8], linear: &[u8], upper: &Matrix) -> u8 {
    let mut sum = 0_usize;
    for j in 0..x.len() {
        if x[j] != 0 {
            sum += usize::from(linear[j]);
            for (k, &bit) in x.iter().enumerate().skip(j + 1) {
                sum += 2 * usize::from(upper[j][k]) * usize::from(bit);
            }
        }
    }
    (sum & 3) as u8
}
fn check_quadratic(c: &QuadraticCase, n: usize, k: usize, density: u64, rng: &mut Mt19937) {
    assert_eq!((c.n, c.k, c.density), (n, k, density));
    let mut linear = vec![0; n];
    let mut upper = vec![vec![0; n]; n];
    for j in 0..n {
        linear[j] = (rng.next() % 4) as u8;
        for bit in upper[j].iter_mut().skip(j + 1) {
            *bit = u8::from(rng.next().is_multiple_of(density));
        }
    }
    let mut basis = (0..k).map(|_| rng.row(n, density)).collect::<Matrix>();
    if k > 2 {
        basis[0].fill(0);
        basis[k - 1] = basis[k - 2].clone();
    }
    let shift = rng.row(n, 2);
    assert_eq!(c.linear, linear);
    assert_eq!(
        c.upper.iter().map(|row| bytes(row, n)).collect::<Matrix>(),
        upper
    );
    assert_eq!(
        c.basis.iter().map(|row| bytes(row, n)).collect::<Matrix>(),
        basis
    );
    assert_eq!(bytes(&c.shift, n), shift);
    let original =
        QuadraticForm::new(0, linear.clone(), upper.iter().map(|r| pack(r)).collect()).unwrap();
    let packed_shift = pack(&shift);
    let packed_basis = basis.iter().map(|r| pack(r)).collect::<Vec<_>>();
    let restricted = original.restrict(&packed_shift, &packed_basis).unwrap();
    let omitted = original
        .restrict_without_constant(&packed_shift, &packed_basis)
        .unwrap();
    assert_eq!(restricted.constant(), polynomial(&shift, &linear, &upper));
    assert_eq!(omitted.constant(), 0);
    assert_eq!(restricted.linear(), omitted.linear());
    assert_eq!(restricted.upper(), omitted.upper());
    assert_eq!(restricted.num_variables(), k);
    for (j, row) in restricted.upper().iter().enumerate() {
        assert_eq!(row.width(), k);
        assert!(row.ones().all(|l| l > j));
    }
    let arithmetic = ScalarArithmetic::default();
    let mut literal_sum = ExactScalar::zero();
    let samples = if k <= 8 { 1 << k } else { 64 };
    assert_eq!(c.samples.len(), samples);
    for (i, sample) in c.samples.iter().enumerate() {
        let y = if k <= 8 {
            integer_row(k, i)
        } else {
            rng.row(k, 2)
        };
        assert_eq!(bytes(sample, k), y);
        let mut x = combination(&basis, &y, n);
        for (b, &s) in x.iter_mut().zip(&shift) {
            *b ^= s;
        }
        let expected = polynomial(&x, &linear, &upper);
        let actual = restricted.evaluate(&pack(&y)).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            (omitted.evaluate(&pack(&y)).unwrap() + restricted.constant()) & 3,
            expected
        );
        for phase in 0..8 {
            assert_eq!(
                arithmetic.omega(phase + 2 * i64::from(actual), 0).unwrap(),
                arithmetic
                    .omega(phase + 2 * i64::from(expected), 0)
                    .unwrap()
            );
        }
        if k <= 8 {
            literal_sum = arithmetic
                .add(
                    &literal_sum,
                    &arithmetic.omega(2 * i64::from(expected), 0).unwrap(),
                )
                .unwrap();
        }
    }
    if k <= 8 {
        assert_eq!(restricted.gauss_sum(&arithmetic).unwrap(), literal_sum);
    }
    assert_eq!(
        restricted.gauss_sum(&arithmetic).unwrap(),
        arithmetic
            .multiply(
                &arithmetic
                    .omega(2 * i64::from(restricted.constant()), 0)
                    .unwrap(),
                &omitted.gauss_sum(&arithmetic).unwrap(),
            )
            .unwrap()
    );
    assert_eq!(original.constant(), 0);
    assert_eq!(original.linear(), linear);
    assert_eq!(
        original.upper(),
        upper.iter().map(|r| pack(r)).collect::<Vec<_>>()
    );
    equals_bytes(&packed_shift, &shift);
    assert_eq!(
        packed_basis,
        basis.iter().map(|r| pack(r)).collect::<Vec<_>>()
    );
    assert_eq!(rng.clone().next(), c.next_word);
}

#[test]
fn complete_original_factor_and_quadratic_contracts_follow_exact_stream() {
    let raw = include_bytes!("fixtures/original_factor_inputs.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "f3f1454a73578e50ac3085e20b130af30b133b4e09de9b5dfd9e5d54f3cb64ef"
    );
    let inputs: Inputs = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        inputs.upstream_commit,
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(inputs.seed, 2026100409);
    assert_eq!(
        (inputs.packed_draws, inputs.total_draws, inputs.fnv64),
        (708504, 1000450, 9751379586251010270)
    );
    assert_eq!(inputs.byte_fnv64, 5074253544447723934);
    assert_eq!(inputs.original_checks, 646357);
    assert_eq!(inputs.factor_cases.len(), 143);
    assert_eq!(inputs.quadratic_cases.len(), 26);
    let mut rng = Mt19937::new(inputs.seed);
    for _ in 0..708504 {
        rng.next();
    }
    assert_eq!(rng.clone().next(), 11929231392562902635);
    let mut factors = inputs.factor_cases.iter();
    for n in 0..=5 {
        for m in 0..=5 {
            for trial in 0..3 {
                let mut a = vec![vec![0; n]; m];
                if trial != 0 {
                    for row in &mut a {
                        *row = rng.row(n, 2);
                    }
                }
                if trial == 2 && m > 1 {
                    a[m - 1] = a[0].clone();
                }
                let c = factors.next().unwrap();
                assert_eq!(c.n, n);
                check_factor(c, a, &mut rng);
            }
        }
    }
    for n in [63, 64, 65, 127, 128, 129, 257] {
        for m in [0, 1, 7, 70] {
            let mut a = (0..m).map(|_| rng.row(n, 2)).collect::<Matrix>();
            if m > 2 {
                a[m - 1] = a[0].clone();
                a[m - 2].fill(0);
            }
            let c = factors.next().unwrap();
            assert_eq!(c.n, n);
            check_factor(c, a, &mut rng);
        }
        let mut identity = vec![vec![0; n]; n];
        for (j, row) in identity.iter_mut().enumerate() {
            row[j] = 1;
        }
        let c = factors.next().unwrap();
        assert_eq!(c.n, n);
        check_factor(c, identity, &mut rng);
    }
    assert!(factors.next().is_none());
    let mut quadratics = inputs.quadratic_cases.iter();
    for (n, k) in [
        (0, 0),
        (0, 5),
        (1, 0),
        (1, 1),
        (3, 5),
        (5, 3),
        (7, 7),
        (63, 6),
        (64, 7),
        (65, 8),
        (65, 65),
        (129, 67),
        (137, 129),
    ] {
        for density in [2, 11] {
            check_quadratic(quadratics.next().unwrap(), n, k, density, &mut rng);
        }
    }
    assert!(quadratics.next().is_none());
    assert_eq!(rng.draws, inputs.total_draws);
    assert_eq!(rng.fingerprint, inputs.fnv64);
}

#[test]
fn certificate_quotas_seeded_defaults_and_quadratic_omission_have_checked_boundaries() {
    let row = BitVector::from_indices(2, [0, 1]).unwrap();
    let default = LinearFactorization::new(2, std::slice::from_ref(&row)).unwrap();
    let rhs = BitVector::from_indices(1, [0]).unwrap();
    let zero = BitVector::zeros(2).unwrap();
    assert_eq!(default.pivot_columns(), &[0]);
    equals_bytes(
        &default.solve_seeded(&rhs, &zero).unwrap().unwrap(),
        &[1, 0],
    );
    let reversed =
        LinearFactorization::new_with_certificates(2, std::slice::from_ref(&row), 1).unwrap();
    assert_eq!(
        reverse(&unpack(
            &reversed.solve_seeded(&rhs, &zero).unwrap().unwrap()
        )),
        vec![0, 1]
    );
    let tall = vec![row; 137];
    let compressed = LinearFactorization::new(2, &tall).unwrap();
    assert!(compressed.row_certificates().is_none());
    assert!(compressed.left_kernel().is_none());
    let full = LinearFactorization::new_with_certificates(2, &tall, 411).unwrap();
    assert_eq!(full.row_certificates().unwrap().len(), 137);
    assert_eq!(full.left_kernel().unwrap().len(), 136);
    assert!(matches!(
        LinearFactorization::new_with_certificates(2, &tall, 410),
        Err(Gf2Error::CertificateWordLimit {
            required: 411,
            limit: 410
        })
    ));
    // Width rejection takes precedence over a zero certificate quota.
    assert!(matches!(
        LinearFactorization::new_with_certificates(usize::MAX, &tall, 0),
        Err(Gf2Error::WidthMismatch {
            expected: usize::MAX,
            actual: 2
        })
    ));
    let tall_rhs = BitVector::from_indices(137, 0..137).unwrap();
    let seed = BitVector::from_indices(2, [1]).unwrap();
    for f in [&compressed, &full] {
        equals_bytes(&f.solve_seeded(&tall_rhs, &seed).unwrap().unwrap(), &[0, 1]);
        assert!(matches!(
            f.solve_seeded(&BitVector::zeros(136).unwrap(), &zero),
            Err(Gf2Error::WidthMismatch {
                expected: 137,
                actual: 136
            })
        ));
        assert!(matches!(
            f.solve_seeded(&tall_rhs, &BitVector::zeros(3).unwrap()),
            Err(Gf2Error::WidthMismatch {
                expected: 2,
                actual: 3
            })
        ));
        let mut contradiction = tall_rhs.clone();
        contradiction.set(136, false).unwrap();
        assert!(f.solve_seeded(&contradiction, &seed).unwrap().is_none());
    }
    // Empty equations do not allocate a solution before checking seed width.
    let huge = LinearFactorization::new_with_certificates(usize::MAX, &[], 0).unwrap();
    assert!(matches!(
        huge.solve_seeded(&BitVector::zeros(0).unwrap(), &zero),
        Err(Gf2Error::WidthMismatch {
            expected: usize::MAX,
            actual: 2
        })
    ));
    let empty = LinearFactorization::new_with_certificates(0, &[], 0).unwrap();
    assert_eq!(empty.left_kernel().unwrap(), &[]);
    let zero_columns = vec![BitVector::zeros(0).unwrap(); 65];
    assert!(matches!(
        LinearFactorization::new_with_certificates(0, &zero_columns, 129),
        Err(Gf2Error::CertificateWordLimit {
            required: 130,
            limit: 129
        })
    ));
    assert_eq!(
        LinearFactorization::new_with_certificates(0, &zero_columns, 130)
            .unwrap()
            .left_kernel()
            .unwrap()
            .len(),
        65
    );
    assert_eq!(
        BitVector::from_words(65, vec![0, 2]),
        Err(Gf2Error::NonzeroPadding)
    );
    assert_eq!(
        BitVector::from_words(65, vec![0]),
        Err(Gf2Error::InvalidWordCount)
    );
    let q = QuadraticForm::new(3, vec![1], vec![BitVector::zeros(1).unwrap()]).unwrap();
    let shift = BitVector::from_indices(1, [0]).unwrap();
    let basis = [shift.clone()];
    let kept = q.restrict(&shift, &basis).unwrap();
    let omitted = q.restrict_without_constant(&shift, &basis).unwrap();
    assert_eq!(kept.constant(), 0); // Original constant 3 plus shift value 1.
    assert_eq!(kept, omitted);
    let q = QuadraticForm::new(2, vec![1], vec![BitVector::zeros(1).unwrap()]).unwrap();
    let kept = q.restrict(&shift, &basis).unwrap();
    let omitted = q.restrict_without_constant(&shift, &basis).unwrap();
    assert_eq!((kept.constant(), omitted.constant()), (3, 0));
    assert_eq!(kept.linear(), omitted.linear());
    assert_eq!(kept.upper(), omitted.upper());
    assert_eq!(q.constant(), 2);
    assert_eq!(
        q.restrict_without_constant(&zero, &basis),
        Err(QuadraticError::DimensionMismatch)
    );
    assert_eq!(
        q.restrict_without_constant(&shift, &[zero]),
        Err(QuadraticError::DimensionMismatch)
    );
    assert_eq!(
        QuadraticForm::new(4, vec![], vec![]),
        Err(QuadraticError::InvalidPhase)
    );
    assert_eq!(
        QuadraticForm::new(0, vec![0], vec![shift]),
        Err(QuadraticError::InvalidUpperTriangle)
    );
}
