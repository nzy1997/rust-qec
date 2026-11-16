//! Shared retention policy and transactional bounded prepared-plan storage.
use super::InstrumentError;
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

/// Retention behavior for each automatic probability, density and moment family.
/// Existing preparation modes and per-family limits remain separate controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryCachePolicy {
    /// Refuse a new layout once the retained family limit is reached.
    #[default]
    Quota,
    /// Retain the most recently requested layouts, evicting only after a new
    /// plan has been constructed successfully. Enabled families require a
    /// positive capacity. Temporary construction can coexist with the old plan.
    Lru,
}
impl QueryCachePolicy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Quota => "quota",
            Self::Lru => "lru",
        }
    }
}
struct Entry<T> {
    value: T,
    used: u128,
}
#[derive(Default)]
struct Clock(u128);
impl Clock {
    fn next<'a>(&mut self, times: impl Iterator<Item = &'a mut u128>) -> u128 {
        if self.0 == u128::MAX {
            // Preserve exact recency even after the counter is exhausted. The
            // number of allocated entries is bounded by usize, below u128.
            let mut times = times.collect::<Vec<_>>();
            times.sort_unstable_by_key(|value| **value);
            self.0 = times.len() as u128;
            for (rank, value) in times.into_iter().enumerate() {
                *value = rank as u128;
            }
        }
        self.0 += 1;
        self.0
    }
}
pub(super) fn admit_new(
    len: usize,
    limit: usize,
    policy: QueryCachePolicy,
    option: &'static str,
) -> Result<(), InstrumentError> {
    if limit == 0 || (policy == QueryCachePolicy::Quota && len >= limit) {
        Err(InstrumentError::ResourceLimit {
            option,
            limit,
            required: len.saturating_add(1),
        })
    } else {
        Ok(())
    }
}
/// Hash lookup is preserved for ordinary probability rows. No key clone occurs
/// on a hit, including when the opt-in policy updates its recency stamp.
pub(super) struct HashPlans<K, T> {
    entries: HashMap<K, Entry<T>>,
    clock: Clock,
    pub evictions: usize,
}
impl<K: Eq + Hash, T> Default for HashPlans<K, T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            clock: Clock::default(),
            evictions: 0,
        }
    }
}
impl<K: Eq + Hash, T> HashPlans<K, T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.values().map(|entry| &entry.value)
    }
    pub fn get<Q: Eq + Hash + ?Sized>(&mut self, key: &Q, policy: QueryCachePolicy) -> Option<&T>
    where
        K: Borrow<Q>,
    {
        let stamp = if policy == QueryCachePolicy::Lru {
            self.clock
                .next(self.entries.values_mut().map(|entry| &mut entry.used))
        } else {
            0
        };
        self.entries.get_mut(key).map(|entry| {
            if policy == QueryCachePolicy::Lru {
                entry.used = stamp;
            }
            &entry.value
        })
    }
    pub fn insert(&mut self, key: K, value: T, limit: usize, policy: QueryCachePolicy)
    where
        K: Clone,
    {
        debug_assert!(limit > 0);
        if self.entries.len() >= limit && policy == QueryCachePolicy::Lru {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone())
                .expect("positive admitted cache capacity");
            self.entries.remove(&oldest);
            self.evictions = self.evictions.saturating_add(1);
        }
        let used = if policy == QueryCachePolicy::Lru {
            self.clock
                .next(self.entries.values_mut().map(|entry| &mut entry.used))
        } else {
            0
        };
        self.entries.insert(key, Entry { value, used });
    }
}
/// Preserve the existing borrowed layout matching of density/moment families;
/// a warm request does not allocate or clone a prospective key.
pub(super) struct VecPlans<T> {
    entries: Vec<Entry<T>>,
    clock: Clock,
    pub evictions: usize,
}
impl<T> Default for VecPlans<T> {
    fn default() -> Self {
        Self {
            entries: vec![],
            clock: Clock::default(),
            evictions: 0,
        }
    }
}
impl<T> VecPlans<T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|entry| &entry.value)
    }
    pub fn get_by(&mut self, matches: impl Fn(&T) -> bool, policy: QueryCachePolicy) -> Option<&T> {
        let index = self
            .entries
            .iter()
            .position(|entry| matches(&entry.value))?;
        if policy == QueryCachePolicy::Lru {
            let stamp = self
                .clock
                .next(self.entries.iter_mut().map(|entry| &mut entry.used));
            self.entries[index].used = stamp;
        }
        Some(&self.entries[index].value)
    }
    pub fn push(&mut self, value: T, limit: usize, policy: QueryCachePolicy) {
        debug_assert!(limit > 0);
        if self.entries.len() >= limit && policy == QueryCachePolicy::Lru {
            let index = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(index, _)| index)
                .expect("positive admitted cache capacity");
            self.entries.remove(index);
            self.evictions = self.evictions.saturating_add(1);
        }
        let used = if policy == QueryCachePolicy::Lru {
            self.clock
                .next(self.entries.iter_mut().map(|entry| &mut entry.used))
        } else {
            0
        };
        self.entries.push(Entry { value, used });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recency_counter_exhaustion_preserves_order_for_both_storage_routes() {
        let mut hash = HashPlans::default();
        hash.insert("a", 1, 2, QueryCachePolicy::Lru);
        hash.insert("b", 2, 2, QueryCachePolicy::Lru);
        assert_eq!(hash.get("a", QueryCachePolicy::Lru), Some(&1));
        hash.clock.0 = u128::MAX;
        assert_eq!(hash.get("b", QueryCachePolicy::Lru), Some(&2));
        hash.insert("c", 3, 2, QueryCachePolicy::Lru);
        assert!(hash.get("a", QueryCachePolicy::Lru).is_none());
        assert_eq!(hash.get("b", QueryCachePolicy::Lru), Some(&2));
        assert_eq!(hash.evictions, 1);
        let mut list = VecPlans::default();
        list.push(1, 2, QueryCachePolicy::Lru);
        list.push(2, 2, QueryCachePolicy::Lru);
        assert_eq!(list.get_by(|&v| v == 1, QueryCachePolicy::Lru), Some(&1));
        list.clock.0 = u128::MAX;
        assert_eq!(list.get_by(|&v| v == 2, QueryCachePolicy::Lru), Some(&2));
        list.push(3, 2, QueryCachePolicy::Lru);
        assert!(list.get_by(|&v| v == 1, QueryCachePolicy::Lru).is_none());
        assert_eq!(list.get_by(|&v| v == 2, QueryCachePolicy::Lru), Some(&2));
        assert_eq!(list.evictions, 1);
    }
}
