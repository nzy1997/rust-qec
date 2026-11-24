//! Sealed scalar trace families, with every physical output summed at preparation.
use super::prepared::{ScalarClosure, moment_joint};
use super::*;
use crate::compact::CompactPauliWord;
use crate::phase::{NativePhaseOptions, NativeTransportMode, ResourcePreparation};
use std::sync::Weak;

/// One fixed `<psi|Pi P Pi|psi>` layout. Identity P gives a probability.
#[derive(Clone)]
pub struct TraceQuery {
    pub word: PauliWord,
    pub event_rows: Vec<BitVector>,
}
#[derive(Clone, Copy)]
pub struct TraceOptions {
    pub max_queries: usize,
    /// Conservative matrix/metadata/scratch words, excluding caller input.
    /// Bounds trace-family closing after the common Clifford compilation,
    /// whose raw paths are bounded separately by `max_path_bits`.
    /// Includes retained transport metadata. Phase resources have their own
    /// `phase` bounds. This is not allocator bytes or process RSS.
    pub max_geometry_words: usize,
    /// Raw common-dilation paths, checked at compilation and when sealing an
    /// existing reference. This is distinct from closed-kernel support rank.
    pub max_path_bits: usize,
    pub max_terms: usize,
    pub phase: NativePhaseOptions,
}
impl Default for TraceOptions {
    fn default() -> Self {
        let mut phase = NativePhaseOptions::default();
        phase.transport.mode = NativeTransportMode::Prepared;
        Self {
            max_queries: 1024,
            max_geometry_words: 16 << 20,
            max_path_bits: 1_000_000,
            max_terms: 65536,
            phase,
        }
    }
}
/// Weak observations of actual amplitude owners. The probe itself retains no
/// payload. External ordinary plans can keep those owners alive independently
/// of a sealed runtime; their last drop is visible through this same probe.
#[derive(Clone)]
pub struct AmplitudeOwnershipProbe {
    raw: Weak<AffineKet>,
    query: Weak<AffineKet>,
    program: Weak<InsertionProgram>,
}
impl AmplitudeOwnershipProbe {
    pub fn strong_counts(&self) -> [usize; 3] {
        [
            self.raw.strong_count(),
            self.query.strong_count(),
            self.program.strong_count(),
        ]
    }
    /// Whether the ordinary compiler allocated a separate reduced query ket.
    /// Weak pointer identity remains observable after the payloads are dropped.
    pub fn has_distinct_query_boundary(&self) -> bool {
        !Weak::ptr_eq(&self.raw, &self.query)
    }
    pub fn is_released(&self) -> bool {
        self.strong_counts() == [0; 3]
    }
}
/// Real retained closed-boundary dimensions and transport lengths. No RSS or
/// Native private-array equivalence is implied by this structural inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStorageStatistics {
    pub event_phase_widths: Vec<usize>,
    pub closed_path_variables: Vec<usize>,
    pub original_positions: usize,
    pub clifford_gates: usize,
    pub phase_variables: usize,
}
struct Query {
    word: PauliWord,
    rows: Vec<BitVector>,
    kernel: Option<AffineKet>,
    closure: Option<ScalarClosure>,
}
/// Owned numeric assignment to one sealed runtime. It contains a phased fault
/// frame and a phase function, with no output-open ket or compiled-program Arc.
pub struct TraceAssignment {
    owner: Arc<()>,
    frame: CompactPauliWord,
    phase: ParityPhase,
}
impl TraceAssignment {
    pub fn transported_frame(&self) -> &CompactPauliWord {
        &self.frame
    }
}
/// A fixed-query trace-purpose owner. Preparation may use the common full
/// Clifford dilation. Runtime retains only physically closed trace kernels,
/// their Gaussian/phase maps/resources, and fault/rotation transport metadata.
/// Arbitrary new query layouts and amplitude queries are deliberately absent.
/// The original input and ordinary prepared plans can be dropped completely.
pub struct TraceRuntime {
    owner: Arc<()>,
    physical: usize,
    raw_width: usize,
    circuit: CliffordCircuit,
    positions: Vec<usize>,
    phase: ParityPhase,
    shared: Option<shared::PhaseUpdates>,
    compilation: Arc<CompilationDiagnostics>,
    work: Arc<work::QueryWorkCounters>,
    probe: AmplitudeOwnershipProbe,
    queries: Vec<Query>,
    max_terms: usize,
}
fn validate_queries(physical: usize, queries: &[TraceQuery]) -> Result<(), UnitaryError> {
    for q in queries {
        if q.word.width() != physical || q.event_rows.iter().any(|r| r.width() != physical) {
            return Err(UnitaryError::DimensionMismatch);
        }
    }
    Ok(())
}
impl CompiledUnitary {
    pub fn amplitude_ownership_probe(&self) -> AmplitudeOwnershipProbe {
        AmplitudeOwnershipProbe {
            raw: Arc::downgrade(&self.program.reference),
            query: Arc::downgrade(&self.program.query_reference),
            program: Arc::downgrade(&self.program),
        }
    }
    /// Seal layouts against an unassigned reference and consume this amplitude
    /// owner. External ordinary plans are excluded from the runtime's ownership.
    pub fn into_trace_runtime(
        self,
        queries: &[TraceQuery],
        options: TraceOptions,
        a: &ScalarArithmetic,
    ) -> Result<TraceRuntime, UnitaryError> {
        self.prepare_trace_runtime(queries, options, a)
    }
    pub(crate) fn prepare_trace_runtime(
        &self,
        queries: &[TraceQuery],
        options: TraceOptions,
        a: &ScalarArithmetic,
    ) -> Result<TraceRuntime, UnitaryError> {
        if self.assignment.is_some() {
            return Err(UnitaryError::AssignedTraceReference);
        }
        if self.compilation.raw_path_variables > options.max_path_bits {
            return Err(UnitaryError::RawPathLimit {
                limit: options.max_path_bits,
                required: self.compilation.raw_path_variables,
            });
        }
        validate_queries(self.width(), queries)?;
        if queries.len() > options.max_queries {
            return Err(UnitaryError::QueryCacheLimit {
                option: "trace.max_queries",
                limit: options.max_queries,
                required: queries.len() as u128,
            });
        }
        let reference = &self.program.reference;
        // Bound the full transient doubled dilation, constraint provenance and
        // retained query maps before tensor/projection allocations. Saturating
        // arithmetic makes every overflow a conservative refusal.
        let mut words = 64u128
            .saturating_add(8u128.saturating_mul(self.program.circuit.gates().len() as u128))
            .saturating_add(self.program.positions.len() as u128)
            .saturating_add(
                self.program
                    .shared
                    .as_ref()
                    .map_or(0, shared::PhaseUpdates::retained_words),
            )
            .saturating_add(
                self.program
                    .reference_phase
                    .terms()
                    .iter()
                    .map(|(axis, _)| 8 + axis.words().len() as u128)
                    .sum::<u128>(),
            );
        for q in queries {
            let d = (reference.width() as u128)
                .saturating_mul(2)
                .saturating_add((reference.columns().len() as u128).saturating_mul(2))
                .saturating_add(q.event_rows.len() as u128)
                .saturating_add(1);
            words =
                words.saturating_add(64u128.saturating_mul(d).saturating_mul(d.div_ceil(64) + 8));
        }
        if words > options.max_geometry_words as u128 {
            return Err(UnitaryError::QueryCacheLimit {
                option: "trace.max_geometry_words",
                limit: options.max_geometry_words,
                required: words,
            });
        }
        let work = Arc::new(work::QueryWorkCounters::default());
        let phase = &self.program.reference_phase;
        let r = phase.num_variables();
        let mut closed = Vec::with_capacity(queries.len());
        for q in queries {
            let zero = q
                .event_rows
                .iter()
                .try_fold(false, |v, row| Ok::<_, Gf2Error>(v | row.dot(q.word.x())?))?;
            let kernel = if zero {
                None
            } else {
                close_physical(reference, self.width(), q, a)?
            };
            let closure = if let Some(kernel) = &kernel {
                let rows = (0..q.event_rows.len())
                    .map(|j| BitVector::from_indices(kernel.width(), [j]))
                    .collect::<Result<Vec<_>, _>>()?;
                let start = q.event_rows.len();
                let mut closure = ScalarClosure::detached(
                    kernel,
                    &rows,
                    vec![
                        (start..start + r).collect(),
                        (start + r..start + 2 * r).collect(),
                    ],
                    &work,
                )?;
                closure.freeze(
                    &[(phase, true), (phase, false)],
                    options.max_terms,
                    ResourcePreparation::Native(options.phase),
                    None,
                    a,
                )?;
                Some(closure)
            } else {
                None
            };
            closed.push(Query {
                word: q.word.clone(),
                rows: q.event_rows.clone(),
                kernel,
                closure,
            });
        }
        Ok(TraceRuntime {
            owner: Arc::new(()),
            physical: self.width(),
            raw_width: reference.width(),
            circuit: self.program.circuit.clone(),
            positions: self.program.positions.clone(),
            phase: (**phase).clone(),
            shared: self.program.shared.clone(),
            compilation: Arc::clone(&self.compilation),
            work,
            probe: self.amplitude_ownership_probe(),
            queries: closed,
            max_terms: options.max_terms,
        })
    }
}
impl UnitaryCircuit {
    /// Explicit trace-purpose compilation. The common full dilation is
    /// transient; no amplitude projection family is prepared or retained.
    pub fn compile_trace(
        &self,
        input: &BitVector,
        strategy: CompilerStrategy,
        queries: &[TraceQuery],
        options: TraceOptions,
        a: &ScalarArithmetic,
    ) -> Result<TraceRuntime, UnitaryError> {
        validate_queries(self.width(), queries)?;
        if queries.len() > options.max_queries {
            return Err(UnitaryError::QueryCacheLimit {
                option: "trace.max_queries",
                limit: options.max_queries,
                required: queries.len() as u128,
            });
        }
        if input.width() != self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let input = CliffordCircuit::computational(input.clone())?;
        input::validate_stabilizer_input(&input, self.width(), a)?;
        let compiled = match strategy {
            CompilerStrategy::ParityCopies => {
                self.compile_parity(&input, a, options.max_path_bits)?
            }
            CompilerStrategy::SharedCommutingLayer => shared::compile(
                self,
                &input,
                a,
                options.max_path_bits,
                CompilationPurpose::Trace,
            )?,
        };
        compiled.into_trace_runtime(queries, options, a)
    }
}
fn close_physical(
    reference: &AffineKet,
    physical: usize,
    q: &TraceQuery,
    a: &ScalarArithmetic,
) -> Result<Option<AffineKet>, UnitaryError> {
    let half = reference.width();
    let lifted = PauliWord::new(
        BitVector::from_indices(half, q.word.x().ones())?,
        BitVector::from_indices(half, q.word.z().ones())?,
        q.word.phase(),
    )?;
    let joint = moment_joint(reference, &lifted, a)?;
    let constraints = (0..physical)
        .map(|j| BitVector::from_indices(joint.width(), [j, half + j]))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(joint) = joint.constrain_z(&constraints, &BitVector::zeros(physical)?)? else {
        return Ok(None);
    };
    // Only event parities and the two magic-coordinate legs survive. All
    // physical bra/ket coordinates are summed before any runtime is returned.
    let mut rows = q
        .event_rows
        .iter()
        .map(|row| BitVector::from_indices(joint.width(), row.ones()))
        .collect::<Result<Vec<_>, _>>()?;
    rows.extend(
        (physical..half)
            .chain(half + physical..joint.width())
            .map(|j| BitVector::from_indices(joint.width(), [j]))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(joint.linear_boundary(&rows)?.contract(a)?)
}
impl TraceRuntime {
    pub fn amplitude_ownership_probe(&self) -> AmplitudeOwnershipProbe {
        self.probe.clone()
    }
    pub fn compilation(&self) -> &CompilationDiagnostics {
        &self.compilation
    }
    pub fn reference_phase(&self) -> &ParityPhase {
        &self.phase
    }
    pub fn query_work_statistics(&self) -> QueryWorkStatistics {
        self.work.snapshot()
    }
    pub fn storage_statistics(&self) -> TraceStorageStatistics {
        TraceStorageStatistics {
            event_phase_widths: self
                .queries
                .iter()
                .filter_map(|q| q.kernel.as_ref().map(AffineKet::width))
                .collect(),
            closed_path_variables: self
                .queries
                .iter()
                .filter_map(|q| q.kernel.as_ref().map(|k| k.columns().len()))
                .collect(),
            original_positions: self.positions.len() - 1,
            clifford_gates: self.circuit.gates().len(),
            phase_variables: self.phase.num_variables(),
        }
    }
    pub fn assignment(
        &self,
        insertions: &[PauliInsertion],
    ) -> Result<TraceAssignment, UnitaryError> {
        let (frame, grouped) = transport_insertions(
            self.physical,
            self.raw_width,
            self.circuit.gates(),
            &self.positions,
            insertions,
        )?;
        let phase = match &self.shared {
            Some(s) => s.phase(&grouped)?,
            None => self.phase.clone(),
        };
        // Change raw magic summation variables by the final frame's X. Its Z
        // on magic legs remains a Clifford phase; physical conjugation is
        // handled separately by each observable/event query. Global frame
        // phase cancels between bra and ket, but is retained in the snapshot.
        let mut constant = phase.constant();
        let mut terms = Vec::with_capacity(phase.terms().len() + self.phase.num_variables());
        for (axis, coefficient) in phase.terms() {
            let coefficient = *coefficient;
            let flip = axis.ones().try_fold(false, |v, j| {
                frame.x().get(self.physical + j).map(|b| v ^ b)
            })?;
            if flip {
                constant = (constant + coefficient) & 7;
            }
            terms.push((
                axis.clone(),
                if flip {
                    (8 - coefficient) & 7
                } else {
                    coefficient
                },
            ));
        }
        for j in 0..self.phase.num_variables() {
            if frame.z().get(self.physical + j)? {
                terms.push((BitVector::from_indices(self.phase.num_variables(), [j])?, 4));
            }
        }
        self.work.record(&self.work.assignments);
        Ok(TraceAssignment {
            owner: Arc::clone(&self.owner),
            frame: CompactPauliWord::from_dense(&frame),
            phase: ParityPhase::new(self.phase.num_variables(), constant, terms)?,
        })
    }
    pub fn evaluate(
        &self,
        index: usize,
        assignment: &TraceAssignment,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if !Arc::ptr_eq(&self.owner, &assignment.owner) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        let q = self
            .queries
            .get(index)
            .ok_or(UnitaryError::InvalidTraceQuery {
                index,
                count: self.queries.len(),
            })?;
        if outcomes.width() != q.rows.len() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let (Some(kernel), Some(closure)) = (&q.kernel, &q.closure) else {
            return Ok(zero_network_value());
        };
        let frame = assignment.frame.materialize()?;
        let mut rhs = outcomes.clone();
        for (j, row) in q.rows.iter().enumerate() {
            let flip = row
                .ones()
                .try_fold(false, |v, k| frame.x().get(k).map(|b| v ^ b))?;
            rhs.set(j, rhs.get(j)? ^ flip)?;
        }
        let mut value = closure.evaluate(
            kernel,
            &rhs,
            &[(&assignment.phase, true), (&assignment.phase, false)],
            self.max_terms,
            a,
        )?;
        let mut negative = false;
        for j in q.word.x().ones() {
            negative ^= frame.z().get(j)?;
        }
        for j in q.word.z().ones() {
            negative ^= frame.x().get(j)?;
        }
        if negative {
            value.value = value.value.negated();
        }
        Ok(value)
    }
}

/// A trace-only conditional record draw and its unnormalized prefix mass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRecordDraw {
    pub values: BitVector,
    pub mass: ExactScalar,
    pub condition_mass: ExactScalar,
    pub entropy_bits: usize,
}
#[derive(Debug)]
pub enum TraceSamplingError {
    Unitary(UnitaryError),
    Sampling(crate::records::BitSamplingError),
    InvalidChain,
}
impl fmt::Display for TraceSamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unitary(e) => e.fmt(f),
            Self::Sampling(e) => e.fmt(f),
            Self::InvalidChain => write!(
                f,
                "trace sampling requires identity-observable queries with one appended event row per prefix"
            ),
        }
    }
}
impl std::error::Error for TraceSamplingError {}
impl TraceRuntime {
    /// Sample a declared prefix chain using the closed trace kernels only.
    /// The first layout is conditioned on `given`; each subsequent layout adds
    /// one row. Whole-chain identity/shape/owner validation precedes entropy.
    /// True Bernoulli decisions choose the zero branch, matching low-word
    /// sampling. Failed refinement can consume a prefix of the bit budget.
    pub fn sample_event_chain(
        &self,
        chain: &[usize],
        assignment: &TraceAssignment,
        given: &BitVector,
        random_bit: &mut impl FnMut() -> bool,
        max_entropy_bits: usize,
        a: &ScalarArithmetic,
    ) -> Result<TraceRecordDraw, TraceSamplingError> {
        use TraceSamplingError::{InvalidChain, Unitary};
        if !Arc::ptr_eq(&self.owner, &assignment.owner) {
            return Err(Unitary(UnitaryError::IncompatibleProgram));
        }
        if chain.is_empty() {
            return Err(InvalidChain);
        }
        let mut previous: Option<&Query> = None;
        for &index in chain {
            let q = self
                .queries
                .get(index)
                .ok_or(Unitary(UnitaryError::InvalidTraceQuery {
                    index,
                    count: self.queries.len(),
                }))?;
            if !q.word.x().is_zero() || !q.word.z().is_zero() || q.word.phase() != 0 {
                return Err(InvalidChain);
            }
            match previous {
                None if q.rows.len() != given.width() => return Err(InvalidChain),
                Some(old)
                    if q.rows.len() != old.rows.len() + 1
                        || q.rows[..old.rows.len()] != old.rows =>
                {
                    return Err(InvalidChain);
                }
                _ => {}
            }
            previous = Some(q);
        }
        let initial = self
            .evaluate(chain[0], assignment, given, a)
            .map_err(Unitary)?
            .value;
        let mut mass = initial.clone();
        let mut prefix = given.clone();
        let mut values = BitVector::zeros(chain.len() - 1)
            .map_err(UnitaryError::from)
            .map_err(Unitary)?;
        let mut entropy_bits = 0;
        for (j, &index) in chain[1..].iter().enumerate() {
            let candidate = BitVector::from_indices(prefix.width() + 1, prefix.ones())
                .map_err(UnitaryError::from)
                .map_err(Unitary)?;
            let low = self
                .evaluate(index, assignment, &candidate, a)
                .map_err(Unitary)?
                .value;
            let draw = crate::records::bernoulli_ratio_bits(
                &low,
                &mass,
                random_bit,
                max_entropy_bits - entropy_bits,
                a,
            )
            .map_err(TraceSamplingError::Sampling)?;
            entropy_bits += draw.entropy_bits;
            let bit = !draw.selected;
            prefix = candidate;
            prefix
                .set(given.width() + j, bit)
                .map_err(UnitaryError::from)
                .map_err(Unitary)?;
            values
                .set(j, bit)
                .map_err(UnitaryError::from)
                .map_err(Unitary)?;
            mass = if bit {
                a.subtract(&mass, &low)
                    .map_err(UnitaryError::from)
                    .map_err(Unitary)?
            } else {
                low
            };
        }
        // Even an empty chain must reject a zero/nonreal conditioning mass.
        crate::records::bernoulli_ratio_bits(
            &mass,
            &mass,
            &mut || unreachable!("endpoint consumes no entropy"),
            0,
            a,
        )
        .map_err(TraceSamplingError::Sampling)?;
        Ok(TraceRecordDraw {
            values,
            mass,
            condition_mass: initial,
            entropy_bits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_compiler_preserves_the_exact_unreduced_dilation_and_phase_metadata() {
        let a = ScalarArithmetic::default();
        let input = CliffordCircuit::computational(BitVector::zeros(2).unwrap()).unwrap();
        let circuit = UnitaryCircuit::new(
            2,
            vec![
                UnitaryGate::Clifford(CliffordGate::H(0)),
                UnitaryGate::T { qubit: 0, turns: 1 },
                UnitaryGate::Clifford(CliffordGate::Cx {
                    control: 0,
                    target: 1,
                }),
                UnitaryGate::T { qubit: 1, turns: 3 },
                UnitaryGate::Clifford(CliffordGate::H(0)),
                UnitaryGate::T { qubit: 0, turns: 7 },
            ],
        )
        .unwrap();
        let open = shared::compile(&circuit, &input, &a, 4096, CompilationPurpose::Open).unwrap();
        let trace = shared::compile(&circuit, &input, &a, 4096, CompilationPurpose::Trace).unwrap();
        assert_eq!(open.program.reference, trace.program.reference);
        assert_eq!(open.program.reference_phase, trace.program.reference_phase);
        assert_eq!(open.program.positions, trace.program.positions);
        assert_eq!(open.program.circuit.gates(), trace.program.circuit.gates());
        assert!(Arc::ptr_eq(
            &trace.program.reference,
            &trace.program.query_reference
        ));
        let runtime = trace
            .into_trace_runtime(&[], TraceOptions::default(), &a)
            .unwrap();
        assert!(runtime.amplitude_ownership_probe().is_released());
        assert_eq!(
            runtime.reference_phase(),
            open.program.reference_phase.as_ref()
        );
    }
}
