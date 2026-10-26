use super::*;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

const TEXT: &str = "Z_ERROR(0.2) 6\nH 7\nM 7\nX 0\nDEPOLARIZE1(0.25) 0\nH 1\nM 1\nZ_ERROR(0.2) 0\nDEPOLARIZE2(0.3) 0 2\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\nH 3\nT 3\nMX 3\nX_ERROR(0.4) 8\nH 10 11 12 13 14 15 16 17\nM 10 11 12 13 14 15 16 17\n";

#[test]
fn contributing_event_index_preserves_typed_prefixes_with_exact_quota() {
    let noise = RandomKind::Noise {
        probability: 0.2,
        choices: 3,
    };
    let kinds = [
        noise,
        RandomKind::Independent,
        RandomKind::Active,
        noise,
        RandomKind::Independent,
        noise,
        RandomKind::Active,
        RandomKind::Independent,
        noise,
    ];
    // Literal typed prefix ordinals, including leading, internal and trailing gaps.
    let ordinals = [0, 0, 0, 1, 1, 2, 0, 2, 3];
    let mut cases = 0;
    for flags in 0..(1usize << kinds.len()) {
        let inputs: Vec<_> = (0..kinds.len())
            .map(|event| Input {
                contributes: flags & (1 << event) != 0,
                ..Input::default()
            })
            .collect();
        let expected: Vec<_> = ordinals
            .iter()
            .enumerate()
            .filter_map(|(event, &ordinal)| {
                (flags & (1 << event) != 0).then_some(PacketInput { event, ordinal })
            })
            .collect();
        let bytes = size_of::<Vec<PacketInput>>() + expected.len() * size_of::<PacketInput>();
        let work = 2 * inputs.len() + expected.len();
        let mut quota = Quota {
            bytes: 0,
            limit: bytes,
            work,
        };
        let indexed =
            ActiveFreePredicate::index_packet_inputs(&inputs, &kinds, &mut quota).unwrap();
        assert_eq!(indexed, expected);
        assert_eq!((quota.bytes, quota.work), (bytes, 0));
        for (limit, available_work) in [(bytes - 1, work), (bytes, work - 1)] {
            assert!(
                ActiveFreePredicate::index_packet_inputs(
                    &inputs,
                    &kinds,
                    &mut Quota {
                        bytes: 0,
                        limit,
                        work: available_work
                    }
                )
                .is_none()
            );
        }
        cases += 1;
    }
    let mut quota = Quota {
        bytes: 0,
        limit: usize::MAX,
        work: usize::MAX,
    };
    assert!(ActiveFreePredicate::index_packet_inputs(&[], &kinds, &mut quota).is_none());
    assert_eq!((quota.bytes, quota.work), (0, usize::MAX));
    let empty = ActiveFreePredicate::index_packet_inputs(&[], &[], &mut quota).unwrap();
    assert!(empty.is_empty());
    assert_eq!(quota.bytes, size_of::<Vec<PacketInput>>());
    println!(
        "CONTRIBUTING_EVENT_INDEX_QUOTA_CASES {cases}; REFUSALS {}",
        cases * 2 + 1
    );
}

#[test]
fn contributing_event_index_matches_full_ordered_tape_and_scalar_parities() {
    let mut cases = 0;
    let mut omitted_kind_refusals = 0;
    for choices in 1..=15 {
        let mut plan = CompiledNearCliffordExecutor::compile_text(TEXT).unwrap();
        // Exercise every private supported category width, not just public channels.
        // The model/packet inputs below are synthetic; no plan execution is claimed.
        plan.random_kinds = (0..45)
            .map(|event| match event % 5 {
                0 | 3 => RandomKind::Noise {
                    probability: 0.2,
                    choices,
                },
                1 | 4 => RandomKind::Independent,
                _ => RandomKind::Active,
            })
            .collect();
        plan.noise_event_count = 18;
        plan.independent_event_count = 18;
        let mut variable = 1;
        let inputs: Vec<_> = plan
            .random_kinds
            .iter()
            .enumerate()
            .map(|(event, kind)| {
                let count = match kind {
                    RandomKind::Noise { choices, .. } => *choices,
                    _ => 1,
                };
                let contributes = (5..40).contains(&event)
                    && event % 3 != 0
                    && !matches!(kind, RandomKind::Active);
                let categories = if contributes {
                    (0..count)
                        .filter(|c| (event + c) % 3 != 0)
                        .fold(0u16, |mask, c| mask | (1 << c))
                } else {
                    0
                };
                let input = Input {
                    first: variable,
                    count,
                    active: matches!(kind, RandomKind::Active),
                    contributes: categories != 0,
                    contributing_categories: categories,
                };
                variable += count;
                input
            })
            .collect();
        let mut columns = vec![0; variable * 2];
        for (event, input) in inputs.iter().enumerate() {
            for category in 0..input.count {
                if input.contributing_categories & (1 << category) != 0 {
                    let detector = (event * 11 + category * 7) % 65;
                    columns[(input.first + category) * 2 + detector / 64] = 1 << (detector % 64);
                }
            }
        }
        let indexed = ActiveFreePredicate::index_packet_inputs(
            &inputs,
            &plan.random_kinds,
            &mut Quota {
                bytes: 0,
                limit: PACKET_BYTE_BUDGET,
                work: 16_000_000,
            },
        )
        .unwrap();
        assert!(!indexed.is_empty() && indexed.len() < inputs.len());
        for item in &indexed {
            let expected_ordinal = plan.random_kinds[..item.event]
                .iter()
                .filter(|kind| {
                    matches!(
                        (kind, &plan.random_kinds[item.event]),
                        (RandomKind::Noise { .. }, RandomKind::Noise { .. })
                            | (RandomKind::Independent, RandomKind::Independent)
                    )
                })
                .count();
            assert_eq!(item.ordinal, expected_ordinal);
        }
        let model = ActiveFreePredicate {
            eligible: (0..65).collect(),
            inputs,
            packet_inputs: indexed,
            sweeps: vec![],
            sweep_first: variable,
            variables: variable,
            words: 2,
            columns,
        };
        // Full-plan validation must still inspect kinds omitted from the index.
        for (event, malformed) in [
            (
                0,
                RandomKind::Noise {
                    probability: 0.2,
                    choices: 0,
                },
            ),
            (1, RandomKind::Active),
            (2, RandomKind::Independent),
        ] {
            assert!(!model.inputs[event].contributes);
            assert!(!model.packet_inputs.iter().any(|input| input.event == event));
            let original = plan.random_kinds[event];
            plan.random_kinds[event] = malformed;
            assert!(
                model
                    .packet_rejection_mask(
                        &plan,
                        1,
                        &[],
                        &vec![0; plan.random_kinds.len()],
                        &vec![0; plan.noise_event_count],
                        None,
                        None,
                        &mut vec![],
                        PACKET_BYTE_BUDGET,
                        16_000_000,
                    )
                    .is_none()
            );
            plan.random_kinds[event] = original;
            omitted_kind_refusals += 1;
        }
        for lanes in [1, 2, 31, 32, 63, 64] {
            let all = if lanes == 64 {
                u64::MAX
            } else {
                (1u64 << lanes) - 1
            };
            for pattern in 0..3 {
                let mut tape = vec![0; lanes * plan.random_kinds.len()];
                let mut hits = vec![!all; plan.noise_event_count];
                let mut noise = NoisePacket::new(
                    &plan.random_kinds,
                    plan.noise_event_count,
                    PACKET_BYTE_BUDGET,
                )
                .unwrap();
                let mut independent =
                    IndependentPacket::new(plan.independent_event_count, PACKET_BYTE_BUDGET)
                        .unwrap();
                for lane in 0..lanes {
                    let (mut n, mut i) = (0, 0);
                    let row = independent.row(lane);
                    for (event, kind) in plan.random_kinds.iter().enumerate() {
                        let draw = match kind {
                            RandomKind::Noise { choices, .. } => {
                                let selected = match pattern {
                                    0 => false,
                                    1 => (lane + event) % 13 == 0,
                                    _ => true,
                                };
                                let draw = if selected {
                                    1 + (lane + event) % choices
                                } else {
                                    0
                                } as u64;
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
                                let draw = ((event + lane * 3) % 2) as u64;
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
                for planes in noise.planes() {
                    for plane in planes {
                        *plane |= !all;
                    }
                }
                // Literal full-tape oracle never consults the index or support flags.
                let mut expected = vec![];
                for (event, kind) in plan.random_kinds.iter().enumerate() {
                    let input = model.inputs[event];
                    for category in 0..input.count {
                        let effect = model.column(input.first + category).unwrap();
                        let mut mask = 0;
                        for lane in 0..lanes {
                            let draw = tape[lane * plan.random_kinds.len() + event];
                            if match kind {
                                RandomKind::Noise { .. } => draw == (category + 1) as u64,
                                RandomKind::Independent => draw != 0,
                                RandomKind::Active => false,
                            } {
                                mask |= 1 << lane;
                            }
                        }
                        if mask != 0 && effect.iter().any(|&bits| bits != 0) {
                            expected.push((effect.to_vec(), mask));
                        }
                    }
                }
                let mut scalar = 0;
                for lane in 0..lanes {
                    let mut parity = [0; 2];
                    for (event, kind) in plan.random_kinds.iter().enumerate() {
                        let draw = tape[lane * plan.random_kinds.len() + event] as usize;
                        let column = match kind {
                            RandomKind::Noise { .. } if draw != 0 => {
                                Some(model.inputs[event].first + draw - 1)
                            }
                            RandomKind::Independent if draw != 0 => Some(model.inputs[event].first),
                            _ => None,
                        };
                        if let Some(column) = column {
                            for (to, from) in parity.iter_mut().zip(model.column(column).unwrap()) {
                                *to ^= from;
                            }
                        }
                    }
                    if parity != [0; 2] {
                        scalar |= 1 << lane;
                    }
                }
                for compact_noise in [false, true] {
                    for compact_independent in [false, true] {
                        let mut stored = tape.clone();
                        for lane in 0..lanes {
                            for (event, kind) in plan.random_kinds.iter().enumerate() {
                                if compact_noise && matches!(kind, RandomKind::Noise { .. })
                                    || compact_independent
                                        && matches!(kind, RandomKind::Independent)
                                {
                                    stored[lane * plan.random_kinds.len() + event] = u64::MAX;
                                }
                            }
                        }
                        let mut iterator = PacketColumns {
                            model: &model,
                            plan: &plan,
                            tape: &stored,
                            hits: &hits,
                            noise: compact_noise.then_some(&noise),
                            independent: compact_independent.then_some(&independent),
                            lanes,
                            all,
                            event: 0,
                            first: 0,
                            count: 0,
                            category: 0,
                            contributing_categories: 0,
                            masks: [u64::MAX; 15],
                        };
                        let actual: Vec<_> = iterator
                            .by_ref()
                            .map(|(column, mask)| (column.to_vec(), mask))
                            .collect();
                        assert_eq!(actual, expected);
                        assert_eq!(iterator.event, model.packet_inputs.len());
                        assert!(iterator.next().is_none());
                        assert_eq!(
                            model.packet_rejection_mask(
                                &plan,
                                lanes,
                                &[],
                                &stored,
                                &hits,
                                compact_noise.then_some(&noise),
                                compact_independent.then_some(&independent),
                                &mut vec![],
                                PACKET_BYTE_BUDGET,
                                16_000_000
                            ),
                            Some(scalar)
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(omitted_kind_refusals, 45);
    println!(
        "CONTRIBUTING_EVENT_INDEX_MASK_CASES {cases}; OMITTED_KIND_REFUSALS {omitted_kind_refusals}"
    );
}

#[test]
fn contributing_event_index_keeps_counts_records_and_rng_against_physical_execution() {
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
            reference.predicate.set(None).unwrap();
            let mut a = SmallRng::seed_from_u64(0xc017_2026_1024_0001 + cache as u64);
            let mut b = a.clone();
            for shots in [1, 31, 64, 67] {
                for sweep in [&[][..], &[true][..]] {
                    assert_eq!(
                        actual
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut a)
                            .unwrap(),
                        reference
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut b)
                            .unwrap()
                    );
                    if shots == 64 && cache != 0 {
                        let predicate = actual.predicate.get().and_then(Option::as_ref).unwrap();
                        assert!(!predicate.packet_inputs.is_empty());
                        assert!(predicate.packet_inputs.len() < predicate.inputs.len());
                        assert!(
                            predicate
                                .packet_inputs
                                .iter()
                                .any(|item| item.event > 0 && item.ordinal > 0)
                        );
                        assert_ne!(actual.sampler.last_eager_rejected_mask, 0);
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
    println!("CONTRIBUTING_EVENT_INDEX_CONTINUATION_CASES {cases}");
}
