use crate::error::{QecError, Result};

pub(crate) type BinaryRow = Vec<u8>;

#[cfg(test)]
mod tests {
    use super::{try_nullspace_basis, try_select_independent_rows};

    fn dot(lhs: &[u8], rhs: &[u8]) -> u8 {
        lhs.iter()
            .zip(rhs)
            .fold(0, |parity, (left, right)| parity ^ (*left & *right))
    }

    #[test]
    fn nullspace_basis_annihilates_every_constraint_row() {
        let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
        let basis = try_nullspace_basis(&matrix).unwrap();

        assert_eq!(basis.len(), 2);
        assert!(basis.iter().all(|row| row.len() == 4));
        for row in &matrix {
            for vector in &basis {
                assert_eq!(dot(row, vector), 0);
            }
        }
    }

    #[test]
    fn select_independent_rows_drops_dependent_generators() {
        let rows = vec![vec![1, 0, 1], vec![0, 1, 1], vec![1, 1, 0]];

        assert_eq!(
            try_select_independent_rows(&rows).unwrap(),
            vec![vec![1, 0, 1], vec![0, 1, 1]]
        );
    }
}
