use crate::error::{QecError, Result};
use crate::gf2::{self, BinaryRow};

fn validate_symplectic_width(width: usize) -> Result<usize> {
    if width % 2 != 0 {
        return Err(QecError::InvalidSymplecticRowWidth { width });
    }

    Ok(width / 2)
}

fn validate_row(row: &[u8]) -> Result<usize> {
    let qubits = validate_symplectic_width(row.len())?;
    gf2::validate_target(row)?;
    Ok(qubits)
}

pub(crate) fn dual_row(row: &[u8]) -> Result<BinaryRow> {
    let qubits = validate_row(row)?;
    let (x, z) = row.split_at(qubits);
    let mut dual = Vec::with_capacity(row.len());
    dual.extend_from_slice(z);
    dual.extend_from_slice(x);
    Ok(dual)
}

pub(crate) fn symplectic_product(lhs: &[u8], rhs: &[u8]) -> Result<u8> {
    validate_row(lhs)?;
    validate_row(rhs)?;

    if lhs.len() != rhs.len() {
        return Err(QecError::RowWidthMismatch {
            expected: lhs.len(),
            actual: rhs.len(),
        });
    }

    let dual = dual_row(lhs)?;
    Ok(dual
        .iter()
        .zip(rhs)
        .fold(0, |parity, (left, right)| parity ^ (*left & *right)))
}

pub(crate) fn commutes(lhs: &[u8], rhs: &[u8]) -> Result<bool> {
    Ok(symplectic_product(lhs, rhs)? == 0)
}

#[allow(dead_code)]
pub(crate) fn commutation_constraints(rows: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    let width = gf2::validate_rows(rows)?;
    commutation_constraints_with_width(rows, width)
}

#[allow(dead_code)]
pub(crate) fn commutation_constraints_with_width(
    rows: &[BinaryRow],
    width: usize,
) -> Result<Vec<BinaryRow>> {
    validate_symplectic_width(width)?;
    gf2::validate_rows_with_width(rows, width)?;
    rows.iter().map(|row| dual_row(row)).collect()
}

#[cfg(test)]
mod tests {
    use crate::error::QecError;

    use super::{
        commutation_constraints, commutation_constraints_with_width, commutes, dual_row,
        symplectic_product,
    };

    fn dot(lhs: &[u8], rhs: &[u8]) -> u8 {
        lhs.iter()
            .zip(rhs)
            .fold(0, |parity, (left, right)| parity ^ (*left & *right))
    }

    #[test]
    fn dual_row_swaps_x_and_z_halves() {
        assert_eq!(
            dual_row(&[1, 0, 1, 0, 1, 1]).unwrap(),
            vec![0, 1, 1, 1, 0, 1]
        );
    }

    #[test]
    fn dual_row_rejects_odd_width_rows() {
        assert_eq!(
            dual_row(&[1, 0, 1]),
            Err(QecError::InvalidSymplecticRowWidth { width: 3 })
        );
    }

    #[test]
    fn symplectic_product_matches_standard_overlap_parity() {
        let lhs = [1, 0, 1, 0, 1, 1];
        let rhs = [0, 0, 0, 1, 1, 0];

        assert_eq!(symplectic_product(&lhs, &rhs), Ok(1));
    }

    #[test]
    fn commutes_reports_commutation_from_symplectic_product() {
        let lhs = [1, 0, 1, 0, 1, 1];
        let anticommutes = [0, 0, 0, 1, 1, 0];
        let commuting = [1, 0, 0, 0, 1, 0];

        assert_eq!(commutes(&lhs, &anticommutes), Ok(false));
        assert_eq!(commutes(&lhs, &commuting), Ok(true));
    }

    #[test]
    fn commutation_constraints_dualize_rows_for_linear_checks() {
        let stabilizers = vec![vec![1, 0, 0, 0], vec![0, 0, 1, 0]];
        let constraints = commutation_constraints(&stabilizers).unwrap();
        let commuting_candidate = [0, 1, 0, 1];
        let anticommutes = [0, 0, 1, 0];

        assert_eq!(constraints, vec![vec![0, 0, 1, 0], vec![1, 0, 0, 0]]);
        assert!(
            constraints
                .iter()
                .all(|row| dot(row, &commuting_candidate) == 0)
        );
        assert_eq!(dot(&constraints[0], &anticommutes), 1);
    }

    #[test]
    fn commutation_constraints_reject_odd_width_rows() {
        assert_eq!(
            commutation_constraints(&[vec![1, 0, 1]]),
            Err(QecError::InvalidSymplecticRowWidth { width: 3 })
        );
    }

    #[test]
    fn commutation_constraints_with_width_accept_empty_rows_at_known_even_width() {
        assert_eq!(commutation_constraints_with_width(&[], 4), Ok(Vec::new()));
    }

    #[test]
    fn commutation_constraints_with_width_reject_odd_width() {
        assert_eq!(
            commutation_constraints_with_width(&[], 3),
            Err(QecError::InvalidSymplecticRowWidth { width: 3 })
        );
    }

    #[test]
    fn commutation_constraints_with_width_reject_row_width_mismatch() {
        assert_eq!(
            commutation_constraints_with_width(&[vec![1, 0]], 4),
            Err(QecError::RowWidthMismatch {
                expected: 4,
                actual: 2,
            })
        );
    }
}
