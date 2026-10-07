// Included only into the cfg(test) lazy uniform reference module, on retained
// candidate16 Noise source. Uses its literal malformed rank-one/CDF oracle;
// this is a bookkeeping/error witness, not a valid-circuit physics reference.
#[test]
fn lazy_cache_noise_independent_producer_matches_frozen_rows_on_first_and_second_errors() {
    for policy in policies() {
        for noise in [false, true] {
            for fail_second in [false, true] {
                let (plan, node, _) = error_plan(policy, true, fail_second, noise);
                assert_eq!(plan.peak_active_rank, 1);
                assert!(plan.independent_event_count >= 129 && plan.random_runs.is_some());
                assert_eq!(plan.noise_event_count > 0, noise);
                let initial = plan
                    .prepare_sampler()
                    .unwrap()
                    .coefficient_cache_reserved_bytes();
                let expected_initial = plan.operations.len() * size_of::<CachedOp>()
                    + plan.initial_coefficients.len() * size_of::<ComplexAmp>()
                    + 512;
                assert_eq!(initial, expected_initial);
                for budget in [initial, DEFAULT_CACHE_BYTE_BUDGET] {
                    for lanes in [63, 64] {
                        let (mut lazy, mut reference) = pair(&plan, budget);
                        for sampler in [&mut lazy, &mut reference] {
                            sampler.observe_lazy_error_producer = true;
                            let cache = sampler.cache.as_ref().unwrap();
                            assert_ne!(cache.start, 0, "actual rank-one input must be admitted");
                            assert_eq!(cache.states.len(), 2);
                            assert!(sampler.pack_enabled);
                            // Sentinel proves omitted Noise/Independent slots are
                            // observed through sidecars, not stale retained tape.
                            resize_packet(&mut sampler.packet_tape, 64 * plan.random_kinds.len())
                                .unwrap();
                            sampler.packet_tape.fill(u64::MAX);
                            inject_inconsistent_cdf(sampler, node);
                        }
                        let mut a = StdRng::seed_from_u64(583);
                        let mut b = a.clone();
                        let mut frozen = a.clone();
                        let rows = scalar_prepared_tape_tests::draw_frozen_packet_rows(
                            &plan.random_kinds,
                            lanes,
                            &mut frozen,
                        );
                        let event = plan
                            .random_kinds
                            .iter()
                            .position(|k| matches!(k, RandomKind::Active))
                            .unwrap();
                        let ones = rows
                            .iter()
                            .filter(|row| f64::from_bits(row[event]) >= 0.5)
                            .count();
                        assert!(
                            ones > 0 && ones < lanes,
                            "both cached calls need actual nonempty masks"
                        );
                        let actual = lazy.sample_measurements_u8(lanes, &mut a);
                        let expected = reference.sample_measurements_u8(lanes, &mut b);
                        let context = format!(
                            "noise={noise}; fail_second={fail_second}; {}",
                            diagnostic(&lazy, policy, 583, budget, lanes, 0)
                        );
                        assert_eq!(
                            actual.as_ref().unwrap_err(),
                            "compiled near-Clifford zero-probability measurement",
                            "{context}"
                        );
                        assert_eq!(actual, expected, "{context}");
                        assert_eq!(snapshot(&lazy), snapshot(&reference), "{context}");
                        let observation = lazy
                            .last_packet_error_producer
                            .as_ref()
                            .expect("public packet actually executed");
                        assert_eq!(
                            lazy.last_packet_error_producer, reference.last_packet_error_producer,
                            "{context}"
                        );
                        assert_eq!(observation.lanes, lanes, "{context}");
                        assert!(!observation.coherent, "{context}");
                        assert_eq!(
                            observation.noise_admitted,
                            noise && lanes == 64,
                            "actual call-local admission, not eligibility; {context}"
                        );
                        assert_eq!(
                            observation.noise_reserved_bytes > 0,
                            observation.noise_admitted,
                            "{context}"
                        );
                        let occupied = lazy.scalar_tape_other_bytes().unwrap()
                            + lazy.packet_tape.capacity() * size_of::<u64>();
                        assert!(
                            occupied + observation.noise_reserved_bytes <= PACKET_BYTE_BUDGET,
                            "actual retained capacities plus admitted Noise; {context}"
                        );
                        assert_eq!(
                            observation.rows, rows,
                            "every typed producer slot including omitted Noise and >128 Independent bits; {context}"
                        );
                        assert!(lazy.packet_independent.is_some(), "{context}");
                        for (lane, row) in rows.iter().enumerate() {
                            for (event, kind) in plan.random_kinds.iter().enumerate() {
                                if matches!(kind, RandomKind::Noise { .. }) {
                                    assert_eq!(
                                        lazy.packet_tape[lane * plan.random_kinds.len() + event],
                                        if observation.noise_admitted {
                                            u64::MAX
                                        } else {
                                            row[event]
                                        },
                                        "lane={lane}; event={event}; {context}"
                                    );
                                }
                            }
                        }
                        let cache = lazy.cache.as_ref().unwrap();
                        let Some(CachedOp::Measure(entry)) = cache.entry(cache.start, node) else {
                            unreachable!()
                        };
                        assert_eq!(
                            entry.probability_zero.to_bits(),
                            0.5f64.to_bits(),
                            "{context}"
                        );
                        assert_eq!(
                            entry.next,
                            if fail_second {
                                [Some(0), None]
                            } else {
                                [None, None]
                            },
                            "{context}"
                        );
                        assert_eq!(
                            cache.reserved, initial,
                            "only a successful scalar alias before second Err; {context}"
                        );
                        assert_eq!(cache.states.len(), 2, "{context}");
                        assert_eq!(lazy.last_packet_live_mask, 0, "{context}");
                        assert_eq!(lazy.last_packet_live, 0, "{context}");
                        assert!(
                            lazy.pack_enabled,
                            "Err precedes strategy-accounting; {context}"
                        );
                        assert_eq!(a.clone().next_u64(), b.clone().next_u64(), "{context}");
                        assert_eq!(
                            a.clone().next_u64(),
                            frozen.clone().next_u64(),
                            "one whole packet draw, no retry or replay; {context}"
                        );
                    }
                }
            }
        }
    }
}
