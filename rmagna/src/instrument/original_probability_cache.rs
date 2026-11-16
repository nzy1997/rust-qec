//! Explicit probability-value LRU, separate from prepared query geometry.
use super::*;
use crate::unitary::ScalarStateKey;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalProbabilityCacheStatistics {
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub bypasses: usize,
    pub probability_evaluations: usize,
    /// Exact key and scalar encoding payload, excluding Rust container/token
    /// metadata, allocator rounding, fresh branch construction and scratch.
    pub serialized_bytes: usize,
}
struct Entry {
    identity: Arc<()>,
    key: Vec<u8>,
    value: ExactScalar,
    bytes: usize,
}
pub struct OriginalCachedAssignment {
    certificate: OriginalAssignmentCertificate,
    cache_hit: bool,
}
impl OriginalCachedAssignment {
    pub fn certificate(&self) -> &OriginalAssignmentCertificate {
        &self.certificate
    }
    pub fn cache_hit(&self) -> bool {
        self.cache_hit
    }
    pub fn into_certificate(self) -> OriginalAssignmentCertificate {
        self.certificate
    }
}
/// Bounded exact probability values for explicit immutable source snapshots.
/// Every request still captures inputs, admits the source's limits, executes
/// all actual callbacks and compiles their fresh emissions before lookup.
/// Entries retain neither branches nor callbacks. Keys encode the complete
/// compiled wavefunction and physical projection, not just callback versions
/// or input hashes. Proposal probabilities are deliberately outside this API.
/// A new plan (including changed limits/arithmetic) has a different identity.
pub struct OriginalProbabilityCache {
    max_entries: usize,
    max_serialized_bytes: usize,
    entries: VecDeque<Entry>,
    statistics: OriginalProbabilityCacheStatistics,
}
impl Default for OriginalProbabilityCache {
    fn default() -> Self {
        Self::new(32, 1 << 20)
    }
}
impl OriginalProbabilityCache {
    pub fn new(max_entries: usize, max_serialized_bytes: usize) -> Self {
        Self {
            max_entries,
            max_serialized_bytes,
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
    pub fn statistics(&self) -> OriginalProbabilityCacheStatistics {
        self.statistics
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.statistics.serialized_bytes = 0;
    }
    pub fn limits(&self) -> (usize, usize) {
        (self.max_entries, self.max_serialized_bytes)
    }
    pub fn set_limits(&mut self, max_entries: usize, max_serialized_bytes: usize) {
        self.max_entries = max_entries;
        self.max_serialized_bytes = max_serialized_bytes;
        while self.entries.len() > max_entries
            || self.statistics.serialized_bytes > max_serialized_bytes
        {
            self.evict();
        }
    }
    fn evict(&mut self) {
        if let Some(entry) = self.entries.pop_front() {
            self.statistics.serialized_bytes -= entry.bytes;
            self.statistics.evictions = self.statistics.evictions.saturating_add(1);
        }
    }
    fn key(&self, branch: &OriginalBranch) -> Option<Vec<u8>> {
        if self.max_entries == 0 {
            return None;
        }
        let mut key = ScalarStateKey::new(self.max_serialized_bytes);
        branch.unitary.write_scalar_state_key(&mut key)?;
        key.number(branch.keep.len())?;
        for slot in &branch.keep {
            key.number(*slot)?;
        }
        key.number(branch.fixed.len())?;
        for (slot, value) in &branch.fixed {
            key.number(*slot)?;
            key.bytes(&[u8::from(*value)])?;
        }
        Some(key.finish())
    }
    pub fn compile(
        &mut self,
        plan: &OriginalAssignmentPlan,
        input: impl Into<OriginalAssignmentInput>,
    ) -> Result<OriginalCachedAssignment, OriginalError> {
        // The normal batch path captures and validates before invoking this
        // kernel. No result lookup can bypass an actual callback or compiler.
        plan.probabilities([input.into()])
            .next_with_kernel(|assignment, branch, arithmetic| {
                let key = self.key(&branch);
                let position = key.as_ref().and_then(|key| {
                    self.entries.iter().position(|entry| {
                        Arc::ptr_eq(&entry.identity, &plan.0.identity) && entry.key == *key
                    })
                });
                let cache_hit = position.is_some();
                let value = if let Some(position) = position {
                    let entry = self
                        .entries
                        .remove(position)
                        .expect("located probability entry");
                    let value = entry.value.clone();
                    self.entries.push_back(entry);
                    self.statistics.hits = self.statistics.hits.saturating_add(1);
                    value
                } else {
                    self.statistics.misses = self.statistics.misses.saturating_add(1);
                    self.statistics.probability_evaluations =
                        self.statistics.probability_evaluations.saturating_add(1);
                    let value = branch.probability(&arithmetic)?;
                    let retained = key.and_then(|key| {
                        let mut encoding =
                            ScalarStateKey::new(self.max_serialized_bytes - key.len());
                        encoding.scalar(&value)?;
                        let bytes = key.len() + encoding.finish().len();
                        Some((key, bytes))
                    });
                    if let Some((key, bytes)) = retained {
                        while self.entries.len() >= self.max_entries
                            || bytes > self.max_serialized_bytes - self.statistics.serialized_bytes
                        {
                            self.evict();
                        }
                        self.statistics.serialized_bytes += bytes;
                        self.entries.push_back(Entry {
                            identity: Arc::clone(&plan.0.identity),
                            key,
                            value: value.clone(),
                            bytes,
                        });
                    } else {
                        self.statistics.bypasses = self.statistics.bypasses.saturating_add(1);
                    }
                    value
                };
                let encoded = certificate::input_key_json(&assignment).to_string();
                plan.0.input_key_generations.fetch_add(1, Ordering::Relaxed);
                let input_fingerprint = format!("{:x}", Sha256::digest(encoded.as_bytes()));
                Ok(OriginalCachedAssignment {
                    certificate: OriginalAssignmentCertificate::new(
                        assignment,
                        branch,
                        value,
                        input_fingerprint,
                        Arc::clone(&plan.0.identity),
                    ),
                    cache_hit,
                })
            })
            .expect("one captured assignment")
    }
}
