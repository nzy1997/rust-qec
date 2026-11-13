use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Opt-in compatibility ownership. Every call on this object is nonblocking
/// and exclusive, including reentrant calls from its decoder/observer. Other
/// objects remain independent. The ordinary CompiledFileTask stays concurrent.
#[derive(Debug)]
pub struct ExclusiveCompiledFileTask {
    compiled: CompiledFileTask,
    busy: AtomicBool,
}

#[derive(Debug)]
pub enum ExclusiveFileTaskError {
    InUse,
    File(FileApiError),
}
impl fmt::Display for ExclusiveFileTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InUse => write!(
                f,
                "compiled task is in use by another thread or reentrant call"
            ),
            Self::File(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ExclusiveFileTaskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InUse => None,
            Self::File(error) => Some(error),
        }
    }
}
impl From<FileApiError> for ExclusiveFileTaskError {
    fn from(error: FileApiError) -> Self {
        Self::File(error)
    }
}
struct ActiveCall<'a>(&'a AtomicBool);
impl Drop for ActiveCall<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl CompiledFileTask {
    /// Consume the concurrent task to opt into per-object busy rejection.
    pub fn into_exclusive(self) -> ExclusiveCompiledFileTask {
        ExclusiveCompiledFileTask {
            compiled: self,
            busy: AtomicBool::new(false),
        }
    }
}
impl ExclusiveCompiledFileTask {
    fn enter(&self) -> Result<ActiveCall<'_>, ExclusiveFileTaskError> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ActiveCall(&self.busy))
            .map_err(|_| ExclusiveFileTaskError::InUse)
    }
    pub fn inspect(&self, run: &TaskRunOptions) -> Result<Value, ExclusiveFileTaskError> {
        let _active = self.enter()?;
        Ok(self.compiled.inspect(run)?)
    }
    pub fn run(&self, run: &TaskRunOptions) -> Result<FileRunReport, ExclusiveFileTaskError> {
        self.run_observed(run, None, None)
    }
    pub fn run_with_progress(
        &self,
        run: &TaskRunOptions,
        observer: &mut ProgressObserver<'_>,
    ) -> Result<FileRunReport, ExclusiveFileTaskError> {
        self.run_observed(run, Some(observer), None)
    }
    pub fn run_cancellable(
        &self,
        run: &TaskRunOptions,
        token: &CancellationToken,
    ) -> Result<FileRunReport, ExclusiveFileTaskError> {
        self.run_observed(run, None, Some(token))
    }
    pub fn run_cancellable_with_progress(
        &self,
        run: &TaskRunOptions,
        token: &CancellationToken,
        observer: &mut ProgressObserver<'_>,
    ) -> Result<FileRunReport, ExclusiveFileTaskError> {
        self.run_observed(run, Some(observer), Some(token))
    }
    fn run_observed(
        &self,
        run: &TaskRunOptions,
        observer: Option<&mut ProgressObserver<'_>>,
        token: Option<&CancellationToken>,
    ) -> Result<FileRunReport, ExclusiveFileTaskError> {
        // Guard covers validation, all callbacks, sampling and final metadata.
        // Drop also releases it if a caller-owned callback unwinds.
        let _active = self.enter()?;
        Ok(self.compiled.run_observed(run, observer, token)?)
    }
}
