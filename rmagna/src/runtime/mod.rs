//! Causal reported-history proposals, controllers and guarded-noise execution.
pub mod process;
mod registry;
mod resolve;
use crate::circuit::{BitAtom, BitExpr, Circuit, LocatedOperation, Operation};
use crate::gf2::BitVector;
use crate::instrument::{
    CompileOptions, CompiledInstrument, ExactMasses, FaultSelection, InstrumentError,
    compile_affine, compile_causal,
};
use crate::noise::{NoiseDraw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler, RandomSource};
use crate::records::{
    RecordQuery, SamplingError, SamplingOptions, propose_affine_reported,
    sample_reported_conditioned,
};
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::unitary::CompiledUnitary;
use num_rational::BigRational;
use num_traits::One;
pub use registry::{Controller, ControllerRegistry};
use std::{collections::BTreeMap, fmt};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    MissingBinding {
        name: String,
    },
    InvalidBinding {
        name: String,
    },
    DuplicateBinding {
        name: String,
    },
    Controller {
        name: String,
        call: usize,
        message: String,
    },
    OutputShape {
        name: String,
        call: usize,
        expected: usize,
        actual: usize,
    },
    UnresolvedDecision {
        call: usize,
        bit: usize,
    },
    ResourceLimit {
        option: &'static str,
        limit: usize,
    },
    InvalidHistory {
        group: usize,
        message: String,
    },
    InternalSite {
        operation: usize,
    },
    Noise(NoiseError),
    Instrument(InstrumentError),
    Sampling(SamplingError),
}
impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBinding { name } => write!(f, "controller {name:?} has no binding"),
            Self::InvalidBinding { name } => write!(
                f,
                "controller binding {name:?} requires a nonempty non-built-in name and version"
            ),
            Self::DuplicateBinding { name } => {
                write!(f, "controller {name:?} is already registered")
            }
            Self::Controller {
                name,
                call,
                message,
            } => write!(f, "controller {name:?} call {call} failed: {message}"),
            Self::OutputShape {
                name,
                call,
                expected,
                actual,
            } => write!(
                f,
                "controller {name:?} call {call} returned {actual} bits; expected {expected}"
            ),
            Self::UnresolvedDecision { call, bit } => write!(
                f,
                "decision bit {bit} of controller call {call} is not available"
            ),
            Self::ResourceLimit { option, limit } => {
                write!(f, "causal runtime exhausted {option} = {limit}")
            }
            Self::InvalidHistory { group, message } => {
                write!(f, "prescribed history group {group}: {message}")
            }
            Self::InternalSite { operation } => write!(
                f,
                "noise source operation {operation} is absent from the reference category map"
            ),
            Self::Noise(error) => error.fmt(f),
            Self::Instrument(error) => error.fmt(f),
            Self::Sampling(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RuntimeError {}
impl From<NoiseError> for RuntimeError {
    fn from(e: NoiseError) -> Self {
        Self::Noise(e)
    }
}
impl From<InstrumentError> for RuntimeError {
    fn from(e: InstrumentError) -> Self {
        Self::Instrument(e)
    }
}
impl From<SamplingError> for RuntimeError {
    fn from(e: SamplingError) -> Self {
        Self::Sampling(e)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CausalStrategy {
    FixedTemplate,
    ColdPrefix,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistorySampler {
    ExactMarginal,
    AffineSupport,
}
#[derive(Clone, Copy, Debug)]
pub struct RuntimeOptions {
    pub history_sampler: HistorySampler,
    pub strategy: CausalStrategy,
    pub max_history_queries: usize,
    pub compile: CompileOptions,
    pub noise: NoiseOptions,
    pub sampling: SamplingOptions,
    pub max_history_inputs: usize,
    pub max_prefix_compiles: usize,
}
impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            strategy: CausalStrategy::FixedTemplate,
            history_sampler: HistorySampler::ExactMarginal,
            max_history_queries: 100_000,
            compile: CompileOptions::default(),
            noise: NoiseOptions::default(),
            sampling: SamplingOptions::default(),
            max_history_inputs: 1_000_000,
            max_prefix_compiles: 100_000,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerCall {
    pub name: String,
    pub call: usize,
    pub inputs: Vec<bool>,
    pub outputs: Vec<bool>,
}
pub struct CausalDraw {
    pub masses: ExactMasses,
    pub history_mass: ExactScalar,
    pub history_proposal_mass: Option<ExactScalar>,
    /// Probability of the active physical noise choices along this history.
    pub noise_probability: BigRational,
    pub conditions: Vec<(BitExpr, bool)>,
    pub controller_calls: Vec<ControllerCall>,
    pub noise_entropy_words: usize,
    pub history_entropy_words: usize,
    pub network_compilations: usize,
    pub history_queries: usize,
}
pub struct ResolvedHistory {
    instrument: CompiledInstrument,
    faults: Vec<FaultSelection>,
    history_mass: ExactScalar,
    history_proposal_mass: Option<ExactScalar>,
    noise_probability: BigRational,
    conditions: Vec<(BitExpr, bool)>,
    controller_calls: Vec<ControllerCall>,
    requested_values: Vec<bool>,
    decisions: BTreeMap<(usize, usize), bool>,
    noise_entropy_words: usize,
    history_entropy_words: usize,
    network_compilations: usize,
    history_queries: usize,
}
impl ResolvedHistory {
    pub fn instrument(&self) -> &CompiledInstrument {
        &self.instrument
    }
    pub fn faults(&self) -> &[FaultSelection] {
        &self.faults
    }
    pub fn history_mass(&self) -> &ExactScalar {
        &self.history_mass
    }
    /// Present only for actual affine-support proposals; prescribed diagnostic
    /// histories retain physical conditional normalization.
    pub fn history_proposal_mass(&self) -> Option<&ExactScalar> {
        self.history_proposal_mass.as_ref()
    }
    pub fn noise_probability(&self) -> &BigRational {
        &self.noise_probability
    }
    pub fn conditions(&self) -> &[(BitExpr, bool)] {
        &self.conditions
    }
    pub fn controller_calls(&self) -> &[ControllerCall] {
        &self.controller_calls
    }
    pub fn requested_values(&self) -> &[bool] {
        &self.requested_values
    }
    pub fn entropy_words(&self) -> (usize, usize) {
        (self.noise_entropy_words, self.history_entropy_words)
    }
    /// Actual post-preparation Clifford network constructions for this history.
    pub fn network_compilations(&self) -> usize {
        self.network_compilations
    }
    pub fn history_queries(&self) -> usize {
        self.history_queries
    }
    pub fn resolve_expression(&self, expression: &BitExpr) -> Result<BitExpr, RuntimeError> {
        resolve::expression(expression, &self.decisions, false)
    }
    fn pure_draw(
        self,
        outputs: &[usize],
        target: &CompiledUnitary,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CausalDraw, RuntimeError> {
        let masses =
            self.instrument
                .masses(outputs, target, &self.faults, max_terms, arithmetic)?;
        Ok(CausalDraw {
            masses,
            history_mass: self.history_mass,
            history_proposal_mass: self.history_proposal_mass,
            noise_probability: self.noise_probability,
            conditions: self.conditions,
            controller_calls: self.controller_calls,
            noise_entropy_words: self.noise_entropy_words,
            history_entropy_words: self.history_entropy_words,
            network_compilations: self.network_compilations,
            history_queries: self.history_queries,
        })
    }
}
pub struct CausalProgram {
    circuit: Circuit,
    model: NoiseModel,
    sampler: NoiseSampler,
    registry: ControllerRegistry,
    source_sites: BTreeMap<usize, usize>,
    options: RuntimeOptions,
    reference: Option<CompiledInstrument>,
    compilation: crate::unitary::CompilationDiagnostics,
    guarded_sites: std::collections::BTreeSet<usize>,
}
impl CausalProgram {
    /// Compile one fixed coherent template, or the explicitly selected cold
    /// prefix baseline. Controller names are checked before drawing entropy.
    /// Future operations are trace preserving in physical history queries.
    pub fn new(
        circuit: &Circuit,
        registry: &ControllerRegistry,
        options: RuntimeOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, RuntimeError> {
        let names = registry.names();
        for op in circuit.operations() {
            if let Operation::Control {
                name,
                call: Some(_),
                ..
            } = &op.operation
                && !names.contains(name)
            {
                return Err(RuntimeError::MissingBinding { name: name.clone() });
            }
        }
        let reference = match options.strategy {
            CausalStrategy::FixedTemplate => compile_causal(circuit, &options.compile, arithmetic)?,
            CausalStrategy::ColdPrefix => {
                compile_affine(&resolve::potential(circuit), &options.compile, arithmetic)?
            }
        };
        let model = NoiseModel::physical(&reference, &options.noise)?;
        let mut source_sites = BTreeMap::new();
        for (site, category) in reference.noise_sites().iter().enumerate() {
            for &operation in category.source_operations() {
                source_sites.insert(operation, site);
            }
        }
        let guarded_sites = circuit
            .operations()
            .iter()
            .filter(|op| matches!(op.operation, Operation::Noise { guard: Some(_), .. }))
            .map(|op| source_sites[&op.id])
            .collect();
        Ok(Self {
            compilation: reference.compilation().clone(),
            reference: (options.strategy == CausalStrategy::FixedTemplate).then_some(reference),
            guarded_sites,
            circuit: circuit.clone(),
            sampler: NoiseSampler::physical(model.clone(), options.noise),
            model,
            registry: registry.clone(),
            source_sites,
            options,
        })
    }
    pub fn compilation(&self) -> &crate::unitary::CompilationDiagnostics {
        &self.compilation
    }
    pub fn strategy(&self) -> CausalStrategy {
        self.options.strategy
    }
    pub fn reference_compilations(&self) -> usize {
        1
    }
    pub fn sample(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CausalDraw, RuntimeError> {
        self.validate_target(outputs, target)?;
        self.resolve(&[], random, arithmetic)?.pure_draw(
            outputs,
            target,
            self.options.sampling.max_terms,
            arithmetic,
        )
    }
    pub fn potential_site(&self, operation: usize) -> Option<usize> {
        self.source_sites.get(&operation).copied()
    }
    pub fn controller_versions(&self) -> BTreeMap<String, String> {
        self.registry.versions()
    }
    pub fn sample_prescribed(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        faults: &[FaultSelection],
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CausalDraw, RuntimeError> {
        self.validate_target(outputs, target)?;
        self.resolve_prescribed(&[], faults, random, arithmetic)?
            .pure_draw(outputs, target, self.options.sampling.max_terms, arithmetic)
    }
    /// Resolve histories with the selected sampler, independently of the final error metric.
    /// Requested reported parities are diagnostics sampled after controller calls.
    pub fn resolve(
        &self,
        requested: &[BitExpr],
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ResolvedHistory, RuntimeError> {
        self.validate_requested(requested)?;
        let draw = self.sampler.draw(random)?;
        self.execute(requested, draw, random, arithmetic, None)
    }
    pub fn resolve_prescribed(
        &self,
        requested: &[BitExpr],
        faults: &[FaultSelection],
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ResolvedHistory, RuntimeError> {
        self.validate_requested(requested)?;
        let probability = self.model.probability(faults)?;
        self.execute(
            requested,
            NoiseDraw {
                faults: faults.to_vec(),
                physical_probability: probability.clone(),
                proposal_probability: probability,
                likelihood_ratio: BigRational::one(),
                entropy_words: 0,
            },
            random,
            arithmetic,
            None,
        )
    }
    /// Evaluate explicit physical observations, grouped by each external call,
    /// first guarded category, and final requested-record query, in causal order.
    /// This is a conditional query, not a Monte Carlo history proposal.
    pub fn resolve_prescribed_history(
        &self,
        requested: &[BitExpr],
        faults: &[FaultSelection],
        groups: &[Vec<bool>],
        arithmetic: &ScalarArithmetic,
    ) -> Result<ResolvedHistory, RuntimeError> {
        self.validate_requested(requested)?;
        let mut shapes = Vec::new();
        let mut guarded = std::collections::BTreeSet::new();
        for located in self.circuit.operations() {
            match &located.operation {
                Operation::Control {
                    call: Some(_),
                    inputs,
                    ..
                } => shapes.push(inputs.len()),
                Operation::Noise { guard: Some(_), .. } => {
                    let site =
                        self.source_sites
                            .get(&located.id)
                            .ok_or(RuntimeError::InternalSite {
                                operation: located.id,
                            })?;
                    if guarded.insert(site) {
                        shapes.push(1);
                    }
                }
                _ => {}
            }
        }
        if !requested.is_empty() {
            shapes.push(requested.len());
        }
        if groups.len() != shapes.len() {
            return Err(RuntimeError::InvalidHistory {
                group: groups.len().min(shapes.len()),
                message: format!(
                    "received {} groups; expected {}",
                    groups.len(),
                    shapes.len()
                ),
            });
        }
        for (group, (values, &count)) in groups.iter().zip(&shapes).enumerate() {
            if values.len() != count {
                return Err(RuntimeError::InvalidHistory {
                    group,
                    message: format!("received {} bits; expected {count}", values.len()),
                });
            }
        }
        let probability = self.model.probability(faults)?;
        self.execute(
            requested,
            NoiseDraw {
                faults: faults.to_vec(),
                physical_probability: probability.clone(),
                proposal_probability: probability,
                likelihood_ratio: BigRational::one(),
                entropy_words: 0,
            },
            &mut crate::noise::SplitMix64::new(0),
            arithmetic,
            Some(groups),
        )
    }
    fn validate_requested(&self, requested: &[BitExpr]) -> Result<(), RuntimeError> {
        for parity in requested {
            for atom in &parity.terms {
                match atom {
                    BitAtom::Record(record) if *record < self.circuit.num_records() => {}
                    BitAtom::Record(record) => {
                        return Err(InstrumentError::RecordOutOfBounds {
                            record: *record,
                            count: self.circuit.num_records(),
                        }
                        .into());
                    }
                    _ => return Err(InstrumentError::NonAffineQuery.into()),
                }
            }
        }
        Ok(())
    }
    fn validate_target(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
    ) -> Result<(), RuntimeError> {
        if target.width() != outputs.len() {
            return Err(
                InstrumentError::Unitary(crate::unitary::UnitaryError::DimensionMismatch).into(),
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        for &qubit in outputs {
            if qubit >= self.circuit.num_qubits() {
                return Err(InstrumentError::QubitOutOfBounds {
                    qubit,
                    width: self.circuit.num_qubits(),
                }
                .into());
            }
            if !seen.insert(qubit) {
                return Err(
                    InstrumentError::Unitary(crate::unitary::UnitaryError::RepeatedOutput).into(),
                );
            }
        }
        Ok(())
    }
    fn execute(
        &self,
        requested: &[BitExpr],
        draw: NoiseDraw,
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
        prescribed: Option<&[Vec<bool>]>,
    ) -> Result<ResolvedHistory, RuntimeError> {
        // Choices for inactive guarded categories are discarded, including their
        // probabilities; they have no physical action or path weight.
        let chosen = draw
            .faults
            .iter()
            .map(|f| (f.site, f.alternative))
            .collect::<BTreeMap<_, _>>();
        let mut state = PathState {
            program: self,
            chosen,
            decisions: BTreeMap::new(),
            guards: BTreeMap::new(),
            conditions: Vec::new(),
            calls: Vec::new(),
            entropy_words: 0,
            prefix_compiles: 0,
            network_compilations: 0,
            history_queries: 0,
            prescribed,
            next_group: 0,
            proposal_mass: (self.options.history_sampler == HistorySampler::AffineSupport
                && prescribed.is_none())
            .then_some(ExactScalar::one()),
        };
        for (end, located) in self.circuit.operations().iter().enumerate() {
            match &located.operation {
                Operation::Control {
                    name,
                    call: Some(call),
                    inputs,
                    outputs,
                } => {
                    let inputs = inputs
                        .iter()
                        .map(|e| resolve::expression(e, &state.decisions, false))
                        .collect::<Result<Vec<_>, _>>()?;
                    let values = state.sample_inputs(end, &inputs, random, arithmetic)?;
                    let result = self
                        .registry
                        .evaluate(name, *call, &values, outputs.len())?;
                    for (bit, &value) in result.iter().enumerate() {
                        state.decisions.insert((*call, bit), value);
                    }
                    state.calls.push(ControllerCall {
                        name: name.clone(),
                        call: *call,
                        inputs: values,
                        outputs: result,
                    });
                }
                Operation::Noise {
                    guard: Some(expression),
                    ..
                } => {
                    let site =
                        *self
                            .source_sites
                            .get(&located.id)
                            .ok_or(RuntimeError::InternalSite {
                                operation: located.id,
                            })?;
                    if !state.guards.contains_key(&site) {
                        let expression = resolve::expression(expression, &state.decisions, false)?;
                        let values = state.sample_inputs(end, &[expression], random, arithmetic)?;
                        state.guards.insert(site, values[0]);
                    }
                }
                _ => {}
            }
        }
        let requested_values = if requested.is_empty() {
            vec![]
        } else {
            state.sample_inputs(
                self.circuit.operations().len(),
                requested,
                random,
                arithmetic,
            )?
        };
        let instrument = state
            .instrument(self.circuit.operations().len(), arithmetic)?
            .conditioned(&state.conditions)?;
        let faults = state.faults(&instrument)?;
        let parities = state
            .conditions
            .iter()
            .map(|(p, _)| p.clone())
            .collect::<Vec<_>>();
        let values = BitVector::from_indices(
            parities.len(),
            state
                .conditions
                .iter()
                .enumerate()
                .filter_map(|(j, (_, v))| v.then_some(j)),
        )
        .map_err(InstrumentError::from)?;
        let history_mass = instrument
            .reported_probability(
                &parities,
                &values,
                &faults,
                self.options.sampling.max_terms,
                arithmetic,
            )?
            .value;
        let mut noise_probability = BigRational::one();
        for category in instrument.noise_sites() {
            let original = self.source_sites[&category.source_operations()[0]];
            noise_probability *= self.model.probabilities(original).unwrap()
                [state.chosen.get(&original).map_or(0, |a| a + 1)]
            .value();
        }
        Ok(ResolvedHistory {
            instrument,
            faults,
            requested_values,
            decisions: state.decisions,
            history_mass,
            history_proposal_mass: state.proposal_mass,
            noise_probability,
            conditions: state.conditions,
            controller_calls: state.calls,
            noise_entropy_words: draw.entropy_words,
            history_entropy_words: state.entropy_words,
            network_compilations: state.network_compilations,
            history_queries: state.history_queries,
        })
    }
}
struct PathState<'a> {
    program: &'a CausalProgram,
    chosen: BTreeMap<usize, usize>,
    decisions: BTreeMap<(usize, usize), bool>,
    guards: BTreeMap<usize, bool>,
    conditions: Vec<(BitExpr, bool)>,
    calls: Vec<ControllerCall>,
    entropy_words: usize,
    prefix_compiles: usize,
    network_compilations: usize,
    history_queries: usize,
    prescribed: Option<&'a [Vec<bool>]>,
    next_group: usize,
    proposal_mass: Option<ExactScalar>,
}
impl PathState<'_> {
    fn resolved(&self, end: usize) -> Result<Vec<LocatedOperation>, RuntimeError> {
        resolve::prefix(
            &self.program.circuit,
            end,
            &self.decisions,
            &self.guards,
            &self.program.source_sites,
        )
    }
    fn instrument(
        &mut self,
        end: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledInstrument, RuntimeError> {
        if let Some(reference) = &self.program.reference {
            let active = reference
                .noise_sites()
                .iter()
                .enumerate()
                .filter_map(|(site, _)| {
                    (!self.program.guarded_sites.contains(&site)
                        || self.guards.get(&site) == Some(&true))
                    .then_some(site)
                })
                .collect();
            Ok(reference.branch(&self.decisions, &active)?)
        } else {
            let prefix = self.program.circuit.runtime_view(self.resolved(end)?);
            let instrument = compile_affine(&prefix, &self.program.options.compile, arithmetic)?;
            self.network_compilations += 1;
            Ok(instrument)
        }
    }
    fn faults(&self, instrument: &CompiledInstrument) -> Result<Vec<FaultSelection>, RuntimeError> {
        let mut faults = Vec::new();
        for (site, category) in instrument.noise_sites().iter().enumerate() {
            let operation = category.source_operations()[0];
            let original = self
                .program
                .source_sites
                .get(&operation)
                .ok_or(RuntimeError::InternalSite { operation })?;
            if let Some(&alternative) = self.chosen.get(original) {
                faults.push(FaultSelection { site, alternative });
            }
        }
        Ok(faults)
    }
    fn sample_inputs(
        &mut self,
        end: usize,
        inputs: &[BitExpr],
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<bool>, RuntimeError> {
        if inputs.len()
            > self
                .program
                .options
                .max_history_inputs
                .saturating_sub(self.conditions.len())
        {
            return Err(RuntimeError::ResourceLimit {
                option: "max_history_inputs",
                limit: self.program.options.max_history_inputs,
            });
        }
        if self.history_queries >= self.program.options.max_history_queries {
            return Err(RuntimeError::ResourceLimit {
                option: "max_history_queries",
                limit: self.program.options.max_history_queries,
            });
        }
        self.history_queries += 1;
        if self.program.reference.is_none()
            && self.prefix_compiles >= self.program.options.max_prefix_compiles
        {
            return Err(RuntimeError::ResourceLimit {
                option: "max_prefix_compiles",
                limit: self.program.options.max_prefix_compiles,
            });
        }
        if self.program.reference.is_none() {
            self.prefix_compiles += 1;
        }
        let instrument = self.instrument(end, arithmetic)?;
        let faults = self.faults(&instrument)?;
        if let Some(groups) = self.prescribed {
            let group = self.next_group;
            self.next_group += 1;
            let values = &groups[group]; // Shapes were checked before execution.
            let mut parities = self
                .conditions
                .iter()
                .map(|(p, _)| p.clone())
                .collect::<Vec<_>>();
            parities.extend_from_slice(inputs);
            let mut outcomes = self.conditions.iter().map(|(_, v)| *v).collect::<Vec<_>>();
            outcomes.extend_from_slice(values);
            let bits = BitVector::from_indices(
                outcomes.len(),
                outcomes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &v)| v.then_some(i)),
            )
            .map_err(InstrumentError::from)?;
            if instrument
                .reported_probability(
                    &parities,
                    &bits,
                    &faults,
                    self.program.options.sampling.max_terms,
                    arithmetic,
                )?
                .value
                .is_zero()
            {
                return Err(RuntimeError::InvalidHistory {
                    group,
                    message: "observations have zero physical prefix mass".to_owned(),
                });
            }
            self.conditions
                .extend(inputs.iter().cloned().zip(values.iter().copied()));
            return Ok(values.clone());
        }
        let mut options = self.program.options.sampling;
        options.max_entropy_words = options.max_entropy_words.saturating_sub(self.entropy_words);
        let query = RecordQuery {
            given: &self.conditions,
            parities: inputs,
        };
        let (values, entropy_words) = match self.program.options.history_sampler {
            HistorySampler::ExactMarginal => {
                let draw = sample_reported_conditioned(
                    &instrument,
                    &query,
                    &faults,
                    random,
                    &options,
                    arithmetic,
                )?;
                (draw.values, draw.entropy_words)
            }
            HistorySampler::AffineSupport => {
                let draw = propose_affine_reported(
                    &instrument,
                    &query,
                    &faults,
                    random,
                    &options,
                    arithmetic,
                )?;
                let previous = self.proposal_mass.as_ref().expect("actual affine proposal");
                self.proposal_mass = Some(
                    arithmetic
                        .multiply(previous, &draw.proposal_mass)
                        .map_err(InstrumentError::from)?,
                );
                (draw.values, draw.entropy_words)
            }
        };
        self.entropy_words += entropy_words;
        self.conditions
            .extend(inputs.iter().cloned().zip(values.iter().copied()));
        Ok(values)
    }
}
