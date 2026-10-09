//! Optional real scratch for uncached counts rows. Cached vectors stay complex.
use super::*;

pub(super) struct RealFallback {
    before: Vec<usize>,
    values: Vec<f64>,
    reduced: Vec<f64>,
    mask: usize,
    constant: bool,
    pub(super) active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(mask: usize, constant: bool, values: Vec<f64>) -> RealFallback {
        let width = values.len() * 2;
        let mut r = RealFallback {
            before: vec![mask],
            values,
            reduced: Vec::with_capacity(width),
            mask,
            constant,
            active: true,
        };
        r.values.reserve_exact(width - r.values.len());
        r
    }
    fn complex(r: &RealFallback) -> Vec<ComplexAmp> {
        r.values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if parity(i & r.mask) ^ r.constant {
                    ComplexAmp::new(0., v)
                } else {
                    ComplexAmp::new(v, 0.)
                }
            })
            .collect()
    }
    fn bits(v: &[ComplexAmp]) -> Vec<(u64, u64)> {
        v.iter().map(|a| (a.re.to_bits(), a.im.to_bits())).collect()
    }
    fn pauli(x: usize, z: usize, phase: u8) -> CompactPauli {
        CompactPauli {
            x,
            z,
            physical: PackedPauli {
                x: vec![0],
                z: vec![0],
                phase,
            },
        }
    }
    fn values(n: usize, kind: usize) -> Vec<f64> {
        (0..n)
            .map(|i| match kind {
                0 => ((i * 173 + 37) % 509) as f64 / 509. - 0.5,
                1 => {
                    if i % 3 == 0 {
                        -0.
                    } else {
                        0.
                    }
                }
                2 => f64::from_bits((i as u64 % 19 + 1) | ((i as u64 & 1) << 63)),
                _ => (if i % 2 == 0 { 1. } else { -1. }) * 1e200,
            })
            .collect()
    }

    #[test]
    fn real_rotation_preserves_both_component_bits_of_retained_kernels() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
                "H 0\nT 0\nMX 0\n",
                policy,
            )
            .unwrap();
            let mut original = plan.prepare_sampler_with_cache_budget(0).unwrap();
            for rank in 1..=7 {
                let n = 1 << rank;
                for mask in [0, 1, n - 1, n / 2, (n - 1) & 0x55] {
                    for constant in [false, true] {
                        for kind in 0..3 {
                            for x in 0..n {
                                let imaginary = parity(x & mask);
                                let p = pauli(x, (x * 13 + 1) % n, if imaginary { 0 } else { 1 });
                                if p.x == 0 && p.z == 0 {
                                    continue;
                                }
                                for (dagger, flip) in [(false, false), (false, true), (true, true)]
                                {
                                    let mut r = real(mask, constant, values(n, kind));
                                    original.coefficients = complex(&r);
                                    original.rotate_signed(&p, false, dagger, flip).unwrap();
                                    assert!(r.rotate(&p, false, dagger, flip, policy));
                                    assert_eq!(
                                        bits(&complex(&r)),
                                        bits(&original.coefficients),
                                        "{policy:?} rank={rank} mask={mask} constant={constant} kind={kind} x={x} dagger={dagger} flip={flip}"
                                    );
                                }
                            }
                        }
                    }
                }
                for mask in [0, n - 1] {
                    for constant in [false, true] {
                        for phase in [0, 1, 2, 3, 4, 255] {
                            let mut r = real(mask, constant, values(n, 0));
                            let p = pauli(n | (n - 1), n - 1, phase);
                            original.coefficients = complex(&r);
                            original.rotate_signed(&p, true, false, true).unwrap();
                            assert!(r.rotate(&p, true, false, true, policy));
                            assert_eq!(bits(&complex(&r)), bits(&original.coefficients));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn four_lane_rotation_preserves_retained_complex_bits_through_rank_ten() {
        eprintln!(
            "real scalar kernel execution: arch={} avx2_fma={} ranks=2..10 strict_and_fused=true",
            std::env::consts::ARCH,
            vector_supported(),
        );
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
                "H 0\nT 0\nMX 0\n",
                policy,
            )
            .unwrap();
            let mut original = plan.prepare_sampler_with_cache_budget(0).unwrap();
            for rank in 2..=10 {
                let n = 1 << rank;
                for mask in [0, 1, 2, 3, n / 2, n - 1] {
                    for constant in [false, true] {
                        for kind in 0..4 {
                            for x in [
                                0,
                                1,
                                2,
                                3,
                                n / 2,
                                (n / 2 + 1) % n,
                                (n / 2 + 2) % n,
                                (n / 2 + 3) % n,
                                n - 1,
                            ] {
                                for z in [0, 1, 2, 3, n - 1] {
                                    let phase = if parity(x & mask) { 0 } else { 1 };
                                    let p = pauli(x, z, phase);
                                    if x == 0 && z == 0 {
                                        continue;
                                    }
                                    for (dagger, flip) in
                                        [(false, false), (true, false), (false, true)]
                                    {
                                        let mut r = real(mask, constant, values(n, kind));
                                        original.coefficients = complex(&r);
                                        original.rotate_signed(&p, false, dagger, flip).unwrap();
                                        assert!(r.rotate(&p, false, dagger, flip, policy));
                                        assert_eq!(
                                            bits(&complex(&r)),
                                            bits(&original.coefficients),
                                            "four lanes {policy:?} rank={rank} mask={mask} constant={constant} kind={kind} x={x} z={z} dagger={dagger} flip={flip}"
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
    fn real_cdf_preserves_literal_complex_reduction_bits_and_phase_aliases() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMX 0\n").unwrap();
        let mut original = plan.prepare_sampler_with_cache_budget(0).unwrap();
        for rank in 1..=7 {
            let n = 1 << rank;
            for mask in [0, 1, n - 1, n / 2, (n - 1) & 0x55] {
                for constant in [false, true] {
                    for kind in 0..4 {
                        let r = real(mask, constant, values(n, kind));
                        original.coefficients = complex(&r);
                        for x in 0..n {
                            for z in [0, 1, n - 1, (x * 13) % n] {
                                for phase in [0, 1, 2, 3, 4, 5, 254, 255] {
                                    let p = pauli(x, z, phase);
                                    assert_eq!(
                                        r.probability_zero(&p).to_bits(),
                                        original.probability_zero(&p).to_bits(),
                                        "rank={rank} mask={mask} constant={constant} kind={kind} x={x} z={z} phase={phase}"
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
    fn real_projection_preserves_component_bits_global_phase_and_rank_reset() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMX 0\n").unwrap();
        let mut original = plan.prepare_sampler_with_cache_budget(0).unwrap();
        for rank in 1..=7 {
            let n = 1 << rank;
            for mask in [0, 1, n - 1, n / 2, (n - 1) & 0x55] {
                for constant in [false, true] {
                    for kind in 0..3 {
                        for index in 0..rank {
                            for x in [0, 1 << index, (1 << index) | ((n - 1) & !(1 << index))] {
                                let y = parity(x & mask);
                                let p = pauli(x, ((x * 13) % n) | (1 << index), 0);
                                for fixed in [false, true] {
                                    let mut r = real(mask, constant, values(n, kind));
                                    original.coefficients = complex(&r);
                                    let a = original.project(&p, index, y, fixed);
                                    let b = r.project(&p, index, y, fixed);
                                    assert_eq!(a, b);
                                    if a.is_ok() {
                                        assert_eq!(
                                            bits(&complex(&r)),
                                            bits(&original.coefficients),
                                            "rank={rank} mask={mask} constant={constant} kind={kind} index={index} x={x} fixed={fixed}"
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
    fn converter_declines_omitted_negative_zero_mixed_axes_and_nonfinite_values() {
        let mut r = real(21, true, values(64, 0));
        let good = complex(&r);
        r.active = false;
        assert!(r.start(0, &good));
        let mut restored = Vec::new();
        r.write_complex(&mut restored).unwrap();
        assert_eq!(bits(&restored), bits(&good));
        for value in [-0., f64::NAN, f64::INFINITY, 0.125] {
            let mut bad = good.clone();
            bad[0].re = value; // Constant phase one: real component is omitted.
            assert!(!r.start(0, &bad));
            assert!(!r.active);
        }
        let mut zero = vec![ComplexAmp::new(0., 0.); 64];
        zero[0].re = -0.;
        assert!(r.start(0, &zero));
        r.write_complex(&mut restored).unwrap();
        assert_eq!(bits(&restored), bits(&zero));
    }

    #[test]
    fn original_cultivation_activates_real_fallback_with_default_cache_and_exact_rng() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        let text = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        );
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
            let model = RealFallback::build(&plan)
                .expect("the original compiled plan must close on real axes");
            assert!(model.reserved_bytes().unwrap() <= plan.counts_plan_budget);
            let mut baseline_plan = plan.clone();
            baseline_plan.counts_plan_budget = 0;
            assert!(RealFallback::build(&baseline_plan).is_none());
            let mut native = plan.prepare_sampler().unwrap();
            let mut original = baseline_plan.prepare_sampler().unwrap();
            let mut a = StdRng::seed_from_u64(1739);
            let mut b = a.clone();
            for shots in [0, 1, 32, 63] {
                assert_eq!(
                    native.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                    original
                        .sample_postselected_counts(shots, 0, &mut b)
                        .unwrap()
                );
                assert!(!native.real_attempted);
            }
            for shots in [64, 65, 1024, 8192] {
                assert_eq!(
                    native.sample_postselected_counts(shots, 0, &mut a).unwrap(),
                    original
                        .sample_postselected_counts(shots, 0, &mut b)
                        .unwrap()
                );
                assert!(!native.real_enabled);
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            if vector_supported() {
                let real = native
                    .real_fallback
                    .first()
                    .expect("default 64 MiB cache must exercise real scratch on AVX2/FMA");
                assert!(
                    !real.values.is_empty(),
                    "allocation alone is not an execution witness"
                );
                assert!(!real.active);
            } else {
                assert!(!native.real_attempted);
                assert!(native.real_fallback.is_empty());
            }
            let nc = native.cache.as_ref().unwrap();
            let oc = original.cache.as_ref().unwrap();
            assert_eq!(nc.reserved, oc.reserved);
            assert_eq!(nc.states.len(), oc.states.len());
            for (n, o) in nc.states.iter().zip(&oc.states) {
                assert_eq!(bits(&n.coefficients), bits(&o.coefficients));
                assert_eq!(n.next_node, o.next_node);
            }
            for shots in [0, 17, 0, 1] {
                assert_eq!(
                    native.sample(shots, &mut a).unwrap(),
                    original.sample(shots, &mut b).unwrap()
                );
                assert_eq!(
                    native.sample_measurements_u8(shots, &mut a).unwrap(),
                    original.sample_measurements_u8(shots, &mut b).unwrap()
                );
                assert!(!native.real_enabled);
                assert!(native.real_fallback.first().is_none_or(|real| !real.active));
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }

    #[test]
    fn a_new_counts_row_discards_real_scratch_left_by_materialization_failure() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        let text = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        );
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
            let mut native = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut original = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut stale = RealFallback::build(&plan).unwrap();
            // Deterministically reproduce the state left when write_complex
            // clears its destination and its subsequent allocation fails.
            // This injects stale state without attempting host-memory exhaustion.
            stale.values.resize(64, 0.);
            stale.active = true;
            native.real_fallback = vec![stale];
            native.real_attempted = true;
            native.coefficients.clear();
            // Exercise the real-row control flow even on a scalar-only host;
            // actual SIMD execution is validated separately on native x86.
            native.real_enabled = true;
            let mut a = StdRng::seed_from_u64(1739);
            let mut b = a.clone();
            let result = native
                .row_with_random_kernel::<true, false, true>(&[], &mut RowRandom::live(&mut a));
            let expected = original
                .row_with_random_kernel::<true, false, false>(&[], &mut RowRandom::live(&mut b));
            assert!(expected.is_ok());
            assert_eq!(
                result, expected,
                "new row after unmaterialized real failure: {policy:?}"
            );
            assert!(!native.real_fallback.first().unwrap().active);
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }

    #[test]
    fn optional_model_declines_budget_rank_and_incompatible_plan_without_mutation() {
        let text = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
        );
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        assert!(RealFallback::build(&plan).is_some());
        let owned = RealFallback::build_owned(&plan).unwrap();
        assert_eq!(owned.len(), 1);
        let required = owned[0]
            .reserved_bytes()
            .unwrap()
            .checked_add(size_of::<Vec<RealFallback>>())
            .unwrap()
            .checked_add((owned.capacity() - 1) * size_of::<RealFallback>())
            .unwrap();
        let mut boundary = plan.clone();
        boundary.counts_plan_budget = required - 1;
        assert!(RealFallback::build(&boundary).is_some());
        assert!(RealFallback::build_owned(&boundary).is_none());
        boundary.counts_plan_budget = required;
        let exact = RealFallback::build_owned(&boundary).unwrap();
        assert!(
            exact[0].reserved_bytes().unwrap()
                + size_of::<Vec<RealFallback>>()
                + (exact.capacity() - 1) * size_of::<RealFallback>()
                <= boundary.counts_plan_budget
        );
        boundary.counts_plan_budget = 0;
        let mut sampler = boundary.prepare_sampler().unwrap();
        sampler.real_enabled = true;
        sampler
            .coefficients
            .resize(64, ComplexAmp { re: 1., im: 0. });
        sampler.try_start_real(0);
        assert!(sampler.real_attempted);
        assert!(sampler.real_fallback.is_empty());
        assert_eq!(sampler.real_fallback.capacity(), 0);
        let mut denied = plan.clone();
        denied.counts_plan_budget = 1;
        assert!(RealFallback::build(&denied).is_none());
        assert!(RealFallback::build_owned(&denied).is_none());
        denied = plan.clone();
        denied.peak_active_rank = 11;
        assert!(RealFallback::build(&denied).is_none());
        assert!(RealFallback::build_owned(&denied).is_none());
        denied = plan.clone();
        let first = denied
            .operations
            .iter_mut()
            .find_map(|op| {
                if let PlanOp::Rotate {
                    pauli,
                    expand: true,
                    ..
                } = op
                {
                    Some(pauli)
                } else {
                    None
                }
            })
            .unwrap();
        first.x = 0;
        first.z = 1;
        assert!(RealFallback::build(&denied).is_none());
        assert!(RealFallback::build_owned(&denied).is_none());
        let original = include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d3_inject_cultivate_p1e-3.stim"
        );
        let small = CompiledNearCliffordExecutor::compile_text(original).unwrap();
        assert!(RealFallback::build(&small).is_none());
    }
}

fn parity(value: usize) -> bool {
    value.count_ones() & 1 != 0
}
fn drop_bit(value: usize, index: usize) -> usize {
    let low = (1usize << index) - 1;
    (value & low) | ((value >> (index + 1)) << index)
}

pub(super) fn vector_supported() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

#[cfg(target_arch = "x86_64")]
mod vector {
    use std::arch::x86_64::*;

    #[target_feature(enable = "avx2,fma")]
    unsafe fn permute<const LOW: usize>(v: __m256d) -> __m256d {
        match LOW {
            0 => v,
            1 => _mm256_permute4x64_pd::<0b10_11_00_01>(v),
            2 => _mm256_permute4x64_pd::<0b01_00_11_10>(v),
            3 => _mm256_permute4x64_pd::<0b00_01_10_11>(v),
            _ => unreachable!(),
        }
    }

    #[target_feature(enable = "avx2,fma")]
    unsafe fn update<const FUSED: bool>(
        own: __m256d,
        partner: __m256d,
        negate: __m256d,
        factor: __m256d,
        cosine: __m256d,
    ) -> __m256d {
        let partner = _mm256_xor_pd(partner, negate);
        let own = _mm256_mul_pd(own, cosine);
        if FUSED {
            _mm256_fmadd_pd(partner, factor, own)
        } else {
            _mm256_add_pd(own, _mm256_mul_pd(partner, factor))
        }
    }

    // The four contiguous lanes share all high index bits. Hoist the low-bit
    // sign patterns, and evaluate each remaining parity only once per group.
    #[target_feature(enable = "avx2,fma")]
    unsafe fn kernel<const FUSED: bool, const LOW: usize>(
        values: &mut [f64],
        mask: usize,
        constant: bool,
        x: usize,
        z: usize,
        imaginary: bool,
        c: f64,
        factor: f64,
    ) {
        unsafe {
            let cosine = _mm256_set1_pd(c);
            let sign = _mm256_castsi256_pd(_mm256_set1_epi64x(i64::MIN));
            let zero = _mm256_setzero_pd();
            let factor_bits =
                |lane: usize| factor.to_bits() ^ (((lane & z).count_ones() as u64 & 1) << 63);
            let factors = _mm256_castsi256_pd(_mm256_set_epi64x(
                factor_bits(3) as i64,
                factor_bits(2) as i64,
                factor_bits(1) as i64,
                factor_bits(0) as i64,
            ));
            let negate_bits = |lane: usize| {
                let target_imag = ((lane & mask).count_ones() & 1 != 0) ^ constant;
                (u64::from(imaginary && !target_imag) << 63) as i64
            };
            let negates = _mm256_castsi256_pd(_mm256_set_epi64x(
                negate_bits(3),
                negate_bits(2),
                negate_bits(1),
                negate_bits(0),
            ));
            let source_flip = if (x & z).count_ones() & 1 != 0 {
                sign
            } else {
                zero
            };
            let gauge_flip = if imaginary { sign } else { zero };
            let ptr = values.as_mut_ptr();
            if x < 4 {
                for base in (0..values.len()).step_by(4) {
                    let z_flip = _mm256_castsi256_pd(_mm256_set1_epi64x(
                        (((base & z).count_ones() as u64 & 1) << 63) as i64,
                    ));
                    let target_flip = _mm256_castsi256_pd(_mm256_set1_epi64x(
                        ((u64::from(imaginary) * ((base & mask).count_ones() as u64 & 1)) << 63)
                            as i64,
                    ));
                    let old = _mm256_loadu_pd(ptr.add(base));
                    let partner = permute::<LOW>(old);
                    let output = update::<FUSED>(
                        old,
                        partner,
                        _mm256_xor_pd(negates, target_flip),
                        _mm256_xor_pd(_mm256_xor_pd(factors, z_flip), source_flip),
                        cosine,
                    );
                    _mm256_storeu_pd(ptr.add(base), output);
                }
            } else {
                // Pair groups using the highest X bit. Each group is visited
                // once and all eight original coefficients are loaded before
                // either output is stored. The low XOR permutes within a group.
                let pivot = 1usize << (usize::BITS - 1 - x.leading_zeros());
                let other_x = x ^ pivot;
                for block in (0..values.len()).step_by(pivot * 2) {
                    for offset in (0..pivot).step_by(4) {
                        let base = block + offset;
                        let other = (base ^ x) & !3;
                        debug_assert_eq!(other, block + pivot + (offset ^ (other_x & !3)));
                        let z_flip = _mm256_castsi256_pd(_mm256_set1_epi64x(
                            (((base & z).count_ones() as u64 & 1) << 63) as i64,
                        ));
                        let target_flip = _mm256_castsi256_pd(_mm256_set1_epi64x(
                            ((u64::from(imaginary) * ((base & mask).count_ones() as u64 & 1)) << 63)
                                as i64,
                        ));
                        let fa = _mm256_xor_pd(factors, z_flip);
                        let na = _mm256_xor_pd(negates, target_flip);
                        let a = _mm256_loadu_pd(ptr.add(base));
                        let b = permute::<LOW>(_mm256_loadu_pd(ptr.add(other)));
                        let out_a =
                            update::<FUSED>(a, b, na, _mm256_xor_pd(fa, source_flip), cosine);
                        let out_b =
                            update::<FUSED>(b, a, _mm256_xor_pd(na, gauge_flip), fa, cosine);
                        _mm256_storeu_pd(ptr.add(base), out_a);
                        _mm256_storeu_pd(ptr.add(other), permute::<LOW>(out_b));
                    }
                }
            }
        }
    }

    #[target_feature(enable = "avx2,fma")]
    pub(super) unsafe fn rotate<const FUSED: bool>(
        values: &mut [f64],
        mask: usize,
        constant: bool,
        x: usize,
        z: usize,
        imaginary: bool,
        c: f64,
        factor: f64,
    ) {
        // Dispatch the lane permutation once, outside the arithmetic loops.
        unsafe {
            match x & 3 {
                0 => kernel::<FUSED, 0>(values, mask, constant, x, z, imaginary, c, factor),
                1 => kernel::<FUSED, 1>(values, mask, constant, x, z, imaginary, c, factor),
                2 => kernel::<FUSED, 2>(values, mask, constant, x, z, imaginary, c, factor),
                3 => kernel::<FUSED, 3>(values, mask, constant, x, z, imaginary, c, factor),
                _ => unreachable!(),
            }
        }
    }
}

impl RealFallback {
    pub(super) fn build_owned(plan: &CompiledNearCliffordExecutor) -> Option<Vec<Self>> {
        if !(6..=10).contains(&plan.peak_active_rank) {
            return None;
        }
        let header = size_of::<Vec<Self>>();
        if header.checked_add(size_of::<Self>())? > plan.counts_plan_budget {
            return None;
        }
        let mut owned = Vec::<Self>::new();
        owned.try_reserve_exact(1).ok()?;
        // The model budget already charges one Self. Charge the owner header
        // and any excess slot capacity before constructing the model.
        let extra = header.checked_add(
            owned
                .capacity()
                .checked_sub(1)?
                .checked_mul(size_of::<Self>())?,
        )?;
        let budget = plan.counts_plan_budget.checked_sub(extra)?;
        let model = Self::build_with_budget(plan, budget)?;
        owned.push(model);
        Some(owned)
    }

    #[cfg(test)]
    fn build(plan: &CompiledNearCliffordExecutor) -> Option<Self> {
        Self::build_with_budget(plan, plan.counts_plan_budget)
    }

    fn build_with_budget(plan: &CompiledNearCliffordExecutor, budget: usize) -> Option<Self> {
        if !(6..=10).contains(&plan.peak_active_rank) {
            return None;
        }
        let width = 1usize << plan.peak_active_rank;
        let requested = plan
            .operations
            .len()
            .checked_mul(size_of::<usize>())?
            .checked_add(width.checked_mul(2 * size_of::<f64>())?)?
            .checked_add(size_of::<Self>())?;
        if requested > budget {
            return None;
        }
        let mut result = Self {
            before: Vec::new(),
            values: Vec::new(),
            reduced: Vec::new(),
            mask: 0,
            constant: false,
            active: false,
        };
        result
            .before
            .try_reserve_exact(plan.operations.len())
            .ok()?;
        result.values.try_reserve_exact(width).ok()?;
        result.reduced.try_reserve_exact(width).ok()?;
        if result.reserved_bytes()? > budget {
            return None;
        }
        let mut rank = 0;
        let mut mask = 0;
        for op in &plan.operations {
            result.before.push(mask);
            match op {
                PlanOp::Rotate {
                    pauli: p, expand, ..
                } if p.x != 0 || p.z != 0 => {
                    let imaginary = p.physical.phase % 2 == 0;
                    if *expand {
                        if rank >= plan.peak_active_rank
                            || p.x & (1 << rank) == 0
                            || p.x >= (1 << (rank + 1))
                        {
                            return None;
                        }
                        mask |= usize::from(imaginary ^ parity(p.x & mask)) << rank;
                        rank += 1;
                    } else if parity(p.x & mask) != imaginary {
                        return None;
                    }
                    if p.x >= 1 << rank || p.z >= 1 << rank {
                        return None;
                    }
                }
                PlanOp::Measure(m) => {
                    if let Projection::Active { index, y, .. } = m.projection {
                        if index >= rank || m.pauli.x >= 1 << rank || m.pauli.z >= 1 << rank {
                            return None;
                        }
                        if m.pauli.x == 0 {
                            if mask & (1 << index) != 0 {
                                mask ^= m.pauli.z & !(1 << index);
                            }
                        } else if parity(m.pauli.x & mask) != y {
                            return None;
                        }
                        mask = drop_bit(mask, index);
                        rank -= 1;
                        if rank == 0 {
                            mask = 0;
                        }
                    }
                }
                _ => {}
            }
        }
        Some(result)
    }

    fn reserved_bytes(&self) -> Option<usize> {
        self.before
            .capacity()
            .checked_mul(size_of::<usize>())?
            .checked_add(
                self.values
                    .capacity()
                    .checked_add(self.reduced.capacity())?
                    .checked_mul(size_of::<f64>())?,
            )?
            .checked_add(size_of::<Self>())
    }

    pub(super) fn start(&mut self, node: usize, coefficients: &[ComplexAmp]) -> bool {
        let Some(&mask) = self.before.get(node) else {
            return false;
        };
        if coefficients.len() < 64
            || !coefficients.len().is_power_of_two()
            || coefficients.len() > self.values.capacity()
            || mask >= coefficients.len()
        {
            return false;
        }
        let mut constant = None;
        for (i, amp) in coefficients.iter().enumerate() {
            if !amp.re.is_finite() || !amp.im.is_finite() {
                return false;
            }
            if amp.re != 0. && amp.im != 0. {
                return false;
            }
            if amp.re != 0. || amp.im != 0. {
                let c = (amp.im != 0.) ^ parity(i & mask);
                if constant.is_some_and(|old| old != c) {
                    return false;
                }
                constant = Some(c);
            }
        }
        let constant = constant.unwrap_or(false);
        // Preserve every active signed zero. An omitted negative zero declines
        // before scratch is changed; reconstruction always writes positive zero.
        for (i, amp) in coefficients.iter().enumerate() {
            let omitted = if parity(i & mask) ^ constant {
                amp.re
            } else {
                amp.im
            };
            if omitted.to_bits() != 0 {
                return false;
            }
        }
        self.values.clear();
        self.values
            .extend(coefficients.iter().enumerate().map(|(i, amp)| {
                if parity(i & mask) ^ constant {
                    amp.im
                } else {
                    amp.re
                }
            }));
        self.mask = mask;
        self.constant = constant;
        self.active = true;
        true
    }

    pub(super) fn write_complex(&mut self, out: &mut Vec<ComplexAmp>) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        out.clear();
        out.try_reserve_exact(self.values.len())
            .map_err(|e| format!("compiled coefficient allocation failed: {e}"))?;
        out.extend(self.values.iter().enumerate().map(|(i, &v)| {
            if parity(i & self.mask) ^ self.constant {
                ComplexAmp::new(0., v)
            } else {
                ComplexAmp::new(v, 0.)
            }
        }));
        self.active = false;
        Ok(())
    }

    pub(super) fn len(&self) -> usize {
        self.values.len()
    }

    // False requests materialization and the retained complex kernel. No
    // coefficient changes precede compatibility/capacity rejection.
    pub(super) fn rotate(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        flip: bool,
        policy: CompiledRotationArithmetic,
    ) -> bool {
        let mut mask = self.mask;
        let imaginary = p.physical.phase % 2 == 0;
        let len = self.values.len();
        if expand {
            if len * 2 > self.values.capacity() || p.x & len == 0 || p.x >= len * 2 {
                return false;
            }
            mask |= usize::from(imaginary ^ parity(p.x & mask)) * len;
        } else if parity(p.x & mask) != imaginary {
            return false;
        }
        let target_len = if expand { len * 2 } else { len };
        if p.x >= target_len || p.z >= target_len {
            return false;
        }
        if expand {
            self.values.resize(target_len, 0.);
        }
        self.mask = mask;
        if policy == CompiledRotationArithmetic::Fused {
            self.rotate_kernel::<true>(p, dagger, flip);
        } else {
            self.rotate_kernel::<false>(p, dagger, flip);
        }
        true
    }

    fn rotate_kernel<const FUSED: bool>(&mut self, p: &CompactPauli, dagger: bool, flip: bool) {
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
        #[cfg(target_arch = "x86_64")]
        if self.values.len() >= 4 && vector_supported() {
            // The runtime feature guard covers every intrinsic. The slice is
            // a power-of-two vector; every four-lane group and XOR partner is
            // in bounds, with disjoint stores for the high-bit paired case.
            unsafe {
                vector::rotate::<FUSED>(
                    &mut self.values,
                    self.mask,
                    self.constant,
                    p.x,
                    p.z,
                    imaginary,
                    c,
                    factor,
                );
            }
            return;
        }
        let mask = self.mask;
        let constant = self.constant;
        let update = |own: f64, partner: f64, target: usize, source: usize| {
            let f = if parity(source & p.z) {
                -factor
            } else {
                factor
            };
            let partner = if imaginary && !(parity(target & mask) ^ constant) {
                -partner
            } else {
                partner
            };
            if FUSED {
                partner.mul_add(f, own * c)
            } else {
                own * c + partner * f
            }
        };
        if p.x == 0 {
            for (i, a) in self.values.iter_mut().enumerate() {
                *a = update(*a, *a, i, i);
            }
        } else {
            let pivot = 1 << p.x.trailing_zeros();
            for block in (0..self.values.len()).step_by(pivot * 2) {
                let other = block ^ p.x;
                let (a, b) = if block < other {
                    let (left, right) = self.values.split_at_mut(other);
                    (&mut left[block..block + pivot], &mut right[..pivot])
                } else {
                    let (left, right) = self.values.split_at_mut(block);
                    (&mut right[..pivot], &mut left[other..other + pivot])
                };
                for (offset, (a, b)) in a.iter_mut().zip(b).enumerate() {
                    let (old_a, old_b) = (*a, *b);
                    *a = update(old_a, old_b, block + offset, other + offset);
                    *b = update(old_b, old_a, other + offset, block + offset);
                }
            }
        }
    }

    pub(super) fn probability_zero(&self, p: &CompactPauli) -> f64 {
        // Nonfinite values require the literal complex expression (including
        // its zero*infinity terms). This guard also covers rotation overflow.
        if self.values.iter().any(|v| !v.is_finite()) {
            let amp = |i: usize| {
                if parity(i & self.mask) ^ self.constant {
                    ComplexAmp::new(0., self.values[i])
                } else {
                    ComplexAmp::new(self.values[i], 0.)
                }
            };
            let mut expectation = 0.;
            let mut norm = 0.;
            for i in 0..self.values.len() {
                let a = amp(i);
                let sign = if parity(i & p.z) { -1. } else { 1. };
                expectation += (amp(i ^ p.x).conj() * i_pow(p.physical.phase) * (a * sign)).re;
                norm += a.norm_sqr();
            }
            return ((1. + expectation / norm) * 0.5).clamp(0., 1.);
        }
        let phase = p.physical.phase & 3;
        let phase_imag = phase & 1 != 0;
        let phase_sign = if phase < 2 { 1. } else { -1. };
        let mut expectation = 0.;
        let mut norm = 0.;
        for (i, &a) in self.values.iter().enumerate() {
            let j = i ^ p.x;
            let target_imag = parity(i & self.mask) ^ self.constant;
            let partner_imag = parity(j & self.mask) ^ self.constant;
            let mut term = 0.;
            if partner_imag ^ phase_imag == target_imag {
                let b = if !phase_imag && partner_imag {
                    -self.values[j]
                } else {
                    self.values[j]
                };
                let sign = if parity(i & p.z) { -1. } else { 1. };
                term = (b * phase_sign) * (a * sign);
                if target_imag {
                    term = -term;
                }
            }
            expectation += term;
            norm += a * a;
        }
        ((1. + expectation / norm) * 0.5).clamp(0., 1.)
    }

    pub(super) fn project(
        &mut self,
        p: &CompactPauli,
        index: usize,
        y: bool,
        fixed: bool,
    ) -> Result<(), String> {
        let pivot = 1 << index;
        let low = pivot - 1;
        let other = p.z & !pivot;
        self.reduced.clear();
        let mut norm = 0.;
        for i in 0..self.values.len() / 2 {
            let without = (i & low) | ((i & !low) << 1);
            let value = if p.x == 0 {
                let bit = fixed ^ parity(without & other);
                self.values[without | (usize::from(bit) << index)]
            } else {
                let a = self.values[without];
                let mut b = self.values[without ^ p.x];
                if y && (parity(without & self.mask) ^ self.constant) {
                    b = -b;
                }
                let sign = if fixed ^ parity(without & other) {
                    -1.
                } else {
                    1.
                };
                (a + b * sign) * std::f64::consts::FRAC_1_SQRT_2
            };
            norm += value * value;
            self.reduced.push(value);
        }
        if !norm.is_finite() || norm <= 0. {
            return Err("compiled near-Clifford zero-probability measurement".into());
        }
        let scale = 1. / norm.sqrt();
        for value in &mut self.reduced {
            *value *= scale;
        }
        std::mem::swap(&mut self.values, &mut self.reduced);
        if p.x == 0 && self.mask & pivot != 0 {
            self.mask ^= other;
            self.constant ^= fixed;
        }
        self.mask = drop_bit(self.mask, index);
        if self.values.len() == 1 {
            self.values[0] = 1.;
            self.mask = 0;
            self.constant = false;
        }
        Ok(())
    }
}
