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

    #[cfg(test)]
    pub(super) fn rotate(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        anti: u64,
    ) -> Result<(), String> {
        self.rotate_with_arithmetic(p, expand, dagger, anti, CompiledRotationArithmetic::Strict)
    }

    pub(super) fn rotate_with_arithmetic(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        anti: u64,
        arithmetic: CompiledRotationArithmetic,
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
        if arithmetic == CompiledRotationArithmetic::Fused {
            return self.rotate_fused(p, dagger, anti);
        }
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
            let parity_swap = (p.x & p.z).count_ones() % 2 != 0;
            for block in (0..self.len).step_by(pivot * 2) {
                let other = block ^ p.x;
                // The pivot bit separates both entire amplitude blocks.
                let span = pivot * lanes; // Bounded by checked count above.
                let (a_re, b_re) = Self::pair(&mut self.re, block * lanes, other * lanes, span);
                let (a_im, b_im) = Self::pair(&mut self.im, block * lanes, other * lanes, span);
                for (offset, ((a_re, a_im), (b_re, b_im))) in a_re
                    .chunks_exact_mut(lanes)
                    .zip(a_im.chunks_exact_mut(lanes))
                    .zip(
                        b_re.chunks_exact_mut(lanes)
                            .zip(b_im.chunks_exact_mut(lanes)),
                    )
                    .enumerate()
                {
                    let i = block + offset;
                    let parity_i = (i & p.z).count_ones() % 2 != 0;
                    let parity_j = parity_i ^ parity_swap;
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

    fn rotate_fused(&mut self, p: &CompactPauli, dagger: bool, anti: u64) -> Result<(), String> {
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
        // Highest-X pairing borrows full halves once; every lower X mask is
        // a permutation of their amplitude rows. Keep the fixed 64-lane leaf
        // and arbitrary per-shot signs, without a call per lowest-pivot block.
        // Larger lowest pivots retain the incumbent amortized span kernel.
        if lanes == 64 && p.z == 0 && p.x >= 2 && p.x & 7 != 0 {
            let pivot = 1usize << (usize::BITS - 1 - p.x.leading_zeros());
            let span = pivot * 64; // 2*span <= checked len*lanes above.
            let row_xor = p.x ^ pivot;
            for (re, im) in self
                .re
                .chunks_exact_mut(span * 2)
                .zip(self.im.chunks_exact_mut(span * 2))
            {
                let (a_re, b_re) = re.split_at_mut(span);
                let (a_im, b_im) = im.split_at_mut(span);
                if imaginary {
                    Self::rotate_fused_highest64_z0_span::<true>(
                        a_re, a_im, b_re, b_im, &factors, c, row_xor,
                    );
                } else {
                    Self::rotate_fused_highest64_z0_span::<false>(
                        a_re, a_im, b_re, b_im, &factors, c, row_xor,
                    );
                }
            }
            return Ok(());
        }
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
                        *re = (-old_im).mul_add(factor, old_re * c);
                        *im = old_re.mul_add(factor, old_im * c);
                    } else {
                        *re = old_re.mul_add(factor, old_re * c);
                        *im = old_im.mul_add(factor, old_im * c);
                    }
                }
            }
        } else {
            let pivot = 1usize << p.x.trailing_zeros();
            let parity_swap = (p.x & p.z).count_ones() % 2 != 0;
            let uniform64 = lanes == 64 && p.z & (pivot - 1) == 0;
            for block in (0..self.len).step_by(pivot * 2) {
                let other = block ^ p.x;
                // The pivot bit separates both entire amplitude blocks.
                let span = pivot * lanes; // Bounded by checked count above.
                let (a_re, b_re) = Self::pair(&mut self.re, block * lanes, other * lanes, span);
                let (a_im, b_im) = Self::pair(&mut self.im, block * lanes, other * lanes, span);
                if uniform64 {
                    let parity_i = (block & p.z).count_ones() % 2 != 0;
                    let parity_j = parity_i ^ parity_swap;
                    Self::dispatch_fused_uniform64_span(
                        a_re,
                        a_im,
                        b_re,
                        b_im,
                        &factors,
                        c,
                        (imaginary, parity_i, parity_j),
                    );
                    continue;
                }
                for (offset, ((a_re, a_im), (b_re, b_im))) in a_re
                    .chunks_exact_mut(lanes)
                    .zip(a_im.chunks_exact_mut(lanes))
                    .zip(
                        b_re.chunks_exact_mut(lanes)
                            .zip(b_im.chunks_exact_mut(lanes)),
                    )
                    .enumerate()
                {
                    let i = block + offset;
                    let parity_i = (i & p.z).count_ones() % 2 != 0;
                    let parity_j = parity_i ^ parity_swap;
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
                            *ar = (-old_bi).mul_add(factor_j, old_ar * c);
                            *ai = old_br.mul_add(factor_j, old_ai * c);
                            *br = (-old_ai).mul_add(factor_i, old_br * c);
                            *bi = old_ar.mul_add(factor_i, old_bi * c);
                        } else {
                            *ar = old_br.mul_add(factor_j, old_ar * c);
                            *ai = old_bi.mul_add(factor_j, old_ai * c);
                            *br = old_ar.mul_add(factor_i, old_br * c);
                            *bi = old_ai.mul_add(factor_i, old_bi * c);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // Four separately borrowed planes keep a function-level noalias contract.
    // One call processes an entire highest-pivot half, not an amplitude row.
    #[inline(never)]
    fn rotate_fused_highest64_z0_span<const IMAGINARY: bool>(
        a_re: &mut [f64],
        a_im: &mut [f64],
        b_re: &mut [f64],
        b_im: &mut [f64],
        factors: &[f64; 64],
        c: f64,
        row_xor: usize,
    ) {
        debug_assert_eq!(a_re.len(), a_im.len());
        debug_assert_eq!(a_re.len(), b_re.len());
        debug_assert_eq!(a_re.len(), b_im.len());
        debug_assert_eq!(a_re.len() & 63, 0);
        debug_assert!(row_xor < a_re.len() / 64);
        for (row, (a_re, a_im)) in a_re
            .chunks_exact_mut(64)
            .zip(a_im.chunks_exact_mut(64))
            .enumerate()
        {
            // The partner stays in the other half. XOR permutes whole rows;
            // it never permutes shot lanes or their original factors.
            let other = (row ^ row_xor) * 64;
            let b_re = &mut b_re[other..other + 64];
            let b_im = &mut b_im[other..other + 64];
            let a_re: &mut [f64; 64] = a_re.try_into().unwrap();
            let a_im: &mut [f64; 64] = a_im.try_into().unwrap();
            let b_re: &mut [f64; 64] = b_re.try_into().unwrap();
            let b_im: &mut [f64; 64] = b_im.try_into().unwrap();
            for ((((ar, ai), br), bi), &factor) in
                a_re.iter_mut().zip(a_im).zip(b_re).zip(b_im).zip(factors)
            {
                let old_ar = *ar;
                let old_ai = *ai;
                let old_br = *br;
                let old_bi = *bi;
                // z==0 gives identical false parities on both logical sides.
                // Preserve source negation and every original Fused rounding.
                if IMAGINARY {
                    *ar = (-old_bi).mul_add(factor, old_ar * c);
                    *ai = old_br.mul_add(factor, old_ai * c);
                    *br = (-old_ai).mul_add(factor, old_br * c);
                    *bi = old_ar.mul_add(factor, old_bi * c);
                } else {
                    *ar = old_br.mul_add(factor, old_ar * c);
                    *ai = old_bi.mul_add(factor, old_ai * c);
                    *br = old_ar.mul_add(factor, old_br * c);
                    *bi = old_ai.mul_add(factor, old_bi * c);
                }
            }
        }
    }

    // Only a small dispatch is inlined. Keep the arithmetic helper boundary
    // so its four mutable slice arguments carry independent noalias contracts.
    #[inline(always)]
    fn dispatch_fused_uniform64_span(
        a_re: &mut [f64],
        a_im: &mut [f64],
        b_re: &mut [f64],
        b_im: &mut [f64],
        factors: &[f64; 64],
        c: f64,
        flags: (bool, bool, bool),
    ) {
        match flags {
            (false, false, false) => Self::rotate_fused_uniform64_span::<false, false, false>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (false, false, true) => Self::rotate_fused_uniform64_span::<false, false, true>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (false, true, false) => Self::rotate_fused_uniform64_span::<false, true, false>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (false, true, true) => Self::rotate_fused_uniform64_span::<false, true, true>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (true, false, false) => Self::rotate_fused_uniform64_span::<true, false, false>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (true, false, true) => Self::rotate_fused_uniform64_span::<true, false, true>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (true, true, false) => Self::rotate_fused_uniform64_span::<true, true, false>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
            (true, true, true) => Self::rotate_fused_uniform64_span::<true, true, true>(
                a_re, a_im, b_re, b_im, factors, c,
            ),
        }
    }

    #[inline(never)]
    fn rotate_fused_uniform64_span<const IMAGINARY: bool, const NEG_I: bool, const NEG_J: bool>(
        a_re: &mut [f64],
        a_im: &mut [f64],
        b_re: &mut [f64],
        b_im: &mut [f64],
        factors: &[f64; 64],
        c: f64,
    ) {
        debug_assert_eq!(a_re.len(), a_im.len());
        debug_assert_eq!(a_re.len(), b_re.len());
        debug_assert_eq!(a_re.len(), b_im.len());
        debug_assert_eq!(a_re.len() & 63, 0);
        // Every amplitude contains exactly 64 consecutive shot lanes. The
        // same signed factor table repeats once per amplitude, without modulo
        // indexing or runtime chunk division. Low-Z parity is block-constant.
        for (((a_re, a_im), b_re), b_im) in a_re
            .chunks_exact_mut(64)
            .zip(a_im.chunks_exact_mut(64))
            .zip(b_re.chunks_exact_mut(64))
            .zip(b_im.chunks_exact_mut(64))
        {
            for ((((ar, ai), br), bi), &factor) in
                a_re.iter_mut().zip(a_im).zip(b_re).zip(b_im).zip(factors)
            {
                let factor_i = if NEG_I { -factor } else { factor };
                let factor_j = if NEG_J { -factor } else { factor };
                let old_ar = *ar;
                let old_ai = *ai;
                let old_br = *br;
                let old_bi = *bi;
                if IMAGINARY {
                    *ar = (-old_bi).mul_add(factor_j, old_ar * c);
                    *ai = old_br.mul_add(factor_j, old_ai * c);
                    *br = (-old_ai).mul_add(factor_i, old_br * c);
                    *bi = old_ar.mul_add(factor_i, old_bi * c);
                } else {
                    *ar = old_br.mul_add(factor_j, old_ar * c);
                    *ai = old_bi.mul_add(factor_j, old_ai * c);
                    *br = old_ar.mul_add(factor_i, old_br * c);
                    *bi = old_ai.mul_add(factor_i, old_bi * c);
                }
            }
        }
    }

    pub(super) fn probability_zero(&self, p: &CompactPauli) -> [f64; 64] {
        assert!(self.lanes != 0, "coherent packet has not been initialized");
        debug_assert!(self.check_pauli(p, self.len).is_ok());
        match p.physical.phase & 3 {
            0 => self.probability_zero_phase::<0>(p),
            1 => self.probability_zero_phase::<1>(p),
            2 => self.probability_zero_phase::<2>(p),
            _ => self.probability_zero_phase::<3>(p),
        }
    }
    #[inline]
    fn probability_zero_phase<const PHASE: u8>(&self, p: &CompactPauli) -> [f64; 64] {
        if self.lanes == 64 {
            let mut probabilities = [0.; 64];
            // Bound each accumulator set while retaining amplitude order per lane.
            for start in (0..64).step_by(8) {
                let block = self.probability_zero_block::<PHASE, 8>(p, start);
                probabilities[start..start + 8].copy_from_slice(&block);
            }
            probabilities
        } else {
            self.probability_zero_block::<PHASE, 64>(p, 0)
        }
    }

    #[inline]
    fn probability_zero_block<const PHASE: u8, const BLOCK: usize>(
        &self,
        p: &CompactPauli,
        lane_start: usize,
    ) -> [f64; BLOCK] {
        let active_lanes = if BLOCK == 64 { self.lanes } else { BLOCK };
        debug_assert!(lane_start + active_lanes <= self.lanes);
        let mut expectation = [0.; BLOCK];
        let mut norm = [0.; BLOCK];
        let phase = i_pow(PHASE);
        for amplitude in 0..self.len {
            let sign = if (amplitude & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            let partner = amplitude ^ p.x;
            let i = amplitude * self.lanes + lane_start;
            let j = partner * self.lanes + lane_start;
            let amplitudes = self.re[i..i + active_lanes]
                .iter()
                .zip(&self.im[i..i + active_lanes]);
            let images = self.re[j..j + active_lanes]
                .iter()
                .zip(&self.im[j..j + active_lanes]);
            let accumulators = expectation[..active_lanes]
                .iter_mut()
                .zip(&mut norm[..active_lanes]);
            for (((&re, &im), (&image_re, &image_im)), (lane_expectation, lane_norm)) in
                amplitudes.zip(images).zip(accumulators)
            {
                let amp = ComplexAmp::new(re, im);
                let image = ComplexAmp::new(image_re, image_im);
                // Preserve the incumbent's exact left-associated complex expression
                // and per-lane amplitude accumulation order, including roundoff.
                *lane_expectation += (image.conj() * phase * (amp * sign)).re;
                *lane_norm += amp.norm_sqr();
            }
        }
        let mut probabilities = [0.; BLOCK];
        for lane in 0..active_lanes {
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

// This frozen arithmetic oracle complements the independent dense-physics and
// full compiled raw/RNG tests. It deliberately retains the old two-index parity
// calculation, allocation/zero-fill behavior and exact multiply/add association.
#[cfg(test)]
mod frozen_rotation_coefficient_bits_tests {
    use super::*;

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

    fn frozen_v24_scalar_rotate(
        coefficients: &mut Vec<ComplexAmp>,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        flip: bool,
    ) -> Result<(), String> {
        if p.x == 0 && p.z == 0 {
            return Ok(());
        }
        if expand {
            let len = coefficients
                .len()
                .checked_mul(2)
                .ok_or("compiled coefficient size overflow")?;
            coefficients
                .try_reserve_exact(len - coefficients.len())
                .map_err(|e| format!("compiled coefficient allocation failed: {e}"))?;
            coefficients.resize(len, ComplexAmp::default());
        }
        let c = (std::f64::consts::PI / 8.).cos();
        let s = (std::f64::consts::PI / 8.).sin();
        let imaginary = p.physical.phase % 2 == 0;
        let mut factor = if matches!(p.physical.phase, 0 | 3) {
            -s
        } else {
            s
        };
        if dagger ^ flip {
            factor = -factor;
        }
        let term = |index: usize, amp: ComplexAmp| {
            let factor = if (index & p.z).count_ones() % 2 != 0 {
                -factor
            } else {
                factor
            };
            if imaginary {
                ComplexAmp::new(-amp.im * factor, amp.re * factor)
            } else {
                amp * factor
            }
        };
        if p.x == 0 {
            for (i, amp) in coefficients.iter_mut().enumerate() {
                *amp = *amp * c + term(i, *amp);
            }
        } else {
            let pivot = 1 << p.x.trailing_zeros();
            for block in (0..coefficients.len()).step_by(pivot * 2) {
                let other = block ^ p.x;
                // The chosen pivot bit is zero in block and one in other, so
                // these equally sized ranges are disjoint. Borrow both once;
                // the iterator removes repeated bounds checks inside the kernel.
                let (a, b) = if block < other {
                    let (left, right) = coefficients.split_at_mut(other);
                    (&mut left[block..block + pivot], &mut right[..pivot])
                } else {
                    let (left, right) = coefficients.split_at_mut(block);
                    (&mut right[..pivot], &mut left[other..other + pivot])
                };
                for (offset, (a, b)) in a.iter_mut().zip(b).enumerate() {
                    let old_a = *a;
                    let old_b = *b;
                    *a = old_a * c + term(other + offset, old_b);
                    *b = old_b * c + term(block + offset, old_a);
                }
            }
        }
        Ok(())
    }
    fn rotation_cases() -> Vec<(usize, usize, usize)> {
        let mut cases = Vec::new();
        // Exhaustive masks cover diagonal/global phase, both pair parities,
        // every rank-3 pivot, and every multi-X mask, even for non-Hermitian
        // x/z/phase combinations accepted by the arithmetic kernel.
        for rank in [1, 3] {
            for x in 0..1usize << rank {
                for z in 0..1usize << rank {
                    cases.push((rank, x, z));
                }
            }
        }
        // Larger blocks exercise the packet block borrow and scalar constant-
        // parity branch, its low-Z fallback, and reversed block/other ordering.
        for (x, z) in [
            (0, 0),
            (0, 63),
            (1, 0),
            (1, 63),
            (2, 1),
            (2, 2),
            (4, 0),
            (4, 3),
            (4, 4),
            (8, 0),
            (8, 7),
            (8, 8),
            (32, 0),
            (32, 31),
            (32, 32),
            (3, 0),
            (3, 1),
            (5, 4),
            (12, 1),
            (12, 8),
            (33, 31),
            (48, 0),
            (48, 15),
            (63, 63),
        ] {
            cases.push((6, x, z));
        }
        cases
    }

    fn prefix(len: usize, kind: usize) -> Vec<ComplexAmp> {
        (0..len)
            .map(|index| match kind {
                0 => match index % 4 {
                    0 => ComplexAmp::new(-0., 0.),
                    1 => ComplexAmp::new(0., -0.),
                    2 => ComplexAmp::new(-0., -0.),
                    _ => ComplexAmp::new(0., 0.),
                },
                1 => match index % 4 {
                    0 => ComplexAmp::new(-0.3, 0.4),
                    1 => ComplexAmp::new(1., -1.),
                    2 => ComplexAmp::new(0.5, -0.5),
                    _ => ComplexAmp::new(1e-16, -1e-16),
                },
                2 => {
                    let tiny = [
                        f64::from_bits(1),
                        -f64::from_bits(2),
                        f64::MIN_POSITIVE,
                        -f64::MIN_POSITIVE,
                        1e-160,
                        -1e-300,
                    ];
                    ComplexAmp::new(tiny[index % tiny.len()], tiny[(index + 3) % tiny.len()])
                }
                _ => {
                    if index % 8 == 0 {
                        ComplexAmp::new(0.75, -0.25)
                    } else {
                        ComplexAmp::new(-0., 0.)
                    }
                }
            })
            .collect()
    }

    fn assert_scalar_bits(actual: &[ComplexAmp], expected: &[ComplexAmp], context: &str) {
        assert_eq!(actual.len(), expected.len(), "{context}; length");
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                (actual.re.to_bits(), actual.im.to_bits()),
                (expected.re.to_bits(), expected.im.to_bits()),
                "{context}; amplitude={index}",
            );
        }
    }

    fn assert_packet_bits(
        packet: &CoherentPacket,
        lane: usize,
        expected: &[ComplexAmp],
        context: &str,
    ) {
        assert_eq!(packet.len, expected.len(), "{context}; packet length");
        assert_eq!(packet.re.len(), packet.len * packet.lanes);
        assert_eq!(packet.im.len(), packet.len * packet.lanes);
        for (index, expected) in expected.iter().enumerate() {
            let cell = index * packet.lanes + lane;
            assert_eq!(
                (packet.re[cell].to_bits(), packet.im[cell].to_bits()),
                (expected.re.to_bits(), expected.im.to_bits()),
                "{context}; lane={lane}; amplitude={index}",
            );
        }
    }

    #[test]
    fn scalar_rotation_preserves_frozen_coefficient_bits_for_all_phase_and_pair_cases() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 5\n").unwrap();
        let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
        for (rank, x, z) in rotation_cases() {
            for phase in 0..4 {
                let p = compact(x, z, phase);
                for expand in [false, true] {
                    let len = 1 << (rank - usize::from(expand));
                    for kind in 0..4 {
                        let before = prefix(len, kind);
                        for dagger in [false, true] {
                            for flip in [false, true] {
                                let mut expected = before.clone();
                                frozen_v24_scalar_rotate(&mut expected, &p, expand, dagger, flip)
                                    .unwrap();
                                sampler.coefficients.clone_from(&before);
                                sampler.rotate_signed(&p, expand, dagger, flip).unwrap();
                                let context = format!(
                                    "rank={rank}; x={x}; z={z}; phase={phase}; expand={expand}; kind={kind}; dagger={dagger}; flip={flip}"
                                );
                                assert_scalar_bits(&sampler.coefficients, &expected, &context);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packet_rotation_preserves_frozen_coefficient_bits_for_lanes_and_expansion() {
        let mut packet = CoherentPacket::default();
        for (rank, x, z) in rotation_cases() {
            for phase in 0..4 {
                let p = compact(x, z, phase);
                for expand in [false, true] {
                    let len = 1 << (rank - usize::from(expand));
                    for kind in 0..4 {
                        let before = prefix(len, kind);
                        for dagger in [false, true] {
                            // These expectations come directly from the frozen
                            // kernel, never from the current scalar candidate.
                            let mut expected = [before.clone(), before.clone()];
                            for (flip, state) in expected.iter_mut().enumerate() {
                                frozen_v24_scalar_rotate(state, &p, expand, dagger, flip != 0)
                                    .unwrap();
                            }
                            for lanes in [1, 7, 32, 64] {
                                // Distribute masks across the full phase/input
                                // matrix to keep the test bounded while checking
                                // empty/full, alternating and the high lane bit.
                                let masks =
                                    [0, u64::MAX, 0xaaaa_aaaa_aaaa_aaaa, 1 << 63, (1 << 63) | 1];
                                let anti = masks[(phase as usize
                                    + kind
                                    + usize::from(dagger)
                                    + usize::from(expand))
                                    % masks.len()];
                                packet.reset(&before, lanes, rank).unwrap();
                                packet.rotate(&p, expand, dagger, anti).unwrap();
                                let context = format!(
                                    "rank={rank}; x={x}; z={z}; phase={phase}; expand={expand}; kind={kind}; dagger={dagger}; lanes={lanes}; anti={anti:#x}"
                                );
                                for lane in 0..lanes {
                                    assert_packet_bits(
                                        &packet,
                                        lane,
                                        &expected[(anti >> lane & 1) as usize],
                                        &context,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_packet_and_scalar_rotations_preserve_frozen_bits_after_expansions() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 5\n").unwrap();
        let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut packet = CoherentPacket::default();
        let operations = [
            (0, 15, 0, false, false),
            (8, 0, 1, false, true),
            (12, 1, 2, false, false),
            // Expansion intentionally includes low X bits: planner-only x==old_len
            // assumptions would fail this accepted arithmetic input.
            (21, 31, 3, true, true),
            (24, 0, 0, false, false),
            (48, 15, 1, true, false),
            (32, 32, 2, false, true),
            (48, 15, 3, false, false),
            (48, 16, 0, false, true),
            (63, 63, 1, false, false),
            (0, 63, 2, false, true),
            (0, 0, 3, false, true),
        ];
        for kind in 0..4 {
            let before = prefix(16, kind);
            for lanes in [1, 7, 32, 64] {
                packet.reset(&before, lanes, 6).unwrap();
                let mut expected = vec![before.clone(); lanes];
                for (step, &(x, z, phase, expand, dagger)) in operations.iter().enumerate() {
                    let p = compact(x, z, phase);
                    let anti = [0, u64::MAX, 0x5555_5555_5555_5555, (1 << 63) | 1][step % 4];
                    packet.rotate(&p, expand, dagger, anti).unwrap();
                    for (lane, state) in expected.iter_mut().enumerate() {
                        let flip = anti >> lane & 1 != 0;
                        sampler.coefficients.clone_from(state);
                        sampler.rotate_signed(&p, expand, dagger, flip).unwrap();
                        frozen_v24_scalar_rotate(state, &p, expand, dagger, flip).unwrap();
                        let context = format!(
                            "sequence step={step}; kind={kind}; lanes={lanes}; lane={lane}"
                        );
                        assert_scalar_bits(&sampler.coefficients, state, &context);
                        assert_packet_bits(&packet, lane, state, &context);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod phase_specialized_cdf_tests {
    use super::*;

    // These are frozen original kernels; neither calls the new specialized CDF.
    fn frozen_scalar_probability_zero(coefficients: &[ComplexAmp], p: &CompactPauli) -> f64 {
        let mut expectation = 0.;
        let mut norm = 0.;
        for (i, &amp) in coefficients.iter().enumerate() {
            let sign = if (i & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            expectation +=
                (coefficients[i ^ p.x].conj() * i_pow(p.physical.phase) * (amp * sign)).re;
            norm += amp.norm_sqr();
        }
        ((1. + expectation / norm) * 0.5).clamp(0., 1.)
    }
    fn frozen_packet_probability_zero(packet: &CoherentPacket, p: &CompactPauli) -> [f64; 64] {
        assert!(
            packet.lanes != 0,
            "coherent packet has not been initialized"
        );
        debug_assert!(packet.check_pauli(p, packet.len).is_ok());
        let mut expectation = [0.; 64];
        let mut norm = [0.; 64];
        let phase = i_pow(p.physical.phase);
        for amplitude in 0..packet.len {
            let sign = if (amplitude & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            let partner = amplitude ^ p.x;
            for lane in 0..packet.lanes {
                let i = amplitude * packet.lanes + lane;
                let j = partner * packet.lanes + lane;
                let amp = ComplexAmp::new(packet.re[i], packet.im[i]);
                let image = ComplexAmp::new(packet.re[j], packet.im[j]);
                // Preserve the incumbent's exact left-associated complex expression
                // and per-lane amplitude accumulation order, including roundoff.
                expectation[lane] += (image.conj() * phase * (amp * sign)).re;
                norm[lane] += amp.norm_sqr();
            }
        }
        let mut probabilities = [0.; 64];
        for lane in 0..packet.lanes {
            probabilities[lane] = ((1. + expectation[lane] / norm[lane]) * 0.5).clamp(0., 1.);
        }
        probabilities
    }
    fn compact(x: usize, z: usize, phase: u8) -> CompactPauli {
        CompactPauli {
            physical: PackedPauli {
                x: vec![0],
                z: vec![0],
                phase,
            },
            x,
            z,
        }
    }

    fn coefficients(len: usize, kind: usize, lane: usize) -> Vec<ComplexAmp> {
        let mut result = (0..len)
            .map(|i| {
                let selector = (i * 7 + lane * 3) % 8;
                match kind {
                    0 => match selector {
                        0 | 1 => ComplexAmp::new(0., -0.),
                        2 | 3 => ComplexAmp::new(-0., 0.),
                        4 => ComplexAmp::new(1., 0.),
                        5 => ComplexAmp::new(0., -1.),
                        _ => ComplexAmp::new(-0.5, 0.25),
                    },
                    1 => ComplexAmp::new(
                        ((i * 11 + lane * 7) % 23) as f64 / 17. - 0.5,
                        ((i * 13 + lane * 5) % 19) as f64 / 13. - 0.25,
                    ),
                    2 => match selector {
                        0 => ComplexAmp::new(f64::from_bits(1), -0.),
                        1 => ComplexAmp::new(-f64::from_bits(1), 0.),
                        2 => ComplexAmp::new(f64::MIN_POSITIVE, -f64::MIN_POSITIVE),
                        3 => ComplexAmp::new(1e-160, -1e-160),
                        4 => ComplexAmp::new(-0., 0.),
                        _ => ComplexAmp::new(0.125, -0.25),
                    },
                    3 => ComplexAmp::new(
                        (((i + lane) % 7) as f64 - 3.) * 1e-150,
                        (((i * 3 + lane) % 5) as f64 - 2.) * 1e-160,
                    ),
                    _ => ComplexAmp::new(
                        (((i + lane) % 11) as f64 - 5.) * 2f64.powi(200),
                        (((i * 5 + lane) % 7) as f64 - 3.) * 2f64.powi(190),
                    ),
                }
            })
            .collect::<Vec<_>>();
        // Every tested row has finite positive norm, including len=1 and the
        // all-tiny case. Do not normalize or reorder the kernel's reductions.
        result[len / 2] = if kind == 3 {
            ComplexAmp::new(1e-150, -0.)
        } else if kind == 4 {
            ComplexAmp::new(2f64.powi(200), 0.)
        } else {
            ComplexAmp::new(0.75, -0.25)
        };
        let norm = result.iter().map(|amp| amp.norm_sqr()).sum::<f64>();
        assert!(norm.is_finite() && norm > 0.);
        result
    }

    fn coordinates(len: usize) -> Vec<(usize, usize)> {
        let values = if len <= 8 {
            (0..len).collect::<Vec<_>>()
        } else {
            vec![0, 1, 2, len / 2, len - 2, len - 1]
        };
        values
            .iter()
            .flat_map(|&x| values.iter().map(move |&z| (x, z)))
            .collect()
    }

    #[test]
    fn phase_specialized_scalar_and_packet_cdfs_match_independent_frozen_bits() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 5\n").unwrap();
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut packet = CoherentPacket::default();
        for len in [1usize, 2, 4, 8, 32, 64] {
            let rank = len.trailing_zeros() as usize;
            for (x, z) in coordinates(len) {
                for phase in 0..4 {
                    let p = compact(x, z, phase);
                    for kind in 0..5 {
                        for lanes in [1, 7, 32, 64] {
                            let prefix = coefficients(len, kind, 0);
                            packet.reset(&prefix, lanes, rank).unwrap();
                            for lane in 0..lanes {
                                let state = coefficients(len, (kind + lane) % 5, lane);
                                for (amplitude, amp) in state.into_iter().enumerate() {
                                    packet.re[amplitude * lanes + lane] = amp.re;
                                    packet.im[amplitude * lanes + lane] = amp.im;
                                }
                            }
                            let expected = frozen_packet_probability_zero(&packet, &p);
                            let actual = packet.probability_zero(&p);
                            for lane in 0..64 {
                                assert_eq!(
                                    actual[lane].to_bits(),
                                    expected[lane].to_bits(),
                                    "packet len={len}; x={x}; z={z}; phase={phase}; kind={kind}; lanes={lanes}; lane={lane}"
                                );
                            }
                            for lane in 0..lanes {
                                scalar.coefficients = (0..len)
                                    .map(|amplitude| {
                                        let i = amplitude * lanes + lane;
                                        ComplexAmp::new(packet.re[i], packet.im[i])
                                    })
                                    .collect();
                                let scalar_expected =
                                    frozen_scalar_probability_zero(&scalar.coefficients, &p);
                                assert_eq!(
                                    scalar.probability_zero(&p).to_bits(),
                                    scalar_expected.to_bits(),
                                    "scalar len={len}; x={x}; z={z}; phase={phase}; kind={kind}; lanes={lanes}; lane={lane}"
                                );
                                assert_eq!(expected[lane].to_bits(), scalar_expected.to_bits());
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn full_packet_cdfs_keep_frozen_bits_at_high_rank_and_after_partial_reset() {
        let mut packet = CoherentPacket::default();
        for len in [1024usize, 4096] {
            let rank = len.trailing_zeros() as usize;
            // Reuse the allocation across full and partial packet strides.
            for lanes in [64, 7, 64] {
                packet.reset(&coefficients(len, 0, 0), lanes, rank).unwrap();
                for lane in 0..lanes {
                    let state = coefficients(len, lane % 5, lane);
                    for (amplitude, amp) in state.into_iter().enumerate() {
                        packet.re[amplitude * lanes + lane] = amp.re;
                        packet.im[amplitude * lanes + lane] = amp.im;
                    }
                }
                for (x, z) in [(0, len - 1), (len / 2, 0), (5, len - 2), (len - 1, len - 1)] {
                    for phase in 0..4 {
                        let p = compact(x, z, phase);
                        let actual = packet.probability_zero(&p);
                        let frozen = frozen_packet_probability_zero(&packet, &p);
                        for lane in 0..64 {
                            assert_eq!(
                                actual[lane].to_bits(),
                                frozen[lane].to_bits(),
                                "len={len}; lanes={lanes}; x={x}; z={z}; phase={phase}; lane={lane}"
                            );
                            if lane < lanes {
                                let state = (0..len)
                                    .map(|amplitude| {
                                        let index = amplitude * lanes + lane;
                                        ComplexAmp::new(packet.re[index], packet.im[index])
                                    })
                                    .collect::<Vec<_>>();
                                assert_eq!(
                                    actual[lane].to_bits(),
                                    frozen_scalar_probability_zero(&state, &p).to_bits(),
                                    "independent scalar CDF len={len}; lanes={lanes}; lane={lane}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn scalar_cdf_preserves_all_unsigned_modulo_phase_aliases() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 5\n").unwrap();
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        for kind in 0..5 {
            scalar.coefficients = coefficients(32, kind, 7);
            for (x, z) in [(0, 0), (0, 31), (31, 0), (31, 31), (5, 19)] {
                for phase in u8::MIN..=u8::MAX {
                    let p = compact(x, z, phase);
                    assert_eq!(
                        scalar.probability_zero(&p).to_bits(),
                        frozen_scalar_probability_zero(&scalar.coefficients, &p).to_bits(),
                        "phase alias kind={kind}; x={x}; z={z}; phase={phase}"
                    );
                }
            }
        }
    }

    #[test]
    fn i_pow_modulo_literals_keep_exact_components_for_all_u8_values() {
        let phases = [
            ComplexAmp::new(1., 0.),
            ComplexAmp::new(0., 1.),
            ComplexAmp::new(-1., 0.),
            ComplexAmp::new(0., -1.),
        ];
        for phase in u8::MIN..=u8::MAX {
            let actual = i_pow(phase);
            let expected = phases[(phase & 3) as usize];
            assert_eq!(actual.re.to_bits(), expected.re.to_bits());
            assert_eq!(actual.im.to_bits(), expected.im.to_bits());
        }
    }
}

// Append to coherent_packet.rs; independent snapshot/gather arithmetic oracle.
#[cfg(test)]
mod rotation_arithmetic_policy_bits_tests {
    use super::*;
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

    fn rotation_cases() -> Vec<(usize, usize, usize)> {
        let mut cases = Vec::new();
        // Exhaustive masks cover diagonal/global phase, both pair parities,
        // every rank-3 pivot, and every multi-X mask, even for non-Hermitian
        // x/z/phase combinations accepted by the arithmetic kernel.
        for rank in [1, 3] {
            for x in 0..1usize << rank {
                for z in 0..1usize << rank {
                    cases.push((rank, x, z));
                }
            }
        }
        // Larger blocks exercise the packet block borrow and scalar constant-
        // parity branch, its low-Z fallback, and reversed block/other ordering.
        for (x, z) in [
            (0, 0),
            (0, 63),
            (1, 0),
            (1, 63),
            (2, 1),
            (2, 2),
            (4, 0),
            (4, 3),
            (4, 4),
            (8, 0),
            (8, 7),
            (8, 8),
            (32, 0),
            (32, 31),
            (32, 32),
            (3, 0),
            (3, 1),
            (5, 4),
            (12, 1),
            (12, 8),
            (33, 31),
            (48, 0),
            (48, 15),
            (63, 63),
        ] {
            cases.push((6, x, z));
        }
        cases
    }

    fn prefix(len: usize, kind: usize) -> Vec<ComplexAmp> {
        (0..len)
            .map(|index| match kind {
                0 => match index % 4 {
                    0 => ComplexAmp::new(-0., 0.),
                    1 => ComplexAmp::new(0., -0.),
                    2 => ComplexAmp::new(-0., -0.),
                    _ => ComplexAmp::new(0., 0.),
                },
                1 => match index % 4 {
                    0 => ComplexAmp::new(-0.3, 0.4),
                    1 => ComplexAmp::new(1., -1.),
                    2 => ComplexAmp::new(0.5, -0.5),
                    _ => ComplexAmp::new(1e-16, -1e-16),
                },
                2 => {
                    let tiny = [
                        f64::from_bits(1),
                        -f64::from_bits(2),
                        f64::MIN_POSITIVE,
                        -f64::MIN_POSITIVE,
                        1e-160,
                        -1e-300,
                    ];
                    ComplexAmp::new(tiny[index % tiny.len()], tiny[(index + 3) % tiny.len()])
                }
                _ => {
                    if index % 8 == 0 {
                        ComplexAmp::new(0.75, -0.25)
                    } else {
                        ComplexAmp::new(-0., 0.)
                    }
                }
            })
            .collect()
    }

    fn assert_scalar_bits(actual: &[ComplexAmp], expected: &[ComplexAmp], context: &str) {
        assert_eq!(actual.len(), expected.len(), "{context}; length");
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                (actual.re.to_bits(), actual.im.to_bits()),
                (expected.re.to_bits(), expected.im.to_bits()),
                "{context}; amplitude={index}",
            );
        }
    }

    fn assert_packet_bits(
        packet: &CoherentPacket,
        lane: usize,
        expected: &[ComplexAmp],
        context: &str,
    ) {
        assert_eq!(packet.len, expected.len(), "{context}; packet length");
        assert_eq!(packet.re.len(), packet.len * packet.lanes);
        assert_eq!(packet.im.len(), packet.len * packet.lanes);
        for (index, expected) in expected.iter().enumerate() {
            let cell = index * packet.lanes + lane;
            assert_eq!(
                (packet.re[cell].to_bits(), packet.im[cell].to_bits()),
                (expected.re.to_bits(), expected.im.to_bits()),
                "{context}; lane={lane}; amplitude={index}",
            );
        }
    }

    fn frozen_scalar_probability_zero(coefficients: &[ComplexAmp], p: &CompactPauli) -> f64 {
        let mut expectation = 0.;
        let mut norm = 0.;
        for (i, &amp) in coefficients.iter().enumerate() {
            let sign = if (i & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            expectation +=
                (coefficients[i ^ p.x].conj() * i_pow(p.physical.phase) * (amp * sign)).re;
            norm += amp.norm_sqr();
        }
        ((1. + expectation / norm) * 0.5).clamp(0., 1.)
    }

    // Full-vector gather deliberately avoids production pair traversal, block
    // parity reuse, const-phase dispatch, and the packet/scalar rotation helpers.
    fn reference_rotate(
        coefficients: &mut Vec<ComplexAmp>,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        flip: bool,
        policy: CompiledRotationArithmetic,
    ) {
        if p.x == 0 && p.z == 0 {
            return;
        }
        if expand {
            coefficients.resize(coefficients.len() * 2, ComplexAmp::default());
        }
        let old = coefficients.clone();
        let c = (std::f64::consts::PI / 8.).cos();
        let s = (std::f64::consts::PI / 8.).sin();
        let mut factor = if matches!(p.physical.phase, 0 | 3) {
            -s
        } else {
            s
        };
        if dagger ^ flip {
            factor = -factor;
        }
        for (i, output) in coefficients.iter_mut().enumerate() {
            let j = i ^ p.x;
            let f = if (j & p.z).count_ones() % 2 != 0 {
                -factor
            } else {
                factor
            };
            let own = old[i];
            let partner = old[j];
            let source = if p.physical.phase % 2 == 0 {
                ComplexAmp::new(-partner.im, partner.re)
            } else {
                partner
            };
            *output = match policy {
                CompiledRotationArithmetic::Strict => own * c + source * f,
                // The base multiply is rounded before the explicit FMA. In
                // particular, -partner.im is formed before calling mul_add.
                CompiledRotationArithmetic::Fused => ComplexAmp::new(
                    source.re.mul_add(f, own.re * c),
                    source.im.mul_add(f, own.im * c),
                ),
            };
        }
    }

    fn check_cdf(
        sampler: &CompiledNearCliffordSampler,
        packet: &CoherentPacket,
        lane: usize,
        expected: &[ComplexAmp],
        context: &str,
    ) {
        // All four phases and diagonal/non-diagonal CDFs; no call to either
        // candidate rotation or the candidate probability implementation.
        for (x, z) in [
            (0, 0),
            (0, expected.len() - 1),
            (1, 1),
            (expected.len() - 1, expected.len() - 1),
        ] {
            if x >= expected.len() {
                continue;
            }
            for phase in 0..4 {
                let p = compact(x, z, phase);
                let bits = frozen_scalar_probability_zero(expected, &p).to_bits();
                assert_eq!(
                    sampler.probability_zero(&p).to_bits(),
                    bits,
                    "{context}; scalar CDF x={x}, z={z}, phase={phase}"
                );
                assert_eq!(
                    packet.probability_zero(&p)[lane].to_bits(),
                    bits,
                    "{context}; packet CDF lane={lane}, x={x}, z={z}, phase={phase}"
                );
            }
        }
    }

    #[test]
    fn scalar_high_multi_x_matches_gather_coefficient_and_cdf_bits_for_both_policies() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic("I 9\n", policy)
                .unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            // Actual d5 masks: odd/even row permutation and lowest pivots 1/2/4.
            // Keep this scalar-only; do not multiply rank10 by all packet lanes.
            for x in [515, 998, 556] {
                for phase in 0..4 {
                    let p = compact(x, 0, phase);
                    for expand in [false, true] {
                        for kind in 0..4 {
                            let mut before = prefix(1 << (10 - usize::from(expand)), kind);
                            if kind == 1 {
                                // Distinguish high rows instead of repeating the prefix pattern.
                                for (index, amp) in before.iter_mut().enumerate() {
                                    let scale = (index + 1) as f64 / 1024.;
                                    amp.re *= scale;
                                    amp.im *= scale;
                                }
                            }
                            if matches!(kind, 0 | 2) {
                                // Preserve other signed-zero/subnormal values, but
                                // give the independent CDF a finite nonzero norm.
                                *before.last_mut().unwrap() = ComplexAmp::new(0.75, -0.25);
                            }
                            for dagger in [false, true] {
                                for flip in [false, true] {
                                    let context = format!(
                                        "rank10 {policy:?} x={x}; phase={phase}; expand={expand}; kind={kind}; dagger={dagger}; flip={flip}"
                                    );
                                    let mut expected = before.clone();
                                    reference_rotate(
                                        &mut expected,
                                        &p,
                                        expand,
                                        dagger,
                                        flip,
                                        policy,
                                    );
                                    scalar.coefficients = before.clone();
                                    scalar.rotate_signed(&p, expand, dagger, flip).unwrap();
                                    assert_scalar_bits(&scalar.coefficients, &expected, &context);
                                    for (cdf_x, cdf_z) in [(0, 1023), (x, 0), (1023, 1023)] {
                                        for cdf_phase in 0..4 {
                                            let query = compact(cdf_x, cdf_z, cdf_phase);
                                            let expected_bits =
                                                frozen_scalar_probability_zero(&expected, &query)
                                                    .to_bits();
                                            assert_eq!(
                                                scalar.probability_zero(&query).to_bits(),
                                                expected_bits,
                                                "{context}; CDF x={cdf_x}; z={cdf_z}; phase={cdf_phase}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fused_packet_high_row_xor_matches_gather_coefficient_and_cdf_bits() {
        let policy = CompiledRotationArithmetic::Fused;
        let mut packet = CoherentPacket::default();
        // 64 geometry/input configurations, each with two mixed anti masks.
        // Gather both signs once per configuration, never once per shot lane.
        for x in [515, 998] {
            for phase in 0..4 {
                let p = compact(x, 0, phase);
                for expand in [false, true] {
                    for kind in [1, 2] {
                        let mut before = prefix(1 << (10 - usize::from(expand)), kind);
                        if kind == 1 {
                            // Distinguish every row so periodic prefix data
                            // cannot conceal an incorrect high-row XOR.
                            for (index, amp) in before.iter_mut().enumerate() {
                                let scale = (index + 1) as f64 / 1024.;
                                amp.re *= scale;
                                amp.im *= scale;
                            }
                        } else {
                            // Keep subnormals/tiny amplitudes and a positive
                            // finite CDF norm even after their squares underflow.
                            *before.last_mut().unwrap() = ComplexAmp::new(0.75, -0.25);
                        }
                        for dagger in [false, true] {
                            let mut expected = [before.clone(), before.clone()];
                            for (flip, state) in expected.iter_mut().enumerate() {
                                reference_rotate(state, &p, expand, dagger, flip != 0, policy);
                            }
                            let queries = [compact(0, 1023, 0), compact(x, 0, 0)];
                            let cdf_bits: [[u64; 2]; 2] = std::array::from_fn(|flip| {
                                std::array::from_fn(|query| {
                                    frozen_scalar_probability_zero(&expected[flip], &queries[query])
                                        .to_bits()
                                })
                            });
                            for anti in [0xaaaa_aaaa_aaaa_aaaa, (1u64 << 63) | 1] {
                                let context = format!(
                                    "rank10 packet Fused x={x}; phase={phase}; expand={expand}; kind={kind}; dagger={dagger}; anti={anti:#x}"
                                );
                                packet.reset(&before, 64, 10).unwrap();
                                packet
                                    .rotate_with_arithmetic(&p, expand, dagger, anti, policy)
                                    .unwrap();
                                for lane in 0..64 {
                                    let flip = ((anti >> lane) & 1) as usize;
                                    assert_packet_bits(&packet, lane, &expected[flip], &context);
                                }
                                for (query_index, query) in queries.iter().enumerate() {
                                    let actual = packet.probability_zero(query);
                                    for lane in [0, 63] {
                                        let flip = ((anti >> lane) & 1) as usize;
                                        assert_eq!(
                                            actual[lane].to_bits(),
                                            cdf_bits[flip][query_index],
                                            "{context}; CDF query={query_index}; lane={lane}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn both_rotation_policies_match_independent_coefficient_and_cdf_bits() {
        let mut packet = CoherentPacket::default();
        let mut fused_changes_a_bit = false;
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic("I 5\n", policy)
                .unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            for (rank, x, z) in rotation_cases() {
                for phase in 0..4 {
                    let p = compact(x, z, phase);
                    for expand in [false, true] {
                        for kind in 0..4 {
                            let before = prefix(1 << (rank - usize::from(expand)), kind);
                            for dagger in [false, true] {
                                let mut expected = [before.clone(), before.clone()];
                                for (flip, state) in expected.iter_mut().enumerate() {
                                    reference_rotate(state, &p, expand, dagger, flip != 0, policy);
                                }
                                if policy == CompiledRotationArithmetic::Fused {
                                    let mut strict = before.clone();
                                    reference_rotate(
                                        &mut strict,
                                        &p,
                                        expand,
                                        dagger,
                                        false,
                                        CompiledRotationArithmetic::Strict,
                                    );
                                    fused_changes_a_bit |=
                                        strict.iter().zip(&expected[0]).any(|(a, b)| {
                                            a.re.to_bits() != b.re.to_bits()
                                                || a.im.to_bits() != b.im.to_bits()
                                        });
                                }
                                for lanes in [1, 7, 32, 64] {
                                    let masks = [
                                        0,
                                        u64::MAX,
                                        0xaaaa_aaaa_aaaa_aaaa,
                                        1 << 63,
                                        (1 << 63) | 1,
                                    ];
                                    let anti = masks[(phase as usize
                                        + kind
                                        + usize::from(dagger)
                                        + usize::from(expand))
                                        % masks.len()];
                                    packet.reset(&before, lanes, rank).unwrap();
                                    packet
                                        .rotate_with_arithmetic(&p, expand, dagger, anti, policy)
                                        .unwrap();
                                    for lane in 0..lanes {
                                        let flip = anti >> lane & 1 != 0;
                                        scalar.coefficients.clone_from(&before);
                                        scalar.rotate_signed(&p, expand, dagger, flip).unwrap();
                                        let state = &expected[usize::from(flip)];
                                        let context = format!(
                                            "policy={policy:?}; rank={rank}; x={x}; z={z}; phase={phase}; expand={expand}; kind={kind}; dagger={dagger}; lane={lane}"
                                        );
                                        assert_scalar_bits(&scalar.coefficients, state, &context);
                                        assert_packet_bits(&packet, lane, state, &context);
                                        // Bound duplicate work while retaining both lane signs,
                                        // high lanes, every mask/input, and exact accumulation bits.
                                        if lane == 0 || lane + 1 == lanes {
                                            check_cdf(&scalar, &packet, lane, state, &context);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            fused_changes_a_bit,
            "test corpus must distinguish explicit FMA from strict mul/add"
        );
    }

    #[test]
    fn repeated_both_policy_rotations_match_reference_after_multi_old_x_expansion_and_reset() {
        let operations = [
            (0, 15, 0, false, false),
            (8, 0, 1, false, true),
            (12, 1, 2, false, false),
            (21, 31, 3, true, true),
            (24, 0, 0, false, false),
            (48, 15, 1, true, false),
            (32, 32, 2, false, true),
            (48, 15, 3, false, false),
            (48, 16, 0, false, true),
            (63, 63, 1, false, false),
            (0, 63, 2, false, true),
            (0, 0, 3, false, true),
        ];
        let mut packet = CoherentPacket::default();
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic("I 5\n", policy)
                .unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            for kind in 0..4 {
                // Descending lane counts also verify reset discards retained
                // coefficients from the previous wider packet and larger rank.
                for lanes in [64, 32, 7, 1, 64] {
                    let before = prefix(16, kind);
                    packet.reset(&before, lanes, 6).unwrap();
                    let mut expected = vec![before.clone(); lanes];
                    for (step, &(x, z, phase, expand, dagger)) in operations.iter().enumerate() {
                        let p = compact(x, z, phase);
                        let anti = [0, u64::MAX, 0x5555_5555_5555_5555, (1 << 63) | 1][step % 4];
                        packet
                            .rotate_with_arithmetic(&p, expand, dagger, anti, policy)
                            .unwrap();
                        for (lane, state) in expected.iter_mut().enumerate() {
                            let flip = anti >> lane & 1 != 0;
                            scalar.coefficients.clone_from(state);
                            scalar.rotate_signed(&p, expand, dagger, flip).unwrap();
                            reference_rotate(state, &p, expand, dagger, flip, policy);
                            let context = format!(
                                "policy={policy:?}; step={step}; kind={kind}; lanes={lanes}; lane={lane}"
                            );
                            assert_scalar_bits(&scalar.coefficients, state, &context);
                            assert_packet_bits(&packet, lane, state, &context);
                            if lane == 0 || lane + 1 == lanes {
                                check_cdf(&scalar, &packet, lane, state, &context);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn arithmetic_policy_is_bound_before_nonempty_compile_prefix_and_default_is_strict() {
        let mut text = String::from("H 0 1 2\n");
        for _ in 0..12 {
            text.push_str("T 0 1 2\nH 0 2\nS 1\nCX 0 1\nT_DAG 1\nH 1\nCX 1 2\n");
        }
        text.push_str("MX 0\nMY 1\nM 2\n");
        assert_eq!(
            crate::near_clifford::CompiledRotationArithmetic::default(),
            CompiledRotationArithmetic::Strict
        );
        let default = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        let old_limit = CompiledNearCliffordExecutor::compile_with_limit(
            crate::parser::parse_lines(&text).unwrap(),
            3,
        )
        .unwrap();
        assert_eq!(
            default.rotation_arithmetic,
            CompiledRotationArithmetic::Strict
        );
        assert_eq!(
            old_limit.rotation_arithmetic,
            CompiledRotationArithmetic::Strict
        );
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy).unwrap();
            let limited = CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
                crate::parser::parse_lines(&text).unwrap(),
                3,
                policy,
            )
            .unwrap();
            assert_eq!(plan.rotation_arithmetic, policy);
            assert_eq!(limited.rotation_arithmetic, policy);
            assert!(plan.prefix_len > 0 && plan.initial_coefficients.len() > 1);
            let mut expected = vec![ComplexAmp::new(1., 0.)];
            let mut rotations = 0;
            for op in &plan.operations[..plan.prefix_len] {
                if let PlanOp::Rotate {
                    pauli,
                    expand,
                    dagger,
                } = op
                {
                    reference_rotate(&mut expected, pauli, *expand, *dagger, false, policy);
                    rotations += 1;
                }
            }
            assert!(rotations > 1, "prefix fixture must exercise arithmetic");
            assert_scalar_bits(
                &plan.initial_coefficients,
                &expected,
                "compiled prefix reference",
            );
            assert_scalar_bits(
                &limited.initial_coefficients,
                &expected,
                "limit constructor prefix reference",
            );
            let prepared = plan.prepare_sampler_with_cache_budget(0).unwrap();
            assert_scalar_bits(
                &prepared.coefficients,
                &expected,
                "prepared prefix reference",
            );
            if policy == CompiledRotationArithmetic::Strict {
                assert_scalar_bits(
                    &default.initial_coefficients,
                    &expected,
                    "default strict prefix",
                );
                assert_scalar_bits(
                    &old_limit.initial_coefficients,
                    &expected,
                    "old limit strict prefix",
                );
            }
        }
    }
}
