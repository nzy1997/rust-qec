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

#[test]
fn tall_systems_reuse_an_independent_rhs_basis_without_losing_dependent_checks() {
    // Thirty thousand equations over three bits previously allocated a square
    // 30,000-bit provenance matrix. Every RHS is still verified exactly.
    let rows = (0..30_000)
        .map(|i| {
            let mask = ((i * 5 + 3) ^ (i / 8)) & 7;
            BitVector::from_indices(3, (0..3).filter(|j| mask & (1 << j) != 0)).unwrap()
        })
        .collect::<Vec<_>>();
    let factor = LinearFactorization::new(3, &rows).unwrap();
    assert_eq!(factor.rank(), 3);
    assert_eq!(factor.num_equations(), 30_000);
    assert!(factor.nullspace().unwrap().is_empty());
    for mask in 0..8 {
        let truth = BitVector::from_indices(3, (0..3).filter(|j| mask & (1 << j) != 0)).unwrap();
        // A literal three-bit dot product does not use the solver's elimination.
        let rhs = BitVector::from_indices(
            rows.len(),
            rows.iter().enumerate().filter_map(|(i, row)| {
                let parity = (0..3)
                    .filter(|&j| row.get(j).unwrap() && mask & (1 << j) != 0)
                    .count()
                    & 1;
                (parity != 0).then_some(i)
            }),
        )
        .unwrap();
        assert_eq!(factor.solve(&rhs).unwrap(), Some(truth));
        for altered in [17, 129, 29_999] {
            let mut inconsistent = rhs.clone();
            inconsistent
                .set(altered, !inconsistent.get(altered).unwrap())
                .unwrap();
            assert_eq!(factor.solve(&inconsistent).unwrap(), None);
        }
    }
    let zeros = LinearFactorization::new(0, &vec![BitVector::zeros(0).unwrap(); 100]).unwrap();
    assert_eq!(
        zeros.solve(&BitVector::zeros(100).unwrap()).unwrap(),
        Some(BitVector::zeros(0).unwrap())
    );
    assert_eq!(
        zeros
            .solve(&BitVector::from_indices(100, [99]).unwrap())
            .unwrap(),
        None
    );
}
#[test]
fn tall_rank_deficient_cross_word_systems_preserve_the_entire_solution_set() {
    for width in [65, 129] {
        let axes = [0, width / 2, width - 1];
        let rows = (0..(width * 3 + 7))
            .map(|i| {
                BitVector::from_indices(
                    width,
                    axes.iter()
                        .enumerate()
                        .filter_map(|(j, &q)| (i & (1 << j) != 0).then_some(q)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let factor = LinearFactorization::new(width, &rows).unwrap();
        assert_eq!(factor.rank(), 3);
        let kernel = factor.nullspace().unwrap();
        assert_eq!(kernel.len(), width - 3);
        for v in &kernel {
            assert!(rows.iter().all(|row| !row.dot(v).unwrap()));
        }
        let truth = BitVector::from_indices(width, [axes[0], axes[2]]).unwrap();
        let rhs = BitVector::from_indices(
            rows.len(),
            rows.iter().enumerate().filter_map(|(i, row)| {
                (row.get(axes[0]).unwrap() ^ row.get(axes[2]).unwrap()).then_some(i)
            }),
        )
        .unwrap();
        let particular = factor.solve(&rhs).unwrap().unwrap();
        assert_eq!(particular, truth);
        for direction in kernel {
            let mut solution = particular.clone();
            solution.xor_assign(&direction).unwrap();
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(row.dot(&solution).unwrap(), rhs.get(i).unwrap());
            }
        }
        let mut bad = rhs.clone();
        bad.set(0, true).unwrap();
        assert_eq!(factor.solve(&bad).unwrap(), None);
    }
}

#[test]
fn tall_full_rank_cross_word_systems_handle_pivot_swaps_and_dense_rhs_updates() {
    for width in [65, 129] {
        let count = 3 * width + 7;
        let mut rows = (0..count)
            .map(|i| {
                if i < width {
                    BitVector::from_indices(
                        width,
                        [i].into_iter().chain((i + 1 < width).then_some(i + 1)),
                    )
                    .unwrap()
                } else {
                    BitVector::from_indices(
                        width,
                        (0..width).filter(|&j| ((i * 131 + j * 17) ^ (j * j + i)) % 5 < 2),
                    )
                    .unwrap()
                }
            })
            .collect::<Vec<_>>();
        rows.reverse();
        let factor = LinearFactorization::new(width, &rows).unwrap();
        assert_eq!(factor.rank(), width);
        for residue in 0..5 {
            let truth =
                BitVector::from_indices(width, (0..width).filter(|j| j % 5 == residue)).unwrap();
            let rhs = BitVector::from_indices(
                count,
                rows.iter().enumerate().filter_map(|(i, row)| {
                    let parity = (0..width)
                        .filter(|&j| row.get(j).unwrap() && j % 5 == residue)
                        .count()
                        % 2;
                    (parity != 0).then_some(i)
                }),
            )
            .unwrap();
            assert_eq!(factor.solve(&rhs).unwrap(), Some(truth));
            let mut contradictory = rhs.clone();
            contradictory.set(0, !rhs.get(0).unwrap()).unwrap();
            assert_eq!(factor.solve(&contradictory).unwrap(), None);
        }
    }
}
