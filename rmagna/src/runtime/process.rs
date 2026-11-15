//! Persistent, explicitly launched decoder processes. No shell or circuit code is executed.
mod wire;
use super::{ControllerRegistry, RuntimeError};
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex, MutexGuard, TryLockError,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const PROTOCOL: &str = "rmagna.controller.v1";
#[derive(Clone, Copy, Debug)]
pub struct ProcessOptions {
    pub timeout: Duration,
    pub max_frame_bytes: usize,
    pub max_bits: usize,
    pub max_bindings: usize,
}
impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_frame_bytes: 16 * 1024 * 1024,
            max_bits: 1_000_000,
            max_bindings: 1_000_000,
        }
    }
}
#[derive(Debug)]
pub struct ProcessError(String);
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "controller process: {}", self.0)
    }
}
impl std::error::Error for ProcessError {}
fn error(message: impl Into<String>) -> ProcessError {
    ProcessError(message.into())
}
// Termination is independent of the serialized exchange lock. A scoped
// caller waiting behind another exchange can cancel the whole managed worker,
// exactly as a failed/timeout exchange already poisons every shared binding.
struct Worker {
    child: Mutex<Child>,
    stopped: AtomicBool,
    timeout: Duration,
}
impl Worker {
    fn terminate(&self) -> Result<(), ProcessError> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| error("worker process mutex poisoned"))?;
        if child
            .try_wait()
            .map_err(|e| error(format!("cannot inspect worker: {e}")))?
            .is_none()
        {
            if let Err(kill_error) = child.kill() {
                // A child may exit naturally between try_wait and kill.
                if child
                    .try_wait()
                    .map_err(|e| error(format!("cannot inspect worker: {e}")))?
                    .is_none()
                {
                    return Err(error(format!("cannot terminate worker: {kill_error}")));
                }
            }
            child
                .wait()
                .map_err(|e| error(format!("cannot reap worker: {e}")))?;
        }
        self.stopped.store(true, Ordering::Release);
        Ok(())
    }
}
impl From<crate::scalar::ScalarError> for ProcessError {
    fn from(value: crate::scalar::ScalarError) -> Self {
        error(value.to_string())
    }
}
fn process_checkpoint(worker: &Worker) -> Result<(), ProcessError> {
    crate::cancellation::checkpoint_with_cleanup(
        crate::cancellation::SafePoint::DecoderWait,
        || worker.terminate(),
    )
}
fn lock_session<'a>(
    session: &'a Mutex<Session>,
    worker: &Worker,
    started: Instant,
) -> Result<MutexGuard<'a, Session>, ProcessError> {
    if crate::cancellation::observed().is_none() {
        return session
            .lock()
            .map_err(|_| error("controller process mutex poisoned"));
    }
    loop {
        match session.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(error("controller process mutex poisoned"));
            }
            Err(TryLockError::WouldBlock) => {}
        }
        let remaining = worker.timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            worker.terminate()?;
            return Err(error("session lock exceeded the exchange timeout"));
        }
        process_checkpoint(worker)?;
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}
struct Session {
    worker: Arc<Worker>,
    requests: mpsc::SyncSender<Vec<u8>>,
    responses: mpsc::Receiver<Result<Box<RawValue>, String>>,
    options: ProcessOptions,
    next_id: u64,
    failed: Option<String>,
}
impl Session {
    fn fail(&mut self, message: impl Into<String>) -> ProcessError {
        let message = message.into();
        self.failed = Some(message.clone());
        let _ = self.worker.terminate();
        error(message)
    }
    fn receive(&mut self, started: Instant) -> Result<Box<RawValue>, ProcessError> {
        loop {
            let remaining = self.options.timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(self.fail("response unavailable: exchange timeout"));
            }
            if let Err(e) = process_checkpoint(&self.worker) {
                self.failed = Some(e.to_string());
                return Err(e);
            }
            let wait = if crate::cancellation::observed().is_some() {
                remaining.min(Duration::from_millis(10))
            } else {
                remaining
            };
            match self.responses.recv_timeout(wait) {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(message)) => return Err(self.fail(message)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => return Err(self.fail(format!("response unavailable: {e}"))),
            }
        }
    }
    fn evaluate(
        &mut self,
        name: &str,
        bits: &[bool],
        started: Instant,
    ) -> Result<Vec<bool>, ProcessError> {
        if let Some(message) = &self.failed {
            return Err(error(format!("session previously failed: {message}")));
        }
        if self.worker.stopped.load(Ordering::Acquire) {
            return Err(error("worker process has been stopped"));
        }
        if bits.len() > self.options.max_bits {
            return Err(error("input bit budget exceeded before sending request"));
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| error("request ID exhausted"))?;
        let mut bytes = serde_json::to_vec(&json!({"id":id,"name":name,"bits":bits}))
            .map_err(|e| error(e.to_string()))?;
        bytes.push(b'\n');
        if bytes.len() > self.options.max_frame_bytes {
            return Err(error(
                "request frame budget exceeded before sending request",
            ));
        }
        if started.elapsed() >= self.options.timeout {
            return Err(self.fail("exchange timeout before sending request"));
        }
        if let Err(e) = process_checkpoint(&self.worker) {
            self.failed = Some(e.to_string());
            return Err(e);
        }
        if let Err(e) = self.requests.try_send(bytes) {
            return Err(self.fail(format!("cannot send request: {e}")));
        }
        let response = self.receive(started)?;
        let response: wire::Response = serde_json::from_str(response.get())
            .map_err(|e| self.fail(format!("invalid controller response: {e}")))?;
        if response.id != id {
            return Err(self.fail("response ID does not match request"));
        }
        if let Some(message) = response.error {
            return Err(self.fail(format!("decoder {name:?} failed: {message}")));
        }
        let Some(outputs) = response.outputs else {
            return Err(self.fail("response outputs must be a bit list"));
        };
        if outputs.len() > self.options.max_bits {
            return Err(self.fail("output bit budget exceeded"));
        }
        if started.elapsed() > self.options.timeout {
            return Err(self.fail("response validation exceeded the exchange timeout"));
        }
        Ok(outputs.into_iter().map(|bit| bit.0).collect())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.worker.terminate();
    }
}
fn read_frame(reader: &mut impl BufRead, limit: usize) -> Result<Box<RawValue>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| format!("cannot read frame: {e}"))?;
    if bytes.len() > limit {
        return Err("response frame budget exceeded".to_owned());
    }
    if bytes.last() != Some(&b'\n') {
        return Err("worker closed stdout before a complete response".to_owned());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON response: {e}"))
}
/// Bindings share one serialized process, with a whole-exchange timeout.
/// A failed or timed-out exchange poisons the session; no callback is retried.
#[derive(Clone)]
pub struct ControllerProcess {
    session: Arc<Mutex<Session>>,
    worker: Arc<Worker>,
    versions: BTreeMap<String, String>,
    manifest: Value,
}
impl ControllerProcess {
    /// The caller explicitly supplies the executable, arguments, directory and
    /// environment. Stdin/stdout are replaced by protocol pipes; stderr inherits.
    pub fn spawn(mut command: Command, options: ProcessOptions) -> Result<Self, ProcessError> {
        if options.timeout.is_zero()
            || options.max_frame_bytes < 2
            || options.max_frame_bytes == usize::MAX
        {
            return Err(error(
                "timeout must be positive and frame budget must be bounded",
            ));
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| error(format!("cannot start worker: {e}")))?;
        let mut input = child.stdin.take().expect("piped stdin");
        let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
        let (requests, receiver) = mpsc::sync_channel::<Vec<u8>>(1);
        let (sender, responses) = mpsc::sync_channel(1);
        let limit = options.max_frame_bytes;
        // The I/O thread owns both pipes so timeout includes a blocked write.
        // Dropping the final session kills/reaps the worker and drops its channel.
        std::thread::Builder::new()
            .name("rmagna-controller-io".to_owned())
            .spawn(move || {
                let hello = read_frame(&mut output, limit);
                let failed = hello.is_err();
                if sender.send(hello).is_err() || failed {
                    return;
                }
                while let Ok(request) = receiver.recv() {
                    let response = input
                        .write_all(&request)
                        .and_then(|()| input.flush())
                        .map_err(|e| format!("cannot write request: {e}"))
                        .and_then(|()| read_frame(&mut output, limit));
                    let failed = response.is_err();
                    if sender.send(response).is_err() || failed {
                        return;
                    }
                }
            })
            .map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                error(format!("cannot start worker I/O: {e}"))
            })?;
        let worker = Arc::new(Worker {
            child: Mutex::new(child),
            stopped: AtomicBool::new(false),
            timeout: options.timeout,
        });
        let mut session = Session {
            worker: Arc::clone(&worker),
            requests,
            responses,
            options,
            next_id: 0,
            failed: None,
        };
        let started = Instant::now();
        let hello: wire::Hello = serde_json::from_str(session.receive(started)?.get())
            .map_err(|e| error(format!("invalid worker greeting: {e}")))?;
        if hello.protocol != PROTOCOL {
            return Err(error("unsupported worker greeting protocol or fields"));
        }
        let bindings = hello.bindings;
        if bindings.len() > options.max_bindings {
            return Err(error("worker binding budget exceeded"));
        }
        let mut versions = BTreeMap::new();
        for (name, version) in bindings {
            if version.is_empty() {
                return Err(error("worker binding version must be nonempty"));
            }
            if name.is_empty() || matches!(name.as_str(), "XOR" | "NOT") {
                return Err(error("worker binding name must be a non-built-in name"));
            }
            versions.insert(name, version);
        }
        let manifest = wire::metadata(hello.manifest.as_deref()).map_err(error)?;
        if started.elapsed() > options.timeout {
            return Err(error("greeting validation exceeded the startup timeout"));
        }
        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            worker,
            versions,
            manifest,
        })
    }
    /// Inspect/reap the actual managed child without acquiring the exchange
    /// lock. A returned exit status also prevents any later request/retry.
    pub fn try_wait(&self) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let status = self
            .worker
            .child
            .lock()
            .map_err(|_| error("worker process mutex poisoned"))?
            .try_wait()
            .map_err(|e| error(format!("cannot inspect worker: {e}")))?;
        if status.is_some() {
            self.worker.stopped.store(true, Ordering::Release);
        }
        Ok(status)
    }
    pub fn versions(&self) -> &BTreeMap<String, String> {
        &self.versions
    }
    /// Worker-supplied provenance; callers must independently verify identities
    /// when making reproducibility claims about externally launched programs.
    pub fn manifest(&self) -> &Value {
        &self.manifest
    }
    pub fn evaluate(&self, name: &str, inputs: &[bool]) -> Result<Vec<bool>, ProcessError> {
        if !self.versions.contains_key(name) {
            return Err(error(format!("worker has no binding {name:?}")));
        }
        let started = Instant::now();
        lock_session(&self.session, &self.worker, started)?.evaluate(name, inputs, started)
    }
    /// Atomic registration: a collision leaves the caller's registry unchanged.
    pub fn register_all(&self, registry: &mut ControllerRegistry) -> Result<(), RuntimeError> {
        let mut candidate = registry.clone();
        for (name, version) in &self.versions {
            let session = self.session.clone();
            let worker = Arc::clone(&self.worker);
            let name = name.clone();
            candidate.register(name.clone(), version.clone(), move |bits: &[bool]| {
                let started = Instant::now();
                lock_session(&session, &worker, started)
                    .and_then(|mut session| session.evaluate(&name, bits, started))
                    .map_err(|e| e.to_string())
            })?;
        }
        *registry = candidate;
        Ok(())
    }
}
