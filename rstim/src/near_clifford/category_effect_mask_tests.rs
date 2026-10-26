use super::*;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

const TEXT: &str = "X 0\nDEPOLARIZE1(0.25) 0\nH 1\nM 1\nZ_ERROR(0.2) 0\nDEPOLARIZE2(0.3) 0 2\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\nH 3\nT 3\nMX 3\nH 10 11 12 13 14 15 16 17\nM 10 11 12 13 14 15 16 17\n";

#[test]
fn category_effect_flags_match_complete_columns_with_exact_quota() {
    // The extra private mask must fit the original Input payload, including
    // on targets with a different usize width.
    assert_eq!(size_of::<Input>(), 3 * size_of::<usize>());
    let mut cases = 0;
    for words in [0usize, 1, 2, 3] {
        for count in [1usize, 2, 3, 15] {
            for pattern in 0..(1usize << count) {
                let mut columns = vec![0; (count + 2) * words];
                let mut expected = 0u16;
                for category in 0..count {
                    if words != 0 && pattern & (1 << category) != 0 {
                        columns[(category + 1) * words + category % words] =
                            1u64 << (category * 7 % 64);
                        expected |= 1 << category;
                    }
                }
                let input = Input {
                    first: 1,
                    count,
                    ..Input::default()
                };
                let cost = count * words + 1;
                let mut quota = Quota {
                    bytes: 0,
                    limit: 0,
                    work: cost,
                };
                let mut inputs = [input];
                ActiveFreePredicate::mark_contributing_inputs(
                    &mut inputs,
                    words,
                    &columns,
                    &mut quota,
                )
                .unwrap();
                assert_eq!(inputs[0].contributing_categories, expected);
                assert_eq!(inputs[0].contributes, expected != 0);
                assert_eq!((quota.work, quota.bytes), (0, 0));
                cases += 1;
            }
        }
    }
    for (input, words, columns, work) in [
        (
            Input {
                first: usize::MAX,
                count: 2,
                ..Input::default()
            },
            2,
            vec![0; 6],
            usize::MAX,
        ),
        (
            Input {
                first: 1,
                count: usize::MAX,
                ..Input::default()
            },
            2,
            vec![0; 6],
            usize::MAX,
        ),
        (
            Input {
                first: 1,
                count: 3,
                ..Input::default()
            },
            2,
            vec![0; 6],
            usize::MAX,
        ),
        (
            Input {
                first: 0,
                count: 16,
                ..Input::default()
            },
            0,
            vec![],
            1,
        ),
        (
            Input {
                first: 1,
                count: 2,
                ..Input::default()
            },
            2,
            vec![0; 6],
            4,
        ),
    ] {
        let mut quota = Quota {
            bytes: 0,
            limit: 0,
            work,
        };
        assert!(
            ActiveFreePredicate::mark_contributing_inputs(
                &mut [input],
                words,
                &columns,
                &mut quota
            )
            .is_none()
        );
    }
    println!("CATEGORY_EFFECT_FLAG_CASES {cases}; REFUSALS 5");
}

fn scalar_mask(
    model: &ActiveFreePredicate,
    plan: &CompiledNearCliffordExecutor,
    tape: &[u64],
    lanes: usize,
    sweep: &[bool],
) -> u64 {
    let mut rejected = 0;
    for lane in 0..lanes {
        let mut parity = model.column(0).unwrap().to_vec();
        for (event, kind) in plan.random_kinds.iter().enumerate() {
            let draw = tape[lane * plan.random_kinds.len() + event] as usize;
            let variable = match kind {
                RandomKind::Noise { .. } if draw != 0 => Some(model.inputs[event].first + draw - 1),
                RandomKind::Independent if draw != 0 => Some(model.inputs[event].first),
                _ => None,
            };
            if let Some(variable) = variable {
                for (to, from) in parity.iter_mut().zip(model.column(variable).unwrap()) {
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
        if parity.iter().any(|word| *word != 0) {
            rejected |= 1u64 << lane;
        }
    }
    rejected
}

#[test]
fn category_effect_masks_match_scalar_for_mixed_categories_and_sidecars() {
    let plan = CompiledNearCliffordExecutor::compile_text(TEXT).unwrap();
    let model = ActiveFreePredicate::build(&plan, 8 * 1024 * 1024, 16_000_000).unwrap();
    assert!(model.inputs.iter().any(|input| input.contributes
        && input.contributing_categories.count_ones() < input.count as u32));
    let mut cases = 0;
    for lanes in [1, 3, 30, 33, 62, 64] {
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
                        let draw = ((lane + event * 3) % (choices + 1)) as u64;
                        if draw != 0 {
                            hits[n] |= 1 << lane;
                        }
                        for bit in 0..4 {
                            noise.planes()[n][bit] |= ((draw >> bit) & 1) << lane;
                        }
                        n += 1;
                        draw
                    }
                    RandomKind::Independent => {
                        let draw = ((lane + event) % 2) as u64;
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
                // Construct the ordered emissions from literal completed tape
                // categories and full immutable columns, without cached bits.
                let mut expected = vec![];
                for (event, kind) in plan.random_kinds.iter().enumerate() {
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
                        if selected != 0 && effect.iter().any(|word| *word != 0) {
                            expected.push((effect.to_vec(), selected));
                        }
                    }
                }
                let all = if lanes == 64 {
                    u64::MAX
                } else {
                    (1u64 << lanes) - 1
                };
                let actual: Vec<_> = PacketColumns {
                    model: &model,
                    plan: &plan,
                    tape: &stored,
                    hits: &hits,
                    noise: use_noise.then_some(&noise),
                    independent: use_independent.then_some(&independent),
                    lanes,
                    all,
                    event: 0,
                    first: 0,
                    count: 0,
                    category: 0,
                    contributing_categories: 0,
                    masks: [0; 15],
                }
                .map(|(effect, selected)| (effect.to_vec(), selected))
                .collect();
                assert_eq!(actual, expected);
                for sweep in [&[][..], &[false][..], &[true][..]] {
                    let mut scratch = vec![];
                    assert_eq!(
                        model.packet_rejection_mask(
                            &plan,
                            lanes,
                            sweep,
                            &stored,
                            &hits,
                            use_noise.then_some(&noise),
                            use_independent.then_some(&independent),
                            &mut scratch,
                            PACKET_BYTE_BUDGET,
                            16_000_000
                        ),
                        Some(scalar_mask(&model, &plan, &tape, lanes, sweep))
                    );
                    cases += 1;
                }
            }
        }
    }
    let tape = vec![0; plan.random_kinds.len()];
    let hits = vec![0; plan.noise_event_count];
    for (lanes, stored, selected, bytes, work) in [
        (
            0,
            tape.as_slice(),
            hits.as_slice(),
            PACKET_BYTE_BUDGET,
            16_000_000,
        ),
        (
            65,
            tape.as_slice(),
            hits.as_slice(),
            PACKET_BYTE_BUDGET,
            16_000_000,
        ),
        (1, &[][..], hits.as_slice(), PACKET_BYTE_BUDGET, 16_000_000),
        (1, tape.as_slice(), &[][..], PACKET_BYTE_BUDGET, 16_000_000),
        (1, tape.as_slice(), hits.as_slice(), 0, 16_000_000),
        (1, tape.as_slice(), hits.as_slice(), PACKET_BYTE_BUDGET, 0),
    ] {
        assert!(
            model
                .packet_rejection_mask(
                    &plan,
                    lanes,
                    &[],
                    stored,
                    selected,
                    None,
                    None,
                    &mut vec![],
                    bytes,
                    work
                )
                .is_none()
        );
    }
    println!("CATEGORY_EFFECT_MASK_CASES {cases}");
}

#[test]
fn category_effect_success_streams_match_full_category_traversal() {
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
            let mut reference = plan
                .prepare_eager_postselected_sampler_with_cache_budget(cache)
                .unwrap();
            let mut full_model =
                ActiveFreePredicate::build(&plan, 8 * 1024 * 1024, 16_000_000).unwrap();
            for input in &mut full_model.inputs {
                input.contributes = true;
                input.contributing_categories = (1u16 << input.count) - 1;
            }
            full_model.packet_inputs = ActiveFreePredicate::index_packet_inputs(
                &full_model.inputs,
                &plan.random_kinds,
                &mut Quota {
                    bytes: 0,
                    limit: usize::MAX,
                    work: usize::MAX,
                },
            )
            .unwrap();
            reference.predicate.set(Some(full_model)).unwrap();
            let mut a = SmallRng::seed_from_u64(0xca7e_2026_1023_0001 + cache as u64);
            let mut b = a.clone();
            for shots in [1, 31, 64, 66] {
                for sweep in [&[][..], &[true][..]] {
                    assert_eq!(
                        actual
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut a)
                            .unwrap(),
                        reference
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut b)
                            .unwrap()
                    );
                    if shots >= 32 {
                        let predicate = actual.predicate.get().and_then(Option::as_ref).unwrap();
                        assert!(predicate.inputs.iter().any(|input| input.contributes
                            && input.contributing_categories.count_ones() < input.count as u32));
                        if cache != 0 && shots == 64 {
                            assert_ne!(
                                actual.sampler.last_eager_rejected_mask, 0,
                                "the packed eager predicate must reject at least one lane"
                            );
                        }
                    }
                    assert_eq!(
                        actual.sample_measurements_u8(3, &mut a).unwrap(),
                        reference.sample_measurements_u8(3, &mut b).unwrap()
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
    println!("CATEGORY_EFFECT_SUCCESS_STREAM_CASES {cases}");
}
