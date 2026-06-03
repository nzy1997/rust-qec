use crate::error::DecodeError;
use crate::gf2::{solve_with_column_order, sort_columns_by_unreliability};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) fn decode_osd0(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    reliability: &[f64],
) -> Result<Correction, DecodeError> {
    let column_order = sort_columns_by_unreliability(reliability);
    solve_with_column_order(pcm, syndrome, &column_order).map_err(|_| DecodeError::NoOsdSolution)
}

#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::decode_osd0;

    #[test]
    fn decode_osd0_prefers_the_lower_reliability_pivot_basis() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let reliability = vec![1.0, 1.0, 2.0];

        let correction = decode_osd0(&pcm, &syndrome, &reliability).unwrap();

        assert_eq!(correction, Correction::from(vec![false, true, false]));
    }
}
