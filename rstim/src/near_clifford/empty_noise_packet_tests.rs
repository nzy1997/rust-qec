use super::*;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

const TEXT: &str = "X_ERROR(0) 0\nH 2\nM 2\nDEPOLARIZE1(0.2) 0\nZ_ERROR(0.3) 4\nDEPOLARIZE2(0.25) 0 1\nX_ERROR(0.4) 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\nH 3\nT 3\nMX 3\nH 10 11 12 13 14 15 16 17\nM 10 11 12 13 14 15 16 17\n";

#[test]
fn empty_noise_packets_preserve_ordered_columns_and_sidecar_ordinals() {
    let plan = CompiledNearCliffordExecutor::compile_text(TEXT).unwrap();
    let model = ActiveFreePredicate::build(&plan, 8 * 1024 * 1024, 16_000_000).unwrap();
    assert!(plan.random_kinds.iter().zip(&model.inputs).any(
        |(kind, input)| matches!(kind, RandomKind::Noise { probability, .. } if *probability == 0.)
            && input.contributes
    ));
    assert!(
        plan.random_kinds
            .iter()
            .any(|kind| matches!(kind, RandomKind::Active))
    );
    let mut cases = 0;
    for lanes in [1, 2, 31, 32, 63, 64] {
        let all = if lanes == 64 {
            u64::MAX
        } else {
            (1u64 << lanes) - 1
        };
        // First all noise events are empty, then each ordinal alone is empty.
        // Other ordinals stay nonempty, catching accidental sidecar reuse.
        for empty in 0..=plan.noise_event_count {
            let mut tape = vec![0; lanes * plan.random_kinds.len()];
            let mut hits = vec![0; plan.noise_event_count];
            let mut noise = NoisePacket::new(
                &plan.random_kinds,
                plan.noise_event_count,
                PACKET_BYTE_BUDGET,
            )
            .unwrap();
            let mut independent =
                IndependentPacket::new(plan.independent_event_count, PACKET_BYTE_BUDGET).unwrap();
            for lane in 0..lanes {
                let mut n = 0;
                let mut i = 0;
                let row = independent.row(lane);
                for (event, kind) in plan.random_kinds.iter().enumerate() {
                    let draw = match *kind {
                        RandomKind::Noise { choices, .. } => {
                            let draw = if empty == 0 || n == empty - 1 {
                                0
                            } else {
                                ((lane + n) % choices + 1) as u64
                            };
                            if draw != 0 {
                                hits[n] |= 1u64 << lane;
                            }
                            for bit in 0..4 {
                                noise.planes()[n][bit] |= ((draw >> bit) & 1) << lane;
                            }
                            n += 1;
                            draw
                        }
                        RandomKind::Independent => {
                            let draw = ((lane + i) % 2) as u64;
                            row[i / 64] |= draw << (i % 64);
                            i += 1;
                            draw
                        }
                        RandomKind::Active => 0.375f64.to_bits(),
                    };
                    tape[lane * plan.random_kinds.len() + event] = draw;
                }
            }
            independent.transpose(lanes);
            // Inactive tail bits must not turn an empty valid-lane event into
            // an emitted category, including when retained tape cells are stale.
            for selected in &mut hits {
                *selected |= !all;
            }
            let mut expected = vec![];
            for (event, kind) in plan.random_kinds.iter().enumerate() {
                if matches!(kind, RandomKind::Active) {
                    continue;
                }
                let input = model.inputs[event];
                for category in 0..input.count {
                    let effect = model.column(input.first + category).unwrap();
                    let mut selected = 0;
                    for lane in 0..lanes {
                        let draw = tape[lane * plan.random_kinds.len() + event];
                        let chosen = match kind {
                            RandomKind::Noise { .. } => draw == (category + 1) as u64,
                            RandomKind::Independent => draw != 0,
                            RandomKind::Active => false,
                        };
                        if chosen {
                            selected |= 1u64 << lane;
                        }
                    }
                    if selected != 0 && effect.iter().any(|&word| word != 0) {
                        expected.push((effect.to_vec(), selected));
                    }
                }
            }
            for use_noise in [false, true] {
                for use_independent in [false, true] {
                    let mut stored = tape.clone();
                    for lane in 0..lanes {
                        for (event, kind) in plan.random_kinds.iter().enumerate() {
                            if use_noise && matches!(kind, RandomKind::Noise { .. })
                                || use_independent && matches!(kind, RandomKind::Independent)
                            {
                                stored[lane * plan.random_kinds.len() + event] = u64::MAX;
                            }
                        }
                    }
                    let mut columns = PacketColumns {
                        model: &model,
                        plan: &plan,
                        tape: &stored,
                        hits: &hits,
                        noise: use_noise.then_some(&noise),
                        independent: use_independent.then_some(&independent),
                        lanes,
                        all,
                        event: 0,
                        noise_event: 0,
                        independent_event: 0,
                        first: 0,
                        count: 0,
                        category: 0,
                        contributing_categories: 0,
                        masks: [u64::MAX; 15],
                    };
                    let actual: Vec<_> = columns
                        .by_ref()
                        .map(|(effect, selected)| (effect.to_vec(), selected))
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "lanes={lanes}, empty={empty}, compact_noise={use_noise}, compact_independent={use_independent}"
                    );
                    assert_eq!(columns.noise_event, plan.noise_event_count);
                    assert_eq!(columns.independent_event, plan.independent_event_count);
                    assert!(columns.next().is_none());
                    for sweep in [&[][..], &[true][..]] {
                        let mut expected_mask = 0;
                        for lane in 0..lanes {
                            let mut parity = model.column(0).unwrap().to_vec();
                            for (effect, selected) in &expected {
                                if selected & (1u64 << lane) != 0 {
                                    for (to, from) in parity.iter_mut().zip(effect) {
                                        *to ^= from;
                                    }
                                }
                            }
                            for (slot, index) in model.sweeps.iter().enumerate() {
                                if sweep.get(*index as usize).copied().unwrap_or(false) {
                                    for (to, from) in parity
                                        .iter_mut()
                                        .zip(model.column(model.sweep_first + slot).unwrap())
                                    {
                                        *to ^= from;
                                    }
                                }
                            }
                            if parity.iter().any(|&word| word != 0) {
                                expected_mask |= 1u64 << lane;
                            }
                        }
                        assert_eq!(
                            model.packet_rejection_mask(
                                &plan,
                                lanes,
                                sweep,
                                &stored,
                                &hits,
                                use_noise.then_some(&noise),
                                use_independent.then_some(&independent),
                                &mut vec![],
                                PACKET_BYTE_BUDGET,
                                16_000_000
                            ),
                            Some(expected_mask)
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    println!("EMPTY_NOISE_PACKET_MASK_CASES {cases}");
}

#[test]
fn empty_noise_packets_match_physical_fallback_and_continued_rng() {
    let mut cases = 0;
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(TEXT, arithmetic).unwrap();
        for cache in [0, 8192] {
            let mut actual = plan
                .prepare_eager_postselected_sampler_with_cache_budget(cache)
                .unwrap();
            let mut physical = plan
                .prepare_eager_postselected_sampler_with_cache_budget(cache)
                .unwrap();
            physical.predicate.set(None).unwrap();
            let mut a = SmallRng::seed_from_u64(0xe017_2026_1024_0001 + cache as u64);
            let mut b = a.clone();
            for shots in [1, 31, 64, 67] {
                for sweep in [&[][..], &[true][..]] {
                    assert_eq!(
                        actual
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut a)
                            .unwrap(),
                        physical
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut b)
                            .unwrap()
                    );
                    if cache != 0 && shots == 64 {
                        let model = actual.predicate.get().and_then(Option::as_ref).unwrap();
                        assert!(model.inputs.iter().any(|input| input.contributes));
                        assert_ne!(actual.sampler.last_eager_rejected_mask, 0);
                    }
                    assert_eq!(
                        actual.sample_measurements_u8(3, &mut a).unwrap(),
                        physical.sample_measurements_u8(3, &mut b).unwrap()
                    );
                    assert_eq!(
                        (0..16).map(|_| a.next_u64()).collect::<Vec<_>>(),
                        (0..16).map(|_| b.next_u64()).collect::<Vec<_>>()
                    );
                    cases += 1;
                }
            }
        }
    }
    println!("EMPTY_NOISE_PACKET_CONTINUED_STREAM_CASES {cases}");
}
