//! Bounded, owned complete-certificate snapshots with explicit payload charges.
//!
//! Entry bytes are key bytes, blob bytes, and the caller's declared scalar
//! payload bytes. They describe a serialization budget, excluding Rust/allocator
//! metadata and process RSS. Exclusive mutable access serializes operations;
//! callers sharing a cache between threads can use `Mutex<CertificateCache>`.

use crate::scalar::ExactScalar;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct CertificateSnapshot {
    pub key: Vec<u8>,
    pub blob: Vec<u8>,
    pub probability: Option<ExactScalar>,
    pub scalar_bytes: usize,
    pub saved_seconds: f64,
}

impl CertificateSnapshot {
    /// Overflow is an unrepresentable charge and prevents admission.
    pub fn serialized_bytes(&self) -> Option<usize> {
        self.key
            .len()
            .checked_add(self.blob.len())?
            .checked_add(self.scalar_bytes)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CertificateCacheStatistics {
    pub bytes: usize,
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub oversize: usize,
    pub saved_seconds: f64,
}

/// The deque runs from least recently used to most recently used. Returned
/// snapshots own their data, so subsequent mutation or eviction is isolated.
pub struct CertificateCache {
    budget: usize,
    max_entries: usize,
    entries: VecDeque<CertificateSnapshot>,
    statistics: CertificateCacheStatistics,
}

impl CertificateCache {
    pub fn new(budget: usize, max_entries: usize) -> Self {
        Self {
            budget,
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

    pub fn limits(&self) -> (usize, usize) {
        (self.budget, self.max_entries)
    }

    pub fn statistics(&self) -> CertificateCacheStatistics {
        self.statistics
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.statistics = Default::default();
    }

    pub fn get(&mut self, key: &[u8]) -> Option<CertificateSnapshot> {
        let Some(index) = self.entries.iter().position(|entry| entry.key == key) else {
            self.statistics.misses += 1;
            return None;
        };
        let entry = self
            .entries
            .remove(index)
            .expect("located certificate entry");
        self.statistics.hits += 1;
        self.statistics.saved_seconds += entry.saved_seconds;
        let result = entry.clone();
        self.entries.push_back(entry);
        Some(result)
    }

    /// Oversized replacements preserve the existing entry and its LRU order.
    /// Disabled caches bypass admission without increasing oversize counters.
    pub fn put(&mut self, value: CertificateSnapshot) -> bool {
        if self.budget == 0 || self.max_entries == 0 {
            return false;
        }
        let Some(added) = value
            .serialized_bytes()
            .filter(|bytes| *bytes <= self.budget)
        else {
            self.statistics.oversize += 1;
            return false;
        };
        if let Some(index) = self.entries.iter().position(|entry| entry.key == value.key) {
            let old = self
                .entries
                .remove(index)
                .expect("located replacement entry");
            self.statistics.bytes -= old.serialized_bytes().expect("admitted entry charge");
        }
        while added > self.budget - self.statistics.bytes || self.entries.len() >= self.max_entries
        {
            let old = self
                .entries
                .pop_front()
                .expect("positive capacity and admitted entry charge");
            self.statistics.bytes -= old.serialized_bytes().expect("admitted entry charge");
            self.statistics.evictions += 1;
        }
        self.statistics.bytes += added;
        self.entries.push_back(value);
        true
    }
}
