use crate::error::DecodeError;
use crate::gf2::PreparedLinearSystem;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[cfg(test)]
use crate::gf2::{solve_with_column_order, sort_columns_by_unreliability};

#[derive(Debug)]
pub(crate) struct OsdWorkspace {
    column_order: Vec<usize>,
    prepared: PreparedLinearSystem,
}

impl OsdWorkspace {
    pub(crate) fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            column_order: (0..pcm.num_bits()).collect(),
            prepared: PreparedLinearSystem::from_pcm(pcm),
        }
    }

    pub(crate) fn sort_unreliable_columns(&mut self, reliability: &[f64]) -> &[usize] {
        self.column_order.clear();
        self.column_order.extend(0..reliability.len());
        self.column_order.sort_by(|&a, &b| {
            reliability[a]
                .partial_cmp(&reliability[b])
                .unwrap()
                .then_with(|| a.cmp(&b))
        });
        &self.column_order
    }
}

#[cfg(test)]
pub(crate) fn decode_osd0(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction: &Correction,
    reliability: &[f64],
) -> Result<Correction, DecodeError> {
    let target_syndrome = xor_syndromes(&pcm.multiply(base_correction), syndrome);
    let column_order = sort_columns_by_unreliability(reliability);
    let residual =
        solve_with_column_order(pcm, &target_syndrome, &column_order).map_err(|_| {
            DecodeError::NoOsdSolution
        })?;
    Ok(xor_corrections(base_correction, &residual))
}

pub(crate) fn decode_osd0_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction: &Correction,
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
) -> Result<Correction, DecodeError> {
    let target_syndrome = xor_syndromes(&pcm.multiply(base_correction), syndrome);
    let column_order = workspace.sort_unreliable_columns(reliability).to_vec();
    let residual = workspace
        .prepared
        .solve_with_column_order(&target_syndrome, &column_order)
        .map_err(|_| DecodeError::NoOsdSolution)?;
    Ok(xor_corrections(base_correction, &residual))
}

fn xor_syndromes(lhs: &Syndrome, rhs: &Syndrome) -> Syndrome {
    Syndrome::from(
        lhs.as_slice()
            .iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}

fn xor_corrections(lhs: &Correction, rhs: &Correction) -> Correction {
    Correction::from(
        lhs.as_slice()
            .iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{decode_osd0, OsdWorkspace};

    #[test]
    fn decode_osd0_prefers_the_lower_reliability_pivot_basis() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let base = Correction::from(vec![false, false, false]);
        let reliability = vec![1.0, 1.0, 2.0];

        let correction = decode_osd0(&pcm, &syndrome, &base, &reliability).unwrap();

        assert_eq!(correction, Correction::from(vec![false, true, false]));
    }

    #[test]
    fn osd_workspace_orders_columns_by_unreliability_stably() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let mut workspace = OsdWorkspace::new(&pcm);

        let order = workspace.sort_unreliable_columns(&[1.0, 1.0, 0.4]);

        assert_eq!(order, &[2, 0, 1]);
    }
}
