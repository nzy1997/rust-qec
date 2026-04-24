use crate::error::DecodeError;
use crate::gf2::{solve_with_column_order, sort_columns_by_reliability};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) fn decode_osd0(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    reliability: &[f64],
) -> Result<Correction, DecodeError> {
    let column_order = sort_columns_by_reliability(reliability);
    solve_with_column_order(pcm, syndrome, &column_order).map_err(|_| DecodeError::NoOsdSolution)
}
