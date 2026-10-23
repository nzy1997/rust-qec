use super::*;
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use std::io::Write;

struct NoCallerDraw;
impl RngCore for NoCallerDraw {
    fn next_u32(&mut self) -> u32 {
        panic!("recorded reference requested caller RNG")
    }
    fn next_u64(&mut self) -> u64 {
        panic!("recorded reference requested caller RNG")
    }
    fn fill_bytes(&mut self, _: &mut [u8]) {
        panic!("recorded reference requested caller RNG")
    }
    fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand::Error> {
        panic!("recorded reference requested caller RNG")
    }
}
fn emit(tag: &str, value: serde_json::Value) {
    println!("{tag} {value}");
    std::io::stdout().flush().unwrap();
}
fn fold_rows(rows: &[NearCliffordShot], observable: u32) -> NearCliffordPostselectedCounts {
    let mut out = NearCliffordPostselectedCounts {
        attempted: rows.len(),
        ..Default::default()
    };
    for row in rows {
        if row.detectors.iter().any(|v| *v) {
            continue;
        }
        out.accepted += 1;
        let logical = row
            .observables
            .iter()
            .filter(|(id, _)| *id == observable)
            .fold(false, |a, (_, b)| a ^ b);
        out.logical_errors += usize::from(logical);
    }
    out
}

#[test]
fn eager_original_packets_match_each_recorded_quantum_row_and_rng() {
    for (workload, text, seed) in [
        (
            "distillation",
            include_str!(
                "../../../benchmarks/near_clifford/application_counts/fixtures/distillation.stim"
            ),
            0x5ea6_0000_0000_0001u64,
        ),
        (
            "msc_d5",
            include_str!(
                "../../../benchmarks/near_clifford/application_counts/fixtures/msc_d5_inject_cultivate_p1e-3.stim"
            ),
            0x5ea6_0000_0000_0002u64,
        ),
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let mut eager = plan.prepare_eager_postselected_sampler().unwrap();
            eager.sampler.observe_lazy_error_producer = true;
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut frozen = rng.clone();
            let tapes = scalar_prepared_tape_tests::draw_frozen_packet_rows(
                &plan.random_kinds,
                64,
                &mut frozen,
            );
            let cell = serde_json::json!({"workload":workload,"arithmetic":format!("{arithmetic:?}"),"seed":seed,"shots":64});
            emit("EAGER_BEGIN", cell.clone());
            let result = eager.sample_postselected_counts(64, 0, &mut rng);
            let observed = eager.sampler.last_packet_error_producer.as_ref();
            emit(
                "EAGER_PRODUCER",
                serde_json::json!({"cell":cell,"tapes":observed.map(|o|&o.rows),
                "lanes":observed.map(|o|o.lanes),"coherent":observed.map(|o|o.coherent),
                "noise_admitted":observed.map(|o|o.noise_admitted),"rejected_mask":eager.sampler.last_eager_rejected_mask,
                "result":result.as_ref().map(|r|serde_json::json!({"attempted":r.attempted,"accepted":r.accepted,"logical_errors":r.logical_errors})).map_err(|s|s.as_str())}),
            );
            let actual = result.unwrap();
            let observed = observed.unwrap();
            assert_eq!(observed.rows, tapes);
            assert_eq!(observed.lanes, 64);
            assert!(eager.predicate.get().unwrap().is_some());
            let reject = eager.sampler.last_eager_rejected_mask;
            assert_ne!(
                reject, 0,
                "new eager path must actually mark original-circuit lanes"
            );
            let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut rows = Vec::new();
            for (lane, tape) in tapes.iter().enumerate() {
                emit(
                    "EAGER_REFERENCE_BEGIN",
                    serde_json::json!({"cell":cell,"lane":lane}),
                );
                let mut no_rng = NoCallerDraw;
                let mut recorded = RowRandom::recorded(tape, &mut no_rng);
                let row = reference.row_with_random(&[], &mut recorded).unwrap();
                emit(
                    "EAGER_REFERENCE_ROW",
                    serde_json::json!({"cell":cell,"lane":lane,
                    "cursor":recorded.cursor,"proposed_reject":reject>>lane&1!=0,
                    "measurements":row.measurements,"detectors":row.detectors,"observables":row.observables}),
                );
                assert_eq!(recorded.cursor, plan.random_kinds.len());
                if reject >> lane & 1 != 0 {
                    assert!(
                        row.detectors.iter().any(|v| *v),
                        "{workload}/{arithmetic:?}/lane{lane}"
                    );
                }
                rows.push(row);
            }
            let expected = fold_rows(&rows, 0);
            assert_eq!(actual, expected);
            let a: Vec<_> = (0..16).map(|_| rng.next_u64()).collect();
            let b: Vec<_> = (0..16).map(|_| frozen.next_u64()).collect();
            emit(
                "EAGER_CELL_RESULT",
                serde_json::json!({"cell":cell,"rejected_mask":reject,
                "actual":{"attempted":actual.attempted,"accepted":actual.accepted,"logical_errors":actual.logical_errors},
                "reference":{"attempted":expected.attempted,"accepted":expected.accepted,"logical_errors":expected.logical_errors},
                "actual_next16":a,"frozen_next16":b}),
            );
            assert_eq!(a, b);
        }
    }
}

#[test]
fn eager_tail_mixed_output_zero_and_optional_decline_keep_success_streams() {
    let text =
        "X_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\nH 1\nT 1\nMX 1\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        for cache in [0, DEFAULT_CACHE_BYTE_BUDGET] {
            let mut eager = plan
                .prepare_eager_postselected_sampler_with_cache_budget(cache)
                .unwrap();
            let mut original = plan.prepare_sampler_with_cache_budget(cache).unwrap();
            let mut a = SmallRng::seed_from_u64(0x5ea6_1000 + cache as u64);
            let mut b = a.clone();
            assert!(eager.sample_postselected_counts(0, 7, &mut a).is_err());
            assert!(eager.predicate.get().is_none());
            assert_eq!(
                eager
                    .sample_postselected_counts(0, 0, &mut a)
                    .unwrap()
                    .attempted,
                0
            );
            assert!(eager.predicate.get().is_none());
            for shots in [1, 17, 32, 63, 64, 65] {
                let expected = original
                    .sample_postselected_counts(shots, 0, &mut b)
                    .unwrap();
                let actual = eager.sample_postselected_counts(shots, 0, &mut a).unwrap();
                assert_eq!(actual, expected, "{arithmetic:?}/cache{cache}/shots{shots}");
                assert_eq!(a.clone().next_u64(), b.clone().next_u64());
                assert_eq!(
                    eager.sample_measurements_u8(3, &mut a).unwrap(),
                    original.sample_measurements_u8(3, &mut b).unwrap()
                );
                assert_eq!(a.clone().next_u64(), b.clone().next_u64());
            }
        }
        let mut declined = plan.clone();
        declined.counts_plan_budget = 0;
        let mut a = SmallRng::seed_from_u64(0x5ea6_2000);
        let mut b = a.clone();
        let mut eager = declined.prepare_eager_postselected_sampler().unwrap();
        let mut original = declined.prepare_sampler().unwrap();
        assert_eq!(
            eager.sample_postselected_counts(64, 0, &mut a).unwrap(),
            original.sample_postselected_counts(64, 0, &mut b).unwrap()
        );
        assert!(eager.predicate.get().unwrap().is_none());
        assert_eq!(a.next_u64(), b.next_u64());
    }
    emit(
        "EAGER_CONTROL_RESULT",
        serde_json::json!({"tail_sizes":[1,17,32,63,64,65],"cache_budgets":[0,DEFAULT_CACHE_BYTE_BUDGET],"modes":["Strict","Fused"],"zero_and_missing_observable":true,"mixed_count_records":true,"quota_decline":true,"performance_or_SOTA":false}),
    );
}
