#[inline]
pub(crate) fn transpose_64x64(words: &mut [u64; 64]) {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    // SAFETY: AArch64 NEON is enabled for this compilation target, and the backend only loads
    // and stores pairs of words within the 64-word tile.
    unsafe {
        transpose_64x64_neon(words);
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    transpose_64x64_portable(words);
}

#[cfg(any(test, not(all(target_arch = "aarch64", target_feature = "neon"))))]
pub(crate) fn transpose_64x64_portable(words: &mut [u64; 64]) {
    transpose_64x64_stage::<32>(words, 0x0000_0000_ffff_ffff);
    transpose_64x64_stage::<16>(words, 0x0000_ffff_0000_ffff);
    transpose_64x64_stage::<8>(words, 0x00ff_00ff_00ff_00ff);
    transpose_64x64_stage::<4>(words, 0x0f0f_0f0f_0f0f_0f0f);
    transpose_64x64_stage::<2>(words, 0x3333_3333_3333_3333);
    transpose_64x64_stage::<1>(words, 0x5555_5555_5555_5555);
}

#[inline(always)]
fn transpose_64x64_stage<const SHIFT: usize>(words: &mut [u64; 64], mask: u64) {
    let mut index = 0usize;
    while index < 64 {
        let swap = ((words[index] >> SHIFT) ^ words[index + SHIFT]) & mask;
        words[index] ^= swap << SHIFT;
        words[index + SHIFT] ^= swap;
        index = (index + SHIFT + 1) & !SHIFT;
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[target_feature(enable = "neon")]
unsafe fn transpose_64x64_neon(words: &mut [u64; 64]) {
    unsafe {
        transpose_64x64_stage_neon::<32>(words, 0x0000_0000_ffff_ffff);
        transpose_64x64_stage_neon::<16>(words, 0x0000_ffff_0000_ffff);
        transpose_64x64_stage_neon::<8>(words, 0x00ff_00ff_00ff_00ff);
        transpose_64x64_stage_neon::<4>(words, 0x0f0f_0f0f_0f0f_0f0f);
        transpose_64x64_stage_neon::<2>(words, 0x3333_3333_3333_3333);
    }
    // Adjacent words form one swap pair in the final stage. Keeping this stage scalar avoids
    // spending more lane-shuffle instructions than the two scalar word operations it replaces.
    transpose_64x64_stage::<1>(words, 0x5555_5555_5555_5555);
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
#[target_feature(enable = "neon")]
unsafe fn transpose_64x64_stage_neon<const SHIFT: i32>(words: &mut [u64; 64], mask: u64) {
    use core::arch::aarch64::{
        vandq_u64, vdupq_n_u64, veorq_u64, vld1q_u64, vshlq_n_u64, vshrq_n_u64, vst1q_u64,
    };

    const { assert!(SHIFT >= 2 && SHIFT <= 32 && (SHIFT & (SHIFT - 1)) == 0) };
    let shift = SHIFT as usize;
    let mask = vdupq_n_u64(mask);
    let mut block = 0usize;
    while block < words.len() {
        let mut offset = 0usize;
        while offset < shift {
            let low_ptr = unsafe { words.as_mut_ptr().add(block + offset) };
            let high_ptr = unsafe { low_ptr.add(shift) };
            let low = unsafe { vld1q_u64(low_ptr) };
            let high = unsafe { vld1q_u64(high_ptr) };
            let swap = vandq_u64(veorq_u64(vshrq_n_u64::<SHIFT>(low), high), mask);
            unsafe {
                vst1q_u64(low_ptr, veorq_u64(low, vshlq_n_u64::<SHIFT>(swap)));
                vst1q_u64(high_ptr, veorq_u64(high, swap));
            }
            offset += 2;
        }
        block += 2 * shift;
    }
}
