//! Lossless storage for axis-aligned cached amplitudes. Arithmetic stays complex.
use super::*;

const MIN_AXIS_COEFFICIENTS: usize = 64;

#[derive(Clone)]
pub(super) enum CachedCoefficients {
    Dense(Arc<Vec<ComplexAmp>>),
    Axis(Arc<AxisCoefficients>),
}

pub(super) struct AxisCoefficients {
    values: Vec<f64>,
    // Two bits per coefficient: active imaginary component, unused zero's sign.
    tags: Vec<u64>,
}

pub(super) struct StoragePlan<'a> {
    coefficients: &'a [ComplexAmp],
    axis: bool,
    charge: usize,
}

impl CachedCoefficients {
    pub(super) fn minimum_charge(len: usize) -> Option<usize> {
        if len >= MIN_AXIS_COEFFICIENTS {
            len.checked_add(len.div_ceil(32))?
                .checked_mul(8)?
                .checked_add(256)
        } else {
            len.checked_mul(size_of::<ComplexAmp>())?.checked_add(256)
        }
    }

    pub(super) fn plan(coefficients: &[ComplexAmp]) -> Option<StoragePlan<'_>> {
        let axis = coefficients.len() >= MIN_AXIS_COEFFICIENTS
            && coefficients.iter().all(|v| v.re == 0. || v.im == 0.);
        let charge = if axis {
            Self::minimum_charge(coefficients.len())?
        } else {
            coefficients
                .len()
                .checked_mul(size_of::<ComplexAmp>())?
                .checked_add(256)?
        };
        Some(StoragePlan {
            coefficients,
            axis,
            charge,
        })
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Dense(values) => values.len(),
            Self::Axis(axis) => axis.values.len(),
        }
    }

    pub(super) fn charge(&self) -> Option<usize> {
        let bytes = match self {
            Self::Dense(values) => values.capacity().checked_mul(size_of::<ComplexAmp>())?,
            Self::Axis(axis) => axis
                .values
                .capacity()
                .checked_add(axis.tags.capacity())?
                .checked_mul(8)?,
        };
        bytes.checked_add(256)
    }

    pub(super) fn matches(&self, coefficients: &[ComplexAmp]) -> bool {
        if self.len() != coefficients.len() {
            return false;
        }
        match self {
            Self::Dense(values) => values
                .iter()
                .zip(coefficients)
                .all(|(a, b)| a.re.to_bits() == b.re.to_bits() && a.im.to_bits() == b.im.to_bits()),
            Self::Axis(axis) => {
                axis.values
                    .chunks(32)
                    .zip(&axis.tags)
                    .zip(coefficients.chunks(32))
                    .all(|((values, &tags), coefficients)| {
                        values.iter().zip(coefficients).enumerate().all(
                            |(i, (value, coefficient))| {
                                let tag = (tags >> (2 * i)) & 3;
                                let (active, unused) = if tag & 1 != 0 {
                                    (coefficient.im, coefficient.re)
                                } else {
                                    (coefficient.re, coefficient.im)
                                };
                                value.to_bits() == active.to_bits()
                                    && unused.to_bits() == (tag & 2) << 62
                            },
                        )
                    })
            }
        }
    }

    // The caller clears and fallibly reserves the complete output first.
    pub(super) fn extend_to(&self, output: &mut Vec<ComplexAmp>) {
        match self {
            Self::Dense(values) => output.extend_from_slice(values),
            Self::Axis(axis) => {
                for (values, &tags) in axis.values.chunks(32).zip(&axis.tags) {
                    for (i, &value) in values.iter().enumerate() {
                        let tag = (tags >> (2 * i)) & 3;
                        let zero = f64::from_bits((tag & 2) << 62);
                        output.push(if tag & 1 != 0 {
                            ComplexAmp::new(zero, value)
                        } else {
                            ComplexAmp::new(value, zero)
                        });
                    }
                }
            }
        }
    }
}

impl StoragePlan<'_> {
    pub(super) fn requested_charge(&self) -> usize {
        self.charge
    }

    pub(super) fn allocate(self, remaining: usize) -> Option<CachedCoefficients> {
        if self.charge > remaining {
            return None;
        }
        let storage = if self.axis {
            let mut values = Vec::new();
            let mut tags = Vec::new();
            values.try_reserve_exact(self.coefficients.len()).ok()?;
            tags.try_reserve_exact(self.coefficients.len().div_ceil(32))
                .ok()?;
            if values
                .capacity()
                .checked_add(tags.capacity())?
                .checked_mul(8)?
                .checked_add(256)?
                > remaining
            {
                return None;
            }
            tags.resize(self.coefficients.len().div_ceil(32), 0);
            for (i, coefficient) in self.coefficients.iter().enumerate() {
                let imaginary = coefficient.re == 0. && coefficient.im != 0.;
                let (active, unused) = if imaginary {
                    (coefficient.im, coefficient.re)
                } else {
                    (coefficient.re, coefficient.im)
                };
                values.push(active);
                let tag = u64::from(imaginary) | (u64::from(unused.is_sign_negative()) << 1);
                tags[i / 32] |= tag << (2 * (i % 32));
            }
            CachedCoefficients::Axis(Arc::new(AxisCoefficients { values, tags }))
        } else {
            let mut values = Vec::new();
            values.try_reserve_exact(self.coefficients.len()).ok()?;
            if values
                .capacity()
                .checked_mul(size_of::<ComplexAmp>())?
                .checked_add(256)?
                > remaining
            {
                return None;
            }
            values.extend_from_slice(self.coefficients);
            CachedCoefficients::Dense(Arc::new(values))
        };
        Some(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_storage_preserves_every_component_bit_and_tail() {
        let bits = [
            0,
            1 << 63,
            1,
            (1 << 63) | 1,
            0x0010_0000_0000_0000,
            0x3fe0_0000_0000_0001,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
            0x7ff8_1234_5678_9abc,
            0xfff8_9876_5432_1234,
        ];
        for len in [64, 65, 95, 96, 97, 256, 1024] {
            let coefficients: Vec<_> = (0..len)
                .map(|i| {
                    let value = f64::from_bits(bits[i % bits.len()]);
                    let zero = f64::from_bits(u64::from(i & 2 != 0) << 63);
                    if i & 1 == 0 {
                        ComplexAmp::new(value, zero)
                    } else {
                        ComplexAmp::new(zero, value)
                    }
                })
                .collect();
            let plan = CachedCoefficients::plan(&coefficients).unwrap();
            let charge = plan.requested_charge();
            assert!(plan.allocate(charge - 1).is_none());
            let storage = CachedCoefficients::plan(&coefficients)
                .unwrap()
                .allocate(charge)
                .unwrap();
            assert!(matches!(storage, CachedCoefficients::Axis(_)));
            assert_eq!(storage.charge(), Some(charge));
            assert!(charge < len * size_of::<ComplexAmp>() + 256);
            assert!(storage.matches(&coefficients));
            let mut restored = Vec::with_capacity(len);
            storage.extend_to(&mut restored);
            assert_eq!(restored.len(), len);
            for (a, b) in restored.iter().zip(&coefficients) {
                assert_eq!(a.re.to_bits(), b.re.to_bits());
                assert_eq!(a.im.to_bits(), b.im.to_bits());
            }
            for index in [0, len / 2, len - 1] {
                let mut changed = coefficients.clone();
                changed[index].im = f64::from_bits(changed[index].im.to_bits() ^ (1 << 63));
                assert!(!storage.matches(&changed));
            }
        }
    }

    #[test]
    fn mixed_components_and_small_vectors_keep_dense_storage() {
        for len in [1, 2, 32, 63, 64, 65, 1024] {
            let mut coefficients = vec![ComplexAmp::new(0.5, -0.0); len];
            if len >= 64 {
                coefficients[len - 1] = ComplexAmp::new(0.5, f64::from_bits(1));
            }
            let plan = CachedCoefficients::plan(&coefficients).unwrap();
            let charge = plan.requested_charge();
            let storage = plan.allocate(charge).unwrap();
            assert!(matches!(storage, CachedCoefficients::Dense(_)));
            assert!(storage.matches(&coefficients));
            assert_eq!(storage.charge(), Some(len * size_of::<ComplexAmp>() + 256));
            assert!(!storage.matches(&coefficients[..len - 1]));
        }
        assert!(CachedCoefficients::minimum_charge(usize::MAX).is_none());
    }

    #[test]
    fn original_default_cultivation_uses_axis_storage_with_exact_rows_and_rng() {
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
            let mut cached = plan.prepare_sampler().unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(1739);
            let mut b = a.clone();
            for shots in [1, 32, 63, 64, 65, 1024, 64] {
                let expected = scalar.sample(shots, &mut b).unwrap();
                assert_eq!(cached.sample(shots, &mut a).unwrap(), expected);
                let expected = scalar.sample(shots, &mut b).unwrap();
                let accepted: Vec<_> = expected
                    .iter()
                    .filter(|r| r.detectors.iter().all(|&v| !v))
                    .collect();
                let counts = NearCliffordPostselectedCounts {
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
                    counts
                );
                assert_eq!(
                    cached.sample_measurements_u8(17, &mut a).unwrap(),
                    scalar.sample_measurements_u8(17, &mut b).unwrap()
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            let cache = cached.cache.as_ref().unwrap();
            assert!(
                cache
                    .states
                    .iter()
                    .any(|s| matches!(s.coefficients, CachedCoefficients::Axis(_))),
                "the measured default route must actually retain compressed states"
            );
            assert!(cache.reserved <= DEFAULT_CACHE_BYTE_BUDGET);
            assert!(size_of::<CachedState>() <= 256);
        }
    }
}
