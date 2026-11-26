use super::*;
use crate::circuit::{BitAtom, BitExpr};
use crate::costs::{ExecutionMeter, OperationKind, observe};
use crate::field::{ExactField, FieldArithmetic};
use crate::instrument::{
    CompiledInstrument, FaultSelection, VacuumLayout, compile_unitary, compile_vacuum_task,
};
use crate::noise::{
    NoiseModel, NoiseSampler, NoiseSampling, RandomSource, ScaledProposalSummary,
    validate_supplied_likelihood,
};
use crate::runtime::{
    CausalProgram, ControllerCall, ControllerRegistry, ResolvedHistory, RuntimeOptions,
};
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::unitary::{CompiledUnitary, DensityBoundary};
use num_rational::BigRational;
use num_traits::One;
mod diagnostics;
pub use diagnostics::{TaskSampleDiagnosticOptions, TaskSampleDiagnostics};
mod preparation;
mod supplied_actions;
pub use preparation::{
    FamilyPreparationCounts, SamplingPreparationOptions, SamplingPreparationReport,
};
pub use supplied_actions::{
    SuppliedActionEvaluator, SuppliedActionLimits, SuppliedNoiseActions, SuppliedPauliAction,
};
mod enumeration;
mod run;
pub use enumeration::{
    ExactInterval, TaskEnumeration, TaskEnumerationOptions, TaskPatternResponse, TaskSectorMasses,
    TaskWeightSector,
};
pub use run::{
    BatchProgress, CancellationToken, ProgressObserver, ProgressReceipt, RunFailure, RunPhase,
    RunStatus, RunTimings, StreamRecordRetention, TaskRunOptions, TaskRunResult, TaskRunStatistics,
    TaskStream, TaskStreamIdentity, TaskStreamOptions, TaskStreamReport, TaskStreamStatus,
    TaskVariances, run_task_batch, run_task_batch_cancellable,
};
pub(crate) use run::{RunHooks, run_task_batch_inspected};

/// Which declared external callbacks execute during task sampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControllerExecution {
    /// Preserve every declared invocation, including observable callback effects.
    #[default]
    Declared,
    /// Elide callbacks whose outputs cannot affect any quantum operation, noise
    /// guard, selection, detector or observable, after canonical XOR expansion.
    /// The caller explicitly permits removing those callback effects and entropy.
    RequiredForTask,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct EvaluationOptions {
    pub runtime: RuntimeOptions,
    pub scalars: ScalarArithmetic,
    pub fields: FieldArithmetic,
    pub controller_execution: ControllerExecution,
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
        noise: Option<Box<LinearNoise>>,
    },
    Causal(Box<CausalProgram>),
}
struct LinearNoise {
    model: NoiseModel,
    sampler: NoiseSampler,
}
fn linear_noise(noise: &Option<Box<LinearNoise>>) -> Result<&LinearNoise, TaskError> {
    noise.as_deref().ok_or_else(|| {
        TaskError::new(
            "noise",
            "prescribed contribution evaluator has no internal noise model",
        )
    })
}
/// Independent retained source/target program snapshots. This excludes cold
/// transient owners, manual phase APIs and logical query-cache cardinalities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TaskQueryWorkStatistics {
    pub source: Option<crate::unitary::QueryWorkStatistics>,
    pub target: Option<crate::unitary::QueryWorkStatistics>,
}

/// Quiescent inventories of the two independently compiled source/target
/// owners. Query-cache resources have their own per-family statistics/limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TaskReferenceStorageStatistics {
    pub source: Option<crate::unitary::ReferenceStorageStatistics>,
    pub target: Option<crate::unitary::ReferenceStorageStatistics>,
}

pub struct TaskEvaluator {
    task: ValidatedTask,
    target: Option<Target>,
    error_parities: Option<Vec<BitExpr>>,
    requested: Vec<BitExpr>,
    execution: Execution,
    options: EvaluationOptions,
    sampling: NoiseSampling,
    diagnostic_record_width: usize,
    protocol_call_boundary: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractionResources {
    pub quotient_rank: usize,
    pub terms: usize,
}

/// Integrated, unweighted contributions for one explicitly assigned linear task.
/// The caller owns the law that produced the assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinearPrescribedContributions {
    pub acceptance_resources: ContractionResources,
    pub target_resources: Option<ContractionResources>,
    pub accepted: ExactField,
    pub error: ExactField,
    pub moments: Vec<ExactField>,
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
    /// Active-path physical probability of the categorical fault choices.
    pub noise_probability: BigRational,
    /// Generated noise likelihood ratio, unit prescribed weight, or caller-declared factor.
    pub noise_likelihood_ratio: BigRational,
    pub sampled_records: Vec<bool>,
    pub controller_calls: Vec<ControllerCall>,
}
fn error(field: &str, e: impl fmt::Display) -> TaskError {
    TaskError::new(field, e.to_string())
}
impl TaskEvaluator {
    pub fn reference_storage_statistics(
        &self,
    ) -> Result<TaskReferenceStorageStatistics, TaskError> {
        let source = match &self.execution {
            Execution::Linear { instrument, .. } => Some(
                instrument
                    .reference_storage_statistics()
                    .map_err(|e| error("reference_storage", e))?,
            ),
            Execution::Causal(program) => program
                .reference_storage_statistics()
                .map_err(|e| error("reference_storage", e))?,
        };
        let target = self
            .target
            .as_ref()
            .map(|target| target.unitary.reference_storage_statistics())
            .transpose()
            .map_err(|e| error("reference_storage", e))?;
        Ok(TaskReferenceStorageStatistics { source, target })
    }
    pub fn query_work_statistics(&self) -> TaskQueryWorkStatistics {
        TaskQueryWorkStatistics {
            source: match &self.execution {
                Execution::Linear { instrument, .. } => Some(instrument.query_work_statistics()),
                Execution::Causal(program) => program.query_work_statistics(),
            },
            target: self
                .target
                .as_ref()
                .map(|target| target.unitary.query_work_statistics()),
        }
    }

    /// Successful source assignments on the retained reference. Terminal query
    /// families share one draw-local view; cold transient programs are excluded.
    pub fn source_insertion_assignments(&self) -> Option<u64> {
        match &self.execution {
            Execution::Linear { instrument, .. } => Some(instrument.insertion_assignments()),
            Execution::Causal(program) => program.insertion_assignments(),
        }
    }

    /// Actual execution route selected by task composition and runtime options.
    pub fn execution_route(&self) -> &'static str {
        match &self.execution {
            Execution::Linear { .. } => "linear_affine",
            Execution::Causal(_) => match self.options.runtime.strategy {
                crate::runtime::CausalStrategy::FixedTemplate => "causal_fixed_template",
                crate::runtime::CausalStrategy::ColdPrefix => "causal_cold_prefix",
            },
        }
    }
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
        Self::new_observed(physical, modules, registry, options, Some(sampling), None)
    }
    /// Prepare while recording actual compiler and target-normalization work.
    pub fn new_with_costs(
        physical: &Circuit,
        modules: &LoadedModules,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        sampling: &NoiseSampling,
        meter: &ExecutionMeter,
    ) -> Result<Self, TaskError> {
        Self::new_observed(
            physical,
            modules,
            registry,
            options,
            Some(sampling),
            Some(meter),
        )
    }
    /// Prepare exact contributions for an externally sampled linear fault
    /// law. Integrated scoring and query preparation are available. Methods
    /// requiring the evaluator's own noise sampler or model are unsupported.
    pub fn new_linear_prescribed_with_costs(
        physical: &Circuit,
        modules: &LoadedModules,
        options: EvaluationOptions,
        meter: &ExecutionMeter,
    ) -> Result<Self, TaskError> {
        Self::new_observed(
            physical,
            modules,
            &ControllerRegistry::default(),
            options,
            None,
            Some(meter),
        )
    }
    fn new_observed(
        physical: &Circuit,
        modules: &LoadedModules,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        sampling: Option<&NoiseSampling>,
        meter: Option<&ExecutionMeter>,
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
            let unitary = observe(meter, OperationKind::Compilation, || {
                compile_unitary(&target.circuit, &options.runtime.compile, a)
                    .map_err(|e| error("ideal_state", e))
            })?;
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
            let norm = observe(meter, OperationKind::Query, || {
                unitary
                    .probability_z(&rows, &outcomes, max_terms, a)
                    .map_err(|e| error("ideal_state.normalization", e))
            })?
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
        let composed = match options.controller_execution {
            ControllerExecution::Declared => composed,
            ControllerExecution::RequiredForTask => composed.retaining_required_controllers(),
        };
        let requested = task
            .sample_records
            .iter()
            .map(|&record| BitExpr::atom(BitAtom::Record(record)))
            .collect::<Vec<_>>();
        let execution = if composed.needs_history_sampling() || !requested.is_empty() {
            let sampling = sampling.ok_or_else(|| {
                TaskError::new(
                    "runtime",
                    "prescribed contribution preparation requires a linear task",
                )
            })?;
            let mut program = CausalProgram::new_vacuum_task_observed(
                &composed,
                &task.outputs,
                registry,
                options.runtime,
                a,
                sampling,
                meter,
            )
            .map_err(|e| error("runtime", e))?;
            program
                .capture_support_transport(&requested)
                .map_err(|e| error("runtime.support_transport", e))?;
            Execution::Causal(Box::new(program))
        } else {
            let instrument = observe(meter, OperationKind::Compilation, || {
                let layout = VacuumLayout::for_task(&composed, &task.outputs)
                    .map_err(|e| error("circuit", e))?;
                compile_vacuum_task(&composed, &layout, &options.runtime.compile, a, false)
                    .map_err(|e| error("circuit", e))
            })?;
            let noise = if let Some(sampling) = sampling {
                let model = NoiseModel::physical(&instrument, &options.runtime.noise)
                    .map_err(|e| error("noise", e))?;
                let sampler = NoiseSampler::configured(
                    model.clone(),
                    sampling,
                    &Default::default(),
                    options.runtime.noise,
                )
                .map_err(|e| error("sampling", e))?;
                Some(Box::new(LinearNoise { model, sampler }))
            } else {
                None
            };
            Execution::Linear {
                instrument: Box::new(instrument),
                noise,
            }
        };
        Ok(Self {
            task,
            target,
            error_parities,
            requested,
            execution,
            options,
            sampling: sampling.cloned().unwrap_or(NoiseSampling::Physical),
            diagnostic_record_width: composed.num_records(),
            protocol_call_boundary: physical.external_calls(),
        })
    }
    pub fn scaled_proposal_summary(&self) -> Option<&ScaledProposalSummary> {
        match &self.execution {
            Execution::Linear { noise, .. } => noise.as_ref()?.sampler.scaled_proposal_summary(),
            Execution::Causal(program) => program.scaled_proposal_summary(),
        }
    }
    pub fn prepared_support_statistics(
        &self,
    ) -> Result<Option<crate::instrument::PreparedSupportStatistics>, TaskError> {
        match &self.execution {
            Execution::Linear { instrument, .. } => instrument
                .prepared_support_statistics()
                .map_err(|e| error("prepared_support", e)),
            Execution::Causal(program) => program
                .prepared_support_statistics()
                .map_err(|e| error("prepared_support", e)),
        }
    }
    pub fn prepared_insertion_statistics(
        &self,
    ) -> Option<crate::instrument::PreparedInsertionStatistics> {
        match &self.execution {
            Execution::Linear { instrument, .. } => instrument.prepared_insertion_statistics(),
            Execution::Causal(program) => program.prepared_insertion_statistics(),
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
    /// Successful target-frame cache reuses, cumulative for the retained
    /// reference owner. Cold-prefix execution has no shared reference cache.
    pub fn prepared_density_reuses(&self) -> Option<u64> {
        match &self.execution {
            Execution::Linear { instrument, .. } => Some(instrument.prepared_density_reuses()),
            Execution::Causal(program) => program.prepared_density_reuses(),
        }
    }
    pub fn prepared_moment_statistics(
        &self,
    ) -> Result<Option<crate::instrument::PreparedQueryStatistics>, TaskError> {
        Ok(match &self.execution {
            Execution::Linear { instrument, .. } => Some(
                instrument
                    .prepared_moment_statistics()
                    .map_err(|e| error("prepared_moments", e))?,
            ),
            Execution::Causal(program) => program
                .prepared_moment_statistics()
                .map_err(|e| error("prepared_moments", e))?,
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
        self.sample_observed(random, None)
    }
    /// Lazily evaluate a bounded batch using this prepared evaluator and one
    /// caller-owned RNG. No environment, evaluator or RNG is copied. The first
    /// error is returned once and terminates the iterator; a new iterator may
    /// explicitly continue from the caller's current stream state.
    pub fn samples<'a, R: RandomSource + 'a>(
        &'a self,
        random: &'a mut R,
        count: usize,
    ) -> impl Iterator<Item = Result<TaskEvaluation, TaskError>> + std::iter::FusedIterator + 'a
    {
        self.samples_observed(random, count, None)
    }
    /// The same lazy batch with caller-owned cumulative work/quotas. Counts
    /// include failed attempts and are never reset between iterator items.
    pub fn samples_with_costs<'a, R: RandomSource + 'a>(
        &'a self,
        random: &'a mut R,
        count: usize,
        meter: &'a ExecutionMeter,
    ) -> impl Iterator<Item = Result<TaskEvaluation, TaskError>> + std::iter::FusedIterator + 'a
    {
        self.samples_observed(random, count, Some(meter))
    }
    fn samples_observed<'a, R: RandomSource + 'a>(
        &'a self,
        random: &'a mut R,
        count: usize,
        meter: Option<&'a ExecutionMeter>,
    ) -> impl Iterator<Item = Result<TaskEvaluation, TaskError>> + std::iter::FusedIterator + 'a
    {
        let mut remaining = count;
        std::iter::from_fn(move || {
            if remaining == 0 {
                return None;
            }
            remaining -= 1;
            let next = self.sample_observed(random, meter);
            if next.is_err() {
                remaining = 0;
            }
            Some(next)
        })
        .fuse()
    }
    /// Record logical work, including errors, without changing the draw stream.
    pub fn sample_with_costs(
        &self,
        random: &mut impl RandomSource,
        meter: &ExecutionMeter,
    ) -> Result<TaskEvaluation, TaskError> {
        self.sample_observed(random, Some(meter))
    }
    fn sample_observed(
        &self,
        random: &mut impl RandomSource,
        meter: Option<&ExecutionMeter>,
    ) -> Result<TaskEvaluation, TaskError> {
        self.sample_observed_inner(random, meter, None)
            .map(|(evaluation, _)| evaluation)
    }
    fn sample_observed_inner(
        &self,
        random: &mut impl RandomSource,
        meter: Option<&ExecutionMeter>,
        diagnostics: Option<&TaskSampleDiagnosticOptions>,
    ) -> Result<(TaskEvaluation, Option<TaskSampleDiagnostics>), TaskError> {
        if let Some(options) = diagnostics {
            options.preflight(self.diagnostic_record_width)?;
        }
        match &self.execution {
            Execution::Causal(program) => {
                let history = program
                    .resolve_observed(&self.requested, random, &self.options.scalars, meter)
                    .map_err(|e| error("runtime", e))?;
                let captured = diagnostics
                    .map(|options| options.capture(&history, self.protocol_call_boundary))
                    .transpose()?;
                let evaluation =
                    self.evaluate_observed(EvaluationInput::history(&history), meter)?;
                Ok((evaluation, captured))
            }
            Execution::Linear { instrument, noise } => {
                let draw = linear_noise(noise)?
                    .sampler
                    .draw(random)
                    .map_err(|e| error("noise", e))?;
                let captured = diagnostics.map(|_| TaskSampleDiagnostics {
                    noise_entropy_words: draw.entropy_words,
                    ..Default::default()
                });
                let evaluation = self.evaluate_observed(
                    EvaluationInput {
                        instrument,
                        faults: &draw.faults,
                        history_mass: &ExactScalar::one(),
                        noise_probability: &draw.physical_probability,
                        likelihood: &draw.likelihood_ratio,
                        history: None,
                    },
                    meter,
                )?;
                Ok((evaluation, captured))
            }
        }
    }
    pub fn sample_prescribed(
        &self,
        faults: &[FaultSelection],
        random: &mut impl RandomSource,
    ) -> Result<TaskEvaluation, TaskError> {
        self.sample_with_faults(faults, BigRational::one(), random, None)
    }

    /// Return XOR-composable labels for the joint acceptance and target-error
    /// masses of a prepared linear target task without moment requests.
    ///
    /// Two assignments with the same label have identical phase updates,
    /// virtual-wire Paulis, output Paulis and postselection flips. They differ
    /// only by a global phase and a unitary on traced physical wires that
    /// commutes with the selected parity projectors. This preserves both
    /// integrated masses. Labels do not describe unselected record marginals.
    /// The word cap covers the packed label payload. Inapplicable or oversized
    /// plans return `None`.
    pub fn linear_target_fault_signatures(
        &self,
        max_words: usize,
    ) -> Result<Option<Vec<Vec<BitVector>>>, TaskError> {
        let Execution::Linear { instrument, .. } = &self.execution else {
            return Ok(None);
        };
        if self.target.is_none() || !self.task.observables.is_empty() {
            return Ok(None);
        }
        instrument
            .score_signatures(&self.task.outputs, max_words)
            .map_err(|e| error("fault_signatures", e))
    }

    /// Evaluate linear task contributions for a prescribed fault assignment.
    /// This computes the same unit-weight joint masses as `sample_prescribed`
    /// on linear tasks, without evaluating the original categorical probability.
    /// Fault indices and duplicates are still checked by the assignment path.
    /// Tasks that require causal history sampling must use the sampling API.
    pub fn linear_prescribed_contributions(
        &self,
        faults: &[FaultSelection],
    ) -> Result<LinearPrescribedContributions, TaskError> {
        self.linear_prescribed_contributions_with_screen(faults, false)
    }

    /// Evaluate the accepted and target-error masses of a linear target task.
    /// Exact zero acceptance implies a zero postselected positive operator,
    /// so its target score is zero and `target_resources` is absent.
    pub fn screened_linear_target_contributions(
        &self,
        faults: &[FaultSelection],
    ) -> Result<LinearPrescribedContributions, TaskError> {
        if self.target.is_none() || !self.task.observables.is_empty() {
            return Err(error(
                "runtime",
                "screened target scoring requires a target and no moments",
            ));
        }
        self.linear_prescribed_contributions_with_screen(faults, true)
    }

    fn linear_prescribed_contributions_with_screen(
        &self,
        faults: &[FaultSelection],
        screen_target: bool,
    ) -> Result<LinearPrescribedContributions, TaskError> {
        let Execution::Linear { instrument, .. } = &self.execution else {
            return Err(error(
                "runtime",
                "linear prescribed contributions require a linear task",
            ));
        };
        let unit = BigRational::one();
        // The private evaluation result uses the supplied probability only as
        // metadata. This endpoint returns contributions and omits that metadata.
        let result = self.evaluate_with_target_screen(
            EvaluationInput {
                instrument,
                faults,
                history_mass: &ExactScalar::one(),
                noise_probability: &unit,
                likelihood: &unit,
                history: None,
            },
            None,
            screen_target,
        )?;
        Ok(LinearPrescribedContributions {
            acceptance_resources: result.acceptance_resources,
            target_resources: result.target_resources,
            accepted: result.accepted,
            error: result.error,
            moments: result.moments,
        })
    }

    /// Sample the history for caller-supplied categorical faults and apply a
    /// declared exact factor to the history-normalized contributions.
    ///
    /// The raw numerator must be nonnegative and the denominator positive.
    /// Both must fit the smaller of the scalar and field coefficient budgets
    /// before reduction. The factor and fault identities are checked before
    /// entropy or controller calls. Zero and factors above one are allowed.
    ///
    /// The factor describes the caller's input. This route draws no noise
    /// proposal and does not establish unbiased sampling of any noise law.
    pub fn sample_supplied_noise(
        &self,
        faults: &[FaultSelection],
        likelihood: &BigRational,
        random: &mut impl RandomSource,
    ) -> Result<TaskEvaluation, TaskError> {
        self.sample_supplied_noise_observed(faults, likelihood, random, None)
    }

    /// Evaluate supplied categorical noise and record actual history, query and
    /// decoder work on the caller's meter. This uses the same likelihood and
    /// entropy rules as [Self::sample_supplied_noise].
    pub fn sample_supplied_noise_with_costs(
        &self,
        faults: &[FaultSelection],
        likelihood: &BigRational,
        random: &mut impl RandomSource,
        meter: &ExecutionMeter,
    ) -> Result<TaskEvaluation, TaskError> {
        self.sample_supplied_noise_observed(faults, likelihood, random, Some(meter))
    }

    fn sample_supplied_noise_observed(
        &self,
        faults: &[FaultSelection],
        likelihood: &BigRational,
        random: &mut impl RandomSource,
        meter: Option<&ExecutionMeter>,
    ) -> Result<TaskEvaluation, TaskError> {
        let limit = self
            .options
            .scalars
            .max_coefficient_bits()
            .min(self.options.fields.max_coefficient_bits());
        let likelihood =
            validate_supplied_likelihood(likelihood, limit).map_err(|e| error("noise", e))?;
        self.sample_with_faults(faults, likelihood, random, meter)
    }

    fn sample_with_faults(
        &self,
        faults: &[FaultSelection],
        likelihood: BigRational,
        random: &mut impl RandomSource,
        meter: Option<&ExecutionMeter>,
    ) -> Result<TaskEvaluation, TaskError> {
        match &self.execution {
            Execution::Causal(program) => {
                let history = program
                    .resolve_with_faults(
                        &self.requested,
                        faults,
                        likelihood,
                        random,
                        &self.options.scalars,
                        meter,
                    )
                    .map_err(|e| error("runtime", e))?;
                self.evaluate_observed(EvaluationInput::history(&history), meter)
            }
            Execution::Linear { instrument, noise } => {
                let probability = linear_noise(noise)?
                    .model
                    .probability(faults)
                    .map_err(|e| error("noise", e))?;
                self.evaluate_observed(
                    EvaluationInput {
                        instrument,
                        faults,
                        history_mass: &ExactScalar::one(),
                        noise_probability: &probability,
                        likelihood: &likelihood,
                        history: None,
                    },
                    meter,
                )
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
        self.evaluate_observed(input, None)
    }
    fn evaluate_observed(
        &self,
        input: EvaluationInput<'_>,
        meter: Option<&ExecutionMeter>,
    ) -> Result<TaskEvaluation, TaskError> {
        self.evaluate_with_target_screen(input, meter, false)
    }
    fn evaluate_with_target_screen(
        &self,
        input: EvaluationInput<'_>,
        meter: Option<&ExecutionMeter>,
        screen_target: bool,
    ) -> Result<TaskEvaluation, TaskError> {
        let a = &self.options.scalars;
        let f = &self.options.fields;
        let max_terms = self.options.runtime.sampling.max_terms;
        let (assignment, acceptance_query) = observe(meter, OperationKind::Query, || {
            let assignment = if let Some(assigned) =
                input.history.and_then(ResolvedHistory::terminal_assignment)
            {
                assigned
            } else {
                input
                    .instrument
                    .assigned(input.faults, a)
                    .map_err(|e| error("acceptance", e))?
            };
            let value = assignment
                .acceptance(max_terms, a)
                .map_err(|e| error("acceptance", e))?;
            Ok::<_, TaskError>((assignment, value))
        })?;
        let acceptance_resources = ContractionResources::from(&acceptance_query);
        let raw_acceptance = acceptance_query.value;
        let accepted = f
            .contribution(
                &raw_acceptance,
                input.normalization_mass(),
                input.likelihood,
            )
            .map_err(|e| error("acceptance", e))?;
        let skipped_target = screen_target && raw_acceptance.is_zero();
        let target_query = if let Some(target) = &self.target
            && !skipped_target
        {
            Some(observe(meter, OperationKind::Query, || {
                assignment
                    .target_density_weight(
                        &self.task.outputs,
                        &target.unitary,
                        DensityBoundary {
                            outputs: &target.outputs,
                            event_rows: &target.rows,
                            outcomes: &target.outcomes,
                        },
                        max_terms,
                        a,
                    )
                    .map_err(|e| error("target_success", e))
            })?)
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
        let raw_error = if skipped_target {
            ExactScalar::zero()
        } else if let (Some(target), Some(success)) = (&self.target, &raw_target_success) {
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
            let success = observe(meter, OperationKind::Query, || {
                assignment
                    .accepted_probability(&parities, &zeros, max_terms, a)
                    .map_err(|e| error("error_observables", e))
            })?
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
            let raw = observe(meter, OperationKind::Query, || {
                assignment
                    .pauli_moment(&self.task.outputs, word, max_terms, a)
                    .map_err(|e| error("observables", e))
            })?
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
