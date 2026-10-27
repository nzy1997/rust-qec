//! Continuous streams identified by immutable loaded file-task snapshots.
use super::*;
use crate::statistics::CheckpointOptions;
use crate::task::{
    StreamRecordRetention, TaskStream, TaskStreamIdentity, TaskStreamOptions, TaskStreamReport,
    TaskStreamStatus, fingerprint_with_configuration,
};

impl FileTask {
    /// Bind canonical loaded content and the actual stream configuration without
    /// reopening files, drawing entropy, preparing queries or calling controllers.
    /// The explicit implementation label and controller declarations are compared
    /// on restoration, but do not authenticate executable or callback code.
    /// Batch fingerprints and their record-retention admission remain unchanged.
    pub fn stream_identity(
        &self,
        options: TaskStreamOptions,
        implementation: &str,
    ) -> Result<TaskStreamIdentity, FileApiError> {
        if implementation.trim().is_empty() {
            return Err(input_error(
                "stream.implementation",
                "an explicit nonempty implementation identity is required",
            ));
        }
        let limits = self.inputs.options.fingerprints;
        if implementation.len() > limits.max_json_bytes {
            return Err(input_error(
                "stream.implementation",
                "implementation identity exceeds the fingerprint byte budget",
            ));
        }
        let task = self.inputs.modules.task();
        options.run.validate(
            if options.records == StreamRecordRetention::Retain {
                task.sample_records().len()
            } else {
                0
            },
            task.observables().len(),
        )?;
        let input = self.fingerprint_input(&options.run);
        let mut configuration = resolved_options(&input);
        configuration["stream"] = json!({
            "encoding": "rmagna-file-task-stream-v1",
            "records": match options.records {
                StreamRecordRetention::Discard => "discard",
                StreamRecordRetention::Retain => "retain",
            },
        });
        let content = fingerprint_with_configuration(
            &input,
            &configuration,
            &sampling_configuration(input.sampling),
            limits,
        )?;
        Ok(TaskStreamIdentity {
            content,
            implementation: implementation.to_owned(),
        })
    }
}

impl CompiledFileTask {
    /// Start one continuous seeded stream. Compilation/preparation of this
    /// evaluator is separate from the stream's cumulative sampling budgets.
    pub fn start_stream(
        &self,
        options: TaskStreamOptions,
        implementation: &str,
    ) -> Result<FileTaskStream<'_>, FileApiError> {
        let identity = self.task.stream_identity(options, implementation)?;
        Ok(FileTaskStream {
            stream: self.evaluator.start_stream(options)?,
            identity,
        })
    }

    /// Restore against this compiled task's own loaded snapshot. No caller
    /// fingerprints are accepted and no source paths are reopened. Malformed or
    /// mismatched checkpoints fail before sampling, queries or controller calls.
    pub fn restore_stream_json(
        &self,
        text: &str,
        options: TaskStreamOptions,
        implementation: &str,
        checkpoint: CheckpointOptions,
    ) -> Result<FileTaskStream<'_>, FileApiError> {
        let identity = self.task.stream_identity(options, implementation)?;
        Ok(FileTaskStream {
            stream: self
                .evaluator
                .restore_stream_json(text, options, &identity, checkpoint)?,
            identity,
        })
    }
}

/// A continuous task stream with automatically derived immutable file identity.
/// Callbacks must remain deterministic functions of explicit inputs; hidden
/// mutable callback state and compiled/prepared caches are not checkpointed.
pub struct FileTaskStream<'a> {
    stream: TaskStream<'a>,
    identity: TaskStreamIdentity,
}
impl FileTaskStream<'_> {
    /// Canonical snapshot/configuration fingerprints plus unverified declarations.
    pub fn identity(&self) -> &TaskStreamIdentity {
        &self.identity
    }
    pub fn status(&self) -> TaskStreamStatus {
        self.stream.status()
    }
    pub fn random_state(&self) -> u64 {
        self.stream.random_state()
    }
    pub fn snapshot(&self) -> TaskStreamReport {
        self.stream.snapshot()
    }
    pub fn advance(&mut self, draws: u64) -> Result<TaskStreamReport, FileApiError> {
        Ok(self.stream.advance(draws)?)
    }
    /// Encode exact committed state using this stream's captured identity.
    /// This does not write a file or perform numerical report export.
    pub fn checkpoint_json(&self, options: CheckpointOptions) -> Result<String, FileApiError> {
        Ok(self.stream.checkpoint_json(&self.identity, options)?)
    }
}
