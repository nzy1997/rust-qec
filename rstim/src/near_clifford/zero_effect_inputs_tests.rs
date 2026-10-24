use super::*;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

const TEXT: &str = "X_ERROR(0.2) 0\nH 2\nM 2\nZ_ERROR(0.3) 3\nDEPOLARIZE2(0.25) 3 4\nX_ERROR(0) 5\nX_ERROR(1) 6\nH 7\nT 7\nMX 7\nX_ERROR(0.4) 1\nH 8\nM 8\nCX sweep[0] 1\nM 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";

fn scalar_mask(
    model: &ActiveFreePredicate,
    plan: &CompiledNearCliffordExecutor,
    tape: &[u64],
    lanes: usize,
    sweep: &[bool],
) -> u64 {
    let mut rejected = 0;
    for lane in 0..lanes {
        let mut value = model.column(0).unwrap().to_vec();
        for (event, kind) in plan.random_kinds.iter().enumerate() {
            let draw = tape[lane * plan.random_kinds.len() + event] as usize;
            let input = model.inputs[event];
            let variable = match kind {
                RandomKind::Noise { .. } if draw != 0 => Some(input.first + draw - 1),
                RandomKind::Independent if draw != 0 => Some(input.first),
                _ => None,
            };
            if let Some(variable) = variable {
                for (to, from) in value.iter_mut().zip(model.column(variable).unwrap()) {
                    *to ^= from;
                }
            }
        }
        for (slot, index) in model.sweeps.iter().enumerate() {
            if sweep.get(*index as usize).copied().unwrap_or(false) {
                for (to, from) in value
                    .iter_mut()
                    .zip(model.column(model.sweep_first + slot).unwrap())
                {
                    *to ^= from;
                }
            }
        }
        if value.iter().any(|word| *word != 0) {
            rejected |= 1u64 << lane;
        }
    }
    rejected
}

#[test]
fn zero_effect_inputs_masks_match_scalar_columns_in_all_sidecar_layouts() {
    // Put unused measurements/readouts between detector-relevant measurements
    // so both event types exercise a skipped ordinal before a later used one.
    let mask_text = TEXT
        .replace("M 2\n", "M(0.2) 2\nDETECTOR rec[-1]\n")
        .replace("H 8\n", "H 9\nM(0.25) 9\nH 8\n")
        .replace("M 8\n", "M(0.3) 8\n");
    // The existing compact Independent producer requires at least eight events.
    // Additional unused measurements exercise it without changing that policy.
    let mask_text = format!("{mask_text}H 20 21 22 23 24 25 26 27\nM 20 21 22 23 24 25 26 27\n");
    let plan = CompiledNearCliffordExecutor::compile_text(&mask_text).unwrap();
    let model = ActiveFreePredicate::build(&plan, 8 * 1024 * 1024, 16_000_000).unwrap();
    for independent in [false, true] {
        let selected: Vec<_> = plan
            .random_kinds
            .iter()
            .zip(&model.inputs)
            .filter(|(kind, _)| {
                matches!(
                    (independent, kind),
                    (true, RandomKind::Independent) | (false, RandomKind::Noise { .. })
                )
            })
            .map(|(_, input)| input.contributes)
            .collect();
        assert!(selected.contains(&false) && selected.contains(&true));
        assert!(
            selected.windows(2).any(|pair| pair == [false, true]),
            "independent={independent}, contribution flags={selected:?}"
        );
    }
    let mut cases = 0;
    for lanes in [1, 2, 31, 32, 63, 64] {
        let mut tape = vec![0; lanes * plan.random_kinds.len()];
        let mut hits = vec![0u64; plan.noise_event_count];
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
            let independent_row = independent.row(lane);
            for (event, kind) in plan.random_kinds.iter().enumerate() {
                let draw = match *kind {
                    RandomKind::Active => 0.375f64.to_bits(),
                    RandomKind::Independent => {
                        let bit = (lane + event) % 2;
                        independent_row[i / 64] |= (bit as u64) << (i % 64);
                        i += 1;
                        bit as u64
                    }
                    RandomKind::Noise {
                        probability,
                        choices,
                    } => {
                        let value =
                            if probability == 0. || probability < 1. && (lane + event) % 4 == 0 {
                                0
                            } else {
                                ((lane * 7 + event) % choices + 1) as u64
                            };
                        if value != 0 {
                            hits[n] |= 1u64 << lane;
                        }
                        for bit in 0..4 {
                            noise.planes()[n][bit] |= ((value >> bit) & 1) << lane;
                        }
                        n += 1;
                        value
                    }
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
                for sweep in [&[][..], &[false][..], &[true][..]] {
                    let expected = scalar_mask(&model, &plan, &tape, lanes, sweep);
                    let mut scratch = Vec::new();
                    let actual = model.packet_rejection_mask(
                        &plan,
                        lanes,
                        sweep,
                        &stored,
                        &hits,
                        use_noise.then_some(&noise),
                        use_independent.then_some(&independent),
                        &mut scratch,
                        PACKET_BYTE_BUDGET,
                        16_000_000,
                    );
                    assert_eq!(actual, Some(expected));
                    cases += 1;
                }
            }
        }
    }
    println!("ZERO_EFFECT_MASK_CASES {cases}");
}

#[test]
fn zero_effect_inputs_construction_charges_visits_and_declines_invalid_ranges() {
    let input = Input {
        first: 1,
        count: 2,
        active: false,
        contributes: false,
        contributing_categories: 0,
    };
    let columns = [0, 0, 0, 1, 0, 0];
    let mut exact = [input];
    let mut quota = Quota {
        bytes: 0,
        limit: 0,
        work: 5,
    };
    ActiveFreePredicate::mark_contributing_inputs(&mut exact, 2, &columns, &mut quota).unwrap();
    assert!(exact[0].contributes && quota.work == 0 && quota.bytes == 0);
    let mut short = [input];
    let mut quota = Quota {
        bytes: 0,
        limit: 0,
        work: 4,
    };
    assert!(
        ActiveFreePredicate::mark_contributing_inputs(&mut short, 2, &columns, &mut quota)
            .is_none()
    );
    for bad in [
        Input {
            first: usize::MAX,
            ..input
        },
        Input {
            count: usize::MAX,
            ..input
        },
        Input { first: 3, ..input },
    ] {
        let mut quota = Quota {
            bytes: 0,
            limit: 0,
            work: usize::MAX,
        };
        assert!(
            ActiveFreePredicate::mark_contributing_inputs(&mut [bad], 2, &columns, &mut quota)
                .is_none()
        );
    }
    let mut zero = [input];
    let mut quota = Quota {
        bytes: 0,
        limit: 0,
        work: 5,
    };
    ActiveFreePredicate::mark_contributing_inputs(&mut zero, 2, &[0; 6], &mut quota).unwrap();
    assert!(!zero[0].contributes);
    println!("ZERO_EFFECT_QUOTA_REFUSALS 4");
}

#[test]
fn zero_effect_inputs_success_streams_match_unsparsified_eager_execution() {
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
            assert!(full_model.inputs.iter().any(|input| !input.contributes));
            for input in &mut full_model.inputs {
                input.contributes = true;
                input.contributing_categories = (1u16 << input.count) - 1;
            }
            reference.predicate.set(Some(full_model)).unwrap();
            let mut a = SmallRng::seed_from_u64(0x5e0f_2026_1023_0001 + cache as u64);
            let mut b = a.clone();
            for shots in [32, 33, 63, 65] {
                for sweep in [&[][..], &[true][..]] {
                    assert_eq!(
                        actual
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut a)
                            .unwrap(),
                        reference
                            .sample_postselected_counts_with_sweep(shots, 0, sweep, &mut b)
                            .unwrap()
                    );
                    assert_eq!(
                        actual.sample_measurements_u8(3, &mut a).unwrap(),
                        reference.sample_measurements_u8(3, &mut b).unwrap()
                    );
                    let next_a: Vec<_> = (0..16).map(|_| a.next_u64()).collect();
                    let next_b: Vec<_> = (0..16).map(|_| b.next_u64()).collect();
                    assert_eq!(next_a, next_b);
                    cases += 1;
                }
            }
        }
    }
    println!("ZERO_EFFECT_SUCCESS_STREAM_CASES {cases}");
}
