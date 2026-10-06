// No RNG, Pauli frame, cache, or record handling lives in this arithmetic kernel.
// Each plane is amplitude-major: plane[amplitude * lanes + lane].
use super::*;

#[derive(Default)]
pub(super) struct CoherentPacket {
    pub(super) re: Vec<f64>,
    pub(super) im: Vec<f64>,
    reduced_re: Vec<f64>,
    reduced_im: Vec<f64>,
    pub(super) len: usize,
    pub(super) lanes: usize,
}

impl CoherentPacket {
    const BYTE_BUDGET: usize = 64 * 1024 * 1024;

    fn reserve(plane: &mut Vec<f64>, elements: usize) -> Result<(), String> {
        if plane.capacity() < elements {
            plane
                .try_reserve_exact(elements - plane.len())
                .map_err(|e| format!("coherent packet allocation failed: {e}"))?;
        }
        Ok(())
    }

    pub(super) fn reset(
        &mut self,
        prefix: &[ComplexAmp],
        lanes: usize,
        peak_rank: usize,
    ) -> Result<(), String> {
        if !(1..=64).contains(&lanes) {
            return Err("coherent packet requires 1 through 64 lanes".into());
        }
        if peak_rank >= usize::BITS as usize {
            return Err("coherent packet rank exceeds platform capacity".into());
        }
        let peak_len = 1usize
            .checked_shl(peak_rank as u32)
            .ok_or("coherent packet coefficient size overflow")?;
        if prefix.is_empty() || !prefix.len().is_power_of_two() || prefix.len() > peak_len {
            return Err("coherent packet prefix exceeds its structural peak rank".into());
        }
        let capacity = peak_len
            .checked_mul(lanes)
            .ok_or("coherent packet size overflow")?;
        let bytes = capacity
            .checked_mul(4 * std::mem::size_of::<f64>())
            .ok_or("coherent packet byte overflow")?;
        if bytes > Self::BYTE_BUDGET {
            return Err("coherent packet reservation exceeds 64 MiB".into());
        }
        // All four planes reserve the same structural maximum. Kernel updates
        // then require no allocations. Retaining a prior larger capacity does
        // not grow memory: each previous four-plane reservation also met this cap.
        Self::reserve(&mut self.re, capacity)?;
        Self::reserve(&mut self.im, capacity)?;
        Self::reserve(&mut self.reduced_re, capacity)?;
        Self::reserve(&mut self.reduced_im, capacity)?;
        let count = prefix.len() * lanes; // Checked by the larger capacity above.
        self.re.resize(count, 0.);
        self.im.resize(count, 0.);
        self.reduced_re.clear();
        self.reduced_im.clear();
        for (amplitude, &value) in prefix.iter().enumerate() {
            self.re[amplitude * lanes..(amplitude + 1) * lanes].fill(value.re);
            self.im[amplitude * lanes..(amplitude + 1) * lanes].fill(value.im);
        }
        self.len = prefix.len();
        self.lanes = lanes;
        Ok(())
    }

    fn check_pauli(&self, p: &CompactPauli, len: usize) -> Result<(), String> {
        if self.lanes == 0 || len == 0 || !len.is_power_of_two() {
            return Err("coherent packet has not been initialized".into());
        }
        if p.x >= len || p.z >= len || p.physical.phase >= 4 {
            return Err("coherent packet Pauli exceeds active coordinates".into());
        }
        Ok(())
    }

    // The two amplitude ranges are disjoint; borrowing them once leaves an
    // ordinary contiguous lane loop for the optimizer, without unsafe code.
    #[inline]
    fn pair(plane: &mut [f64], a: usize, b: usize, lanes: usize) -> (&mut [f64], &mut [f64]) {
        debug_assert_ne!(a, b);
        if a < b {
            let (lower, upper) = plane.split_at_mut(b);
            (&mut lower[a..a + lanes], &mut upper[..lanes])
        } else {
            let (lower, upper) = plane.split_at_mut(a);
            (&mut upper[..lanes], &mut lower[b..b + lanes])
        }
    }

    pub(super) fn rotate(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        anti: u64,
    ) -> Result<(), String> {
        if self.lanes == 0 {
            return Err("coherent packet has not been initialized".into());
        }
        if p.x == 0 && p.z == 0 {
            return Ok(()); // Same scalar/global-phase convention as rotate_signed.
        }
        let len = if expand {
            self.len
                .checked_mul(2)
                .ok_or("coherent packet expansion overflow")?
        } else {
            self.len
        };
        self.check_pauli(p, len)?;
        let count = len
            .checked_mul(self.lanes)
            .ok_or("coherent packet size overflow")?;
        let bytes = count
            .checked_mul(4 * std::mem::size_of::<f64>())
            .ok_or("coherent packet byte overflow")?;
        if bytes > Self::BYTE_BUDGET {
            return Err("coherent packet reservation exceeds 64 MiB".into());
        }
        if [
            self.re.capacity(),
            self.im.capacity(),
            self.reduced_re.capacity(),
            self.reduced_im.capacity(),
        ]
        .iter()
        .any(|&capacity| count > capacity)
        {
            return Err("coherent packet exceeds its reserved peak capacity".into());
        }
        self.re.resize(count, 0.);
        self.im.resize(count, 0.);
        self.len = len;
        let c = (std::f64::consts::PI / 8.).cos();
        let s = (std::f64::consts::PI / 8.).sin();
        let imaginary = p.physical.phase % 2 == 0;
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
        let lanes = self.lanes;
        if p.x == 0 {
            for amplitude in 0..self.len {
                let parity = (amplitude & p.z).count_ones() % 2 != 0;
                let range = amplitude * lanes..(amplitude + 1) * lanes;
                let a_re = &mut self.re[range.clone()];
                let a_im = &mut self.im[range];
                for ((re, im), &factor) in a_re.iter_mut().zip(a_im).zip(&factors[..lanes]) {
                    let factor = if parity { -factor } else { factor };
                    let old_re = *re;
                    let old_im = *im;
                    if imaginary {
                        *re = old_re * c + (-old_im * factor);
                        *im = old_im * c + old_re * factor;
                    } else {
                        *re = old_re * c + old_re * factor;
                        *im = old_im * c + old_im * factor;
                    }
                }
            }
        } else {
            let pivot = 1usize << p.x.trailing_zeros();
            for block in (0..self.len).step_by(pivot * 2) {
                let other = block ^ p.x;
                for offset in 0..pivot {
                    let i = block + offset;
                    let j = other + offset;
                    let parity_i = (i & p.z).count_ones() % 2 != 0;
                    let parity_j = (j & p.z).count_ones() % 2 != 0;
                    let (a_re, b_re) = Self::pair(&mut self.re, i * lanes, j * lanes, lanes);
                    let (a_im, b_im) = Self::pair(&mut self.im, i * lanes, j * lanes, lanes);
                    for (((ar, ai), (br, bi)), &factor) in a_re
                        .iter_mut()
                        .zip(a_im)
                        .zip(b_re.iter_mut().zip(b_im))
                        .zip(&factors[..lanes])
                    {
                        let factor_i = if parity_i { -factor } else { factor };
                        let factor_j = if parity_j { -factor } else { factor };
                        let old_ar = *ar;
                        let old_ai = *ai;
                        let old_br = *br;
                        let old_bi = *bi;
                        if imaginary {
                            *ar = old_ar * c + (-old_bi * factor_j);
                            *ai = old_ai * c + old_br * factor_j;
                            *br = old_br * c + (-old_ai * factor_i);
                            *bi = old_bi * c + old_ar * factor_i;
                        } else {
                            *ar = old_ar * c + old_br * factor_j;
                            *ai = old_ai * c + old_bi * factor_j;
                            *br = old_br * c + old_ar * factor_i;
                            *bi = old_bi * c + old_ai * factor_i;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn probability_zero(&self, p: &CompactPauli) -> [f64; 64] {
        assert!(self.lanes != 0, "coherent packet has not been initialized");
        debug_assert!(self.check_pauli(p, self.len).is_ok());
        let mut expectation = [0.; 64];
        let mut norm = [0.; 64];
        let phase = i_pow(p.physical.phase);
        for amplitude in 0..self.len {
            let sign = if (amplitude & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            let partner = amplitude ^ p.x;
            for lane in 0..self.lanes {
                let i = amplitude * self.lanes + lane;
                let j = partner * self.lanes + lane;
                let amp = ComplexAmp::new(self.re[i], self.im[i]);
                let image = ComplexAmp::new(self.re[j], self.im[j]);
                // Preserve the incumbent's exact left-associated complex expression
                // and per-lane amplitude accumulation order, including roundoff.
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
            return Err("coherent packet projection pivot outside active coordinates".into());
        }
        let pivot = 1usize << index;
        if (if p.x == 0 { p.z } else { p.x }) & pivot == 0 {
            return Err("coherent packet projection pivot absent from Pauli".into());
        }
        let low = pivot - 1;
        let other = p.z & !pivot;
        let len = self.len / 2;
        let count = len * self.lanes;
        self.reduced_re.resize(count, 0.);
        self.reduced_im.resize(count, 0.);
        let mut norm = [0.; 64];
        for amplitude in 0..len {
            let without = (amplitude & low) | ((amplitude & !low) << 1);
            let parity = (without & other).count_ones() % 2 != 0;
            let a_base = without * self.lanes;
            let b_base = (without ^ p.x) * self.lanes;
            for lane in 0..self.lanes {
                let fixed = fixed_mask >> lane & 1 != 0;
                let value = if p.x == 0 {
                    let bit = fixed ^ parity;
                    let input = (without | ((bit as usize) << index)) * self.lanes + lane;
                    ComplexAmp::new(self.re[input], self.im[input])
                } else {
                    let a = ComplexAmp::new(self.re[a_base + lane], self.im[a_base + lane]);
                    let mut b = ComplexAmp::new(self.re[b_base + lane], self.im[b_base + lane]);
                    if y {
                        b = ComplexAmp::new(b.im, -b.re);
                    }
                    let sign = if fixed ^ parity { -1. } else { 1. };
                    (a + b * sign) * std::f64::consts::FRAC_1_SQRT_2
                };
                norm[lane] += value.norm_sqr();
                let output = amplitude * self.lanes + lane;
                self.reduced_re[output] = value.re;
                self.reduced_im[output] = value.im;
            }
        }
        let mut scales = [0.; 64];
        for lane in 0..self.lanes {
            if !norm[lane].is_finite() || norm[lane] <= 0. {
                return Err(format!(
                    "coherent packet zero-probability measurement at lane {lane}"
                ));
            }
            scales[lane] = 1. / norm[lane].sqrt();
        }
        for amplitude in 0..len {
            let range = amplitude * self.lanes..(amplitude + 1) * self.lanes;
            for (lane, (re, im)) in self.reduced_re[range.clone()]
                .iter_mut()
                .zip(&mut self.reduced_im[range])
                .enumerate()
            {
                *re *= scales[lane];
                *im *= scales[lane];
            }
        }
        std::mem::swap(&mut self.re, &mut self.reduced_re);
        std::mem::swap(&mut self.im, &mut self.reduced_im);
        self.len = len;
        if len == 1 {
            self.re.fill(1.);
            self.im.fill(0.);
        }
        Ok(())
    }
}

// The independent dense physics tests in
// compiled.rs remain necessary; these compare the new layout to scalar FP64
// arithmetic, including exact CDF bits (not only distribution tolerances).
#[cfg(test)]
mod coherent_packet_kernel_tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn compact(x: usize, z: usize, phase: u8) -> CompactPauli {
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

    fn compare_density(packet: &CoherentPacket, lane: usize, scalar: &[ComplexAmp]) {
        assert_eq!(packet.len, scalar.len());
        for i in 0..scalar.len() {
            let a = ComplexAmp::new(
                packet.re[i * packet.lanes + lane],
                packet.im[i * packet.lanes + lane],
            );
            for j in 0..scalar.len() {
                let b = ComplexAmp::new(
                    packet.re[j * packet.lanes + lane],
                    packet.im[j * packet.lanes + lane],
                );
                let actual = a * b.conj();
                let expected = scalar[i] * scalar[j].conj();
                assert!((actual.re - expected.re).abs() < 2e-13);
                assert!((actual.im - expected.im).abs() < 2e-13);
            }
        }
    }

    #[test]
    fn coherent_packet_soa_preserves_scalar_complex_density_cdf_and_forced_projection() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 2\n").unwrap();
        let mut rng = StdRng::seed_from_u64(2026100702);
        let mut packet = CoherentPacket::default();
        for lanes in [1, 7, 32, 64] {
            for expand in [false, true] {
                for _ in 0..16 {
                    let mut prefix = (0..if expand { 4 } else { 8 })
                        .map(|_| {
                            ComplexAmp::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0))
                        })
                        .collect::<Vec<_>>();
                    let scale = 1. / prefix.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
                    for amp in &mut prefix {
                        *amp = *amp * scale;
                    }
                    packet.reset(&prefix, lanes, 3).unwrap();
                    let x = if expand {
                        rng.gen_range(0..4) | 4
                    } else {
                        rng.gen_range(0..8)
                    };
                    let z = rng.gen_range(0usize..8);
                    let phase =
                        ((x & z).count_ones() as u8 + 2 * u8::from(rng.r#gen::<bool>())) & 3;
                    let rotation = compact(x, z, phase);
                    let dagger = rng.r#gen::<bool>();
                    let anti = rng.r#gen::<u64>();
                    packet.rotate(&rotation, expand, dagger, anti).unwrap();
                    let mx = rng.gen_range(0usize..8);
                    let mz = if mx == 0 {
                        rng.gen_range(1usize..8)
                    } else {
                        rng.gen_range(0usize..8)
                    };
                    let mphase =
                        ((mx & mz).count_ones() as u8 + 2 * u8::from(rng.r#gen::<bool>())) & 3;
                    let measurement = compact(mx, mz, mphase);
                    let index = if mx == 0 {
                        mz.trailing_zeros()
                    } else {
                        mx.trailing_zeros()
                    } as usize;
                    let y = (mx & mz).count_ones() % 2 != 0;
                    let fixed = rng.r#gen::<u64>();
                    let probabilities = packet.probability_zero(&measurement);
                    let mut expected = Vec::new();
                    for lane in 0..lanes {
                        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                        scalar.coefficients = prefix.clone();
                        scalar
                            .rotate_signed(&rotation, expand, dagger, anti >> lane & 1 != 0)
                            .unwrap();
                        compare_density(&packet, lane, &scalar.coefficients);
                        assert_eq!(
                            probabilities[lane].to_bits(),
                            scalar.probability_zero(&measurement).to_bits()
                        );
                        scalar
                            .project(&measurement, index, y, fixed >> lane & 1 != 0)
                            .unwrap();
                        expected.push(scalar.coefficients);
                    }
                    packet.project(&measurement, index, y, fixed).unwrap();
                    for (lane, state) in expected.iter().enumerate() {
                        compare_density(&packet, lane, state);
                    }
                }
            }
        }
    }

    #[test]
    fn coherent_packet_soa_normalizes_tiny_branches_and_rejects_oversized_reservation() {
        let mut packet = CoherentPacket::default();
        assert!(packet.reset(&[ComplexAmp::new(1., 0.)], 64, 16).is_err());
        assert!(packet.reset(&[ComplexAmp::new(1., 0.)], 0, 0).is_err());
        packet
            .reset(
                &[ComplexAmp::new(1., 0.), ComplexAmp::new(1e-16, 0.)],
                64,
                1,
            )
            .unwrap();
        packet
            .project(&compact(0, 1, 0), 0, false, u64::MAX)
            .unwrap();
        assert_eq!(packet.len, 1);
        assert!(packet.re.iter().all(|&value| value == 1.));
        assert!(packet.im.iter().all(|&value| value == 0.));
    }
}
