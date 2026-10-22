use super::*;
use crate::circuit::{BitAtom, BitExpr};
use crate::field::{ExactField, FieldArithmetic};
use crate::instrument::{CompiledInstrument, FaultSelection, compile_affine, compile_unitary};
use crate::noise::{NoiseModel, NoiseSampler, NoiseSampling, RandomSource, ScaledProposalSummary};
use crate::runtime::{
    CausalProgram, ControllerCall, ControllerRegistry, ResolvedHistory, RuntimeOptions,
};
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::unitary::{CompiledUnitary, DensityBoundary};
use num_rational::BigRational;
use num_traits::One;
mod enumeration;
mod run;
pub use enumeration::{
    ExactInterval, TaskEnumeration, TaskEnumerationOptions, TaskSectorMasses, TaskWeightSector,
};
pub use run::{
    RunFailure, RunPhase, RunStatus, RunTimings, TaskRunOptions, TaskRunResult, TaskRunStatistics,
    TaskVariances, run_task_batch,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct EvaluationOptions {
    pub runtime: RuntimeOptions,
    pub scalars: ScalarArithmetic,
    pub fields: FieldArithmetic,
}
struct Target {
    unitary: CompiledUnitary,
    outputs: Vec<usize>,
    rows: Vec<BitVector>,
    outcomes: BitVector,
    norm: ExactScalar,
}
enum Execution {
    Linear {
        instrument: Box<CompiledInstrument>,
        model: NoiseModel,
        sampler: Box<NoiseSampler>,
    },
    Causal(Box<CausalProgram>),
}
pub struct TaskEvaluator {
    task: ValidatedTask,
    target: Option<Target>,
    error_parities: Option<Vec<BitExpr>>,
    requested: Vec<BitExpr>,
    execution: Execution,
    options: EvaluationOptions,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractionResources {
    pub quotient_rank: usize,
    pub terms: usize,
}
impl From<&crate::unitary::NetworkValue> for ContractionResources {
    fn from(value: &crate::unitary::NetworkValue) -> Self {
        Self {
            quotient_rank: value.quotient_rank,
            terms: value.terms,
        }
    }
}
#[derive(PartialEq, Eq)]
pub struct TaskEvaluation {
    /// Post-preparation Clifford circuit constructions, excluding scalar queries.
    pub network_compilations: usize,
    pub history_queries: usize,
    pub acceptance_resources: ContractionResources,
    pub target_resources: Option<ContractionResources>,
    /// Per-draw acceptance contribution, with the selected history weighting.
    pub accepted: ExactField,
    pub error: ExactField,
    pub target_success: Option<ExactField>,
    pub moments: Vec<ExactField>,
    pub raw_acceptance: ExactScalar,
    /// Native-compatible error numerator; divide by target norm when present.
    pub raw_error: ExactScalar,
    pub raw_moments: Vec<ExactScalar>,
    pub raw_target_success: Option<ExactScalar>,
    pub target_normalization: Option<ExactScalar>,
    pub history_mass: ExactScalar,
    pub history_proposal_mass: Option<ExactScalar>,
    pub history_weight: ExactField,
    pub noise_probability: BigRational,
    pub noise_likelihood_ratio: BigRational,
    pub sampled_records: Vec<bool>,
    pub controller_calls: Vec<ControllerCall>,
}
fn error(field: &str, e: impl fmt::Display) -> TaskError {
    TaskError::new(field, e.to_string())
}
impl TaskEvaluator {
    pub fn new(
        physical: &Circuit,
        modules: &LoadedModules,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
    ) -> Result<Self, TaskError> {
        Self::new_with_sampling(
            physical,
            modules,
            registry,
            options,
            &NoiseSampling::Physical,
        )
    }
    /// Prepare a noise proposal separately from circuit, target and history
    /// semantics. Returned contributions include its likelihood exactly once.
    pub fn new_with_sampling(
        physical: &Circuit,
        modules: &LoadedModules,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        sampling: &NoiseSampling,
    ) -> Result<Self, TaskError> {
        let a = &options.scalars;
        let max_terms = options.runtime.sampling.max_terms;
        let task = modules.task.clone();
        validate_indices("outputs", &task.outputs, physical.num_qubits(), true)?;
        validate_indices(
            "sample_records",
            &task.sample_records,
            physical.num_records(),
            false,
        )?;
        let error_parities = task
            .error_observables
            .as_ref()
            .map(|indices| {
                indices
                    .iter()
                    .map(|index| {
                        physical.observables().get(index).cloned().ok_or_else(|| {
                            TaskError::new(
                                "error_observables",
                                format!("OBSERVABLE_INCLUDE index {index} was not declared"),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let target = if let Some(target) = &modules.ideal {
            let unitary = compile_unitary(&target.circuit, &options.runtime.compile, a)
                .map_err(|e| error("ideal_state", e))?;
            let rows = target
                .postselection
                .keys()
                .map(|&q| BitVector::from_indices(unitary.width(), [q]))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| error("ideal_state.postselection", e))?;
            let outcomes = BitVector::from_indices(
                rows.len(),
                target
                    .postselection
                    .values()
                    .enumerate()
                    .filter_map(|(j, &v)| v.then_some(j)),
            )
            .map_err(|e| error("ideal_state.postselection", e))?;
            let norm = unitary
                .probability_z(&rows, &outcomes, max_terms, a)
                .map_err(|e| error("ideal_state.normalization", e))?
                .value;
            if norm.is_zero() {
                return Err(TaskError::new(
                    "ideal_state.normalization",
                    "projected target has zero norm",
                ));
            }
            Some(Target {
                unitary,
                outputs: target.outputs.clone(),
                rows,
                outcomes,
                norm,
            })
        } else {
            None
        };
        let composed = compose::recovery(physical, &task.outputs, modules.recovery.as_ref())?;
        let requested = task
            .sample_records
            .iter()
            .map(|&record| BitExpr::atom(BitAtom::Record(record)))
            .collect::<Vec<_>>();
        let execution = if composed.needs_history_sampling() || !requested.is_empty() {
            Execution::Causal(Box::new(
                CausalProgram::new_with_sampling(&composed, registry, options.runtime, a, sampling)
                    .map_err(|e| error("runtime", e))?,
            ))
        } else {
            let instrument = compile_affine(&composed, &options.runtime.compile, a)
                .map_err(|e| error("circuit", e))?;
            let model = NoiseModel::physical(&instrument, &options.runtime.noise)
                .map_err(|e| error("noise", e))?;
            let sampler = NoiseSampler::configured(
                model.clone(),
                sampling,
                &Default::default(),
                options.runtime.noise,
            )
            .map_err(|e| error("sampling", e))?;
            Execution::Linear {
                instrument: Box::new(instrument),
                model,
                sampler: Box::new(sampler),
            }
        };
        Ok(Self {
            task,
            target,
            error_parities,
            requested,
            execution,
            options,
        })
    }
    pub fn scaled_proposal_summary(&self) -> Option<&ScaledProposalSummary> {
        match &self.execution {
            Execution::Linear { sampler, .. } => sampler.scaled_proposal_summary(),
            Execution::Causal(program) => program.scaled_proposal_summary(),
        }
    }
    pub fn prepared_density_statistics(
        &self,
    ) -> Result<Option<crate::instrument::PreparedQueryStatistics>, TaskError> {
        Ok(match &self.execution {
            Execution::Linear { instrument, .. } => Some(
                instrument
                    .prepared_density_statistics()
                    .map_err(|e| error("prepared_densities", e))?,
            ),
            Execution::Causal(program) => program
                .prepared_density_statistics()
                .map_err(|e| error("prepared_densities", e))?,
        })
    }
    pub fn prepared_probability_statistics(
        &self,
    ) -> Result<Option<crate::instrument::PreparedProbabilityStatistics>, TaskError> {
        Ok(match &self.execution {
            Execution::Linear { instrument, .. } => Some(
                instrument
                    .prepared_probability_statistics()
                    .map_err(|e| error("prepared_probabilities", e))?,
            ),
            Execution::Causal(program) => program
                .prepared_probability_statistics()
                .map_err(|e| error("prepared_probabilities", e))?,
        })
    }
    /// Initial compilation diagnostics. Linear tasks expose their single
    /// instrument; causal tasks expose the initial template or potential network
    /// described by `CausalProgram::compilation`, not final cold-branch geometry.
    pub fn compilation(&self) -> &crate::unitary::CompilationDiagnostics {
        match &self.execution {
            Execution::Linear { instrument, .. } => instrument.compilation(),
            Execution::Causal(program) => program.compilation(),
        }
    }
    pub fn sampled_history(&self) -> bool {
        matches!(self.execution, Execution::Causal(_))
    }
    pub fn potential_site(&self, operation: usize) -> Option<usize> {
        match &self.execution {
            Execution::Causal(program) => program.potential_site(operation),
            Execution::Linear { instrument, .. } => instrument
                .noise_sites()
                .iter()
                .position(|s| s.source_operations().contains(&operation)),
        }
    }
    pub fn sample(&self, random: &mut impl RandomSource) -> Result<TaskEvaluation, TaskError> {
        match &self.execution {
            Execution::Causal(program) => {
                let history = program
                    .resolve(&self.requested, random, &self.options.scalars)
                    .map_err(|e| error("runtime", e))?;
                self.evaluate(EvaluationInput::history(&history))
            }
            Execution::Linear {
                instrument,
                sampler,
                ..
            } => {
                let draw = sampler.draw(random).map_err(|e| error("noise", e))?;
                self.evaluate(EvaluationInput {
                    instrument,
                    faults: &draw.faults,
                    history_mass: &ExactScalar::one(),
                    noise_probability: &draw.physical_probability,
                    likelihood: &draw.likelihood_ratio,
                    history: None,
                })
            }
        }
    }
    pub fn sample_prescribed(
        &self,
        faults: &[FaultSelection],
        random: &mut impl RandomSource,
    ) -> Result<TaskEvaluation, TaskError> {
        match &self.execution {
            Execution::Causal(program) => {
                let history = program
                    .resolve_prescribed(&self.requested, faults, random, &self.options.scalars)
                    .map_err(|e| error("runtime", e))?;
                self.evaluate(EvaluationInput::history(&history))
            }
            Execution::Linear {
                instrument, model, ..
            } => {
                let probability = model.probability(faults).map_err(|e| error("noise", e))?;
                self.evaluate(EvaluationInput {
                    instrument,
                    faults,
                    history_mass: &ExactScalar::one(),
                    noise_probability: &probability,
                    likelihood: &BigRational::one(),
                    history: None,
                })
            }
        }
    }
    /// Query one explicitly chosen causal observation history. Returned values
    /// are conditional contributions; enumeration must include its physical mass.
    pub fn evaluate_prescribed_history(
        &self,
        faults: &[FaultSelection],
        groups: &[Vec<bool>],
    ) -> Result<TaskEvaluation, TaskError> {
        match &self.execution {
            Execution::Causal(program) => {
                let history = program
                    .resolve_prescribed_history(
                        &self.requested,
                        faults,
                        groups,
                        &self.options.scalars,
                    )
                    .map_err(|e| error("runtime", e))?;
                self.evaluate(EvaluationInput::history(&history))
            }
            Execution::Linear { .. } => {
                if !groups.is_empty() {
                    return Err(TaskError::new(
                        "runtime",
                        "linear tasks require an empty prescribed history",
                    ));
                }
                self.sample_prescribed(faults, &mut crate::noise::SplitMix64::new(0))
            }
        }
    }
    fn evaluate(&self, input: EvaluationInput<'_>) -> Result<TaskEvaluation, TaskError> {
        let a = &self.options.scalars;
        let f = &self.options.fields;
        let max_terms = self.options.runtime.sampling.max_terms;
        let acceptance_query = input
            .instrument
            .acceptance(input.faults, max_terms, a)
            .map_err(|e| error("acceptance", e))?;
        let acceptance_resources = ContractionResources::from(&acceptance_query);
        let raw_acceptance = acceptance_query.value;
        let accepted = f
            .contribution(
                &raw_acceptance,
                input.normalization_mass(),
                input.likelihood,
            )
            .map_err(|e| error("acceptance", e))?;
        let target_query = if let Some(target) = &self.target {
            Some(
                input
                    .instrument
                    .target_density_weight(
                        &self.task.outputs,
                        &target.unitary,
                        DensityBoundary {
                            outputs: &target.outputs,
                            event_rows: &target.rows,
                            outcomes: &target.outcomes,
                        },
                        input.faults,
                        max_terms,
                        a,
                    )
                    .map_err(|e| error("target_success", e))?,
            )
        } else {
            None
        };
        let target_resources = target_query.as_ref().map(ContractionResources::from);
        let raw_target_success = target_query.map(|query| query.value);
        let target_success =
            if let (Some(target), Some(success)) = (&self.target, &raw_target_success) {
                let value = f
                    .contribution(success, input.normalization_mass(), input.likelihood)
                    .map_err(|e| error("target_success", e))?;
                Some(
                    f.divide(
                        &value,
                        &f.from_scalar(&target.norm)
                            .map_err(|e| error("target_success", e))?,
                    )
                    .map_err(|e| error("target_success", e))?,
                )
            } else {
                None
            };
        let raw_error = if let (Some(target), Some(success)) = (&self.target, &raw_target_success) {
            a.subtract(
                &a.multiply(&raw_acceptance, &target.norm)
                    .map_err(|e| error("error", e))?,
                success,
            )
            .map_err(|e| error("error", e))?
        } else if let Some(parities) = &self.error_parities {
            let parities = parities
                .iter()
                .map(|p| {
                    if let Some(history) = input.history {
                        history
                            .resolve_expression(p)
                            .map_err(|e| error("error_observables", e))
                    } else {
                        Ok(p.clone())
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let zeros =
                BitVector::zeros(parities.len()).map_err(|e| error("error_observables", e))?;
            let success = input
                .instrument
                .accepted_probability(&parities, &zeros, input.faults, max_terms, a)
                .map_err(|e| error("error_observables", e))?
                .value;
            a.subtract(&raw_acceptance, &success)
                .map_err(|e| error("error", e))?
        } else {
            ExactScalar::zero()
        };
        let mut error_value = f
            .contribution(&raw_error, input.normalization_mass(), input.likelihood)
            .map_err(|e| error("error", e))?;
        if let Some(target) = &self.target {
            error_value = f
                .divide(
                    &error_value,
                    &f.from_scalar(&target.norm).map_err(|e| error("error", e))?,
                )
                .map_err(|e| error("error", e))?;
        }
        let mut moments = Vec::new();
        let mut raw_moments = Vec::new();
        for word in &self.task.observables {
            let raw = input
                .instrument
                .pauli_moment(&self.task.outputs, word, input.faults, max_terms, a)
                .map_err(|e| error("observables", e))?
                .value;
            moments.push(
                f.contribution(&raw, input.normalization_mass(), input.likelihood)
                    .map_err(|e| error("observables", e))?,
            );
            raw_moments.push(raw);
        }
        Ok(TaskEvaluation {
            network_compilations: input.history.map_or(0, |h| h.network_compilations()),
            history_queries: input.history.map_or(0, |h| h.history_queries()),
            acceptance_resources,
            target_resources,
            raw_error,
            raw_moments,
            accepted,
            error: error_value,
            target_success,
            moments,
            raw_acceptance,
            raw_target_success,
            target_normalization: self.target.as_ref().map(|t| t.norm.clone()),
            history_mass: input.history_mass.clone(),
            history_proposal_mass: input
                .history
                .and_then(|h| h.history_proposal_mass().cloned()),
            history_weight: if let Some(proposal) = input
                .history
                .and_then(ResolvedHistory::history_proposal_mass)
            {
                f.divide(
                    &f.from_scalar(input.history_mass)
                        .map_err(|e| error("history_weight", e))?,
                    &f.from_scalar(proposal)
                        .map_err(|e| error("history_weight", e))?,
                )
                .map_err(|e| error("history_weight", e))?
            } else {
                ExactField::one()
            },
            noise_probability: input.noise_probability.clone(),
            noise_likelihood_ratio: input.likelihood.clone(),
            sampled_records: input
                .history
                .map_or_else(Vec::new, |h| h.requested_values().to_vec()),
            controller_calls: input
                .history
                .map_or_else(Vec::new, |h| h.controller_calls().to_vec()),
        })
    }
}
struct EvaluationInput<'a> {
    instrument: &'a CompiledInstrument,
    faults: &'a [FaultSelection],
    history_mass: &'a ExactScalar,
    noise_probability: &'a BigRational,
    likelihood: &'a BigRational,
    history: Option<&'a ResolvedHistory>,
}
impl<'a> EvaluationInput<'a> {
    fn normalization_mass(&self) -> &ExactScalar {
        self.history
            .and_then(ResolvedHistory::history_proposal_mass)
            .unwrap_or(self.history_mass)
    }
    fn history(history: &'a ResolvedHistory) -> Self {
        // Noise likelihood and the selected history denominator are independent.
        Self {
            instrument: history.instrument(),
            faults: history.faults(),
            history_mass: history.history_mass(),
            noise_probability: history.noise_probability(),
            likelihood: history.noise_likelihood_ratio(),
            history: Some(history),
        }
    }
}
