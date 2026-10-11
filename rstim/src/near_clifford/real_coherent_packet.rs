//! Exact binary phase gauge for eligible coherent packets.
//!
//! Coefficients are G[lane] * i^parity(amplitude & gauge) * values[amplitude,lane].
//! G is an unobservable lane-global phase. Noise/frame signs are always real.
use super::*;

fn parity(value: usize) -> bool {
    value.count_ones() & 1 != 0
}

fn remove_bit(value: usize, index: usize) -> usize {
    let low = (1usize << index) - 1;
    (value & low) | ((value >> 1) & !low)
}

/// Conservatively certify the entire plan, including the already executed prefix.
/// No allocation, tolerance, amplitude cancellation or noise outcome is involved.
pub(super) fn prefix_gauge(
    operations: &[PlanOp],
    prefix_len: usize,
    prefix: &[ComplexAmp],
) -> Option<usize> {
    if prefix_len > operations.len() {
        return None;
    }
    let mut rank = 0u32;
    let mut gauge = 0;
    let mut initial = (prefix_len == 0).then_some((0, 0));
    for (node, op) in operations.iter().enumerate() {
        match op {
            PlanOp::Rotate { pauli, expand, .. } => {
                let imaginary = pauli.physical.phase & 1 == 0;
                if pauli.physical.phase >= 4 {
                    return None;
                }
                if *expand {
                    let axis = 1usize.checked_shl(rank)?;
                    if pauli.x != axis {
                        return None;
                    }
                    if imaginary {
                        gauge |= axis;
                    }
                    rank += 1;
                }
                let len = 1usize.checked_shl(rank)?;
                if pauli.x >= len || pauli.z >= len {
                    return None;
                }
                if (pauli.x != 0 || pauli.z != 0) && parity(gauge & pauli.x) != imaginary {
                    return None;
                }
            }
            PlanOp::Measure(Measurement {
                pauli,
                projection: Projection::Active { index, y, .. },
                ..
            }) => {
                let len = 1usize.checked_shl(rank)?;
                let coordinates = if pauli.x == 0 { pauli.z } else { pauli.x };
                if pauli.x >= len
                    || pauli.z >= len
                    || *index >= rank as usize
                    || coordinates == 0
                    || coordinates.trailing_zeros() as usize != *index
                    || parity(pauli.x & pauli.z) != *y
                    || (pauli.x != 0 && parity(gauge & pauli.x) != *y)
                {
                    return None;
                }
                let pivot = 1usize << index;
                if pauli.x == 0 && gauge & pivot != 0 {
                    gauge ^= pauli.z & !pivot;
                }
                gauge = remove_bit(gauge, *index);
                rank -= 1;
            }
            _ => {} // These operators only alter the Pauli frame or classical records.
        }
        if node + 1 == prefix_len {
            initial = Some((rank, gauge));
        }
    }
    let (rank, gauge) = initial?;
    if prefix.len() != 1usize.checked_shl(rank)? {
        return None;
    }
    for (index, value) in prefix.iter().enumerate() {
        let unused = if parity(index & gauge) {
            value.re
        } else {
            value.im
        };
        if unused != 0. {
            return None;
        }
    }
    Some(gauge)
}

#[derive(Default)]
pub(super) struct RealCoherentPacket {
    values: Vec<f64>,
    reduced: Vec<f64>,
    len: usize,
    lanes: usize,
    gauge: usize,
}

impl RealCoherentPacket {
    const BYTE_BUDGET: usize = 64 * 1024 * 1024;

    pub(super) fn reset(
        &mut self,
        prefix: &[ComplexAmp],
        lanes: usize,
        peak_rank: usize,
        gauge: usize,
    ) -> Result<(), String> {
        if !(1..=64).contains(&lanes) {
            return Err("real coherent packet requires 1 through 64 lanes".into());
        }
        let peak_len = 1usize
            .checked_shl(peak_rank.try_into().unwrap_or(u32::MAX))
            .ok_or("real coherent packet rank exceeds platform capacity")?;
        if prefix.is_empty()
            || !prefix.len().is_power_of_two()
            || prefix.len() > peak_len
            || gauge >= prefix.len()
        {
            return Err("real coherent packet prefix exceeds its structural coordinates".into());
        }
        let capacity = peak_len
            .checked_mul(lanes)
            .ok_or("real coherent packet size overflow")?;
        if capacity
            .checked_mul(2 * size_of::<f64>())
            .is_none_or(|n| n > Self::BYTE_BUDGET)
        {
            return Err("real coherent packet reservation exceeds 64 MiB".into());
        }
        for plane in [&mut self.values, &mut self.reduced] {
            if plane.capacity() < capacity {
                plane
                    .try_reserve_exact(capacity - plane.len())
                    .map_err(|e| format!("real coherent packet allocation failed: {e}"))?;
            }
        }
        if self
            .values
            .capacity()
            .checked_add(self.reduced.capacity())
            .and_then(|n| n.checked_mul(size_of::<f64>()))
            .is_none_or(|n| n > Self::BYTE_BUDGET)
        {
            return Err("real coherent packet retained capacity exceeds 64 MiB".into());
        }
        self.values.resize(prefix.len() * lanes, 0.);
        self.reduced.clear();
        for (amplitude, value) in prefix.iter().enumerate() {
            let imaginary = parity(amplitude & gauge);
            let (value, unused) = if imaginary {
                (value.im, value.re)
            } else {
                (value.re, value.im)
            };
            if unused != 0. {
                return Err("real coherent packet prefix has incompatible phase support".into());
            }
            self.values[amplitude * lanes..(amplitude + 1) * lanes].fill(value);
        }
        self.len = prefix.len();
        self.lanes = lanes;
        self.gauge = gauge;
        Ok(())
    }

    fn check_pauli(&self, p: &CompactPauli, len: usize) -> Result<(), String> {
        if self.lanes == 0
            || len == 0
            || !len.is_power_of_two()
            || p.x >= len
            || p.z >= len
            || p.physical.phase >= 4
        {
            return Err("real coherent packet Pauli exceeds active coordinates".into());
        }
        Ok(())
    }

    pub(super) fn rotate(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        anti: u64,
        policy: CompiledRotationArithmetic,
    ) -> Result<(), String> {
        if p.x == 0 && p.z == 0 {
            return Ok(());
        }
        let len = if expand {
            self.len
                .checked_mul(2)
                .ok_or("real coherent packet expansion overflow")?
        } else {
            self.len
        };
        self.check_pauli(p, len)?;
        let imaginary = p.physical.phase & 1 == 0;
        if expand {
            if p.x != self.len {
                return Err("real coherent packet expansion has unisolated new axis".into());
            }
            if imaginary {
                self.gauge |= self.len;
            }
        }
        if parity(self.gauge & p.x) != imaginary {
            return Err("real coherent packet rotation has incompatible phase gauge".into());
        }
        let count = len
            .checked_mul(self.lanes)
            .ok_or("real coherent packet size overflow")?;
        if count > self.values.capacity() || count > self.reduced.capacity() {
            return Err("real coherent packet exceeds its reserved peak capacity".into());
        }
        self.values.resize(count, 0.);
        self.len = len;
        match (policy, imaginary) {
            (CompiledRotationArithmetic::Strict, false) => {
                self.rotate_pairs::<false, false>(p, dagger, anti)
            }
            (CompiledRotationArithmetic::Strict, true) => {
                self.rotate_pairs::<false, true>(p, dagger, anti)
            }
            (CompiledRotationArithmetic::Fused, false) => {
                self.rotate_pairs::<true, false>(p, dagger, anti)
            }
            (CompiledRotationArithmetic::Fused, true) => {
                self.rotate_pairs::<true, true>(p, dagger, anti)
            }
        }
        Ok(())
    }

    fn rotate_pairs<const FUSED: bool, const IMAGINARY: bool>(
        &mut self,
        p: &CompactPauli,
        dagger: bool,
        anti: u64,
    ) {
        let c = (std::f64::consts::PI / 8.).cos();
        let s = (std::f64::consts::PI / 8.).sin();
        let base = if matches!(p.physical.phase, 0 | 3) {
            -s
        } else {
            s
        };
        let factors: [f64; 64] = std::array::from_fn(|lane| {
            if dagger ^ (anti >> lane & 1 != 0) {
                -base
            } else {
                base
            }
        });
        if p.x == 0 {
            for amplitude in 0..self.len {
                let negative = parity(amplitude & p.z);
                for (value, &factor) in self.values
                    [amplitude * self.lanes..(amplitude + 1) * self.lanes]
                    .iter_mut()
                    .zip(&factors[..self.lanes])
                {
                    let factor = if negative { -factor } else { factor };
                    let old = *value;
                    *value = if FUSED {
                        old.mul_add(factor, old * c)
                    } else {
                        old * c + old * factor
                    };
                }
            }
            return;
        }
        let pivot = 1usize << p.x.trailing_zeros();
        for block in (0..self.len).step_by(pivot * 2) {
            let other = block ^ p.x;
            let a = block * self.lanes;
            let b = other * self.lanes;
            let span = pivot * self.lanes;
            let (a, b) = if a < b {
                let (lower, upper) = self.values.split_at_mut(b);
                (&mut lower[a..a + span], &mut upper[..span])
            } else {
                let (lower, upper) = self.values.split_at_mut(a);
                (&mut upper[..span], &mut lower[b..b + span])
            };
            for (offset, (a, b)) in a
                .chunks_exact_mut(self.lanes)
                .zip(b.chunks_exact_mut(self.lanes))
                .enumerate()
            {
                let i = block + offset;
                let j = i ^ p.x;
                let negative_i = parity(i & p.z);
                let negative_j = parity(j & p.z);
                let source_i_negative = IMAGINARY && parity(i & self.gauge);
                let source_j_negative = IMAGINARY && parity(j & self.gauge);
                for ((a, b), &factor) in a.iter_mut().zip(b).zip(&factors[..self.lanes]) {
                    let old_a = *a;
                    let old_b = *b;
                    let source_i = if source_i_negative { -old_a } else { old_a };
                    let source_j = if source_j_negative { -old_b } else { old_b };
                    let factor_i = if negative_i { -factor } else { factor };
                    let factor_j = if negative_j { -factor } else { factor };
                    *a = if FUSED {
                        source_j.mul_add(factor_j, old_a * c)
                    } else {
                        old_a * c + source_j * factor_j
                    };
                    *b = if FUSED {
                        source_i.mul_add(factor_i, old_b * c)
                    } else {
                        old_b * c + source_i * factor_i
                    };
                }
            }
        }
    }

    fn amplitude(&self, index: usize, lane: usize) -> ComplexAmp {
        let value = self.values[index * self.lanes + lane];
        if parity(index & self.gauge) {
            ComplexAmp::new(0., value)
        } else {
            ComplexAmp::new(value, 0.)
        }
    }

    pub(super) fn probability_zero(&self, p: &CompactPauli) -> [f64; 64] {
        assert!(
            self.lanes != 0,
            "real coherent packet has not been initialized"
        );
        debug_assert!(self.check_pauli(p, self.len).is_ok());
        let phase = i_pow(p.physical.phase);
        let mut norm = [0.; 64];
        let mut expectation = [0.; 64];
        for amplitude in 0..self.len {
            let sign = if parity(amplitude & p.z) { -1. } else { 1. };
            for lane in 0..self.lanes {
                let amp = self.amplitude(amplitude, lane);
                let image = self.amplitude(amplitude ^ p.x, lane);
                // Keep the incumbent left association and accumulation order.
                expectation[lane] += (image.conj() * phase * (amp * sign)).re;
                norm[lane] += amp.norm_sqr();
            }
        }
        let mut probabilities = [0.; 64];
        for lane in 0..self.lanes {
            probabilities[lane] = ((1. + expectation[lane] / norm[lane]) * 0.5).clamp(0., 1.);
        }
        probabilities
    }

    pub(super) fn project(
        &mut self,
        p: &CompactPauli,
        index: usize,
        y: bool,
        fixed_mask: u64,
    ) -> Result<(), String> {
        self.check_pauli(p, self.len)?;
        if self.len < 2 || index >= self.len.trailing_zeros() as usize {
            return Err("real coherent packet projection pivot outside active coordinates".into());
        }
        let pivot = 1usize << index;
        if (if p.x == 0 { p.z } else { p.x }) & pivot == 0 {
            return Err("real coherent packet projection pivot absent from Pauli".into());
        }
        if p.x != 0 && parity(self.gauge & p.x) != y {
            return Err("real coherent packet projection has incompatible phase gauge".into());
        }
        let low = pivot - 1;
        let other = p.z & !pivot;
        let old_pivot = self.gauge & pivot != 0;
        let new_gauge = remove_bit(
            self.gauge ^ if p.x == 0 && old_pivot { other } else { 0 },
            index,
        );
        let len = self.len / 2;
        self.reduced.resize(len * self.lanes, 0.);
        let mut norm = [0.; 64];
        for amplitude in 0..len {
            let without = (amplitude & low) | ((amplitude & !low) << 1);
            let negative = parity(without & other);
            let imaginary = parity(without & self.gauge);
            for lane in 0..self.lanes {
                let fixed = fixed_mask >> lane & 1 != 0;
                let value = if p.x == 0 {
                    let input = without | (usize::from(fixed ^ negative) << index);
                    let value = self.values[input * self.lanes + lane];
                    // Removing lane-global i also changes relative real signs.
                    if old_pivot && fixed && parity(amplitude & new_gauge) {
                        -value
                    } else {
                        value
                    }
                } else {
                    let a = self.values[without * self.lanes + lane];
                    let b = self.values[(without ^ p.x) * self.lanes + lane];
                    let b = if y && imaginary { -b } else { b };
                    let sign = if fixed ^ negative { -1. } else { 1. };
                    (a + b * sign) * std::f64::consts::FRAC_1_SQRT_2
                };
                norm[lane] += value * value;
                self.reduced[amplitude * self.lanes + lane] = value;
            }
        }
        let mut scales = [0.; 64];
        for lane in 0..self.lanes {
            if !norm[lane].is_finite() || norm[lane] <= 0. {
                return Err(format!(
                    "real coherent packet zero-probability measurement at lane {lane}"
                ));
            }
            scales[lane] = 1. / norm[lane].sqrt();
        }
        for values in self.reduced.chunks_exact_mut(self.lanes) {
            for (value, &scale) in values.iter_mut().zip(&scales[..self.lanes]) {
                *value *= scale;
            }
        }
        std::mem::swap(&mut self.values, &mut self.reduced);
        self.len = len;
        self.gauge = new_gauge;
        if len == 1 {
            self.values.fill(1.);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn pauli(x: usize, z: usize, phase: u8) -> CompactPauli {
        CompactPauli {
            physical: PackedPauli {
                x: vec![x as u64],
                z: vec![z as u64],
                phase,
            },
            x,
            z,
        }
    }

    fn encoded(len: usize, gauge: usize) -> Vec<ComplexAmp> {
        (0..len)
            .map(|index| {
                let value = match index % 8 {
                    0 => -0.,
                    1 => 1e-160,
                    2 => -0.37,
                    3 => 0.91,
                    4 => f64::from_bits(1),
                    5 => -f64::MIN_POSITIVE,
                    6 => 0.25,
                    _ => -0.13,
                };
                if parity(index & gauge) {
                    ComplexAmp::new(-0., value)
                } else {
                    ComplexAmp::new(value, -0.)
                }
            })
            .collect()
    }

    fn compare(real: &RealCoherentPacket, complex: &CoherentPacket) {
        assert_eq!((real.len, real.lanes), (complex.len, complex.lanes));
        for lane in 0..real.lanes {
            let error = (0..4)
                .map(|phase| {
                    (0..real.len)
                        .map(|index| {
                            let actual = real.amplitude(index, lane) * i_pow(phase);
                            let offset = index * complex.lanes + lane;
                            let expected = ComplexAmp::new(complex.re[offset], complex.im[offset]);
                            (actual.re - expected.re)
                                .abs()
                                .max((actual.im - expected.im).abs())
                        })
                        .fold(0., f64::max)
                })
                .fold(f64::INFINITY, f64::min);
            assert!(
                error < 3e-13,
                "lane={lane}, gauge={}, error={error}",
                real.gauge
            );
        }
    }

    fn compare_cdf(real: &RealCoherentPacket, complex: &CoherentPacket) {
        for z in [0, real.len - 1, (real.len - 1) >> 1] {
            for x in [0, real.len - 1, real.len / 2] {
                if x != 0 && parity(real.gauge & x) != parity(x & z) {
                    continue;
                }
                for negative in [0, 2] {
                    let p = pauli(x, z, u8::from(parity(x & z)) + negative);
                    let a = real.probability_zero(&p);
                    let b = complex.probability_zero(&p);
                    for lane in 0..real.lanes {
                        assert_eq!(
                            a[lane].to_bits(),
                            b[lane].to_bits(),
                            "g={} x={x} z={z} phase={} lane={lane}",
                            real.gauge,
                            p.physical.phase
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn real_projection_matches_complex_for_all_small_gauges_and_mixed_branches() {
        for rank in 2..=4 {
            let len = 1 << rank;
            for gauge in 0..len {
                let prefix = encoded(len, gauge);
                for x in 0..len {
                    for z in 0..len {
                        if x == 0 && z == 0 {
                            continue;
                        }
                        let y = parity(x & z);
                        if x != 0 && parity(gauge & x) != y {
                            continue;
                        }
                        let p = pauli(x, z, u8::from(y));
                        let index = (if x == 0 { z } else { x }).trailing_zeros() as usize;
                        for fixed in [0, u64::MAX, 0xa596_6987_1234_ffff] {
                            let mut real = RealCoherentPacket::default();
                            let mut complex = CoherentPacket::default();
                            real.reset(&prefix, 7, rank, gauge).unwrap();
                            complex.reset(&prefix, 7, rank).unwrap();
                            compare_cdf(&real, &complex);
                            let a = real.project(&p, index, y, fixed);
                            let b = complex.project(&p, index, y, fixed);
                            assert_eq!(a.is_ok(), b.is_ok(), "x={x} z={z} g={gauge}");
                            if a.is_ok() {
                                compare(&real, &complex);
                                compare_cdf(&real, &complex);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn real_rotations_match_complex_policies_expansion_and_wide_lanes() {
        let mut rng = StdRng::seed_from_u64(2026101101);
        for rank in [2, 4, 6, 10] {
            let len = 1usize << rank;
            for lanes in [1, 3, 17, 32, 63, 64] {
                for policy in [
                    CompiledRotationArithmetic::Strict,
                    CompiledRotationArithmetic::Fused,
                ] {
                    for _ in 0..4 {
                        let gauge = rng.gen_range(0..len);
                        let prefix = encoded(len, gauge);
                        let mut real = RealCoherentPacket::default();
                        let mut complex = CoherentPacket::default();
                        real.reset(&prefix, lanes, rank + 1, gauge).unwrap();
                        complex.reset(&prefix, lanes, rank + 1).unwrap();
                        for _ in 0..4 {
                            let (x, z, phase) = loop {
                                let x = rng.gen_range(1..len);
                                let z = rng.gen_range(0..len);
                                let phase =
                                    u8::from(parity(x & z)) + 2 * u8::from(rng.r#gen::<bool>());
                                if parity(gauge & x) == (phase & 1 == 0) {
                                    break (x, z, phase);
                                }
                            };
                            let p = pauli(x, z, phase);
                            let anti = rng.next_u64();
                            let dagger = rng.r#gen::<bool>();
                            real.rotate(&p, false, dagger, anti, policy).unwrap();
                            complex
                                .rotate_with_arithmetic(&p, false, dagger, anti, policy)
                                .unwrap();
                            compare(&real, &complex);
                            compare_cdf(&real, &complex);
                        }
                        let phase = if rng.r#gen::<bool>() { 0 } else { 1 };
                        let p = pauli(len, usize::from(phase == 1) * len, phase);
                        let anti = rng.next_u64();
                        real.rotate(&p, true, false, anti, policy).unwrap();
                        complex
                            .rotate_with_arithmetic(&p, true, false, anti, policy)
                            .unwrap();
                        compare(&real, &complex);
                        compare_cdf(&real, &complex);
                    }
                }
            }
        }
    }

    #[test]
    fn real_packet_limits_and_tiny_positive_branches_match_complex() {
        let mut real = RealCoherentPacket::default();
        assert!(real.reset(&[ComplexAmp::new(1., 0.)], 0, 0, 0).is_err());
        assert!(
            real.reset(&[ComplexAmp::new(1., 0.)], 64, usize::BITS as usize, 0)
                .is_err()
        );
        assert!(real.reset(&[ComplexAmp::new(1., 0.)], 64, 17, 0).is_err());
        assert!(real.reset(&[ComplexAmp::new(1., 1e-300)], 1, 0, 0).is_err());
        real.reset(
            &[ComplexAmp::new(1., 0.), ComplexAmp::new(1e-160, 0.)],
            64,
            1,
            0,
        )
        .unwrap();
        real.project(&pauli(0, 1, 0), 0, false, u64::MAX).unwrap();
        assert_eq!(real.len, 1);
        assert!(real.values.iter().all(|&v| v.to_bits() == 1f64.to_bits()));
        real.reset(&[ComplexAmp::new(1., 0.), ComplexAmp::new(0., 0.)], 1, 1, 0)
            .unwrap();
        assert!(real.project(&pauli(0, 1, 0), 0, false, 1).is_err());
        assert!(real.project(&pauli(0, 1, 0), 1, false, 0).is_err());
    }

    #[test]
    fn eligible_counts_preserve_raw_records_rng_and_positive_logical_errors() {
        let text = "H 0 1 2 3\nT 0 1 2 3\nMPP X0*X1*X2*X3\nOBSERVABLE_INCLUDE(0) rec[-1]\nX_ERROR(0.5) 4\nM 4\nOBSERVABLE_INCLUDE(0) rec[-1]\nX_ERROR(0.23) 5\nM 5\nDETECTOR rec[-1]\n";
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
            assert!(plan.real_prefix_gauge.is_some());
            assert!(plan.peak_active_rank >= 4);
            let mut reference = plan.clone();
            reference.real_prefix_gauge = None;
            let mut real = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut complex = reference.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(777);
            let mut b = a.clone();
            for shots in [1, 31, 32, 63, 64, 65, 1024] {
                let expected = complex.sample_measurements_u8(shots, &mut a).unwrap();
                let actual = real.sample_measurements_u8(shots, &mut b).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(a.next_u64(), b.next_u64());
                let expected = complex
                    .sample_postselected_counts(shots, 0, &mut a)
                    .unwrap();
                let actual = real.sample_postselected_counts(shots, 0, &mut b).unwrap();
                assert_eq!(actual, expected);
                if shots == 1024 {
                    assert!(
                        actual.accepted > 0 && actual.accepted < shots && actual.logical_errors > 0
                    );
                }
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert!(!real.real_coherent.values.is_empty());
            assert!(real.coherent.re.is_empty() && real.coherent.im.is_empty());
        }
        let mixed =
            CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nH 0\nT 0\nMY 0\n").unwrap();
        assert!(mixed.real_prefix_gauge.is_none());
    }

    #[test]
    fn msc_real_packets_preserve_both_counts_policies_and_rng() {
        for text in [
            include_str!(
                "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d3_inject_cultivate_p1e-3.stim"
            ),
            include_str!(
                "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
            ),
        ] {
            for policy in [
                CompiledRotationArithmetic::Strict,
                CompiledRotationArithmetic::Fused,
            ] {
                let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy)
                    .unwrap();
                assert!(plan.real_prefix_gauge.is_some());
                let mut reference = plan.clone();
                reference.real_prefix_gauge = None;
                let mut real = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut complex = reference.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(1024);
                let mut b = a.clone();
                for shots in [32, 63, 64, 65, 1024] {
                    assert_eq!(
                        real.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                        complex
                            .sample_postselected_counts(shots, 0, &mut b)
                            .unwrap()
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }
}
