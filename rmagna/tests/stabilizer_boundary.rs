use rmagna::gf2::BitVector;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::{AffineKet, PreparedLinearBoundary, StabilizerError};
use std::collections::HashMap;

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}
fn vector(width: usize, seed: &mut u64) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|_| next(seed) & 3 == 0)).unwrap()
}
fn map(rows: &[BitVector], v: &BitVector) -> BitVector {
    BitVector::from_indices(
        rows.len(),
        rows.iter().enumerate().filter_map(|(j, row)| {
            let parity = (0..v.width())
                .filter(|&i| row.get(i).unwrap() && v.get(i).unwrap())
                .count()
                % 2
                != 0;
            parity.then_some(j)
        }),
    )
    .unwrap()
}
#[test]
fn fixed_boundary_maps_preserve_literal_complex_path_sums_and_validate_zero_inputs() {
    let a = ScalarArithmetic::default();
    let mut seed = 5483279;
    for width in [0, 3, 64, 65, 128, 129] {
        let mut columns = (0..6).map(|_| vector(width, &mut seed)).collect::<Vec<_>>();
        columns[4] = columns[1].clone();
        columns[5] = BitVector::zeros(width).unwrap();
        for outputs in [0, 7, 65] {
            let mut rows = (0..outputs)
                .map(|_| vector(width, &mut seed))
                .collect::<Vec<_>>();
            if outputs > 0 {
                rows[0] = BitVector::zeros(width).unwrap();
                rows[outputs - 1] = rows[outputs / 2].clone();
            }
            let zero =
                QuadraticForm::new(0, vec![0; 6], vec![BitVector::zeros(6).unwrap(); 6]).unwrap();
            let reference = AffineKet::new(
                BitVector::zeros(width).unwrap(),
                columns.clone(),
                zero.clone(),
                ExactScalar::one(),
            )
            .unwrap();
            let prepared = PreparedLinearBoundary::new(&reference, &rows).unwrap();
            for assignment in 0..4 {
                let offset = vector(width, &mut seed);
                let linear = (0..6)
                    .map(|_| (next(&mut seed) & 3) as u8)
                    .collect::<Vec<_>>();
                let mut upper = vec![BitVector::zeros(6).unwrap(); 6];
                for (i, row) in upper.iter_mut().enumerate() {
                    for j in i + 1..6 {
                        row.set(j, next(&mut seed) & 1 != 0).unwrap();
                    }
                }
                let constant = (next(&mut seed) & 3) as u8;
                let phase = QuadraticForm::new(constant, linear.clone(), upper.clone()).unwrap();
                let coefficient = if assignment == 3 {
                    ExactScalar::zero()
                } else {
                    a.omega(3, -5).unwrap()
                };
                let input =
                    AffineKet::new(offset.clone(), columns.clone(), phase, coefficient.clone())
                        .unwrap();
                let mapped = prepared.apply(&input).unwrap();
                assert_eq!(mapped.offset(), &map(&rows, &offset));
                assert_eq!(
                    mapped.columns(),
                    columns.iter().map(|v| map(&rows, v)).collect::<Vec<_>>()
                );
                assert_eq!(mapped.phase(), input.phase());
                assert_eq!(mapped.coefficient(), &coefficient);
                let mut oracle = HashMap::<BitVector, ExactScalar>::new();
                for word in 0usize..64 {
                    let mut physical = offset.clone();
                    for (j, col) in columns.iter().enumerate() {
                        if word >> j & 1 != 0 {
                            physical.xor_assign(col).unwrap();
                        }
                    }
                    let mut q = i64::from(constant);
                    for (i, &c) in linear.iter().enumerate() {
                        if word >> i & 1 != 0 {
                            q += i64::from(c);
                            for j in i + 1..6 {
                                if word >> j & 1 != 0 && upper[i].get(j).unwrap() {
                                    q += 2;
                                }
                            }
                        }
                    }
                    let label = map(&rows, &physical);
                    let contribution = if coefficient.is_zero() {
                        ExactScalar::zero()
                    } else {
                        a.omega(3 + 2 * q, -5).unwrap()
                    };
                    let old = oracle
                        .get(&label)
                        .cloned()
                        .unwrap_or_else(ExactScalar::zero);
                    oracle.insert(label, a.add(&old, &contribution).unwrap());
                }
                let query = mapped.prepare().unwrap();
                for (label, expected) in oracle {
                    assert_eq!(
                        query.amplitude(&label, &a).unwrap(),
                        expected,
                        "width={width},outputs={outputs},assignment={assignment}"
                    );
                }
            }
            let changed = AffineKet::new(
                BitVector::zeros(width + 1).unwrap(),
                vec![BitVector::zeros(width + 1).unwrap(); 6],
                zero.clone(),
                ExactScalar::zero(),
            )
            .unwrap();
            assert!(matches!(
                prepared.apply(&changed),
                Err(StabilizerError::IncompatibleBoundaryMap)
            ));
            if width > 0 {
                let mut changed = columns.clone();
                let bit = changed[0].get(0).unwrap();
                changed[0].set(0, !bit).unwrap();
                let changed = AffineKet::new(
                    BitVector::zeros(width).unwrap(),
                    changed,
                    zero.clone(),
                    ExactScalar::zero(),
                )
                .unwrap();
                assert!(matches!(
                    prepared.apply(&changed),
                    Err(StabilizerError::IncompatibleBoundaryMap)
                ));
            }
        }
        assert!(
            PreparedLinearBoundary::new(
                &AffineKet::new(
                    BitVector::zeros(width).unwrap(),
                    columns,
                    QuadraticForm::new(0, vec![0; 6], vec![BitVector::zeros(6).unwrap(); 6])
                        .unwrap(),
                    ExactScalar::one()
                )
                .unwrap(),
                &[BitVector::zeros(width + 1).unwrap()]
            )
            .is_err()
        );
    }
}
