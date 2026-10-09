//! Lossless optional storage for coefficient vectors whose entries lie on an axis.
use super::*;

pub(super) enum CachedCoefficients {
    Complex(Vec<ComplexAmp>),
    Axis(AxisCoefficients),
    // Preserve sharing of the compiled deterministic prefix.
    Shared(Arc<Vec<ComplexAmp>>),
}

pub(super) struct AxisCoefficients {
    values: Vec<f64>,
    // Two bits per entry: stored component is imaginary; omitted zero is negative.
    tags: Vec<u64>,
}

impl CachedCoefficients {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Complex(values) => values.len(),
            Self::Shared(values) => values.len(),
            Self::Axis(values) => values.values.len(),
        }
    }

    fn at(&self, index: usize) -> ComplexAmp {
        match self {
            Self::Complex(values) => values[index],
            Self::Shared(values) => values[index],
            Self::Axis(axis) => {
                let tag = (axis.tags[index / 32] >> (2 * (index % 32))) & 3;
                let zero = f64::from_bits((tag >> 1) << 63);
                if tag & 1 == 0 {
                    ComplexAmp::new(axis.values[index], zero)
                } else {
                    ComplexAmp::new(zero, axis.values[index])
                }
            }
        }
    }

    pub(super) fn same_bits(&self, query: &[ComplexAmp]) -> bool {
        self.len() == query.len()
            && query.iter().enumerate().all(|(i, b)| {
                let a = self.at(i);
                a.re.to_bits() == b.re.to_bits() && a.im.to_bits() == b.im.to_bits()
            })
    }

    pub(super) fn restore(&self, output: &mut Vec<ComplexAmp>) -> Result<(), String> {
        output.clear();
        output
            .try_reserve_exact(self.len())
            .map_err(|e| format!("compiled coefficient allocation failed: {e}"))?;
        match self {
            Self::Complex(values) => output.extend_from_slice(values),
            Self::Shared(values) => output.extend_from_slice(values),
            Self::Axis(_) => output.extend((0..self.len()).map(|i| self.at(i))),
        }
        Ok(())
    }

    /// An unsuccessful optional compression leaves ordinary cache storage available.
    pub(super) fn axis(values: &[ComplexAmp]) -> Option<(Self, usize)> {
        if values.len() < 64 || values.iter().any(|a| a.re != 0. && a.im != 0.) {
            return None;
        }
        let mut packed = Vec::new();
        packed.try_reserve_exact(values.len()).ok()?;
        let mut tags = Vec::new();
        tags.try_reserve_exact(values.len().div_ceil(32)).ok()?;
        tags.resize(values.len().div_ceil(32), 0);
        for (i, a) in values.iter().enumerate() {
            let (value, zero, imaginary) = if a.im == 0. {
                (a.re, a.im, 0)
            } else {
                (a.im, a.re, 1)
            };
            packed.push(value);
            let tag = imaginary | ((zero.to_bits() >> 63) << 1);
            tags[i / 32] |= tag << (2 * (i % 32));
        }
        let charge = packed
            .capacity()
            .checked_add(tags.capacity())?
            .checked_mul(size_of::<u64>())?
            .checked_add(256)?;
        Some((
            Self::Axis(AxisCoefficients {
                values: packed,
                tags,
            }),
            charge,
        ))
    }

    #[cfg(test)]
    pub(super) fn materialize(&self) -> Vec<ComplexAmp> {
        (0..self.len()).map(|i| self.at(i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_storage_preserves_every_bit_including_signed_zero_and_nan() {
        let special = [
            ComplexAmp::new(0., -0.),
            ComplexAmp::new(-0., 0.),
            ComplexAmp::new(-0., -0.),
            ComplexAmp::new(0.25, -0.),
            ComplexAmp::new(-0., -0.5),
            ComplexAmp::new(f64::from_bits(1), 0.),
            ComplexAmp::new(0., f64::from_bits(0x7ff8000000000123)),
            ComplexAmp::new(f64::NEG_INFINITY, -0.),
        ];
        let values: Vec<_> = special.into_iter().cycle().take(129).collect();
        let (packed, charge) = CachedCoefficients::axis(&values).unwrap();
        assert!(charge < CoefficientCache::state_charge(&values).unwrap());
        assert!(packed.same_bits(&values));
        let mut restored = vec![ComplexAmp::new(7., 9.); 256];
        packed.restore(&mut restored).unwrap();
        assert_eq!(restored.len(), values.len());
        for (actual, expected) in restored.iter().zip(&values) {
            assert_eq!(actual.re.to_bits(), expected.re.to_bits());
            assert_eq!(actual.im.to_bits(), expected.im.to_bits());
        }
        let mut changed = values.clone();
        changed[32].im = 0.;
        assert!(!packed.same_bits(&changed));
        assert!(!packed.same_bits(&values[..128]));
    }

    #[test]
    fn mixed_axis_or_small_vectors_keep_original_storage() {
        assert!(CachedCoefficients::axis(&[ComplexAmp::new(1., 0.); 63]).is_none());
        let mut values = vec![ComplexAmp::new(0., 0.5); 128];
        values[117] = ComplexAmp::new(0.5, 0.25);
        assert!(CachedCoefficients::axis(&values).is_none());
        values[117] = ComplexAmp::new(f64::NAN, f64::NAN);
        assert!(CachedCoefficients::axis(&values).is_none());
    }

    #[test]
    fn warmed_cache_compresses_exact_states_without_weakening_alias_checks_or_budget() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMY 0\n").unwrap();
        let mut cache = CoefficientCache::new(&plan, DEFAULT_CACHE_BYTE_BUDGET).unwrap();
        assert!(!cache.axis_enabled);
        cache.axis_enabled = true;
        let small = [ComplexAmp::new(0.5, 0.25); 2];
        for node in 100..100 + coefficient_intern::MIN_STATES {
            cache.store(node, &small).unwrap();
        }
        let values: Vec<_> = (0..128)
            .map(|i| {
                if i % 2 == 0 {
                    ComplexAmp::new(0.5, -0.)
                } else {
                    ComplexAmp::new(-0., 0.25)
                }
            })
            .collect();
        let id = cache.store(23, &values).unwrap();
        assert!(matches!(
            cache.states[id].coefficients.as_ref(),
            CachedCoefficients::Axis(_)
        ));
        let reserved = cache.reserved;
        assert_eq!(cache.store(23, &values), Some(id));
        assert_eq!(cache.reserved, reserved);
        let mut changed = values.clone();
        changed[11].re = 0.;
        let distinct = cache.store(23, &changed).unwrap();
        assert_ne!(distinct, id);
        assert!(cache.states[distinct].coefficients.same_bits(&changed));
        assert!(cache.states[id].coefficients.same_bits(&values));
        cache.budget = cache.reserved;
        let states = cache.states.len();
        assert_eq!(cache.store(24, &values), None);
        assert_eq!(cache.states.len(), states);
        assert!(cache.reserved <= cache.budget);
    }

    fn count_records(rows: &[NearCliffordShot]) -> NearCliffordPostselectedCounts {
        let accepted: Vec<_> = rows
            .iter()
            .filter(|r| r.detectors.iter().all(|&bit| !bit))
            .collect();
        NearCliffordPostselectedCounts {
            attempted: rows.len(),
            accepted: accepted.len(),
            logical_errors: accepted
                .iter()
                .filter(|r| {
                    r.observables
                        .iter()
                        .filter(|(index, _)| *index == 0)
                        .fold(false, |parity, (_, bit)| parity ^ bit)
                })
                .count(),
        }
    }

    #[test]
    fn cultivation_cache_actually_uses_axis_storage_and_keeps_native_counts_rng() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        let text = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        );
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let mut native = plan.prepare_sampler().unwrap();
            let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut records = plan.prepare_sampler().unwrap();
            let mut records_rng = StdRng::seed_from_u64(2026100937);
            let mut a = StdRng::seed_from_u64(2026100937);
            let mut b = a.clone();
            for shots in [1, 32, 63, 64, 65, 1024, 8192, 1] {
                let rows = reference.sample(shots, &mut b).unwrap();
                assert_eq!(records.sample(shots, &mut records_rng).unwrap(), rows);
                let expected = count_records(&rows);
                assert_eq!(
                    native.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                    expected
                );
                for _ in 0..16 {
                    let expected_word = b.next_u64();
                    assert_eq!(a.next_u64(), expected_word);
                    assert_eq!(records_rng.next_u64(), expected_word);
                }
            }
            let records_cache = records.cache.as_ref().unwrap();
            assert!(!records_cache.axis_enabled);
            assert!(
                records_cache.states.iter().all(|state| !matches!(
                    state.coefficients.as_ref(),
                    CachedCoefficients::Axis(_)
                ))
            );
            let cache = native.cache.as_ref().unwrap();
            assert!(cache.axis_enabled);
            assert!(
                cache.states.iter().any(|state| matches!(
                    state.coefficients.as_ref(),
                    CachedCoefficients::Axis(_)
                ))
            );
            assert!(cache.reserved <= DEFAULT_CACHE_BYTE_BUDGET);
            // Structured rows bypass sample_batch, so exercise that entry after counts.
            assert_eq!(
                native.sample_with_sweep(65, &[], &mut a).unwrap(),
                reference.sample_with_sweep(65, &[], &mut b).unwrap()
            );
            assert!(!native.cache.as_ref().unwrap().axis_enabled);
            let rows = reference.sample(65, &mut b).unwrap();
            assert_eq!(
                native.sample_postselected_counts(65, 0, &mut a).unwrap(),
                count_records(&rows)
            );
            assert!(native.cache.as_ref().unwrap().axis_enabled);
            assert!(native.sample(0, &mut a).unwrap().is_empty());
            assert!(!native.cache.as_ref().unwrap().axis_enabled);
            assert_eq!(
                native.sample_measurements_u8(129, &mut a).unwrap(),
                reference.sample_measurements_u8(129, &mut b).unwrap()
            );
            assert!(!native.cache.as_ref().unwrap().axis_enabled);
            let rows = reference.sample(65, &mut b).unwrap();
            assert_eq!(
                native.sample_postselected_counts(65, 0, &mut a).unwrap(),
                count_records(&rows)
            );
            assert!(native.cache.as_ref().unwrap().axis_enabled);
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}
