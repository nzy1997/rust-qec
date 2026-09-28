use qec_code::Pauli;
use qec_code::phased_pauli::{Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup};

fn pauli(x: &[u8], z: &[u8], phase: Phase) -> PhasedPauli {
    PhasedPauli::new(Pauli::from_xz_bits(x.to_vec(), z.to_vec()).unwrap(), phase)
}

fn single(letter: char) -> PhasedPauli {
    match letter {
        'I' => pauli(&[0], &[0], Phase::PlusOne),
        'X' => pauli(&[1], &[0], Phase::PlusOne),
        'Y' => pauli(&[1], &[1], Phase::PlusI),
        'Z' => pauli(&[0], &[1], Phase::PlusOne),
        _ => panic!("unknown Pauli letter"),
    }
}

#[test]
fn exact_phase_identities_and_adjoint_hold() {
    assert_eq!(Phase::from_exponent(5), Phase::PlusI);
    let x = single('X');
    let y = single('Y');
    let z = single('Z');
    assert_eq!(x.multiply(&y).unwrap(), pauli(&[0], &[1], Phase::PlusI));
    assert_eq!(y.multiply(&x).unwrap(), pauli(&[0], &[1], Phase::MinusI));
    assert_eq!(y.multiply(&y).unwrap(), PhasedPauli::identity(1));
    assert!(y.is_hermitian());
    assert!(!pauli(&[1], &[1], Phase::PlusOne).is_hermitian());
    assert_eq!(x.multiply(&z).unwrap().adjoint(), z.multiply(&x).unwrap());
    assert_eq!(x.multiply(&y).unwrap().multiply(&y.inverse()).unwrap(), x);
    assert_eq!(
        x.multiply(&PhasedPauli::identity(2)),
        Err(PhaseAlgebraError::WidthMismatch {
            expected: 1,
            actual: 2,
        })
    );
}

#[test]
fn signed_bell_group_membership_and_witness_are_exact() {
    let xx = pauli(&[1, 1], &[0, 0], Phase::PlusOne);
    let zz = pauli(&[0, 0], &[1, 1], Phase::PlusOne);
    let minus_yy = pauli(&[1, 1], &[1, 1], Phase::PlusOne);
    let group = SignedStabilizerGroup::new(2, vec![xx.clone(), zz.clone()]).unwrap();
    assert_eq!(group.rank(), 2);
    for expected in [
        PhasedPauli::identity(2),
        xx.clone(),
        zz.clone(),
        minus_yy.clone(),
    ] {
        let witness = group.contains_with_witness(&expected).unwrap().unwrap();
        assert!(
            witness
                .generator_indices()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert_eq!(
            group
                .product_from_indices(witness.generator_indices())
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        group.contains_with_witness(&pauli(&[1, 1], &[1, 1], Phase::MinusOne)),
        Ok(None)
    );
    assert_eq!(
        group.contains_with_witness(&pauli(&[1, 0], &[0, 0], Phase::PlusOne)),
        Ok(None)
    );
    assert_eq!(
        group.contains_with_witness(&pauli(&[1, 1], &[1, 1], Phase::PlusI)),
        Ok(None)
    );
    assert_eq!(
        group.product_from_indices(&[2]),
        Err(PhaseAlgebraError::WitnessIndex { index: 2, count: 2 })
    );
}

#[test]
fn redundant_signed_checks_are_validated_before_elimination() {
    let xx = pauli(&[1, 1], &[0, 0], Phase::PlusOne);
    let zz = pauli(&[0, 0], &[1, 1], Phase::PlusOne);
    let minus_yy = pauli(&[1, 1], &[1, 1], Phase::PlusOne);
    let plus_yy = pauli(&[1, 1], &[1, 1], Phase::MinusOne);
    let consistent = SignedStabilizerGroup::new(2, vec![xx.clone(), zz.clone(), minus_yy]).unwrap();
    assert_eq!(consistent.rank(), 2);
    assert_eq!(consistent.generators().len(), 3);
    assert_eq!(
        SignedStabilizerGroup::new(2, vec![xx.clone(), zz.clone(), plus_yy]).unwrap_err(),
        PhaseAlgebraError::ContradictoryGenerator { index: 2 }
    );
    assert_eq!(
        SignedStabilizerGroup::new(1, vec![pauli(&[0], &[0], Phase::MinusOne)]).unwrap_err(),
        PhaseAlgebraError::ContradictoryGenerator { index: 0 }
    );
    assert_eq!(
        SignedStabilizerGroup::new(1, vec![pauli(&[1], &[1], Phase::PlusOne)]).unwrap_err(),
        PhaseAlgebraError::NonHermitianGenerator { index: 0 }
    );
    assert_eq!(
        SignedStabilizerGroup::new(1, vec![single('X'), single('Z')]).unwrap_err(),
        PhaseAlgebraError::NonCommutingGenerators { left: 0, right: 1 }
    );
    assert_eq!(
        SignedStabilizerGroup::new(2, vec![single('X')]).unwrap_err(),
        PhaseAlgebraError::GeneratorWidth {
            index: 0,
            expected: 2,
            actual: 1
        }
    );
}

#[test]
fn caller_logical_signs_representatives_and_order_are_preserved() {
    let zz = pauli(&[0, 0], &[1, 1], Phase::PlusOne);
    let group = SignedStabilizerGroup::new(2, vec![zz]).unwrap();
    let x = pauli(&[1, 1], &[0, 0], Phase::PlusOne);
    let z = pauli(&[0, 0], &[1, 0], Phase::MinusOne);
    let basis = group
        .validate_logical_basis(vec![x.clone()], vec![z.clone()])
        .unwrap();
    assert_eq!(basis.k(), 1);
    assert_eq!(basis.logical_x(), &[x]);
    assert_eq!(basis.logical_z(), &[z]);
}

#[test]
fn invalid_logical_bases_report_the_offending_operator_or_pair() {
    let group =
        SignedStabilizerGroup::new(2, vec![pauli(&[0, 0], &[1, 1], Phase::PlusOne)]).unwrap();
    let x = pauli(&[1, 1], &[0, 0], Phase::PlusOne);
    let z = pauli(&[0, 0], &[1, 0], Phase::PlusOne);
    assert_eq!(
        group.validate_logical_basis(vec![], vec![z.clone()]),
        Err(PhaseAlgebraError::LogicalCount {
            expected: 1,
            x_count: 0,
            z_count: 1
        })
    );
    assert_eq!(
        group.validate_logical_basis(vec![single('X')], vec![z.clone()]),
        Err(PhaseAlgebraError::LogicalWidth {
            kind: "X",
            index: 0,
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(
        group.validate_logical_basis(vec![pauli(&[1, 1], &[0, 0], Phase::PlusI)], vec![z.clone()],),
        Err(PhaseAlgebraError::NonHermitianLogical {
            kind: "X",
            index: 0
        })
    );
    assert_eq!(
        group.validate_logical_basis(
            vec![pauli(&[1, 0], &[0, 0], Phase::PlusOne)],
            vec![z.clone()],
        ),
        Err(PhaseAlgebraError::NonCentralizingLogical {
            kind: "X",
            index: 0,
            generator: 0
        })
    );
    assert_eq!(
        group.validate_logical_basis(vec![x.clone()], vec![x]),
        Err(PhaseAlgebraError::LogicalCommutation {
            left_kind: "X",
            left_index: 0,
            right_kind: "Z",
            right_index: 0,
            expected_anticommutation: true,
        })
    );
}

#[test]
fn non_css_code_and_multi_logical_basis_are_supported() {
    let y_code = SignedStabilizerGroup::new(1, vec![single('Y')]).unwrap();
    assert_eq!(y_code.rank(), 1);
    assert_eq!(
        y_code.validate_logical_basis(vec![], vec![]).unwrap().k(),
        0
    );

    let yi = pauli(&[1, 0], &[1, 0], Phase::PlusI);
    let non_css = SignedStabilizerGroup::new(2, vec![yi]).unwrap();
    let non_css_x = pauli(&[0, 1], &[0, 0], Phase::PlusOne);
    let non_css_z = pauli(&[0, 0], &[0, 1], Phase::PlusOne);
    assert_eq!(
        non_css
            .validate_logical_basis(vec![non_css_x.clone()], vec![non_css_z])
            .unwrap()
            .logical_x(),
        &[non_css_x]
    );

    let group =
        SignedStabilizerGroup::new(3, vec![pauli(&[0, 0, 0], &[1, 1, 0], Phase::PlusOne)]).unwrap();
    let x0 = pauli(&[0, 0, 1], &[0, 0, 0], Phase::PlusOne);
    let z0 = pauli(&[0, 0, 0], &[0, 0, 1], Phase::PlusOne);
    let x1 = pauli(&[1, 1, 0], &[0, 0, 0], Phase::MinusOne);
    let z1 = pauli(&[0, 0, 0], &[1, 0, 0], Phase::PlusOne);
    let basis = group
        .validate_logical_basis(vec![x0.clone(), x1.clone()], vec![z0.clone(), z1.clone()])
        .unwrap();
    assert_eq!(basis.logical_x(), &[x0.clone(), x1.clone()]);
    assert_eq!(basis.logical_z(), &[z0.clone(), z1.clone()]);

    let err = group
        .validate_logical_basis(vec![x0, x1], vec![z1, z0])
        .unwrap_err();
    assert_eq!(
        err,
        PhaseAlgebraError::LogicalCommutation {
            left_kind: "X",
            left_index: 0,
            right_kind: "Z",
            right_index: 0,
            expected_anticommutation: true,
        }
    );
}

// Independent computational-basis matrix oracle: an operator maps |b> to
// i^phase (-1)^(z·b) |b xor x>. This does not use the library multiplication.
type Gaussian = (i32, i32);
type Matrix = Vec<Vec<Gaussian>>;

fn dense(operator: &PhasedPauli) -> Matrix {
    let dim = 1 << operator.n();
    let mut matrix = vec![vec![(0, 0); dim]; dim];
    for column in 0..dim {
        let mut row = column;
        let mut exponent = operator.phase().exponent();
        for qubit in 0..operator.n() {
            if operator.support().x_bits()[qubit] != 0 {
                row ^= 1 << qubit;
            }
            if operator.support().z_bits()[qubit] != 0 && column & (1 << qubit) != 0 {
                exponent = (exponent + 2) % 4;
            }
        }
        matrix[row][column] = match exponent {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (0, -1),
        };
    }
    matrix
}

fn dense_product(left: &Matrix, right: &Matrix) -> Matrix {
    let dim = left.len();
    let mut product = vec![vec![(0, 0); dim]; dim];
    for row in 0..dim {
        for column in 0..dim {
            for middle in 0..dim {
                let (a, b) = left[row][middle];
                let (c, d) = right[middle][column];
                product[row][column].0 += a * c - b * d;
                product[row][column].1 += a * d + b * c;
            }
        }
    }
    product
}

fn dense_adjoint(matrix: &Matrix) -> Matrix {
    let dim = matrix.len();
    let mut adjoint = vec![vec![(0, 0); dim]; dim];
    for row in 0..dim {
        for column in 0..dim {
            let (real, imaginary) = matrix[column][row];
            adjoint[row][column] = (real, -imaginary);
        }
    }
    adjoint
}

fn all_two_qubit_paulis() -> Vec<PhasedPauli> {
    let phases = [Phase::PlusOne, Phase::PlusI, Phase::MinusOne, Phase::MinusI];
    let mut operators = Vec::new();
    for bits in 0..16 {
        let x = [(bits & 1) as u8, ((bits >> 1) & 1) as u8];
        let z = [((bits >> 2) & 1) as u8, ((bits >> 3) & 1) as u8];
        for phase in phases {
            operators.push(pauli(&x, &z, phase));
        }
    }
    operators
}

#[test]
fn all_two_qubit_products_match_independent_dense_matrices() {
    let operators = all_two_qubit_paulis();
    let matrices: Vec<_> = operators.iter().map(dense).collect();
    for (operator, matrix) in operators.iter().zip(&matrices) {
        assert_eq!(dense(&operator.adjoint()), dense_adjoint(matrix));
        assert_eq!(operator.adjoint(), operator.inverse());
    }
    for (left, left_matrix) in operators.iter().zip(&matrices) {
        for (right, right_matrix) in operators.iter().zip(&matrices) {
            let product = left.multiply(right).unwrap();
            assert_eq!(dense(&product), dense_product(left_matrix, right_matrix));
        }
    }
}

#[test]
fn two_qubit_membership_matches_independent_dense_generator_products() {
    let xx = pauli(&[1, 1], &[0, 0], Phase::PlusOne);
    let zz = pauli(&[0, 0], &[1, 1], Phase::PlusOne);
    let group = SignedStabilizerGroup::new(2, vec![xx.clone(), zz.clone()]).unwrap();
    let identity = dense(&PhasedPauli::identity(2));
    let xx_matrix = dense(&xx);
    let zz_matrix = dense(&zz);
    let members = [
        identity,
        xx_matrix.clone(),
        zz_matrix.clone(),
        dense_product(&xx_matrix, &zz_matrix),
    ];
    for candidate in all_two_qubit_paulis() {
        let expected = members.contains(&dense(&candidate));
        assert_eq!(
            group.contains_with_witness(&candidate).unwrap().is_some(),
            expected
        );
    }
}
