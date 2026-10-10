//! Optional bounded exact-bit state interning at a producing coherent node.
use super::*;

const SLOTS: usize = 8192;
const PROBES: usize = 8;
// Amortize index construction only after the sampler has retained enough states.
// Small caches keep ordinary admission without allocating the optional table.
pub(super) const MIN_STATES: usize = 128;
// Indexing small vectors cannot amortize filtering and construction. The old
// admission path handles both small vectors and vectors beyond the work cap.
pub(super) const MIN_COEFFICIENTS: usize = 32;
pub(super) const MAX_COEFFICIENTS: usize = 4096;

#[derive(Clone, Copy)]
struct Entry {
    node: usize,
    fingerprint: u64,
    id: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(values: &[ComplexAmp]) -> CachedState {
        CachedState {
            coefficients: Arc::new(values.to_vec()),
            next_node: None,
            transition: CachedOp::None,
            replay_next: [None; 2],
        }
    }

    #[test]
    fn aliases_require_same_node_and_every_coefficient_bit() {
        let values = [ComplexAmp::new(0.0, -0.0), ComplexAmp::new(0.5, 0.25)];
        let states = vec![state(&values)];
        let mut table = CoefficientIntern::new(usize::MAX).unwrap();
        let (_, Some((slot, hash))) = table.find_or_slot(31, &values, &states) else {
            panic!("empty index must offer a slot")
        };
        table.insert(slot, hash, 31, 0);
        assert_eq!(table.find_or_slot(31, &values, &states).0, Some(0));
        assert_eq!(table.find_or_slot(32, &values, &states).0, None);
        assert_eq!(table.find_or_slot(31, &values[..1], &states).0, None);
        let mut changed = values;
        changed[0].im = 0.0;
        assert_eq!(table.find_or_slot(31, &changed, &states).0, None);
        changed = values;
        changed[1].re = f64::from_bits(0.5f64.to_bits() + 1);
        assert_eq!(table.find_or_slot(31, &changed, &states).0, None);
    }

    #[test]
    fn colliding_fingerprints_are_checked_and_full_probe_window_declines() {
        let query = [ComplexAmp::new(0.25, 0.5), ComplexAmp::new(0.5, 0.0)];
        let other = [ComplexAmp::new(0.5, 0.25), ComplexAmp::new(0.5, 0.0)];
        let states = vec![state(&other), state(&query)];
        let mut table = CoefficientIntern::new(usize::MAX).unwrap();
        let (_, Some((start, hash))) = table.find_or_slot(27, &query, &states) else {
            panic!("empty index must offer a slot")
        };
        for offset in 0..PROBES {
            table.insert((start + offset) & (SLOTS - 1), hash, 27, 0);
        }
        // Even identical fingerprint/node cannot alias different FP64 bits.
        assert_eq!(table.find_or_slot(27, &query, &states), (None, None));
        table.entries[(start + PROBES - 1) & (SLOTS - 1)].id = 1;
        assert_eq!(table.find_or_slot(27, &query, &states), (Some(1), None));
    }

    #[test]
    fn unsampled_coefficient_difference_cannot_alias_a_sparse_fingerprint() {
        let values = vec![ComplexAmp::new(0.25, -0.5); 64];
        let mut different = values.clone();
        different[11].im = f64::from_bits((-0.5f64).to_bits() + 1);
        assert_eq!(
            CoefficientIntern::fingerprint(29, &values),
            CoefficientIntern::fingerprint(29, &different)
        );
        let states = vec![state(&values)];
        let mut table = CoefficientIntern::new(usize::MAX).unwrap();
        let (_, Some((slot, hash))) = table.find_or_slot(29, &values, &states) else {
            panic!("empty index must offer a slot")
        };
        table.insert(slot, hash, 29, 0);
        assert_eq!(table.find_or_slot(29, &values, &states).0, Some(0));
        assert_eq!(table.find_or_slot(29, &different, &states).0, None);
    }

    #[test]
    fn original_cultivation_counts_delay_index_then_reuse_states_with_exact_rng() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        use std::collections::HashSet;
        let text = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let mut cached = plan.prepare_sampler().unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(1739);
            let mut b = a.clone();
            for shots in [1, 64, 1024, 64] {
                let rows = scalar.sample(shots, &mut b).unwrap();
                let accepted: Vec<_> = rows
                    .iter()
                    .filter(|r| r.detectors.iter().all(|&v| !v))
                    .collect();
                let expected = NearCliffordPostselectedCounts {
                    attempted: shots,
                    accepted: accepted.len(),
                    logical_errors: accepted
                        .iter()
                        .filter(|r| {
                            r.observables
                                .iter()
                                .filter(|(i, _)| *i == 0)
                                .fold(false, |v, (_, bit)| v ^ bit)
                        })
                        .count(),
                };
                assert_eq!(
                    cached.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                    expected
                );
                if shots == 1 {
                    assert!(cached.cache.as_ref().unwrap().intern.is_none());
                }
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert_eq!(
                cached.sample_measurements_u8(17, &mut a).unwrap(),
                scalar.sample_measurements_u8(17, &mut b).unwrap()
            );
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
            let cache = cached.cache.as_ref().unwrap();
            assert!(
                cache.intern.is_some(),
                "bulk calls must actually activate the optional index"
            );
            // Two independent cached edges to the same non-scalar child at one
            // producing node witness actual interning, rather than cache growth.
            let mut children = HashSet::new();
            let mut reused = false;
            let edges = cache.nodes.iter().copied().enumerate().chain(
                cache
                    .states
                    .iter()
                    .skip(1)
                    .filter_map(|s| s.next_node.map(|n| (n, s.transition))),
            );
            for (node, op) in edges {
                let next = match op {
                    CachedOp::Rotate(next) => next,
                    CachedOp::Measure(m) => m.next,
                    CachedOp::None => [None; 2],
                };
                for child in next.into_iter().flatten().filter(|&id| id != 0) {
                    reused |= !children.insert((node, child));
                }
            }
            assert!(
                reused,
                "the original circuit must witness an exactly reused coherent state"
            );
        }
    }

    #[test]
    fn index_capacity_is_charged_and_small_budget_declines() {
        assert!(CoefficientIntern::new(0).is_none());
        assert!(CoefficientIntern::new(SLOTS * size_of::<Entry>() - 1).is_none());
        let table = CoefficientIntern::new(usize::MAX).unwrap();
        assert!(table.reserved_bytes() >= SLOTS * size_of::<Entry>());
        assert_eq!(table.entries.len(), SLOTS);
    }

    #[test]
    fn small_vectors_keep_ordinary_admission_without_constructing_an_index() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMY 0\n").unwrap();
        let mut cache = CoefficientCache::new(&plan, DEFAULT_CACHE_BYTE_BUDGET).unwrap();
        let values = vec![ComplexAmp::new(0.5, -0.25); MIN_COEFFICIENTS / 2];
        let initial = cache.reserved;
        let charge = CoefficientCache::state_charge(&values).unwrap();
        for node in 0..2 * MIN_STATES {
            let id = cache.store(node, &values).unwrap();
            assert_eq!(&*cache.states[id].coefficients, &values);
            assert!(cache.intern.is_none());
            assert!(!cache.intern_attempted);
            assert_eq!(cache.reserved, initial + (node + 1) * charge);
        }
        let larger = vec![ComplexAmp::new(0.5, -0.25); MIN_COEFFICIENTS];
        let id = cache.store(311, &larger).unwrap();
        assert!(cache.intern.is_some());
        assert_eq!(cache.store(311, &larger), Some(id));
    }

    #[test]
    fn lazy_interning_reuses_exact_states_until_admission_closes() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMY 0\n").unwrap();
        let mut cache = CoefficientCache::new(&plan, DEFAULT_CACHE_BYTE_BUDGET).unwrap();
        let initial = cache.reserved;
        assert!(cache.intern.is_none());
        let values = vec![ComplexAmp::new(0.5, 0.25); MIN_COEFFICIENTS];
        let first_cold = cache.store(23, &values).unwrap();
        assert!(cache.intern.is_none());
        assert!(!cache.intern_attempted);
        assert_eq!(
            cache.reserved,
            initial + CoefficientCache::state_charge(&values).unwrap()
        );
        // Cold states remain usable and unindexed. Ordinary bounded admission
        // continues until there is enough construction work to amortize a table.
        for node in 100.. {
            if cache.states.len() >= MIN_STATES {
                break;
            }
            cache.store(node, &values).unwrap();
            assert!(cache.intern.is_none());
            assert!(!cache.intern_attempted);
        }
        assert_eq!(&*cache.states[first_cold].coefficients, &values);
        let first = cache.store(23, &values).unwrap();
        assert_ne!(first, first_cold);
        assert!(cache.intern.is_some());
        assert!(cache.reserved > initial);
        let reserved = cache.reserved;
        let states = cache.states.len();
        assert_eq!(cache.store(23, &values), Some(first));
        assert_eq!(cache.reserved, reserved);
        assert_eq!(cache.states.len(), states);
        cache.budget = reserved;
        assert_eq!(cache.store(23, &values), None);
        assert_eq!(cache.store(24, &values), None);
        assert_eq!(cache.reserved, reserved);
        assert_eq!(cache.states.len(), states);
    }
}
pub(super) struct CoefficientIntern {
    entries: Vec<Entry>,
}
impl CoefficientIntern {
    pub(super) fn new(remaining_bytes: usize) -> Option<Self> {
        let requested = SLOTS.checked_mul(size_of::<Entry>())?;
        if requested > remaining_bytes {
            return None;
        }
        let mut entries = Vec::new();
        entries.try_reserve_exact(SLOTS).ok()?;
        if entries.capacity().checked_mul(size_of::<Entry>())? > remaining_bytes {
            return None;
        }
        entries.resize(
            SLOTS,
            Entry {
                node: 0,
                fingerprint: 0,
                id: usize::MAX,
            },
        );
        Some(Self { entries })
    }
    pub(super) fn reserved_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<Entry>()
    }
    fn fingerprint(node: usize, coefficients: &[ComplexAmp]) -> u64 {
        let mut hash = (node as u64) ^ (coefficients.len() as u64).rotate_left(17);
        // Sample coordinate axes and two mixed coordinates instead of scanning
        // every amplitude on a cache miss. This fingerprint is only a filter:
        // every possible alias still compares all FP64 bits below. Collisions
        // can reduce optional reuse, never change sampled results.
        let mut mix = |index: usize| {
            if let Some(value) = coefficients.get(index) {
                hash = (hash ^ value.re.to_bits())
                    .rotate_left(13)
                    .wrapping_mul(0x9e3779b185ebca87);
                hash = (hash ^ value.im.to_bits())
                    .rotate_left(17)
                    .wrapping_mul(0xc2b2ae3d27d4eb4f);
            }
        };
        mix(0);
        for bit in 0..usize::BITS {
            let index = 1usize << bit;
            if index >= coefficients.len() {
                break;
            }
            mix(index);
        }
        mix(coefficients.len().saturating_sub(1));
        mix(coefficients.len() / 3);
        mix(2 * (coefficients.len() / 3));
        hash ^ (hash >> 29)
    }
    // A collision is checked against every FP64 bit. At most eight candidates
    // are inspected; a full/colliding bucket just declines optional interning.
    pub(super) fn find_or_slot(
        &self,
        node: usize,
        coefficients: &[ComplexAmp],
        states: &[CachedState],
    ) -> (Option<usize>, Option<(usize, u64)>) {
        let hash = Self::fingerprint(node, coefficients);
        let start = hash as usize & (SLOTS - 1);
        for offset in 0..PROBES {
            let slot = (start + offset) & (SLOTS - 1);
            let entry = self.entries[slot];
            if entry.id == usize::MAX {
                return (None, Some((slot, hash)));
            }
            if entry.node == node && entry.fingerprint == hash {
                let prior = &states[entry.id].coefficients;
                if prior.len() == coefficients.len()
                    && prior.iter().zip(coefficients).all(|(a, b)| {
                        a.re.to_bits() == b.re.to_bits() && a.im.to_bits() == b.im.to_bits()
                    })
                {
                    return (Some(entry.id), None);
                }
            }
        }
        (None, None)
    }
    pub(super) fn insert(&mut self, slot: usize, fingerprint: u64, node: usize, id: usize) {
        debug_assert_eq!(self.entries[slot].id, usize::MAX);
        self.entries[slot] = Entry {
            node,
            fingerprint,
            id,
        };
    }
}
