//! Generic recovery with declared identities and automatically bound settings.
use super::*;
use crate::costs::{ExecutionMeterCheckpoint, OperationCost};
use crate::statistics::checkpoint::{decode, encode, object};
use crate::statistics::{CheckpointError, CheckpointOptions};
use crate::task::{
    ContentFingerprints, FingerprintOptions, canonical_fingerprint, execution_configuration,
    sampling_configuration,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Caller declarations, compared exactly but never authenticated by the generic
/// evaluator. Fingerprints are canonical lowercase SHA-256 hex strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskStreamIdentity {
    pub content: ContentFingerprints,
    /// Identifies the evaluator/controller implementation and checkpoint contract.
    pub implementation: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    circuit: String,
    task: String,
    ideal_state: String,
    recovery: String,
    controllers: String,
    options: String,
    sampling: String,
    implementation: String,
}
impl From<&TaskStreamIdentity> for IdentityWire {
    fn from(identity: &TaskStreamIdentity) -> Self {
        let c = &identity.content;
        Self {
            circuit: c.circuit.clone(),
            task: c.task.clone(),
            ideal_state: c.ideal_state.clone(),
            recovery: c.recovery.clone(),
            controllers: c.controllers.clone(),
            options: c.options.clone(),
            sampling: c.sampling.clone(),
            implementation: identity.implementation.clone(),
        }
    }
}
impl IdentityWire {
    fn matches(&self, identity: &TaskStreamIdentity) -> bool {
        let c = &identity.content;
        self.circuit == c.circuit
            && self.task == c.task
            && self.ideal_state == c.ideal_state
            && self.recovery == c.recovery
            && self.controllers == c.controllers
            && self.options == c.options
            && self.sampling == c.sampling
            && self.implementation == identity.implementation
    }
}
fn fail(field: &str, message: impl std::fmt::Display) -> TaskError {
    TaskError::new(format!("stream.checkpoint.{field}"), message.to_string())
}
fn checkpoint_error(error: CheckpointError) -> TaskError {
    fail(&error.field, error.message)
}
fn validate_identity(identity: &TaskStreamIdentity) -> Result<(), TaskError> {
    let c = &identity.content;
    if [
        &c.circuit,
        &c.task,
        &c.ideal_state,
        &c.recovery,
        &c.controllers,
        &c.options,
        &c.sampling,
    ]
    .iter()
    .any(|s| {
        s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err(fail(
            "identity",
            "content fingerprints must be 64 lowercase hexadecimal digits",
        ));
    }
    if identity.implementation.trim().is_empty() {
        return Err(fail(
            "identity.implementation",
            "an explicit implementation identity is required",
        ));
    }
    Ok(())
}
fn required<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d)
}
fn optional_object<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visitor<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<T> {
        type Value = Option<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("null or a JSON object")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            object(d).map(Some)
        }
    }
    d.deserialize_option(Visitor(std::marker::PhantomData))
}
fn unsigned(text: &str, field: &str) -> Result<u64, TaskError> {
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(fail(field, "expected a canonical u64 decimal string"));
    }
    text.parse().map_err(|e| fail(field, e))
}
fn size(text: &str, field: &str) -> Result<usize, TaskError> {
    usize::try_from(unsigned(text, field)?).map_err(|e| fail(field, e))
}
fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}
fn seconds(text: &str, field: &str) -> Result<f64, TaskError> {
    if text.len() != 16
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(fail(
            field,
            "expected exactly 16 lowercase hexadecimal float bits",
        ));
    }
    let value = f64::from_bits(u64::from_str_radix(text, 16).map_err(|e| fail(field, e))?);
    if !value.is_finite() || value < 0.0 {
        return Err(fail(
            field,
            "diagnostic seconds must be finite and nonnegative",
        ));
    }
    Ok(value)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurationWire {
    seconds: String,
    nanoseconds: String,
}
impl From<Duration> for DurationWire {
    fn from(d: Duration) -> Self {
        Self {
            seconds: d.as_secs().to_string(),
            nanoseconds: d.subsec_nanos().to_string(),
        }
    }
}
impl DurationWire {
    fn restore(&self, field: &str) -> Result<Duration, TaskError> {
        let s = unsigned(&self.seconds, field)?;
        let n = unsigned(&self.nanoseconds, field)?;
        if n >= 1_000_000_000 {
            return Err(fail(
                field,
                "duration nanoseconds must be below one billion",
            ));
        }
        Ok(Duration::new(s, n as u32))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CostWire {
    attempted: String,
    succeeded: String,
    failed: String,
    seconds: String,
}
impl From<OperationCost> for CostWire {
    fn from(c: OperationCost) -> Self {
        Self {
            attempted: c.attempted.to_string(),
            succeeded: c.succeeded.to_string(),
            failed: c.failed.to_string(),
            seconds: bits(c.seconds),
        }
    }
}
impl CostWire {
    fn restore(&self) -> Result<OperationCost, TaskError> {
        Ok(OperationCost {
            attempted: unsigned(&self.attempted, "meter.attempted")?,
            succeeded: unsigned(&self.succeeded, "meter.succeeded")?,
            failed: unsigned(&self.failed, "meter.failed")?,
            seconds: seconds(&self.seconds, "meter.seconds")?,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeterWire {
    max_queries: String,
    #[serde(deserialize_with = "optional_object")]
    max_duration: Option<DurationWire>,
    #[serde(deserialize_with = "object")]
    elapsed: DurationWire,
    #[serde(deserialize_with = "object")]
    compilation: CostWire,
    #[serde(deserialize_with = "object")]
    query: CostWire,
    #[serde(deserialize_with = "object")]
    decoder: CostWire,
    #[serde(deserialize_with = "object")]
    support: CostWire,
}
impl From<ExecutionMeterCheckpoint> for MeterWire {
    fn from(m: ExecutionMeterCheckpoint) -> Self {
        Self {
            max_queries: m.limits.max_queries.to_string(),
            max_duration: m.limits.max_duration.map(Into::into),
            elapsed: m.elapsed.into(),
            compilation: m.costs.compilation.into(),
            query: m.costs.query.into(),
            decoder: m.costs.decoder.into(),
            support: m.costs.support.into(),
        }
    }
}
impl MeterWire {
    fn restore(&self) -> Result<ExecutionMeterCheckpoint, TaskError> {
        Ok(ExecutionMeterCheckpoint {
            limits: ExecutionLimits {
                max_queries: unsigned(&self.max_queries, "meter.max_queries")?,
                max_duration: self
                    .max_duration
                    .as_ref()
                    .map(|d| d.restore("meter.max_duration"))
                    .transpose()?,
            },
            elapsed: self.elapsed.restore("meter.elapsed")?,
            costs: ExecutionCosts {
                compilation: self.compilation.restore()?,
                query: self.query.restore()?,
                decoder: self.decoder.restore()?,
                support: self.support.restore()?,
            },
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureWire {
    phase: String,
    shot: String,
    kind: String,
    message: String,
}
impl FailureWire {
    fn capture(f: &RunFailure) -> Result<Self, TaskError> {
        let phase = match f.phase {
            RunPhase::Sampling => "sampling",
            RunPhase::Accumulation => "accumulation",
            _ => {
                return Err(fail(
                    "failure.phase",
                    "generic streams only fail during sampling or accumulation",
                ));
            }
        };
        Ok(Self {
            phase: phase.to_owned(),
            shot: f
                .shot
                .ok_or_else(|| {
                    fail(
                        "failure.shot",
                        "a terminal draw failure requires its draw index",
                    )
                })?
                .to_string(),
            kind: f.kind.to_owned(),
            message: f.message.clone(),
        })
    }
    fn restore(&self, completed: u64) -> Result<RunFailure, TaskError> {
        let phase = match self.phase.as_str() {
            "sampling" => RunPhase::Sampling,
            "accumulation" => RunPhase::Accumulation,
            _ => return Err(fail("failure.phase", "expected sampling or accumulation")),
        };
        let kind = match self.kind.as_str() {
            "TaskError" => "TaskError",
            "Unwind" => "Unwind",
            _ => return Err(fail("failure.kind", "expected TaskError or Unwind")),
        };
        if unsigned(&self.shot, "failure.shot")? != completed || self.message.is_empty() {
            return Err(fail(
                "failure",
                "terminal failure index/message do not match the committed prefix",
            ));
        }
        Ok(RunFailure {
            phase,
            shot: Some(completed),
            kind,
            message: self.message.clone(),
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamWire {
    schema_version: u32,
    kind: String,
    attestation: String,
    #[serde(deserialize_with = "object")]
    identity: IdentityWire,
    actual_settings: String,
    original_seed: String,
    random_state: String,
    requested: String,
    committed: String,
    failed: String,
    not_attempted: String,
    status: String,
    record_policy: String,
    #[serde(deserialize_with = "required")]
    records: Option<Vec<Vec<bool>>>,
    statistics: String,
    efficiency: String,
    has_error: bool,
    completed_network_compilations: String,
    completed_history_groups: String,
    #[serde(deserialize_with = "object")]
    meter: MeterWire,
    sampling_seconds: String,
    total_seconds: String,
    #[serde(deserialize_with = "optional_object")]
    failure: Option<FailureWire>,
}
fn record_policy(options: TaskStreamOptions) -> &'static str {
    match options.records {
        StreamRecordRetention::Discard => "discard",
        StreamRecordRetention::Retain => "retain",
    }
}
fn settings(
    evaluator: &TaskEvaluator,
    options: TaskStreamOptions,
    checkpoint: CheckpointOptions,
) -> Result<String, TaskError> {
    let material = evaluator.task.sample_records.len() as u128
        + evaluator.task.outputs.len() as u128
        + evaluator
            .task
            .error_observables
            .as_ref()
            .map_or(0, |items| items.len() as u128)
        + evaluator
            .task
            .observables
            .iter()
            .map(|w| w.x().ones().count() as u128 + w.z().ones().count() as u128 + 1)
            .sum::<u128>();
    if material > checkpoint.max_json_bytes as u128 {
        return Err(fail(
            "max_json_bytes",
            "actual settings exceed the byte budget before encoding",
        ));
    }
    if let crate::noise::NoiseSampling::Importance { proposal_scale } = &evaluator.sampling
        && proposal_scale
            .numer()
            .bits()
            .max(proposal_scale.denom().bits())
            .saturating_sub(1) as u128
            / 4
            + 1
            > checkpoint.max_json_bytes as u128
    {
        return Err(fail(
            "max_json_bytes",
            "proposal integer text exceeds the byte budget",
        ));
    }
    let quantities=evaluator.task.observables.iter().map(|w|serde_json::json!({"width":w.width(),"phase":w.phase(),"x":w.x().ones().collect::<Vec<_>>(),"z":w.z().ones().collect::<Vec<_>>() })).collect::<Vec<_>>();
    let value = serde_json::json!({"encoding":"rmagna-task-stream-settings-v1","evaluation_and_run":execution_configuration(evaluator.options,options.run),"noise_sampling":sampling_configuration(&evaluator.sampling),"record_policy":record_policy(options),"execution_route":evaluator.execution_route(),"sample_records":evaluator.task.sample_records,"outputs":evaluator.task.outputs,"error_observables":evaluator.task.error_observables,"observables":quantities,"has_target":evaluator.target.is_some()});
    canonical_fingerprint(
        &value,
        FingerprintOptions {
            max_json_bytes: checkpoint.max_json_bytes,
            max_depth: 64,
        },
    )
    .map_err(|e| fail("actual_settings", e))
}
impl TaskStream<'_> {
    /// Identity retained by a restored checkpoint. These are caller-declared,
    /// unverified labels; they do not authenticate source or controller code.
    pub fn declared_checkpoint_identity(&self) -> Option<&TaskStreamIdentity> {
        self.declared_identity.as_ref()
    }
    /// Encode raw exact totals, RNG/cursors, retained rows and cumulative quotas.
    /// No controller/quantum query or RNG call occurs. The output and nested
    /// accumulator encodings obey max_json_bytes. Controller implementations must
    /// be deterministic functions of explicit inputs/configuration; hidden mutable
    /// controller state is not captured by this generic checkpoint.
    pub fn checkpoint_json(
        &self,
        identity: &TaskStreamIdentity,
        checkpoint: CheckpointOptions,
    ) -> Result<String, TaskError> {
        validate_identity(identity)?;
        if self
            .declared_identity
            .as_ref()
            .is_some_and(|old| old != identity)
        {
            return Err(fail("identity", "a restored stream cannot be relabeled"));
        }
        if identity.implementation.len() > checkpoint.max_json_bytes
            || self
                .failure
                .as_ref()
                .is_some_and(|f| f.message.len() > checkpoint.max_json_bytes)
        {
            return Err(fail(
                "max_json_bytes",
                "identity or failure text exceeds the byte budget before cloning",
            ));
        }
        if let Some(rows) = &self.records {
            let values = rows.iter().map(|r| r.len() as u128).sum::<u128>();
            if values > checkpoint.max_json_bytes as u128 || rows.len() > checkpoint.max_json_bytes
            {
                return Err(fail(
                    "max_json_bytes",
                    "retained rows exceed the byte budget before cloning",
                ));
            }
        }
        let completed = self.accumulator.statistics.count();
        let failed = u64::from(self.failure.is_some());
        let wire = StreamWire {
            schema_version: 1,
            kind: "task_stream".to_owned(),
            attestation: "caller_declared_unverified".to_owned(),
            identity: identity.into(),
            actual_settings: settings(self.evaluator, self.options, checkpoint)?,
            original_seed: self.options.run.seed.to_string(),
            random_state: self.random.state().to_string(),
            requested: self.options.run.shots.to_string(),
            committed: completed.to_string(),
            failed: failed.to_string(),
            not_attempted: (self.options.run.shots - completed - failed).to_string(),
            status: match self.status() {
                TaskStreamStatus::Running => "running",
                TaskStreamStatus::DrawsComplete => "draws_complete",
                TaskStreamStatus::Failed => "failed",
            }
            .to_owned(),
            record_policy: record_policy(self.options).to_owned(),
            records: self.records.clone(),
            statistics: self
                .accumulator
                .statistics
                .checkpoint_json(checkpoint)
                .map_err(checkpoint_error)?,
            efficiency: self
                .accumulator
                .efficiency
                .checkpoint_json(checkpoint)
                .map_err(checkpoint_error)?,
            has_error: self.accumulator.has_error,
            completed_network_compilations: self.accumulator.compilations.to_string(),
            completed_history_groups: self.accumulator.groups.to_string(),
            sampling_seconds: bits(self.sampling_seconds),
            total_seconds: bits(self.total_seconds_offset + self.started.elapsed().as_secs_f64()),
            meter: self.meter.checkpoint().map_err(TaskError::from)?.into(),
            failure: self
                .failure
                .as_ref()
                .map(FailureWire::capture)
                .transpose()?,
        };
        encode(&wire, checkpoint).map_err(checkpoint_error)
    }
}
impl TaskEvaluator {
    /// Restore under explicit matching declarations and actual settings, before
    /// any entropy, quantum query or controller callback. Prepare derived
    /// evaluator/compiler resources separately. Downtime is excluded; the saved
    /// elapsed prefix and quotas stay charged. Failure/unwind state is terminal.
    pub fn restore_stream_json(
        &self,
        text: &str,
        options: TaskStreamOptions,
        identity: &TaskStreamIdentity,
        checkpoint: CheckpointOptions,
    ) -> Result<TaskStream<'_>, TaskError> {
        let wire: StreamWire = decode(text, checkpoint).map_err(checkpoint_error)?;
        if wire.schema_version != 1
            || wire.kind != "task_stream"
            || wire.attestation != "caller_declared_unverified"
        {
            return Err(fail(
                "schema",
                "expected task_stream schema 1 with caller_declared_unverified attestation",
            ));
        }
        validate_identity(identity)?;
        if !wire.identity.matches(identity) {
            return Err(fail(
                "identity",
                "loaded-content/controller fingerprints or implementation identity changed",
            ));
        }
        if wire.actual_settings != settings(self, options, checkpoint)? {
            return Err(fail(
                "actual_settings",
                "actual evaluator, runtime, sampling or run settings changed",
            ));
        }
        let requested = unsigned(&wire.requested, "requested")?;
        let completed = unsigned(&wire.committed, "committed")?;
        let failed = unsigned(&wire.failed, "failed")?;
        let remaining = unsigned(&wire.not_attempted, "not_attempted")?;
        if requested != options.run.shots
            || unsigned(&wire.original_seed, "original_seed")? != options.run.seed
            || failed > 1
            || completed
                .checked_add(failed)
                .and_then(|n| n.checked_add(remaining))
                != Some(requested)
        {
            return Err(fail(
                "cursors",
                "requested, committed, failed and not-attempted counts disagree",
            ));
        }
        let failure = wire
            .failure
            .as_ref()
            .map(|f| f.restore(completed))
            .transpose()?;
        let expected_status = if failure.is_some() {
            "failed"
        } else if completed == requested {
            "draws_complete"
        } else {
            "running"
        };
        if wire.status != expected_status
            || failed != u64::from(failure.is_some())
            || (failure.is_some() && completed >= requested)
        {
            return Err(fail("status", "terminal state and draw cursors disagree"));
        }
        let has_error = self.target.is_some() || self.error_parities.is_some();
        if wire.has_error != has_error || wire.record_policy != record_policy(options) {
            return Err(fail(
                "shape",
                "error/record policies disagree with the evaluator",
            ));
        }
        let retain = options.records == StreamRecordRetention::Retain;
        let dimensions = options.run.validate(
            if retain { self.requested.len() } else { 0 },
            self.task.observables.len(),
        )?;
        match &wire.records {
            Some(rows) if retain && !self.requested.is_empty() => {
                if u64::try_from(rows.len()).ok() != Some(completed)
                    || rows.iter().any(|r| r.len() != self.requested.len())
                {
                    return Err(fail(
                        "records",
                        "retained row count or width differs from the committed prefix",
                    ));
                }
            }
            None if !retain || self.requested.is_empty() => {}
            _ => {
                return Err(fail(
                    "records",
                    "retained/discarded row storage does not match the policy",
                ));
            }
        }
        let statistics = ExactStatistics::from_checkpoint_json(
            &wire.statistics,
            options.run.statistics,
            checkpoint,
        )
        .map_err(checkpoint_error)?;
        let efficiency = HistoryEfficiency::from_checkpoint_json(
            &wire.efficiency,
            options.run.statistics.arithmetic,
            checkpoint,
        )
        .map_err(checkpoint_error)?;
        if statistics.num_quantities() != dimensions
            || statistics.count() != completed
            || !efficiency.consistent_with(&statistics, has_error)
        {
            return Err(fail(
                "statistics",
                "accumulator dimensions, counts or shared exact totals disagree",
            ));
        }
        let meter = wire.meter.restore()?;
        if meter.limits != options.run.execution_limits {
            return Err(fail(
                "meter.limits",
                "saved limits disagree with actual run settings",
            ));
        }
        let compilations = size(
            &wire.completed_network_compilations,
            "completed_network_compilations",
        )?;
        let groups = size(&wire.completed_history_groups, "completed_history_groups")?;
        if (completed == 0 && (compilations != 0 || groups != 0))
            || compilations as u128 > u128::from(meter.costs.compilation.succeeded)
            || groups as u128
                > u128::from(meter.costs.query.succeeded)
                    + u128::from(meter.costs.support.succeeded)
        {
            return Err(fail(
                "resources",
                "committed resource counts exceed completed measured work",
            ));
        }
        let sampling_seconds = seconds(&wire.sampling_seconds, "sampling_seconds")?;
        let total_seconds_offset = seconds(&wire.total_seconds, "total_seconds")?;
        if sampling_seconds > total_seconds_offset
            || total_seconds_offset > meter.elapsed.as_secs_f64()
        {
            return Err(fail(
                "timings",
                "sampling/total prefixes disagree with charged elapsed time",
            ));
        }
        let random = SplitMix64::new(unsigned(&wire.random_state, "random_state")?);
        if completed == 0
            && failure.is_none()
            && (random.state() != options.run.seed || meter.costs != ExecutionCosts::default())
        {
            return Err(fail(
                "empty",
                "an empty running stream has its original RNG and no measured operations",
            ));
        }
        let meter = ExecutionMeter::from_checkpoint(meter).map_err(TaskError::from)?;
        Ok(TaskStream {
            evaluator: self,
            options,
            accumulator: Accumulator {
                statistics,
                efficiency,
                has_error,
                compilations,
                groups,
            },
            records: wire.records,
            random,
            meter,
            started: Instant::now(),
            sampling_seconds,
            total_seconds_offset,
            failure,
            declared_identity: Some(identity.clone()),
        })
    }
}
