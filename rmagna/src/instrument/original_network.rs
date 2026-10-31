//! Scalar global queries that trace physical legs before selecting a resource.
use super::*;
use crate::phase::NativePhaseOptions;
use crate::unitary::{NetworkValue, PreparedPauliMoment, PreparedProbability};

/// Explicit resource policy for the final scalar network, not its coherent side.
#[derive(Clone, Copy, Debug)]
// Like QueryPreparation, keep this configuration allocation-free and Copy.
#[allow(clippy::large_enum_variant)]
pub enum OriginalNetworkResource {
    Catalogue(PhaseResourceOptions),
    Native(NativePhaseOptions),
}
impl Default for OriginalNetworkResource {
    fn default() -> Self {
        Self::Catalogue(PhaseResourceOptions::default())
    }
}
/// Admission for the actual packed scalar closure. In particular, its physical
/// equality rows are quadratic in physical width; these are not thin pair plans.
#[derive(Clone, Copy, Debug)]
pub struct OriginalNetworkLimits {
    pub max_event_rows: usize,
    /// Joint reference path variables, before fixed trace/event constraints.
    pub max_joint_paths: usize,
    pub max_geometry_words: usize,
}
impl Default for OriginalNetworkLimits {
    fn default() -> Self {
        Self {
            max_event_rows: 1 << 20,
            max_joint_paths: 1 << 20,
            max_geometry_words: 1 << 24,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct OriginalNetworkOptions<'a> {
    pub resource: OriginalNetworkResource,
    pub catalogue: Option<&'a PhaseCatalogue>,
    pub max_terms: usize,
    pub max_record_occurrences: usize,
    pub limits: OriginalNetworkLimits,
}
impl Default for OriginalNetworkOptions<'_> {
    fn default() -> Self {
        Self {
            resource: OriginalNetworkResource::default(),
            catalogue: None,
            max_terms: 1 << 20,
            max_record_occurrences: 1 << 20,
            limits: OriginalNetworkLimits::default(),
        }
    }
}

struct Layout<'a> {
    source: &'a OriginalStaticTrace,
    inverted: Vec<bool>,
    report_rows: Vec<Vec<usize>>,
    geometry_words: u128,
    max_terms: usize,
}
impl Layout<'_> {
    fn outcomes(
        &self,
        view: &OriginalTraceView<'_>,
        reported: &BitVector,
    ) -> Result<BitVector, OriginalError> {
        if !std::ptr::eq(self.source, view.source) {
            return Err(UnitaryError::IncompatibleProgram.into());
        }
        if reported.width() != self.inverted.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        let mut physical = reported.clone();
        for (i, indices) in self.report_rows.iter().enumerate() {
            let flip = indices
                .iter()
                .fold(self.inverted[i], |v, &j| v ^ view.flips[j]);
            physical.set(i, physical.get(i)? ^ flip)?;
        }
        Ok(physical)
    }
}

fn layout<'a>(
    source: &'a OriginalStaticTrace,
    events: &[OriginalRecordParity],
    options: &OriginalNetworkOptions<'_>,
) -> Result<(Layout<'a>, Vec<BitVector>), OriginalError> {
    let limits = source.limits;
    admit(
        "max_event_rows",
        options
            .limits
            .max_event_rows
            .min(limits.expansion.max_event_rows)
            .min(limits.completed.max_event_rows),
        events.len() as u128,
    )?;
    let occurrences = events.iter().map(|e| e.records.len() as u128).sum::<u128>();
    admit(
        "max_record_occurrences",
        options.max_record_occurrences,
        occurrences,
    )?;
    let bytes = events
        .iter()
        .flat_map(|e| &e.records)
        .map(|name| 32 + name.len() as u128)
        .sum::<u128>()
        + 64 * events.len() as u128;
    admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
    let words = admit_geometry(source, events.len(), occurrences, options)?;
    let mut rows = Vec::with_capacity(events.len());
    let mut inverted = Vec::with_capacity(events.len());
    let mut report_rows = Vec::with_capacity(events.len());
    for event in events {
        let mut row = BitVector::zeros(source.physical_width())?;
        let mut flip = event.inverted;
        let mut indices = Vec::with_capacity(event.records.len());
        for name in &event.records {
            let (index, slot, inverse) = source.record_coordinate(name)?;
            row.set(slot, !row.get(slot)?)?;
            flip ^= inverse;
            indices.push(index);
        }
        rows.push(row);
        inverted.push(flip);
        report_rows.push(indices);
    }
    Ok((
        Layout {
            source,
            inverted,
            report_rows,
            geometry_words: words,
            max_terms: options.max_terms,
        },
        rows,
    ))
}

/// Shared preflight for a prospective scalar family, before any constructor.
/// Occurrences count sparse report-index storage retained by that family.
pub(super) fn admit_geometry(
    source: &OriginalStaticTrace,
    event_count: usize,
    occurrences: u128,
    options: &OriginalNetworkOptions<'_>,
) -> Result<u128, OriginalError> {
    let limits = source.limits;
    let (boundary, paths) = source.reference.scalar_query_shape();
    let (n, w, p, e) = (
        source.physical_width() as u128,
        2 * boundary as u128,
        2 * paths as u128,
        event_count as u128,
    );
    admit("max_network_joint_paths", options.limits.max_joint_paths, p)?;
    let vectors = |count: u128, bits: u128| count.saturating_mul(bits.div_ceil(64) + 4);
    // Account for the real packed physical-equality rows, joint boundary,
    // path equations, retained nullspaces and contraction programs, including
    // simultaneously live constructor temporaries. No physical-width shortcut.
    let words = 64_u128.saturating_mul(
        vectors(n + e, w)
            .saturating_add(vectors(p + 1, w))
            .saturating_add(vectors(n + e + p + 1, p))
            .saturating_add(vectors(w + p + 1, p))
            .saturating_add(vectors(e, n))
            .saturating_add(n + w + p + e + occurrences + 128),
    );
    admit(
        "max_network_geometry_words",
        options
            .limits
            .max_geometry_words
            .min(limits.expansion.max_geometry_words),
        words,
    )?;
    Ok(words)
}

/// Fixed reported-event layout with an actual retained global-query kernel.
/// Explicit assignment views replace faults/report flips without evolving the
/// coherent program again. No side expansion or side-entry certificate is made.
pub struct OriginalNetworkProbability<'a> {
    layout: Layout<'a>,
    query: PreparedProbability,
}
/// Fixed Hermitian Pauli in output-then-record order. Physical legs outside that
/// ordered boundary carry I and are traced before phase-resource selection.
pub struct OriginalNetworkMoment<'a> {
    layout: Layout<'a>,
    query: PreparedPauliMoment,
    reported_z: Vec<usize>,
}
impl OriginalStaticTrace {
    /// Width of the scalar moment boundary: declared outputs, then named records
    /// in `records()` order. This does not include other physical lifetimes.
    pub fn network_width(&self) -> usize {
        self.outputs.len() + self.record_names.len()
    }
    /// Retain trace/event Gaussian geometry, then select a fixed final-query
    /// phase resource. Supplied reported outcomes and assignment views remain
    /// evaluation inputs. Resource rank and terms describe this actual scalar
    /// family; they are not a coherent-side expansion certificate. A resource
    /// policy failure is returned directly. No catalogue is selected implicitly.
    ///
    /// ```
    /// use rmagna::{clifford::CliffordCircuit, gf2::BitVector,
    ///     instrument::{OriginalInstrument, OriginalLimits, OriginalNetworkOptions,
    ///         OriginalTraceAssignment}, scalar::{ExactScalar, ScalarArithmetic},
    ///     unitary::CompilerStrategy};
    /// let a = ScalarArithmetic::default();
    /// let instrument = OriginalInstrument::new(vec![], vec![],
    ///     CliffordCircuit::computational(BitVector::zeros(0)?)?,
    ///     vec![], vec![], OriginalLimits::default(), &a)?;
    /// let trace = instrument.compile_trace(CompilerStrategy::ParityCopies, &a)?;
    /// let query = trace.prepare_network_probability(&[], &OriginalNetworkOptions::default(), &a)?;
    /// let view = trace.with_assignment(&OriginalTraceAssignment::default(), &a)?;
    /// let result = query.evaluate(&view, &BitVector::zeros(0)?, &a)?;
    /// assert_eq!(result.value, ExactScalar::one());
    /// assert_eq!((result.quotient_rank, result.terms), (0, 1));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prepare_network_probability(
        &self,
        events: &[OriginalRecordParity],
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalNetworkProbability<'_>, OriginalError> {
        let (layout, rows) = layout(self, events, options)?;
        let query = self.reference.prepare_probability(&rows, a)?;
        let query = match options.resource {
            OriginalNetworkResource::Catalogue(resource) => {
                query.with_phase_resource(options.max_terms, resource, options.catalogue, a)?
            }
            OriginalNetworkResource::Native(resource) => query.with_native_phase_resource(
                options.max_terms,
                resource,
                options.catalogue,
                a,
            )?,
        };
        Ok(OriginalNetworkProbability { layout, query })
    }
    /// The Pauli must be Hermitian and commute with every event. Reported Z/Y
    /// factors include original readout inversions and the current view's report
    /// flips; X factors do not change sign. Outcomes remain evaluation inputs.
    pub fn prepare_network_moment(
        &self,
        word: &PauliWord,
        events: &[OriginalRecordParity],
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalNetworkMoment<'_>, OriginalError> {
        if word.width() != self.network_width() {
            return Err(OriginalError::DimensionMismatch);
        }
        if word.phase() & 3 != 2 * u8::from(word.x().dot(word.z())?) {
            return Err(CompletedRecordError::NonHermitianPauli.into());
        }
        let (layout, rows) = layout(self, events, options)?;
        let output_count = self.outputs.len();
        let slot = |j: usize| {
            if j < output_count {
                self.output_slots[j]
            } else {
                self.record_slots[j - output_count].0
            }
        };
        let reported_z = word
            .z()
            .ones()
            .filter_map(|j| j.checked_sub(output_count))
            .collect::<Vec<_>>();
        let inverted = reported_z
            .iter()
            .fold(false, |v, &j| v ^ self.record_slots[j].1);
        let physical = PauliWord::new(
            BitVector::from_indices(self.physical_width(), word.x().ones().map(slot))?,
            BitVector::from_indices(self.physical_width(), word.z().ones().map(slot))?,
            (word.phase() + 4 * u8::from(inverted)) & 7,
        )?;
        for (index, row) in rows.iter().enumerate() {
            if row.dot(physical.x())? {
                return Err(CompletedRecordError::NonCommutingPauli {
                    kind: "event",
                    index,
                }
                .into());
            }
        }
        let query = self.reference.prepare_pauli_moment(&physical, &rows, a)?;
        let query = match options.resource {
            OriginalNetworkResource::Catalogue(resource) => {
                query.with_phase_resource(options.max_terms, resource, options.catalogue, a)?
            }
            OriginalNetworkResource::Native(resource) => query.with_native_phase_resource(
                options.max_terms,
                resource,
                options.catalogue,
                a,
            )?,
        };
        Ok(OriginalNetworkMoment {
            layout,
            query,
            reported_z,
        })
    }
}
impl OriginalNetworkProbability<'_> {
    /// Conservative admitted packed words, including physical equality rows.
    pub fn geometry_words(&self) -> u128 {
        self.layout.geometry_words
    }
    pub fn evaluate(
        &self,
        view: &OriginalTraceView<'_>,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<NetworkValue, OriginalError> {
        let outcomes = self.layout.outcomes(view, outcomes)?;
        Ok(self
            .query
            .evaluate(&view.unitary, &outcomes, self.layout.max_terms, a)?)
    }
}
impl OriginalNetworkMoment<'_> {
    pub fn geometry_words(&self) -> u128 {
        self.layout.geometry_words
    }
    pub fn evaluate(
        &self,
        view: &OriginalTraceView<'_>,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<NetworkValue, OriginalError> {
        let outcomes = self.layout.outcomes(view, outcomes)?;
        let mut result = self
            .query
            .evaluate(&view.unitary, &outcomes, self.layout.max_terms, a)?;
        if self
            .reported_z
            .iter()
            .fold(false, |v, &j| v ^ view.flips[j])
        {
            result.value = a.multiply(&result.value, &ExactScalar::one().negated())?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clifford::{CliffordCircuit, CliffordGate},
        gf2::FACTORIZATION_CALLS,
    };

    #[test]
    fn network_admission_precedes_actual_global_query_factorization() {
        let a = ScalarArithmetic::default();
        let initial = CliffordCircuit::new(1, [CliffordGate::H(0)])
            .unwrap()
            .evolve(
                &CliffordCircuit::computational(BitVector::zeros(1).unwrap()).unwrap(),
                &a,
            )
            .unwrap()
            .unwrap();
        let instrument = OriginalInstrument::new(
            vec![0],
            vec![],
            initial,
            vec![OriginalOperation {
                id: "a".into(),
                gate: OriginalGate::Destructive {
                    wire: 0,
                    basis: ReadoutBasis::Z,
                    record: "a".into(),
                },
            }],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let trace = instrument.compile_trace(strategy, &a).unwrap();
            let events = [OriginalRecordParity {
                records: vec!["a".into()],
                inverted: false,
            }];
            let options = OriginalNetworkOptions::default();
            FACTORIZATION_CALLS.with(|c| c.set(0));
            let good = trace
                .prepare_network_probability(&events, &options, &a)
                .unwrap();
            assert!(FACTORIZATION_CALLS.with(|c| c.get()) > 0);
            for bad in [
                OriginalNetworkOptions {
                    max_record_occurrences: 0,
                    ..options
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_event_rows: 0,
                        ..options.limits
                    },
                    ..options
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_joint_paths: 1,
                        ..options.limits
                    },
                    ..options
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_geometry_words: good.geometry_words() as usize - 1,
                        ..options.limits
                    },
                    ..options
                },
            ] {
                FACTORIZATION_CALLS.with(|c| c.set(0));
                assert!(matches!(
                    trace.prepare_network_probability(&events, &bad, &a),
                    Err(OriginalError::ResourceLimit { .. })
                ));
                assert_eq!(FACTORIZATION_CALLS.with(|c| c.get()), 0);
            }
            let view = trace
                .with_assignment(&OriginalTraceAssignment::default(), &a)
                .unwrap();
            assert_eq!(
                good.evaluate(&view, &BitVector::zeros(1).unwrap(), &a)
                    .unwrap()
                    .value,
                a.scalar([1, 0, 0, 0].map(Into::into), -1).unwrap()
            );
        }
    }
}
