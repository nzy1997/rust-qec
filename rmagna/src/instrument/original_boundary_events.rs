//! Mixed physical output and reported-record scalar event families.
use super::*;
impl OriginalStaticTrace {
    fn boundary_event_layout<'a>(
        &'a self,
        events: &[BitVector],
        options: &OriginalNetworkOptions<'_>,
    ) -> Result<(Layout<'a>, Vec<BitVector>), OriginalError> {
        admit(
            "max_event_rows",
            options
                .limits
                .max_event_rows
                .min(self.limits.expansion.max_event_rows)
                .min(self.limits.completed.max_event_rows),
            events.len() as u128,
        )?;
        let width = self.network_width();
        if events.iter().any(|row| row.width() != width) {
            return Err(OriginalError::DimensionMismatch);
        }
        let outputs = self.outputs.len();
        let occurrences = events
            .iter()
            .map(|row| row.ones().filter(|&j| j >= outputs).count() as u128)
            .sum::<u128>();
        admit(
            "max_record_occurrences",
            options.max_record_occurrences,
            occurrences,
        )?;
        admit(
            "max_metadata_bytes",
            self.limits.max_metadata_bytes,
            64 * events.len() as u128 + 16 * occurrences,
        )?;
        let geometry_words = admit_geometry(self, events.len(), occurrences, options)?;
        let physical = |j: usize| {
            if j < outputs {
                self.output_slots[j]
            } else {
                self.record_slots[j - outputs].0
            }
        };
        let mut rows = Vec::with_capacity(events.len());
        let mut inverted = Vec::with_capacity(events.len());
        let mut report_rows = Vec::with_capacity(events.len());
        for row in events {
            rows.push(BitVector::from_indices(
                self.physical_width(),
                row.ones().map(physical),
            )?);
            let indices = row
                .ones()
                .filter_map(|j| j.checked_sub(outputs))
                .collect::<Vec<_>>();
            inverted.push(
                indices
                    .iter()
                    .fold(false, |v, &j| v ^ self.record_slots[j].1),
            );
            report_rows.push(indices);
        }
        Ok((
            Layout {
                source: self,
                inverted,
                report_rows,
                geometry_words,
                max_terms: options.max_terms,
            },
            rows,
        ))
    }
    /// Scalar Z-parity events in declared output order followed by reported
    /// record order. Trace all other physical legs before resource selection.
    /// Evaluation outcomes use those reported coordinates, including readout
    /// inversions and XOR report flips. This constructs no side expansion.
    pub fn prepare_boundary_event_probability(
        &self,
        events: &[BitVector],
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalNetworkProbability<'_>, OriginalError> {
        let (layout, rows) = self.boundary_event_layout(events, options)?;
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
    /// A Hermitian word and mixed Z events use the same output-then-record
    /// coordinates. Anticommuting word/event pairs fail explicitly. Values are
    /// raw event moments, without division by the event probability.
    pub fn prepare_boundary_event_moment(
        &self,
        word: &PauliWord,
        events: &[BitVector],
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalNetworkMoment<'_>, OriginalError> {
        self.validate_moment_word(word)?;
        let (layout, rows) = self.boundary_event_layout(events, options)?;
        self.prepare_moment_layout(word, layout, rows, options, a)
    }
}
