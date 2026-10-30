//! Owned side kets and their physically traced, inherited record distribution.
use super::{CompiledUnitary, UnitaryError};
use crate::fibers::FiberOptions;
use crate::gf2::BitVector;
use crate::phase::{PhaseResourceKind, PhaseResourceOptions, PreparedPhaseResource};
use crate::records::pair::{
    MassError, PairGeometry as RecordGeometry, evaluate_mass, pair_geometry_words,
    traced_record_pair,
};
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::{AffineKet, StabilizerError};

/// Finite admission limits for this opt-in route. Geometry counts conservative
/// packed payload, headers and scratch in eight-byte units, not process RSS.
/// Existing caller/compiled storage and arbitrary-precision scalar limbs are
/// excluded; the latter have their own `ScalarArithmetic` limits.
#[derive(Clone, Copy, Debug)]
pub struct ExpansionLimits {
    pub max_path_bits: usize,
    pub max_geometry_words: usize,
    pub max_side_terms: usize,
    pub max_record_rows: usize,
    pub max_pairs: usize,
    pub max_event_rows: usize,
}
impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_path_bits: 256,
            max_geometry_words: 1 << 24,
            max_side_terms: 1024,
            max_record_rows: 256,
            max_pairs: 4096,
            max_event_rows: 256,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpansionStatistics {
    pub resource_kind: PhaseResourceKind,
    pub quotient_rank: usize,
    /// Certified fibers before zero terms are removed by contraction.
    pub certified_terms: usize,
    /// Actual owned, contracted side kets.
    pub side_entries: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordExpansionStatistics {
    pub inherited_side_entries: usize,
    /// Actual ordered pairs visited, including pairs whose trace vanishes.
    pub pair_work: usize,
    /// Actual nonzero retained contributions, at most side_entries squared.
    pub constructive_terms: usize,
}
/// Exact state of the current compiled view, independent of its lifetime.
/// This is an explicit expansion, not a replacement for scalar query execution.
pub struct OwnedStabilizerExpansion {
    width: usize,
    entries: Vec<AffineKet>,
    statistics: ExpansionStatistics,
}
impl OwnedStabilizerExpansion {
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn entries(&self) -> &[AffineKet] {
        &self.entries
    }
    pub fn statistics(&self) -> &ExpansionStatistics {
        &self.statistics
    }
    /// Measure ordered computational parity rows and XOR the report flips.
    /// Dependent, repeated and zero rows retain their original label positions.
    /// All physical wires are traced after enforcing ket/bra equality. No
    /// record assignment is enumerated and no contribution is renormalized.
    pub fn retained_records(
        &self,
        rows: &[BitVector],
        report_flips: &BitVector,
        limits: ExpansionLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<RetainedRecordDistribution, UnitaryError> {
        self.records_with_geometry(rows, report_flips, limits, arithmetic, RecordGeometry::new)
    }
    fn records_with_geometry(
        &self,
        rows: &[BitVector],
        report_flips: &BitVector,
        limits: ExpansionLimits,
        arithmetic: &ScalarArithmetic,
        geometry: impl FnOnce(usize, &[BitVector]) -> Result<RecordGeometry, StabilizerError>,
    ) -> Result<RetainedRecordDistribution, UnitaryError> {
        if report_flips.width() != rows.len() || rows.iter().any(|r| r.width() != self.width) {
            return Err(UnitaryError::DimensionMismatch);
        }
        admit(
            "max_record_rows",
            limits.max_record_rows,
            rows.len() as u128,
        )?;
        admit(
            "max_side_terms",
            limits.max_side_terms,
            self.entries.len() as u128,
        )?;
        let pairs = (self.entries.len() as u128).pow(2);
        admit("max_pairs", limits.max_pairs, pairs)?;
        let paths = self
            .entries
            .iter()
            .map(|s| s.columns().len())
            .max()
            .unwrap_or(0)
            .checked_mul(2)
            .ok_or(UnitaryError::DimensionOverflow)?;
        admit("max_path_bits", limits.max_path_bits, paths as u128)?;
        self.width
            .checked_mul(2)
            .ok_or(UnitaryError::DimensionOverflow)?;
        let words = pair_geometry_words(self.width, paths, rows.len(), 0, pairs);
        admit("max_geometry_words", limits.max_geometry_words, words)?;
        // In particular, pair admission precedes all equality/retained matrices,
        // conjugated kets, tensor products and retained-output allocations.
        let geometry = geometry(self.width, rows)?;
        let no_events = BitVector::zeros(0)?;
        let mut contributions = Vec::with_capacity(pairs as usize);
        let mut pair_work = 0;
        for left in &self.entries {
            for right in &self.entries {
                pair_work += 1;
                if let Some(term) = traced_record_pair(
                    left,
                    right,
                    &geometry,
                    &[],
                    &no_events,
                    report_flips,
                    arithmetic,
                )? {
                    contributions.push(term);
                }
            }
        }
        let statistics = RecordExpansionStatistics {
            inherited_side_entries: self.entries.len(),
            pair_work,
            constructive_terms: contributions.len(),
        };
        Ok(RetainedRecordDistribution {
            width: rows.len(),
            contributions,
            limits,
            statistics,
        })
    }
}
/// A classical mass function represented by unnormalized affine quadratic
/// contributions. Sum their amplitudes to obtain mass; do not square them.
/// This type does not represent a general quantum output density operator.
pub struct RetainedRecordDistribution {
    width: usize,
    contributions: Vec<AffineKet>,
    limits: ExpansionLimits,
    statistics: RecordExpansionStatistics,
}
impl RetainedRecordDistribution {
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn resource_expansion(&self) -> &'static str {
        "inherited-product"
    }
    pub fn statistics(&self) -> &RecordExpansionStatistics {
        &self.statistics
    }
    pub fn contributions(&self) -> &[AffineKet] {
        &self.contributions
    }
    /// Unnormalized exact mass of an ordered parity event on retained labels.
    /// Inconsistent constraints return zero. Queries currently rebuild their
    /// constraint factorizations; no single-trace caching is implied.
    pub fn mass(
        &self,
        event_rows: &[BitVector],
        outcomes: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, UnitaryError> {
        if outcomes.width() != event_rows.len()
            || event_rows.iter().any(|r| r.width() != self.width)
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        admit(
            "max_event_rows",
            self.limits.max_event_rows,
            event_rows.len() as u128,
        )?;
        let paths = self
            .contributions
            .iter()
            .map(|t| t.columns().len())
            .max()
            .unwrap_or(0);
        let words = geometry_words(
            self.width.max(event_rows.len()),
            paths,
            0,
            self.contributions.len() as u128,
        );
        admit("max_geometry_words", self.limits.max_geometry_words, words)?;
        evaluate_mass(
            self.width,
            &self.contributions,
            event_rows,
            outcomes,
            arithmetic,
        )
        .map(|value| value.value)
        .map_err(|error| match error {
            MassError::Stabilizer(e) => UnitaryError::Stabilizer(e),
            MassError::Scalar(e) => UnitaryError::Scalar(e),
        })
    }
    pub fn total(&self, arithmetic: &ScalarArithmetic) -> Result<ExactScalar, UnitaryError> {
        self.mass(&[], &BitVector::zeros(0)?, arithmetic)
    }
}
impl CompiledUnitary {
    /// Freeze a certified Clifford-fiber expansion of this actual view,
    /// including stabilizer input, frozen phase constants and Pauli insertions.
    /// Errors are terminal for this call; no other resource route is attempted.
    pub fn stabilizer_expansion(
        &self,
        limits: ExpansionLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, UnitaryError> {
        self.stabilizer_expansion_with_fibers(limits, FiberOptions::default(), arithmetic)
    }
    pub fn stabilizer_expansion_with_fibers(
        &self,
        limits: ExpansionLimits,
        fibers: FiberOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, UnitaryError> {
        self.stabilizer_expansion_with_resource(
            limits,
            PhaseResourceOptions {
                fibers,
                ..Default::default()
            },
            None,
            arithmetic,
        )
    }
    /// Select a side resource explicitly. Only the supplied catalogue is used;
    /// absent keys follow the existing deterministic-fiber selection policy.
    pub fn stabilizer_expansion_with_resource(
        &self,
        limits: ExpansionLimits,
        options: PhaseResourceOptions,
        catalogue: Option<&crate::catalogue::PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, UnitaryError> {
        self.stabilizer_expansion_with_resource_rank_limit(
            limits,
            options,
            catalogue,
            usize::MAX,
            arithmetic,
        )
    }
    /// Preparation-only source quotient admission; the existing cold route
    /// retains its original raw-path/resource semantics.
    pub(crate) fn stabilizer_expansion_with_resource_rank_limit(
        &self,
        limits: ExpansionLimits,
        options: PhaseResourceOptions,
        catalogue: Option<&crate::catalogue::PhaseCatalogue>,
        max_source_quotient_rank: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, UnitaryError> {
        let paths = self.boundary.columns().len();
        let raw_paths = self.compilation.raw_path_variables.max(paths);
        admit("max_path_bits", limits.max_path_bits, raw_paths as u128)?;
        admit("max_side_terms", limits.max_side_terms, 1)?;
        let phase_width = self.phase.num_variables();
        if self.width().checked_add(phase_width) != Some(self.boundary.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let candidates = 1_u128
            .checked_shl(u32::try_from(paths).unwrap_or(u32::MAX))
            .unwrap_or(u128::MAX)
            .min(limits.max_side_terms as u128);
        let words = geometry_words(
            self.boundary.width(),
            paths,
            self.phase.terms().len(),
            candidates,
        );
        admit("max_geometry_words", limits.max_geometry_words, words)?;
        let phase_part = |value: &BitVector| {
            BitVector::from_indices(
                phase_width,
                value.ones().filter_map(|j| j.checked_sub(self.width())),
            )
        };
        let offset = phase_part(self.boundary.offset())?;
        let columns = self
            .boundary
            .columns()
            .iter()
            .map(phase_part)
            .collect::<Result<Vec<_>, _>>()?;
        let reference = self.phase.restrict(&offset, &columns)?;
        if max_source_quotient_rank != usize::MAX {
            admit(
                "max_source_quotient_rank",
                max_source_quotient_rank,
                reference.quotient()?.rank() as u128,
            )?;
        }
        let physical = self
            .boundary
            .retain_boundary(&(0..self.width()).collect::<Vec<_>>())?;
        let resource = PreparedPhaseResource::new(
            &reference,
            limits.max_side_terms,
            options,
            catalogue,
            arithmetic,
        )?;
        let entries = resource.expand(&reference, &physical, arithmetic)?;
        let statistics = ExpansionStatistics {
            resource_kind: resource.kind(),
            quotient_rank: resource.quotient_rank(),
            certified_terms: resource.num_terms(),
            side_entries: entries.len(),
        };
        Ok(OwnedStabilizerExpansion {
            width: self.width(),
            entries,
            statistics,
        })
    }
}
fn admit(option: &'static str, limit: usize, required: u128) -> Result<(), UnitaryError> {
    if required > limit as u128 {
        Err(UnitaryError::ExpansionResourceLimit {
            option,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}
// Saturation is conservative: u128::MAX cannot fit any usize-configured budget.
// Include full-width factorization/provenance, quadratic matrices, transposes,
// temporary phase axes, all retained terms, and their Vec/BitVector headers.
fn geometry_words(width: usize, paths: usize, axes: usize, terms: u128) -> u128 {
    let d = width.max(paths).max(axes) as u128;
    let matrix = (d + 1).saturating_mul(d.div_ceil(64) + 8);
    let ket = (paths as u128 + 1)
        .saturating_mul((width as u128).div_ceil(64) + (paths as u128).div_ceil(64) + 16);
    64_u128.saturating_mul(matrix.saturating_add(terms.saturating_mul(ket)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::clifford::CliffordGate;
    use crate::unitary::{UnitaryCircuit, UnitaryGate};
    #[test]
    fn pair_and_geometry_admission_precede_the_pair_matrix_factory() {
        let a = ScalarArithmetic::default();
        let side = UnitaryCircuit::new(
            1,
            [
                UnitaryGate::Clifford(CliffordGate::H(0)),
                UnitaryGate::T { qubit: 0, turns: 1 },
            ],
        )
        .unwrap()
        .compile(&BitVector::zeros(1).unwrap(), &a)
        .unwrap()
        .stabilizer_expansion(ExpansionLimits::default(), &a)
        .unwrap();
        assert_eq!(side.entries.len(), 2);
        let rows = [BitVector::from_indices(1, [0]).unwrap()];
        let flips = BitVector::zeros(1).unwrap();
        for (option, limits) in [
            (
                "max_pairs",
                ExpansionLimits {
                    max_pairs: 3,
                    ..Default::default()
                },
            ),
            (
                "max_geometry_words",
                ExpansionLimits {
                    max_geometry_words: 0,
                    ..Default::default()
                },
            ),
        ] {
            let result = side.records_with_geometry(&rows, &flips, limits, &a, |_, _| {
                panic!("admission must precede pair matrices")
            });
            assert!(
                matches!(result, Err(UnitaryError::ExpansionResourceLimit { option: actual, .. }) if actual == option)
            );
        }
        let mut calls = 0;
        side.records_with_geometry(
            &rows,
            &flips,
            ExpansionLimits::default(),
            &a,
            |width, rows| {
                calls += 1;
                RecordGeometry::new(width, rows)
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
    }
}
