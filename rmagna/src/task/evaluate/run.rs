//! Seeded batches with exact statistics and explicit partial-failure accounting.
use super::*;
use crate::costs::{ExecutionCosts, ExecutionLimits};
use crate::noise::SplitMix64;
use crate::statistics::{EfficiencySummary, ExactStatistics, HistoryEfficiency, StatisticsOptions};
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct TaskRunOptions {
    pub execution_limits: ExecutionLimits,
    pub shots: u64,
    pub seed: u64,
    pub statistics: StatisticsOptions,
    /// Total requested Boolean values retained across the entire run.
    pub max_retained_record_values: usize,
}
impl Default for TaskRunOptions {
    fn default() -> Self {
        Self {
            execution_limits: ExecutionLimits::default(),
            shots: 1,
            seed: 0,
            statistics: StatisticsOptions::default(),
            max_retained_record_values: 1_000_000,
        }
    }
}
impl TaskRunOptions {
    pub(crate) fn validate(&self, records: usize, moments: usize) -> Result<usize, TaskError> {
        if self.shots == 0 {
            return Err(TaskError::new("run.shots", "shot count must be positive"));
        }
        let required = self.shots as u128 * records as u128;
        if required > self.max_retained_record_values as u128 {
            return Err(TaskError::new(
                "run.max_retained_record_values",
                format!(
                    "requested samples need {required} Boolean values; configured limit is {}",
                    self.max_retained_record_values
                ),
            ));
        }
        let dimensions = moments
            .checked_add(2)
            .ok_or_else(|| TaskError::new("run.statistics", "quantity count overflows usize"))?;
        ExactStatistics::new(dimensions, self.statistics)
            .map_err(|e| error("run.statistics", e))?;
        Ok(dimensions)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Complete,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunPhase {
    Preparation,
    Sampling,
    Accumulation,
    Finalization,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFailure {
    pub phase: RunPhase,
    /// Zero-based failed draw index; absent outside the draw loop.
    pub shot: Option<u64>,
    pub kind: &'static str,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunTimings {
    /// Measured only by run_task_batch, which owns preparation.
    pub preparation_seconds: Option<f64>,
    pub sampling_seconds: f64,
    pub total_seconds: f64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskVariances {
    /// Variances of mean estimators; absent with fewer than two completed draws.
    pub accepted: Option<ExactField>,
    pub error: Option<ExactField>,
    /// Paired delta-method variance, rather than an exact finite-sample variance.
    pub conditional_error: Option<ExactField>,
    pub moments: Vec<Option<ExactField>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRunStatistics {
    pub completed: u64,
    pub accepted: ExactField,
    pub error: Option<ExactField>,
    pub conditional_error: Option<ExactField>,
    pub moments: Vec<ExactField>,
    pub variances: TaskVariances,
    pub mean_weight: ExactField,
    pub effective_sample_size: ExactField,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRunResult {
    pub execution_limits: ExecutionLimits,
    /// Logical work of this call, including preparation when owned and failed attempts.
    pub execution_costs: ExecutionCosts,
    pub status: RunStatus,
    pub requested: u64,
    pub completed: u64,
    pub failed: u64,
    pub not_attempted: u64,
    pub seed: u64,
    /// Complete-run estimates only; always absent when status is Failed.
    pub statistics: Option<TaskRunStatistics>,
    pub partial_statistics: Option<TaskRunStatistics>,
    /// Diagnostic of completed rows, including a failed run's retained prefix.
    pub efficiency: Option<EfficiencySummary>,
    pub sampled_records: Option<Vec<Vec<bool>>>,
    pub failures: Vec<RunFailure>,
    pub timings: RunTimings,
    /// Circuit constructions reported by completed draws only.
    pub completed_network_compilations: usize,
    /// Causal observation groups, not a count of all marginal contractions.
    pub completed_history_groups: usize,
}
struct Accumulator {
    statistics: ExactStatistics,
    efficiency: HistoryEfficiency,
    has_error: bool,
    compilations: usize,
    groups: usize,
}
impl Accumulator {
    fn new(
        dimensions: usize,
        has_error: bool,
        options: StatisticsOptions,
    ) -> Result<Self, TaskError> {
        Ok(Self {
            statistics: ExactStatistics::new(dimensions, options)
                .map_err(|e| error("run.statistics", e))?,
            efficiency: HistoryEfficiency::new(options.arithmetic)
                .map_err(|e| error("run.efficiency", e))?,
            has_error,
            compilations: 0,
            groups: 0,
        })
    }
    fn push(&mut self, row: &TaskEvaluation, f: &FieldArithmetic) -> Result<(), TaskError> {
        let noise = f
            .from_rational(row.noise_likelihood_ratio.clone())
            .map_err(|e| error("run.weight", e))?;
        let weight = f
            .multiply(&noise, &row.history_weight)
            .map_err(|e| error("run.weight", e))?;
        let mut values = vec![row.accepted.clone(), row.error.clone()];
        values.extend_from_slice(&row.moments);
        let compilations = self
            .compilations
            .checked_add(row.network_compilations)
            .ok_or_else(|| TaskError::new("run.network_compilations", "counter overflow"))?;
        let groups = self
            .groups
            .checked_add(row.history_queries)
            .ok_or_else(|| TaskError::new("run.history_groups", "counter overflow"))?;
        // Stage statistics before the atomic efficiency update, so either
        // failure leaves all committed counts aligned.
        let mut statistics = self.statistics.clone();
        statistics
            .push(&values, &weight)
            .map_err(|e| error("run.statistics", e))?;
        self.efficiency
            .push(
                &row.accepted,
                self.has_error.then_some(&row.error),
                &weight,
                &noise,
            )
            .map_err(|e| error("run.efficiency", e))?;
        self.statistics = statistics;
        self.compilations = compilations;
        self.groups = groups;
        Ok(())
    }
    fn summarize(&self) -> Result<Option<TaskRunStatistics>, TaskError> {
        let Some(summary) = self
            .statistics
            .summarize()
            .map_err(|e| error("run.statistics", e))?
        else {
            return Ok(None);
        };
        let ratio = if self.has_error {
            self.statistics
                .paired_ratio(1, 0)
                .map_err(|e| error("run.conditional_error", e))?
        } else {
            None
        };
        let variance = |j: usize| summary.covariance_of_mean.as_ref().map(|m| m[j][j].clone());
        Ok(Some(TaskRunStatistics {
            completed: summary.count,
            accepted: summary.means[0].clone(),
            error: self.has_error.then(|| summary.means[1].clone()),
            conditional_error: ratio.as_ref().map(|r| r.ratio.clone()),
            moments: summary.means[2..].to_vec(),
            variances: TaskVariances {
                accepted: variance(0),
                error: if self.has_error { variance(1) } else { None },
                conditional_error: ratio.and_then(|r| r.variance),
                moments: (2..summary.means.len()).map(variance).collect(),
            },
            mean_weight: summary.mean_weight,
            effective_sample_size: summary.effective_sample_size,
        }))
    }
}
impl TaskEvaluator {
    /// Run from a fresh SplitMix64 seed, reusing this evaluator's preparation.
    /// Ordinary task/arithmetic failures return Failed results; invalid run
    /// configuration returns an error before any entropy or callbacks.
    pub fn run_batch(&self, options: &TaskRunOptions) -> Result<TaskRunResult, TaskError> {
        self.run_batch_observed(options, &ExecutionMeter::new(options.execution_limits))
    }
    fn run_batch_observed(
        &self,
        options: &TaskRunOptions,
        meter: &ExecutionMeter,
    ) -> Result<TaskRunResult, TaskError> {
        let started = Instant::now();
        let dimensions = options.validate(self.requested.len(), self.task.observables.len())?;
        let has_error = self.target.is_some() || self.error_parities.is_some();
        let mut accumulator = Accumulator::new(dimensions, has_error, options.statistics)?;
        let mut records = (!self.requested.is_empty()).then(Vec::new);
        let mut random = SplitMix64::new(options.seed);
        let mut failures = Vec::new();
        let mut failed = 0;
        let sampling = Instant::now();
        for shot in 0..options.shots {
            let row = match meter
                .check_time()
                .map_err(TaskError::from)
                .and_then(|()| self.sample_with_costs(&mut random, meter))
            {
                Ok(row) => row,
                Err(e) => {
                    failures.push(RunFailure {
                        phase: RunPhase::Sampling,
                        shot: Some(shot),
                        kind: "TaskError",
                        message: e.to_string(),
                    });
                    failed = 1;
                    break;
                }
            };
            if let Err(e) = accumulator.push(&row, &options.statistics.arithmetic) {
                failures.push(RunFailure {
                    phase: RunPhase::Accumulation,
                    shot: Some(shot),
                    kind: "TaskError",
                    message: e.to_string(),
                });
                failed = 1;
                break;
            }
            if let Some(records) = &mut records {
                records.push(row.sampled_records);
            }
        }
        let sampling_seconds = sampling.elapsed().as_secs_f64();
        let statistics = match accumulator.summarize() {
            Ok(summary) => summary,
            Err(e) => {
                failures.push(RunFailure {
                    phase: RunPhase::Finalization,
                    shot: None,
                    kind: "TaskError",
                    message: e.to_string(),
                });
                None
            }
        };
        let efficiency = match accumulator.efficiency.summarize() {
            Ok(summary) => Some(summary),
            Err(e) => {
                failures.push(RunFailure {
                    phase: RunPhase::Finalization,
                    shot: None,
                    kind: "StatisticsError",
                    message: e.to_string(),
                });
                None
            }
        };
        let completed = accumulator.statistics.count();
        let status = if failures.is_empty() {
            RunStatus::Complete
        } else {
            RunStatus::Failed
        };
        let (statistics, partial_statistics) = if status == RunStatus::Complete {
            (statistics, None)
        } else {
            (None, statistics)
        };
        Ok(TaskRunResult {
            execution_limits: meter.limits(),
            execution_costs: meter.snapshot(),
            status,
            requested: options.shots,
            completed,
            failed,
            not_attempted: options.shots - completed - failed,
            seed: options.seed,
            statistics,
            partial_statistics,
            efficiency,
            sampled_records: records,
            failures,
            timings: RunTimings {
                preparation_seconds: None,
                sampling_seconds,
                total_seconds: started.elapsed().as_secs_f64(),
            },
            completed_network_compilations: accumulator.compilations,
            completed_history_groups: accumulator.groups,
        })
    }
}

/// Own preparation and retain ordinary preparation failures as Failed results
/// with zero attempted draws. Invalid run configuration remains an error.
pub fn run_task_batch(
    physical: &Circuit,
    modules: &LoadedModules,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
    sampling: &NoiseSampling,
    run: &TaskRunOptions,
) -> Result<TaskRunResult, TaskError> {
    run_task_batch_inspected(
        physical,
        modules,
        registry,
        evaluation,
        sampling,
        run,
        |_, _, _| (),
    )
    .map(|(result, _)| result)
}

pub(crate) fn run_task_batch_inspected<R>(
    physical: &Circuit,
    modules: &LoadedModules,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
    sampling: &NoiseSampling,
    run: &TaskRunOptions,
    inspect: impl FnOnce(&TaskEvaluator, ExecutionCosts, f64) -> R,
) -> Result<(TaskRunResult, Option<R>), TaskError> {
    let started = Instant::now();
    run.validate(
        modules.task.sample_records.len(),
        modules.task.observables.len(),
    )?;
    let preparation = Instant::now();
    let meter = ExecutionMeter::new(run.execution_limits);
    let prepared =
        TaskEvaluator::new_with_costs(physical, modules, registry, evaluation, sampling, &meter);
    let preparation_seconds = preparation.elapsed().as_secs_f64();
    match prepared {
        Ok(evaluator) => {
            let preparation_costs = meter.snapshot();
            let mut result = evaluator.run_batch_observed(run, &meter)?;
            result.timings.preparation_seconds = Some(preparation_seconds);
            result.timings.total_seconds = started.elapsed().as_secs_f64();
            let metadata = inspect(&evaluator, preparation_costs, preparation_seconds);
            Ok((result, Some(metadata)))
        }
        Err(e) => Ok((
            TaskRunResult {
                execution_limits: meter.limits(),
                execution_costs: meter.snapshot(),
                status: RunStatus::Failed,
                requested: run.shots,
                completed: 0,
                failed: 0,
                not_attempted: run.shots,
                seed: run.seed,
                statistics: None,
                partial_statistics: None,
                efficiency: None,
                sampled_records: (!modules.task.sample_records.is_empty()).then(Vec::new),
                failures: vec![RunFailure {
                    phase: RunPhase::Preparation,
                    shot: None,
                    kind: "TaskError",
                    message: e.to_string(),
                }],
                timings: RunTimings {
                    preparation_seconds: Some(preparation_seconds),
                    sampling_seconds: 0.0,
                    total_seconds: started.elapsed().as_secs_f64(),
                },
                completed_network_compilations: 0,
                completed_history_groups: 0,
            },
            None,
        )),
    }
}
