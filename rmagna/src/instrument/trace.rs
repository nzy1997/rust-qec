//! Frozen record/output scalar families over the released trace-purpose owner.
use super::*;
use crate::unitary::{
    AmplitudeOwnershipProbe, TraceAssignment, TraceOptions, TraceQuery, TraceRuntime,
};

/// Empty outputs and an identity word give a record probability. `accepted`
/// includes this instrument's selection predicates; otherwise they are ignored.
/// Parity constants are applied to the caller's requested outcomes exactly.
#[derive(Clone)]
pub struct InstrumentTraceQuery {
    pub outputs: Vec<usize>,
    pub word: PauliWord,
    pub parities: Vec<BitExpr>,
    pub accepted: bool,
}
struct Layout {
    selection: BitVector,
    constants: Vec<bool>,
}
/// A sealed coherent file/instrument trace. It owns direct physical fault
/// metadata and closed query families, not the source instrument or its cached
/// amplitude-capable plans. Adaptive callbacks and target overlaps are separate
/// services; callers must choose all record/output scalar layouts up front.
pub struct InstrumentTraceRuntime {
    runtime: TraceRuntime,
    initial: Vec<PauliInsertion>,
    sites: Vec<(usize, Vec<PauliWord>)>,
    layouts: Vec<Layout>,
}
impl CompiledInstrument {
    pub fn amplitude_ownership_probe(&self) -> AmplitudeOwnershipProbe {
        self.unitary.amplitude_ownership_probe()
    }
    /// Close physical outputs for every layout, then drop this instrument's
    /// original caches/catalog/program owners. Independently cloned instruments
    /// can keep their own amplitude owners alive, as the weak probe reports.
    pub fn into_trace_runtime(
        self,
        queries: &[InstrumentTraceQuery],
        options: TraceOptions,
        a: &ScalarArithmetic,
    ) -> Result<InstrumentTraceRuntime, InstrumentError> {
        if queries.len() > options.max_queries {
            return Err(UnitaryError::QueryCacheLimit {
                option: "trace.max_queries",
                limit: options.max_queries,
                required: queries.len() as u128,
            }
            .into());
        }
        // Include both BitVector headers, the Pauli phase and padding.
        let word_cost = |word: &PauliWord| 16 + 2 * word.x().words().len() as u128;
        let mut words = self
            .initial_insertions
            .iter()
            .fold(0u128, |sum, insertion| {
                sum.saturating_add(8 + word_cost(&insertion.word))
            });
        for site in self.sites.iter() {
            words = words.saturating_add(8);
            for alternative in site.alternatives.iter() {
                words = words.saturating_add(word_cost(&alternative.word));
            }
        }
        // Include the independent instrument metadata and mapped record rows
        // in the same budget as the unitary closed-kernel preparation.
        words = queries.iter().fold(words, |sum, q| {
            sum.saturating_add(16 + q.parities.len() as u128)
                .saturating_add(
                    (self.coherent_qubits().div_ceil(64) as u128 + 8).saturating_mul(
                        (q.parities.len() as u128).saturating_add(if q.accepted {
                            self.event_rows.len() as u128
                        } else {
                            0
                        }),
                    ),
                )
        });
        if words > options.max_geometry_words as u128 {
            return Err(UnitaryError::QueryCacheLimit {
                option: "trace.max_geometry_words",
                limit: options.max_geometry_words,
                required: words,
            }
            .into());
        }
        let mut mapped = Vec::with_capacity(queries.len());
        let mut layouts = Vec::with_capacity(queries.len());
        for q in queries {
            let mut rows = if q.accepted {
                self.event_rows.as_ref().clone()
            } else {
                vec![]
            };
            rows.extend(
                q.parities
                    .iter()
                    .map(|p| self.parity_row(p))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            mapped.push(TraceQuery {
                word: self.physical_word(&q.outputs, &q.word)?,
                event_rows: rows,
            });
            layouts.push(Layout {
                selection: if q.accepted {
                    self.outcomes.clone()
                } else {
                    BitVector::zeros(0)?
                },
                constants: q.parities.iter().map(|p| p.constant).collect(),
            });
        }
        let runtime = self
            .unitary
            .prepare_trace_runtime(
                &mapped,
                TraceOptions {
                    max_geometry_words: options.max_geometry_words - words as usize,
                    ..options
                },
                a,
            )
            .map_err(|error| match error {
                UnitaryError::QueryCacheLimit {
                    option: "trace.max_geometry_words",
                    required,
                    ..
                } => UnitaryError::QueryCacheLimit {
                    option: "trace.max_geometry_words",
                    limit: options.max_geometry_words,
                    required: required.saturating_add(words),
                },
                other => other,
            })?;
        let sites = self
            .sites
            .iter()
            .map(|site| {
                (
                    site.before_gate,
                    site.alternatives
                        .iter()
                        .map(|alt| alt.word.clone())
                        .collect(),
                )
            })
            .collect();
        Ok(InstrumentTraceRuntime {
            runtime,
            initial: self.initial_insertions.clone(),
            sites,
            layouts,
        })
    }
}
impl InstrumentTraceRuntime {
    pub fn runtime(&self) -> &TraceRuntime {
        &self.runtime
    }
    /// Repeated sites are invalid mutually exclusive assignments. An assignment
    /// is prepared once and can then serve all this runtime's scalar families.
    pub fn assignment(
        &self,
        faults: &[FaultSelection],
    ) -> Result<TraceAssignment, InstrumentError> {
        let mut seen = BTreeSet::new();
        let mut insertions = self.initial.clone();
        for f in faults {
            if !seen.insert(f.site) {
                return Err(InstrumentError::RepeatedFault { site: f.site });
            }
            let (position, words) =
                self.sites
                    .get(f.site)
                    .ok_or(InstrumentError::InvalidFault {
                        site: f.site,
                        alternative: f.alternative,
                    })?;
            let word = words
                .get(f.alternative)
                .ok_or(InstrumentError::InvalidFault {
                    site: f.site,
                    alternative: f.alternative,
                })?;
            insertions.push(PauliInsertion {
                before_gate: *position,
                word: word.clone(),
            });
        }
        Ok(self.runtime.assignment(&insertions)?)
    }
    pub fn evaluate(
        &self,
        index: usize,
        assignment: &TraceAssignment,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let layout = self
            .layouts
            .get(index)
            .ok_or(UnitaryError::InvalidTraceQuery {
                index,
                count: self.layouts.len(),
            })?;
        if outcomes.width() != layout.constants.len() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let n = layout.selection.width();
        let rhs = BitVector::from_indices(
            n + outcomes.width(),
            layout
                .selection
                .ones()
                .chain(layout.constants.iter().enumerate().filter_map(|(j, &c)| {
                    (outcomes.get(j).expect("validated outcome") ^ c).then_some(n + j)
                })),
        )?;
        Ok(self.runtime.evaluate(index, assignment, &rhs, a)?)
    }
}
