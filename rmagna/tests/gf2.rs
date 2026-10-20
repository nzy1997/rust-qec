use rmagna::gf2::{BitVector, Gf2Error, LinearFactorization};
use std::collections::BTreeSet;

#[test]
fn every_small_binary_system_matches_literal_solution_enumeration() {
    for width in 0_usize..=3 {
        for equations in 0_usize..=3 {
            for matrix in 0_u64..1_u64 << (width * equations) {
                let masks = (0..equations)
                    .map(|i| (matrix >> (i * width)) & ((1 << width) - 1))
                    .collect::<Vec<_>>();
                let rows = masks
                    .iter()
                    .map(|&row| {
                        BitVector::from_words(width, if width == 0 { vec![] } else { vec![row] })
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let factor = LinearFactorization::new(width, &rows).unwrap();
                assert_eq!(factor.num_equations(), equations);
                assert_eq!(factor.num_variables(), width);
                let basis = factor.nullspace().unwrap();
                assert_eq!(basis.len() + factor.rank(), width);
                for rhs_mask in 0_u64..1_u64 << equations {
                    let rhs = BitVector::from_words(
                        equations,
                        if equations == 0 {
                            vec![]
                        } else {
                            vec![rhs_mask]
                        },
                    )
                    .unwrap();
                    let expected = (0_u64..1_u64 << width)
                        .filter(|&x| {
                            masks.iter().enumerate().all(|(i, &row)| {
                                (row & x).count_ones() % 2 == ((rhs_mask >> i) & 1) as u32
                            })
                        })
                        .collect::<BTreeSet<_>>();
                    let solved = factor.solve(&rhs).unwrap();
                    if expected.is_empty() {
                        assert!(solved.is_none());
                        continue;
                    }
                    let solution = solved.unwrap();
                    let base = solution.words().first().copied().unwrap_or(0);
                    let actual = (0_usize..1 << basis.len())
                        .map(|mask| {
                            basis
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| mask & (1 << i) != 0)
                                .fold(base, |acc, (_, row)| {
                                    acc ^ row.words().first().copied().unwrap_or(0)
                                })
                        })
                        .collect::<BTreeSet<_>>();
                    assert_eq!(
                        actual, expected,
                        "width={width} equations={equations} matrix={matrix} rhs={rhs_mask}"
                    );
                    assert_eq!(actual.len(), 1 << basis.len());
                }
            }
        }
    }
}

#[test]
fn packed_operations_preserve_cross_word_indices_and_padding() {
    for width in [0, 1, 63, 64, 65, 127, 128, 129, 4097] {
        let indices = [0, 1, 62, 63, 64, 65, 126, 127, 128, 4096]
            .into_iter()
            .filter(|&i| i < width)
            .collect::<Vec<_>>();
        let a = BitVector::from_indices(width, indices.iter().copied()).unwrap();
        assert_eq!(a.ones().collect::<Vec<_>>(), indices);
        assert_eq!(a.count_ones(), indices.len());
        assert_eq!(a.dot(&a).unwrap(), indices.len() % 2 == 1);
        for i in 0..width {
            assert_eq!(a.get(i).unwrap(), indices.contains(&i));
        }
        let mut cancelled = a.clone();
        cancelled.xor_assign(&a).unwrap();
        assert!(cancelled.is_zero());
        let before = a.clone();
        let mut modified = a;
        assert!(modified.set(width, true).is_err());
        assert_eq!(before, modified);
    }
}

#[test]
fn wide_repeated_solves_preserve_dependent_equation_constraints() {
    let width = 4097;
    let rows = vec![
        BitVector::from_indices(width, [0, 64, 128, 4096]).unwrap(),
        BitVector::from_indices(width, [63, 64, 127]).unwrap(),
        BitVector::from_indices(width, [0, 63, 127, 128, 4096]).unwrap(),
        BitVector::zeros(width).unwrap(),
    ];
    let factor = LinearFactorization::new(width, &rows).unwrap();
    assert_eq!(factor.rank(), 2);
    let basis = factor.nullspace().unwrap();
    assert_eq!(basis.len(), 4095);
    for vector in &basis {
        for row in &rows {
            assert!(!row.dot(vector).unwrap());
        }
    }
    let independent = LinearFactorization::new(width, &basis).unwrap();
    assert_eq!(independent.rank(), basis.len());
    for rhs_mask in 0..16 {
        let rhs = BitVector::from_words(4, vec![rhs_mask]).unwrap();
        let consistent =
            rhs_mask & 8 == 0 && ((rhs_mask & 1) ^ ((rhs_mask >> 1) & 1)) == (rhs_mask >> 2) & 1;
        let solution = factor.solve(&rhs).unwrap();
        assert_eq!(solution.is_some(), consistent);
        if let Some(solution) = solution {
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(row.dot(&solution).unwrap(), rhs_mask & (1 << i) != 0);
            }
        }
    }
}

#[test]
fn malformed_dimensions_and_nonzero_padding_are_rejected() {
    assert_eq!(
        BitVector::from_words(65, vec![0]),
        Err(Gf2Error::InvalidWordCount)
    );
    assert_eq!(
        BitVector::from_words(65, vec![0, 2]),
        Err(Gf2Error::NonzeroPadding)
    );
    assert_eq!(
        BitVector::from_words(0, vec![0]),
        Err(Gf2Error::InvalidWordCount)
    );
    assert!(BitVector::from_indices(64, [64]).is_err());
    let mut a = BitVector::from_indices(64, [0, 63]).unwrap();
    let before = a.clone();
    assert!(a.xor_assign(&BitVector::zeros(65).unwrap()).is_err());
    assert_eq!(a, before);
    assert!(a.dot(&BitVector::zeros(63).unwrap()).is_err());
    assert!(LinearFactorization::new(65, &[a.clone()]).is_err());
    let factor = LinearFactorization::new(64, &[a]).unwrap();
    assert!(factor.solve(&BitVector::zeros(2).unwrap()).is_err());
}
