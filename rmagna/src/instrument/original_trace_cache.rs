//! Explicit bounded reuse of immutable source traces, without cached callbacks.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalTraceCacheStatistics {
    /// Actual calls to the trace compiler, including failed preparations.
    pub compilation_attempts: usize,
    pub compilations: usize,
    pub hits: usize,
    pub evictions: usize,
}
struct Entry {
    identity: Arc<()>,
    max_path_bits: usize,
    trace: Arc<OriginalStaticTrace>,
}
/// LRU keyed by explicit source-session identity and the requested path budget.
/// An entry owns a trace and a token, never the source IR or its callbacks.
/// Eviction drops this cache's strong reference; an externally held Arc remains
/// usable. No result-value cache, automatic source matching, or fallback exists.
/// Opaque callbacks are refused by the trace compiler; assignment evaluation
/// continues to execute callbacks freshly through [`OriginalAssignmentPlan`].
pub struct OriginalTraceCache {
    max_entries: usize,
    entries: VecDeque<Entry>,
    statistics: OriginalTraceCacheStatistics,
}
impl OriginalTraceCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: VecDeque::new(),
            statistics: Default::default(),
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn statistics(&self) -> OriginalTraceCacheStatistics {
        self.statistics
    }
    /// Compile under an explicit override bounded by this source's admitted
    /// path budget. A smaller budget never reuses a trace made with another one.
    pub fn prepare(
        &mut self,
        plan: &OriginalAssignmentPlan,
        max_path_bits: usize,
    ) -> Result<Arc<OriginalStaticTrace>, OriginalError> {
        let snapshot = &plan.0;
        if let Some(index) = self.entries.iter().position(|entry| {
            Arc::ptr_eq(&entry.identity, &snapshot.identity) && entry.max_path_bits == max_path_bits
        }) {
            let entry = self.entries.remove(index).expect("located cache entry");
            let trace = Arc::clone(&entry.trace);
            self.entries.push_back(entry);
            self.statistics.hits += 1;
            return Ok(trace);
        }
        let mut instrument = snapshot.instrument.clone();
        // This override cannot silently weaken the immutable source's quota.
        instrument.limits.max_path_bits = instrument.limits.max_path_bits.min(max_path_bits);
        self.statistics.compilation_attempts += 1;
        let trace = Arc::new(instrument.compile_trace(snapshot.strategy, &snapshot.arithmetic)?);
        self.statistics.compilations += 1;
        if self.max_entries > 0 {
            if self.entries.len() == self.max_entries {
                self.entries.pop_front();
                self.statistics.evictions += 1;
            }
            self.entries.push_back(Entry {
                identity: Arc::clone(&snapshot.identity),
                max_path_bits,
                trace: Arc::clone(&trace),
            });
        }
        Ok(trace)
    }
}
