//! Optional diagnostics captured from the same resolved task draw.
use super::{TaskError, TaskEvaluation, TaskEvaluator};
use crate::{circuit::BitAtom, gf2::BitVector, noise::RandomSource, runtime::ResolvedHistory};
use std::collections::BTreeMap;

/// Bounds for optional post-resolution aggregation. These limits cover logical
/// packed basis/working-row words, not process RSS, runtime expressions or map
/// metadata. The condition and record limits separately bound that metadata.
#[derive(Clone, Copy, Debug)]
pub struct TaskSampleDiagnosticOptions {
    pub max_record_bits: usize,
    pub max_conditions: usize,
    pub max_basis_words: usize,
}
impl Default for TaskSampleDiagnosticOptions {
    fn default() -> Self {
        Self {
            max_record_bits: 65536,
            max_conditions: 65536,
            max_basis_words: 1 << 20,
        }
    }
}
/// Actual observation coordinates and callback invocations for one draw.
/// Learned rank counts independent record rows, including deterministic rows;
/// it is distinct from probability-query count and history entropy consumption.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskSampleDiagnostics {
    pub learned_record_rank: usize,
    pub protocol_controller_calls: usize,
    pub recovery_controller_calls: usize,
    pub noise_entropy_words: usize,
    pub history_entropy_words: usize,
}
fn error(message: impl ToString) -> TaskError {
    TaskError::new("diagnostics", message.to_string())
}
impl TaskSampleDiagnosticOptions {
    pub(super) fn preflight(&self, width: usize) -> Result<(), TaskError> {
        if width > self.max_record_bits {
            return Err(error(format!(
                "composed record width {width} exceeds diagnostic limit {}",
                self.max_record_bits
            )));
        }
        Ok(())
    }
    pub(super) fn capture(
        &self,
        history: &ResolvedHistory,
        protocol_calls: usize,
    ) -> Result<TaskSampleDiagnostics, TaskError> {
        if history.conditions().len() > self.max_conditions {
            return Err(error(format!(
                "resolved condition count {} exceeds diagnostic limit {}",
                history.conditions().len(),
                self.max_conditions
            )));
        }
        let width = history.instrument().num_records();
        self.preflight(width)?;
        let words = width.div_ceil(64);
        let mut basis = BTreeMap::<usize, BitVector>::new();
        for (expression, _) in history.conditions() {
            let resolved = history.resolve_expression(expression).map_err(error)?;
            // Constants do not add record knowledge or need a packed working row.
            if resolved.terms.is_empty() {
                continue;
            }
            let required = basis
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(words))
                .ok_or_else(|| error("diagnostic packed workspace size overflow"))?;
            if required > self.max_basis_words {
                return Err(error(format!(
                    "diagnostic basis and working row require {required} packed words; limit is {}",
                    self.max_basis_words
                )));
            }
            let mut row = BitVector::zeros(width).map_err(error)?;
            for atom in &resolved.terms {
                match atom {
                    BitAtom::Record(index) => row.set(*index, true).map_err(error)?,
                    BitAtom::Decision { .. } => {
                        return Err(error("resolved history retained an unresolved decision"));
                    }
                }
            }
            // Incremental highest-pivot elimination retains no provenance matrix.
            while let Some(pivot) = row.words().iter().enumerate().rev().find_map(|(i, &word)| {
                (word != 0).then(|| i * 64 + 63 - word.leading_zeros() as usize)
            }) {
                if let Some(known) = basis.get(&pivot) {
                    row.xor_assign(known).map_err(error)?;
                } else {
                    basis.insert(pivot, row);
                    break;
                }
            }
        }
        let protocol = history
            .controller_calls()
            .iter()
            .filter(|call| call.call < protocol_calls)
            .count();
        let (noise, history_words) = history.entropy_words();
        Ok(TaskSampleDiagnostics {
            learned_record_rank: basis.len(),
            protocol_controller_calls: protocol,
            recovery_controller_calls: history.controller_calls().len() - protocol,
            noise_entropy_words: noise,
            history_entropy_words: history_words,
        })
    }
}
impl TaskEvaluator {
    /// Resolve and evaluate exactly one ordinary draw, adding optional diagnostics
    /// without resampling, callback replay or an altered RNG stream. Width limits
    /// fail before entropy or callbacks. Condition/workspace limits are checked
    /// after history resolution, so a failure may retain those ordinary effects;
    /// the method never retries. Integrated linear tasks have rank and calls zero.
    pub fn sample_with_diagnostics(
        &self,
        random: &mut impl RandomSource,
        options: &TaskSampleDiagnosticOptions,
    ) -> Result<(TaskEvaluation, TaskSampleDiagnostics), TaskError> {
        let (evaluation, diagnostics) = self.sample_observed_inner(random, None, Some(options))?;
        Ok((
            evaluation,
            diagnostics.expect("requested diagnostics captured on both execution routes"),
        ))
    }
}
