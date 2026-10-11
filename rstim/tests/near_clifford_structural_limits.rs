use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordPostselectedCounts,
};

#[path = "support/near_clifford_structural.rs"]
mod structural;

#[test]
fn feasible_rank_five_order_preserves_records_counts_and_rng_at_the_caller_limit() {
    for text in [
        structural::circuit(16, 8, false),
        structural::circuit(16, 8, false)
            .replace("T ", "R_Z(0.17) ")
            .replace("MPP ", "MPP(0.23) "),
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let compile = |limit| {
                CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
                    rstim::parser::parse_lines(&text).unwrap(),
                    limit,
                    arithmetic,
                )
            };
            let reference = compile(16).unwrap();
            let bounded = compile(5).unwrap();
            assert_eq!(reference.peak_active_rank(), 5);
            assert_eq!(bounded.peak_active_rank(), 5);
            assert!(compile(4).unwrap_err().contains("active-rank limit"));
            for budget in [0, 64 * 1024 * 1024] {
                let mut scalar = reference.prepare_sampler_with_cache_budget(0).unwrap();
                let mut candidate = bounded.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut a = StdRng::seed_from_u64(5729);
                let mut b = a.clone();
                for shots in [0, 1, 31, 32, 65, 129, 17] {
                    let expected = scalar.sample(shots, &mut a).unwrap();
                    assert_eq!(candidate.sample(shots, &mut b).unwrap(), expected);
                    let rows = scalar.sample(shots, &mut a).unwrap();
                    let accepted: Vec<_> = rows
                        .iter()
                        .filter(|s| s.detectors.iter().all(|&bit| !bit))
                        .collect();
                    let expected_counts = NearCliffordPostselectedCounts {
                        attempted: shots,
                        accepted: accepted.len(),
                        logical_errors: accepted
                            .iter()
                            .filter(|s| {
                                s.observables
                                    .iter()
                                    .filter(|(index, _)| *index == 0)
                                    .fold(false, |parity, (_, bit)| parity ^ bit)
                            })
                            .count(),
                    };
                    assert_eq!(
                        candidate
                            .sample_postselected_counts(shots, 0, &mut b)
                            .unwrap(),
                        expected_counts
                    );
                    assert_eq!(
                        candidate.sample_measurements_u8(shots, &mut b).unwrap(),
                        scalar.sample_measurements_u8(shots, &mut a).unwrap()
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }
}
