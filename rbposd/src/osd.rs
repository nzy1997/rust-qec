use crate::error::DecodeError;
use crate::gf2::PreparedLinearSystem;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub(crate) struct OsdWorkspace {
    column_order: Vec<usize>,
    prepared: PreparedLinearSystem,
    num_checks: usize,
    num_bits: usize,
}

impl OsdWorkspace {
    pub(crate) fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            column_order: (0..pcm.num_bits()).collect(),
            prepared: PreparedLinearSystem::from_pcm(pcm),
            num_checks: pcm.num_checks(),
            num_bits: pcm.num_bits(),
        }
    }

    pub(crate) fn sort_unreliable_columns(&mut self, reliability: &[f64]) -> &[usize] {
        debug_assert_eq!(reliability.len(), self.num_bits);
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

    fn solve_residual_by_unreliability(
        &mut self,
        target_syndrome: &Syndrome,
        reliability: &[f64],
    ) -> Result<Correction, DecodeError> {
        debug_assert_eq!(target_syndrome.len(), self.num_checks);
        self.sort_unreliable_columns(reliability);
        self.prepared
            .solve_with_column_order(target_syndrome, &self.column_order)
    }
}

pub(crate) fn decode_osd0_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
) -> Result<Correction, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(reliability.len(), pcm.num_bits());
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    let residual = workspace
        .solve_residual_by_unreliability(&target_syndrome, reliability)
        .map_err(|_| DecodeError::NoOsdSolution)?;
    Ok(xor_correction_bits(base_correction_bits, &residual))
}

fn multiply_bits(pcm: &ParityCheckMatrix, bits: &[bool]) -> Syndrome {
    let mut syndrome = vec![false; pcm.num_checks()];
    for (check, value) in syndrome.iter_mut().enumerate() {
        let mut parity = false;
        for &bit in pcm.row_neighbors(check) {
            parity ^= bits[bit];
        }
        *value = parity;
    }
    Syndrome::from(syndrome)
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

fn xor_correction_bits(lhs: &[bool], rhs: &Correction) -> Correction {
    Correction::from(
        lhs.iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{decode_osd0_with_workspace, OsdWorkspace};

    #[test]
    fn decode_osd0_with_workspace_prefers_the_lower_reliability_pivot_basis() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let base = Correction::from(vec![false, false, false]);
        let reliability = vec![1.0, 1.0, 2.0];
        let mut workspace = OsdWorkspace::new(&pcm);

        let correction = decode_osd0_with_workspace(
            &pcm,
            &syndrome,
            base.as_slice(),
            &reliability,
            &mut workspace,
        )
        .unwrap();

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
