use qec_code::binary_chain_complex::{BinaryBoundaryMap, BinaryChainComplex};
use qec_code::sparse_gf2::SparseGf2Matrix;
use qec_code::QecError;

fn square_complex(face_boundary: Vec<usize>) -> Result<BinaryChainComplex, QecError> {
    let boundary_1 = BinaryBoundaryMap::new(
        1,
        0,
        SparseGf2Matrix::new(4, 4, vec![vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]])?,
    )?;
    let boundary_2 = BinaryBoundaryMap::new(
        2,
        1,
        SparseGf2Matrix::new(4, 1, face_rows(4, face_boundary))?,
    )?;

    BinaryChainComplex::new(vec![boundary_2, boundary_1])
}

fn face_rows(num_edges: usize, face_boundary: Vec<usize>) -> Vec<Vec<usize>> {
    let mut rows = vec![Vec::new(); num_edges];
    for edge in face_boundary {
        rows[edge].push(0);
    }
    rows
}

fn rows_are_orthogonal(hx: &SparseGf2Matrix, hz: &SparseGf2Matrix) -> bool {
    hx.rows().iter().all(|x_row| {
        hz.rows()
            .iter()
            .all(|z_row| sparse_dot_mod_2(x_row, z_row) == 0)
    })
}

fn sparse_dot_mod_2(left: &[usize], right: &[usize]) -> usize {
    let mut parity = 0;
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                parity ^= 1;
                left_index += 1;
                right_index += 1;
            }
        }
    }
    parity
}

#[test]
fn square_cell_boundary_maps_match_fixture() {
    let complex = square_complex(vec![0, 1, 2, 3]).unwrap();

    let ordered_dimensions = complex
        .boundaries()
        .iter()
        .map(BinaryBoundaryMap::domain_dimension)
        .collect::<Vec<_>>();
    assert_eq!(ordered_dimensions, vec![1, 2]);

    let boundary_1_map = complex.boundary_map(1).unwrap();
    assert_eq!(boundary_1_map.domain_dimension(), 1);
    assert_eq!(boundary_1_map.codomain_dimension(), 0);
    assert_eq!(boundary_1_map.num_domain_cells(), 4);
    assert_eq!(boundary_1_map.num_codomain_cells(), 4);
    assert_eq!(
        boundary_1_map.matrix().rows(),
        &[vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]]
    );

    let boundary_1 = complex.boundary(1).unwrap();
    assert_eq!(boundary_1.num_rows(), 4);
    assert_eq!(boundary_1.num_cols(), 4);
    assert_eq!(
        boundary_1.rows(),
        &[vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]]
    );

    let boundary_2 = complex.boundary(2).unwrap();
    assert_eq!(boundary_2.num_rows(), 4);
    assert_eq!(boundary_2.num_cols(), 1);
    assert_eq!(boundary_2.rows(), &[vec![0], vec![0], vec![0], vec![0]]);

    let css = complex.css_view(1).unwrap();
    assert_eq!(css.qubit_dimension(), 1);
    assert_eq!(css.num_qubits(), 4);
    assert_eq!(css.num_x_checks(), 4);
    assert_eq!(css.num_z_checks(), 1);
    assert_eq!(
        css.hx().rows(),
        &[vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]]
    );
    assert_eq!(css.hz().rows(), &[vec![0, 1, 2, 3]]);
    assert!(rows_are_orthogonal(css.hx(), css.hz()));
}

#[test]
fn chain_complex_orders_nonconsecutive_boundaries_without_composing() {
    let boundary_1 =
        BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(1, 3, vec![vec![]]).unwrap()).unwrap();
    let boundary_3 = BinaryBoundaryMap::new(
        3,
        2,
        SparseGf2Matrix::new(2, 1, vec![vec![0], vec![0]]).unwrap(),
    )
    .unwrap();

    let complex = BinaryChainComplex::new(vec![boundary_3, boundary_1]).unwrap();
    let ordered_dimensions = complex
        .boundaries()
        .iter()
        .map(BinaryBoundaryMap::domain_dimension)
        .collect::<Vec<_>>();

    assert_eq!(ordered_dimensions, vec![1, 3]);
    assert!(complex.boundary(2).is_none());
}

#[test]
fn corrupt_face_boundary_is_rejected() {
    assert_eq!(
        square_complex(vec![0, 1, 2]),
        Err(QecError::NonzeroBoundaryComposition {
            lower_dimension: 1,
            upper_dimension: 2,
            row: 0,
            support: vec![0],
        })
    );
}

#[test]
fn boundary_maps_reject_invalid_cell_dimensions() {
    assert_eq!(
        BinaryBoundaryMap::new(2, 0, SparseGf2Matrix::new(1, 1, vec![vec![0]]).unwrap(),),
        Err(QecError::InvalidBoundaryMapDimensions {
            domain_dimension: 2,
            codomain_dimension: 0,
        })
    );
}

#[test]
fn chain_complex_rejects_duplicate_boundary_dimensions() {
    let boundary_a =
        BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(0, 0, vec![]).unwrap()).unwrap();
    let boundary_b =
        BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(0, 0, vec![]).unwrap()).unwrap();

    assert_eq!(
        BinaryChainComplex::new(vec![boundary_a, boundary_b]),
        Err(QecError::DuplicateBoundaryMapDimension {
            domain_dimension: 1,
        })
    );
}

#[test]
fn chain_complex_reports_composition_shape_mismatch() {
    let lower =
        BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(1, 3, vec![vec![]]).unwrap()).unwrap();
    let upper = BinaryBoundaryMap::new(
        2,
        1,
        SparseGf2Matrix::new(2, 1, vec![vec![], vec![]]).unwrap(),
    )
    .unwrap();

    assert_eq!(
        BinaryChainComplex::new(vec![lower, upper]),
        Err(QecError::BoundaryCompositionDimensionMismatch {
            lower_dimension: 1,
            upper_dimension: 2,
            lower_domain_cells: 3,
            upper_codomain_cells: 2,
        })
    );
}

#[test]
fn css_view_reports_missing_boundary_maps() {
    let boundary_1 =
        BinaryBoundaryMap::new(1, 0, SparseGf2Matrix::new(1, 1, vec![vec![0]]).unwrap()).unwrap();
    let only_boundary_1 = BinaryChainComplex::new(vec![boundary_1]).unwrap();
    assert_eq!(
        only_boundary_1.css_view(1),
        Err(QecError::MissingBoundaryMap {
            domain_dimension: 2
        })
    );

    let boundary_2 =
        BinaryBoundaryMap::new(2, 1, SparseGf2Matrix::new(1, 1, vec![vec![0]]).unwrap()).unwrap();
    let only_boundary_2 = BinaryChainComplex::new(vec![boundary_2]).unwrap();
    assert_eq!(
        only_boundary_2.css_view(1),
        Err(QecError::MissingBoundaryMap {
            domain_dimension: 1
        })
    );
}
