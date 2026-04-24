use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) fn sort_columns_by_reliability(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    order
}

pub(crate) fn solve_with_column_order(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    column_order: &[usize],
) -> Result<Correction, DecodeError> {
    let mut matrix = pcm.dense_rows();
    let mut rhs = syndrome.as_slice().to_vec();
    let mut pivot_columns = Vec::new();
    let mut row = 0usize;

    for (pivot_position, &column) in column_order.iter().enumerate() {
        if row == matrix.len() {
            break;
        }
        let pivot = (row..matrix.len()).find(|&candidate| matrix[candidate][column]);
        if let Some(pivot_row) = pivot {
            matrix.swap(row, pivot_row);
            rhs.swap(row, pivot_row);
            for other in 0..matrix.len() {
                if other != row && matrix[other][column] {
                    for c in pivot_position..column_order.len() {
                        let physical = column_order[c];
                        matrix[other][physical] ^= matrix[row][physical];
                    }
                    rhs[other] ^= rhs[row];
                }
            }
            pivot_columns.push(column);
            row += 1;
        }
    }

    if rhs.iter().skip(row).any(|&bit| bit) {
        return Err(DecodeError::SingularSystem);
    }

    let mut solution = vec![false; pcm.num_bits()];
    for (pivot_row, &column) in pivot_columns.iter().enumerate().rev() {
        let mut value = rhs[pivot_row];
        for later_column in pivot_columns.iter().skip(pivot_row + 1) {
            value ^= matrix[pivot_row][*later_column] && solution[*later_column];
        }
        solution[column] = value;
    }

    Ok(Correction::from(solution))
}

#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::Syndrome;

    use super::{solve_with_column_order, sort_columns_by_reliability};

    #[test]
    fn reliability_sort_is_stable_for_equal_scores() {
        let order = sort_columns_by_reliability(&[0.9, 0.9, 0.4, 0.9]);
        assert_eq!(order, vec![0, 1, 3, 2]);
    }

    #[test]
    fn solve_with_column_order_returns_a_valid_solution() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let order = vec![0, 1, 2];

        let correction = solve_with_column_order(&pcm, &syndrome, &order).unwrap();

        assert_eq!(pcm.multiply(&correction), syndrome);
    }

    #[test]
    fn solve_with_column_order_reordered_returns_a_valid_solution() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let order = vec![2, 0, 1];

        let correction = solve_with_column_order(&pcm, &syndrome, &order).unwrap();

        assert_eq!(pcm.multiply(&correction), syndrome);
    }
}
