//! Bounded direct static boundary tasks over reported record coordinates.
use super::*;
use crate::gf2::LinearFactorization;
use crate::noise::RandomSource;
use crate::unitary::NetworkValue;

/// Independent admission for record-coordinate algebra and retained task queries.
#[derive(Clone, Copy, Debug)]
pub struct OriginalBoundaryLimits {
    pub max_records: usize,
    pub max_postselection_rows: usize,
    pub max_record_occurrences: usize,
    pub max_observables: usize,
    /// Prospective families: unconditional mass, an acceptance query for nonempty
    /// postselection, and one per observable. Incompatible equations can skip
    /// conditional families; admission still reserves their conservative bound.
    pub max_queries: usize,
    pub max_metadata_bytes: usize,
    pub max_layout_words: usize,
    /// Conservative aggregate packed words, including simultaneous constructor
    /// temporaries and all prospective scalar families, even for an empty event.
    pub max_task_words: usize,
}
impl Default for OriginalBoundaryLimits {
    fn default() -> Self {
        Self {
            max_records: 1 << 20,
            max_postselection_rows: 1 << 20,
            max_record_occurrences: 1 << 20,
            max_observables: 1024,
            max_queries: 1026,
            max_metadata_bytes: 1 << 24,
            max_layout_words: 1 << 24,
            max_task_words: 1 << 26,
        }
    }
}

/// Fixed reported postselection and ordered signed physical-output effects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OriginalBoundaryTask {
    pub postselection: Vec<OriginalRecordEvent>,
    pub observables: Vec<PauliWord>,
}
/// The final scalar resource/catalogue and its physical geometry limits remain
/// explicit, independently of the layout and aggregate task budgets.
#[derive(Clone, Copy, Debug, Default)]
pub struct OriginalBoundaryOptions<'a> {
    pub network: OriginalNetworkOptions<'a>,
    pub limits: OriginalBoundaryLimits,
}

fn occurrences(events: &[OriginalRecordEvent]) -> u128 {
    events.iter().fold(0_u128, |n, e| {
        n.saturating_add(e.parity.records.len() as u128)
    })
}
fn metadata_bytes(records: &[String], events: &[OriginalRecordEvent]) -> u128 {
    records
        .iter()
        .fold(0_u128, |n, r| n.saturating_add(32 + r.len() as u128))
        .saturating_add(events.iter().fold(0_u128, |n, e| {
            e.parity.records.iter().fold(n.saturating_add(64), |n, r| {
                n.saturating_add(32 + r.len() as u128)
            })
        }))
}
fn admit_layout(
    records: &[String],
    events: &[OriginalRecordEvent],
    limits: OriginalBoundaryLimits,
) -> Result<u128, OriginalError> {
    admit(
        "max_boundary_records",
        limits.max_records,
        records.len() as u128,
    )?;
    admit(
        "max_boundary_postselection_rows",
        limits.max_postselection_rows,
        events.len() as u128,
    )?;
    admit(
        "max_boundary_record_occurrences",
        limits.max_record_occurrences,
        occurrences(events),
    )?;
    admit(
        "max_boundary_metadata_bytes",
        limits.max_metadata_bytes,
        metadata_bytes(records, events),
    )?;
    let (n, e) = (records.len() as u128, events.len() as u128);
    // Covers the full coordinate transform and inverse, retained/reduced input
    // rows, dense GF2 provenance (also bounds its tall-system alternative),
    // borrowed-name index nodes and simultaneously live constructor copies.
    let words = 32_u128.saturating_mul(
        (4 * n + 2 * e + 1)
            .saturating_mul(n.div_ceil(64) + 8)
            .saturating_add(e.saturating_mul(e.div_ceil(64) + 4))
            .saturating_add(occurrences(events))
            .saturating_add(n + e + 128),
    );
    admit("max_boundary_layout_words", limits.max_layout_words, words)?;
    Ok(words)
}

/// Invertible linear change of reported-record coordinates. Postselection basis
/// rows come first; the deterministic complement is traced, including unused
/// deterministic records. This classifies record roles, not random outcomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecordLayout {
    transform: Vec<BitVector>,
    inverse: Vec<BitVector>,
    fixed: Option<BitVector>,
    rank: usize,
    geometry_words: u128,
}
impl OriginalRecordLayout {
    /// Indices in both transforms follow `records`. Repeated parity names cancel;
    /// inverted parity constants participate in RHS compatibility. Unknown and
    /// duplicate record identities are rejected after metadata admission.
    pub fn new(
        records: &[String],
        events: &[OriginalRecordEvent],
        limits: OriginalBoundaryLimits,
    ) -> Result<Self, OriginalError> {
        let geometry_words = admit_layout(records, events, limits)?;
        let n = records.len();
        let mut names = BTreeMap::new();
        for (i, name) in records.iter().enumerate() {
            if names.insert(name.as_str(), i).is_some() {
                return Err(invalid("duplicate boundary record", name));
            }
        }
        let mut rows = Vec::with_capacity(events.len());
        let mut rhs = BitVector::zeros(events.len())?;
        for (i, event) in events.iter().enumerate() {
            let mut row = BitVector::zeros(n)?;
            for name in &event.parity.records {
                let &j = names
                    .get(name.as_str())
                    .ok_or_else(|| invalid("unknown boundary record", name))?;
                row.set(j, !row.get(j)?)?;
            }
            rows.push(row);
            rhs.set(i, event.value ^ event.parity.inverted)?;
        }
        let factor = LinearFactorization::new(n, &rows)?;
        let rank = factor.rank();
        let solution = factor.solve(&rhs)?;
        let fixed = solution
            .as_ref()
            .map(|point| {
                BitVector::from_indices(
                    rank,
                    factor
                        .pivot_columns()
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &j)| point.get(j).expect("validated pivot").then_some(i)),
                )
            })
            .transpose()?;
        let mut transform = factor.row_space_basis().to_vec();
        let mut inverse = (0..n)
            .map(|_| BitVector::zeros(n))
            .collect::<Result<Vec<_>, _>>()?;
        for (i, &pivot) in factor.pivot_columns().iter().enumerate() {
            inverse[pivot].set(i, true)?;
        }
        for (i, free) in (0..n)
            .filter(|j| factor.pivot_columns().binary_search(j).is_err())
            .enumerate()
        {
            transform.push(BitVector::from_indices(n, [free])?);
            inverse[free].set(rank + i, true)?;
            for (j, &pivot) in factor.pivot_columns().iter().enumerate() {
                inverse[pivot].set(rank + i, factor.row_space_basis()[j].get(free)?)?;
            }
        }
        Ok(Self {
            transform,
            inverse,
            fixed,
            rank,
            geometry_words,
        })
    }
    pub fn compatible(&self) -> bool {
        self.fixed.is_some()
    }
    pub fn postselection_rank(&self) -> usize {
        self.rank
    }
    /// Static tasks have no feedback parents.
    pub fn feedback_rank(&self) -> usize {
        0
    }
    pub fn trace_rank(&self) -> usize {
        self.transform.len() - self.rank
    }
    pub fn transform(&self) -> &[BitVector] {
        &self.transform
    }
    pub fn inverse_transform(&self) -> &[BitVector] {
        &self.inverse
    }
    /// Values of the leading independent postselection coordinates, or `None`
    /// when redundant equations have incompatible RHSs.
    pub fn postselection_values(&self) -> Option<&BitVector> {
        self.fixed.as_ref()
    }
    pub fn geometry_words(&self) -> u128 {
        self.geometry_words
    }
    pub fn to_virtual(&self, records: &BitVector) -> Result<BitVector, OriginalError> {
        self.map(&self.transform, records)
    }
    pub fn to_records(&self, coordinates: &BitVector) -> Result<BitVector, OriginalError> {
        self.map(&self.inverse, coordinates)
    }
    fn map(&self, rows: &[BitVector], value: &BitVector) -> Result<BitVector, OriginalError> {
        if value.width() != rows.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        let mut result = BitVector::zeros(rows.len())?;
        for (i, row) in rows.iter().enumerate() {
            result.set(i, row.dot(value)?)?;
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalBoundaryQuery {
    Mass,
    Acceptance,
    Moment(usize),
}
/// Actual evaluated final scalar family; never a coherent-side certificate.
#[derive(Clone, Debug)]
pub struct OriginalBoundaryResource {
    pub query: OriginalBoundaryQuery,
    pub quotient_rank: usize,
    pub constructive_terms: usize,
    pub native_transport: Option<crate::catalogue::CatalogueBoundaryStatistics>,
}
impl OriginalBoundaryResource {
    fn new(query: OriginalBoundaryQuery, value: &NetworkValue) -> Self {
        Self {
            query,
            quotient_rank: value.quotient_rank,
            constructive_terms: value.terms,
            native_transport: value.native_transport,
        }
    }
}
#[derive(Clone, Debug)]
pub struct OriginalBoundaryDraw {
    pub mass: ExactScalar,
    pub acceptance: ExactScalar,
    /// Joint signed moments with postselection, in declared observable order.
    pub moments: Vec<ExactScalar>,
    /// Direct static tasks require no sampled reports.
    pub reports: BTreeMap<String, bool>,
    pub final_resources: Vec<OriginalBoundaryResource>,
}

/// Prepared static direct service. Assignment replacement uses the original
/// immutable transport; physical tracing precedes final resource selection.
pub struct OriginalBoundaryService<'a> {
    layout: OriginalRecordLayout,
    mass: OriginalNetworkProbability<'a>,
    acceptance: Option<OriginalNetworkProbability<'a>>,
    moments: Vec<OriginalNetworkMoment<'a>>,
    outcomes: BitVector,
    observable_count: usize,
    geometry_words: u128,
}
impl OriginalStaticTrace {
    /// Retain an exact direct static boundary task. Opaque/structured controllers
    /// must already have been rejected by `compile_trace`; no callback is called.
    /// Native two-term catalogue behavior requires an explicitly supplied database.
    ///
    /// ```
    /// use rmagna::{clifford::CliffordCircuit, gf2::BitVector,
    ///     instrument::{OriginalInstrument, OriginalLimits, OriginalBoundaryTask,
    ///         OriginalBoundaryOptions, OriginalTraceAssignment}, noise::SplitMix64,
    ///     scalar::{ExactScalar, ScalarArithmetic}, unitary::CompilerStrategy};
    /// let a = ScalarArithmetic::default();
    /// let instrument = OriginalInstrument::new(vec![], vec![],
    ///     CliffordCircuit::computational(BitVector::zeros(0)?)?, vec![], vec![],
    ///     OriginalLimits::default(), &a)?;
    /// let trace = instrument.compile_trace(CompilerStrategy::ParityCopies, &a)?;
    /// let service = trace.prepare_boundary_task(&OriginalBoundaryTask::default(),
    ///     &OriginalBoundaryOptions::default(), &a)?;
    /// let view = trace.with_assignment(&OriginalTraceAssignment::default(), &a)?;
    /// let mut rng = SplitMix64::new(7);
    /// let state = rng.state();
    /// let draw = service.sample(&view, &mut rng, &a)?;
    /// assert_eq!(draw.acceptance, ExactScalar::one());
    /// assert_eq!(rng.state(), state);
    /// assert!(draw.reports.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prepare_boundary_task(
        &self,
        task: &OriginalBoundaryTask,
        options: &OriginalBoundaryOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalBoundaryService<'_>, OriginalError> {
        let limits = options.limits;
        let layout_words = admit_layout(self.records(), &task.postselection, limits)?;
        let event_count = task.postselection.len();
        let count = 1_u128 + u128::from(event_count != 0) + task.observables.len() as u128;
        admit(
            "max_boundary_observables",
            limits.max_observables,
            task.observables.len() as u128,
        )?;
        admit("max_boundary_queries", limits.max_queries, count)?;
        admit(
            "max_event_rows",
            options
                .network
                .limits
                .max_event_rows
                .min(self.limits.expansion.max_event_rows)
                .min(self.limits.completed.max_event_rows),
            event_count as u128,
        )?;
        let occurrences = occurrences(&task.postselection);
        admit(
            "max_record_occurrences",
            options.network.max_record_occurrences,
            occurrences,
        )?;
        let output_words = (self.outputs.len() as u128).div_ceil(64);
        let bytes = metadata_bytes(self.records(), &task.postselection).saturating_add(
            (task.observables.len() as u128).saturating_mul(64 + 16 * output_words),
        );
        admit(
            "max_metadata_bytes",
            self.limits
                .max_metadata_bytes
                .min(limits.max_metadata_bytes),
            bytes,
        )?;
        for word in &task.observables {
            if word.width() != self.outputs.len() {
                return Err(OriginalError::DimensionMismatch);
            }
            if word.phase() & 3 != 2 * u8::from(word.x().dot(word.z())?) {
                return Err(CompletedRecordError::NonHermitianPauli.into());
            }
        }
        let norm_words = network::admit_geometry(self, 0, 0, &options.network)?;
        let event_words =
            network::admit_geometry(self, event_count, occurrences, &options.network)?;
        let geometry_words = layout_words
            .saturating_add(norm_words)
            .saturating_add((count - 1).saturating_mul(event_words))
            .saturating_add(
                count.saturating_mul(128 + 2 * (self.network_width() as u128).div_ceil(64)),
            );
        admit(
            "max_boundary_task_words",
            limits.max_task_words,
            geometry_words,
        )?;
        let layout = OriginalRecordLayout::new(self.records(), &task.postselection, limits)?;
        let mass = self.prepare_network_probability(&[], &options.network, a)?;
        let outcomes = BitVector::from_indices(
            event_count,
            task.postselection
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.value.then_some(i)),
        )?;
        let mut acceptance = None;
        let mut moments = Vec::new();
        if layout.compatible() {
            let events = task
                .postselection
                .iter()
                .map(|e| e.parity.clone())
                .collect::<Vec<_>>();
            if event_count != 0 {
                acceptance =
                    Some(self.prepare_network_probability(&events, &options.network, a)?);
            }
            for word in &task.observables {
                let word = PauliWord::new(
                    BitVector::from_indices(self.network_width(), word.x().ones())?,
                    BitVector::from_indices(self.network_width(), word.z().ones())?,
                    word.phase(),
                )?;
                moments.push(self.prepare_network_moment(&word, &events, &options.network, a)?);
            }
        }
        Ok(OriginalBoundaryService {
            layout,
            mass,
            acceptance,
            moments,
            outcomes,
            observable_count: task.observables.len(),
            geometry_words,
        })
    }
}

impl OriginalBoundaryService<'_> {
    pub fn layout(&self) -> &OriginalRecordLayout {
        &self.layout
    }
    pub fn geometry_words(&self) -> u128 {
        self.geometry_words
    }
    pub fn required_reports(&self) -> &[String] {
        &[]
    }
    pub fn evaluate(
        &self,
        view: &OriginalTraceView<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalBoundaryDraw, OriginalError> {
        let mass = self.mass.evaluate(view, &BitVector::zeros(0)?, a)?;
        let mut resources = vec![OriginalBoundaryResource::new(
            OriginalBoundaryQuery::Mass,
            &mass,
        )];
        let (acceptance, moments) = if self.layout.compatible() {
            let acceptance = if let Some(query) = &self.acceptance {
                let result = query.evaluate(view, &self.outcomes, a)?;
                resources.push(OriginalBoundaryResource::new(
                    OriginalBoundaryQuery::Acceptance,
                    &result,
                ));
                result.value
            } else {
                mass.value.clone()
            };
            let mut moments = Vec::with_capacity(self.moments.len());
            for (i, query) in self.moments.iter().enumerate() {
                let result = query.evaluate(view, &self.outcomes, a)?;
                resources.push(OriginalBoundaryResource::new(
                    OriginalBoundaryQuery::Moment(i),
                    &result,
                ));
                moments.push(result.value);
            }
            (acceptance, moments)
        } else {
            (
                ExactScalar::zero(),
                vec![ExactScalar::zero(); self.observable_count],
            )
        };
        Ok(OriginalBoundaryDraw {
            mass: mass.value,
            acceptance,
            moments,
            reports: BTreeMap::new(),
            final_resources: resources,
        })
    }
    /// Direct static adapter: exact masses require no sampled reports and consume
    /// no random words, on either success or error. No trajectory is generated.
    pub fn sample<R: RandomSource + ?Sized>(
        &self,
        view: &OriginalTraceView<'_>,
        _rng: &mut R,
        a: &ScalarArithmetic,
    ) -> Result<OriginalBoundaryDraw, OriginalError> {
        self.evaluate(view, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clifford::{CliffordCircuit, CliffordGate};
    use crate::gf2::FACTORIZATION_CALLS;

    #[test]
    fn task_admission_precedes_actual_layout_and_global_query_constructors() {
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
                id: "m".into(),
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
        let task = OriginalBoundaryTask {
            postselection: vec![OriginalRecordEvent {
                parity: OriginalRecordParity {
                    records: vec!["a".into()],
                    inverted: false,
                },
                value: false,
            }],
            observables: vec![PauliWord::identity(0).unwrap()],
        };
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let trace = instrument.compile_trace(strategy, &a).unwrap();
            let options = OriginalBoundaryOptions::default();
            FACTORIZATION_CALLS.with(|n| n.set(0));
            let good = trace.prepare_boundary_task(&task, &options, &a).unwrap();
            assert!(FACTORIZATION_CALLS.with(|n| n.get()) > 0);
            let l = options.limits;
            let bad_limits = [
                OriginalBoundaryLimits {
                    max_records: 0,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_postselection_rows: 0,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_record_occurrences: 0,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_observables: 0,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_queries: 2,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_metadata_bytes: 0,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_layout_words: good.layout().geometry_words() as usize - 1,
                    ..l
                },
                OriginalBoundaryLimits {
                    max_task_words: good.geometry_words() as usize - 1,
                    ..l
                },
            ];
            for limits in bad_limits {
                FACTORIZATION_CALLS.with(|n| n.set(0));
                assert!(matches!(
                    trace.prepare_boundary_task(
                        &task,
                        &OriginalBoundaryOptions { limits, ..options },
                        &a
                    ),
                    Err(OriginalError::ResourceLimit { .. })
                ));
                assert_eq!(FACTORIZATION_CALLS.with(|n| n.get()), 0);
            }
            for network in [
                OriginalNetworkOptions {
                    max_record_occurrences: 0,
                    ..options.network
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_event_rows: 0,
                        ..options.network.limits
                    },
                    ..options.network
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_joint_paths: 1,
                        ..options.network.limits
                    },
                    ..options.network
                },
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_geometry_words: 0,
                        ..options.network.limits
                    },
                    ..options.network
                },
            ] {
                FACTORIZATION_CALLS.with(|n| n.set(0));
                assert!(matches!(
                    trace.prepare_boundary_task(
                        &task,
                        &OriginalBoundaryOptions { network, ..options },
                        &a
                    ),
                    Err(OriginalError::ResourceLimit { .. })
                ));
                assert_eq!(FACTORIZATION_CALLS.with(|n| n.get()), 0);
            }
            let view = trace
                .with_assignment(&OriginalTraceAssignment::default(), &a)
                .unwrap();
            assert_eq!(
                good.evaluate(&view, &a).unwrap().acceptance,
                a.scalar([1, 0, 0, 0].map(Into::into), -1).unwrap()
            );
        }
    }
}
