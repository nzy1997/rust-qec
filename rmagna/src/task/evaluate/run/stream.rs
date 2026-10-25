//! One RNG and atomic exact accumulator across bounded in-memory advances.
use super::*;
use crate::statistics::StatisticsError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamRecordRetention {
    /// Drop committed output rows after their contribution is accumulated.
    #[default]
    Discard,
    /// Retain all rows under run.max_retained_record_values for the total run.
    Retain,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct TaskStreamOptions {
    pub run: TaskRunOptions,
    pub records: StreamRecordRetention,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStreamStatus {
    Running,
    /// Every requested draw has committed; derived summaries may still fail.
    DrawsComplete,
    Failed,
}
/// An owned prefix observation. Derived diagnostics never draw entropy or
/// advance the stream. Errors remain explicit rather than fabricated estimates.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskStreamReport {
    pub status: TaskStreamStatus,
    pub requested: u64,
    pub completed: u64,
    pub failed: u64,
    pub not_attempted: u64,
    pub seed: u64,
    pub random_state: u64,
    pub statistics: Result<Option<TaskRunStatistics>, TaskError>,
    pub efficiency: Result<EfficiencySummary, StatisticsError>,
    pub sampled_records: Option<Vec<Vec<bool>>>,
    pub failure: Option<RunFailure>,
    pub execution_limits: ExecutionLimits,
    pub execution_costs: ExecutionCosts,
    pub timings: RunTimings,
    pub completed_network_compilations: usize,
    pub completed_history_groups: usize,
}
/// Borrowed evaluator, continuous SplitMix64, exact totals and shared quotas.
/// The continuously owned deadline includes pauses between advance calls.
/// This type is not a persistent checkpoint and cannot restore an old session.
pub struct TaskStream<'a> {
    evaluator: &'a TaskEvaluator,
    options: TaskStreamOptions,
    accumulator: Accumulator,
    records: Option<Vec<Vec<bool>>>,
    random: SplitMix64,
    meter: ExecutionMeter,
    started: Instant,
    sampling_seconds: f64,
    failure: Option<RunFailure>,
}
struct SamplingTimer<'a> {
    started: Instant,
    seconds: &'a mut f64,
}
impl Drop for SamplingTimer<'_> {
    fn drop(&mut self) {
        *self.seconds += self.started.elapsed().as_secs_f64();
    }
}
impl TaskEvaluator {
    /// Validate the complete stream before entropy, queries or controller calls.
    /// Discarding stored records does not remove their scientific/runtime use.
    pub fn start_stream(&self, options: TaskStreamOptions) -> Result<TaskStream<'_>, TaskError> {
        let retain = options.records == StreamRecordRetention::Retain;
        let dimensions = options.run.validate(
            if retain { self.requested.len() } else { 0 },
            self.task.observables.len(),
        )?;
        let accumulator = Accumulator::new(
            dimensions,
            self.target.is_some() || self.error_parities.is_some(),
            options.run.statistics,
        )?;
        Ok(TaskStream {
            evaluator: self,
            options,
            accumulator,
            records: (retain && !self.requested.is_empty()).then(Vec::new),
            random: SplitMix64::new(options.run.seed),
            meter: ExecutionMeter::new(options.run.execution_limits),
            started: Instant::now(),
            sampling_seconds: 0.0,
            failure: None,
        })
    }
}
impl TaskStream<'_> {
    pub fn status(&self) -> TaskStreamStatus {
        if self.failure.is_some() {
            TaskStreamStatus::Failed
        } else if self.accumulator.statistics.count() == self.options.run.shots {
            TaskStreamStatus::DrawsComplete
        } else {
            TaskStreamStatus::Running
        }
    }
    pub fn random_state(&self) -> u64 {
        self.random.state()
    }
    /// Observe committed data; this does not spend logical queries or entropy.
    /// Prefix statistics are not a complete-run estimate while status is Running.
    pub fn snapshot(&self) -> TaskStreamReport {
        let completed = self.accumulator.statistics.count();
        let failed = u64::from(self.failure.is_some());
        TaskStreamReport {
            status: self.status(),
            requested: self.options.run.shots,
            completed,
            failed,
            not_attempted: self.options.run.shots - completed - failed,
            seed: self.options.run.seed,
            random_state: self.random.state(),
            statistics: self.accumulator.summarize(),
            efficiency: self.accumulator.efficiency.summarize(),
            sampled_records: self.records.clone(),
            failure: self.failure.clone(),
            execution_limits: self.meter.limits(),
            execution_costs: self.meter.snapshot(),
            timings: RunTimings {
                preparation_seconds: None,
                sampling_seconds: self.sampling_seconds,
                total_seconds: self.started.elapsed().as_secs_f64(),
            },
            completed_network_compilations: self.accumulator.compilations,
            completed_history_groups: self.accumulator.groups,
        }
    }
    /// Commit up to draws more rows from the same RNG. A positive call after
    /// completion only observes the final state. Failures (including unwinding
    /// attempts) are terminal and retain the actual consumed random state.
    pub fn advance(&mut self, draws: u64) -> Result<TaskStreamReport, TaskError> {
        if draws == 0 {
            return Err(TaskError::new(
                "stream.draws",
                "advance count must be positive",
            ));
        }
        if self.failure.is_some() {
            return Err(TaskError::new(
                "stream.status",
                "a failed or unwound stream cannot advance",
            ));
        }
        let count = draws.min(self.options.run.shots - self.accumulator.statistics.count());
        if count == 0 {
            return Ok(self.snapshot());
        }
        let timer = SamplingTimer {
            started: Instant::now(),
            seconds: &mut self.sampling_seconds,
        };
        for _ in 0..count {
            let shot = self.accumulator.statistics.count();
            // A caught unwind must not allow a later call to silently skip the
            // partially consumed draw. Ordinary errors replace this sentinel.
            self.failure = Some(RunFailure {
                phase: RunPhase::Sampling,
                shot: Some(shot),
                kind: "Unwind",
                message: "sample attempt unwound before its commit completed".to_owned(),
            });
            match sample_and_commit(
                self.evaluator,
                &mut self.random,
                &self.meter,
                &mut self.accumulator,
                &self.options.run.statistics.arithmetic,
                &mut self.failure.as_mut().expect("attempt sentinel").phase,
            ) {
                Ok(records) => {
                    if let Some(retained) = &mut self.records {
                        retained.push(records);
                    }
                    self.failure = None;
                }
                Err(error) => {
                    let phase = self.failure.as_ref().expect("attempt sentinel").phase;
                    self.failure = Some(RunFailure {
                        phase,
                        shot: Some(shot),
                        kind: "TaskError",
                        message: error.to_string(),
                    });
                    break;
                }
            }
        }
        drop(timer);
        Ok(self.snapshot())
    }
}
