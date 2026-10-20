//! Causal exact-marginal controller and guarded-noise execution.
pub mod process;
mod registry;
mod resolve;
use crate::circuit::{BitAtom, BitExpr, Circuit, LocatedOperation, Operation};
use crate::gf2::BitVector;
use crate::instrument::{
    CompileOptions, CompiledInstrument, ExactMasses, FaultSelection, InstrumentError,
    compile_affine,
};
use crate::noise::{NoiseDraw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler, RandomSource};
use crate::records::{RecordQuery, SamplingError, SamplingOptions, sample_reported_conditioned};
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
#[derive(Clone, Copy, Debug)]
pub struct RuntimeOptions {
    pub compile: CompileOptions,
    pub noise: NoiseOptions,
    pub sampling: SamplingOptions,
    pub max_history_inputs: usize,
    pub max_prefix_compiles: usize,
}
impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
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
    /// Probability of the active physical noise choices along this history.
    pub noise_probability: BigRational,
    pub conditions: Vec<(BitExpr, bool)>,
    pub controller_calls: Vec<ControllerCall>,
    pub noise_entropy_words: usize,
    pub history_entropy_words: usize,
}
pub struct ResolvedHistory {
    instrument: CompiledInstrument,
    faults: Vec<FaultSelection>,
    history_mass: ExactScalar,
    noise_probability: BigRational,
    conditions: Vec<(BitExpr, bool)>,
    controller_calls: Vec<ControllerCall>,
    requested_values: Vec<bool>,
    decisions: BTreeMap<(usize, usize), bool>,
    noise_entropy_words: usize,
    history_entropy_words: usize,
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
            noise_probability: self.noise_probability,
            conditions: self.conditions,
            controller_calls: self.controller_calls,
            noise_entropy_words: self.noise_entropy_words,
            history_entropy_words: self.history_entropy_words,
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
}
impl CausalProgram {
    /// Compile a structural reference for all potential noise categories. Sampling
    /// later compiles resolved physical prefixes; unknown decisions never enter
    /// a physical query. Controller names are checked before drawing entropy.
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
        let structural = resolve::potential(circuit);
        let reference = compile_affine(&structural, &options.compile, arithmetic)?;
        let model = NoiseModel::physical(&reference, &options.noise)?;
        let mut source_sites = BTreeMap::new();
        for (site, category) in reference.noise_sites().iter().enumerate() {
            for &operation in category.source_operations() {
                source_sites.insert(operation, site);
            }
        }
        Ok(Self {
            circuit: circuit.clone(),
            sampler: NoiseSampler::physical(model.clone(), options.noise),
            model,
            registry: registry.clone(),
            source_sites,
            options,
        })
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
    /// Resolve physical histories independently of the final error metric.
    /// Requested reported parities are diagnostics sampled after controller calls.
    pub fn resolve(
        &self,
        requested: &[BitExpr],
        random: &mut impl RandomSource,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ResolvedHistory, RuntimeError> {
        self.validate_requested(requested)?;
        let draw = self.sampler.draw(random)?;
        self.execute(requested, draw, random, arithmetic)
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
        let mut resolved = state.resolved(self.circuit.operations().len())?;
        for (expression, value) in &state.conditions {
            let mut expression = expression.clone();
            expression.constant ^= *value;
            resolved.push(LocatedOperation {
                id: self.circuit.operations().len() + resolved.len(),
                location: self
                    .circuit
                    .operations()
                    .last()
                    .map(|op| op.location.clone())
                    .unwrap_or(crate::circuit::SourceLocation {
                        source: None,
                        line: 1,
                        column: 1,
                        repeat: vec![],
                    }),
                operation: Operation::Postselect { expression },
            });
        }
        let final_circuit = self.circuit.runtime_view(resolved);
        let instrument = compile_affine(&final_circuit, &self.options.compile, arithmetic)?;
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
            noise_probability,
            conditions: state.conditions,
            controller_calls: state.calls,
            noise_entropy_words: draw.entropy_words,
            history_entropy_words: state.entropy_words,
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
        if self.prefix_compiles >= self.program.options.max_prefix_compiles {
            return Err(RuntimeError::ResourceLimit {
                option: "max_prefix_compiles",
                limit: self.program.options.max_prefix_compiles,
            });
        }
        self.prefix_compiles += 1;
        let prefix = self.program.circuit.runtime_view(self.resolved(end)?);
        let instrument = compile_affine(&prefix, &self.program.options.compile, arithmetic)?;
        let faults = self.faults(&instrument)?;
        let mut options = self.program.options.sampling;
        options.max_entropy_words = options.max_entropy_words.saturating_sub(self.entropy_words);
        let draw = sample_reported_conditioned(
            &instrument,
            &RecordQuery {
                given: &self.conditions,
                parities: inputs,
            },
            &faults,
            random,
            &options,
            arithmetic,
        )?;
        self.entropy_words += draw.entropy_words;
        self.conditions
            .extend(inputs.iter().cloned().zip(draw.values.iter().copied()));
        Ok(draw.values)
    }
}
