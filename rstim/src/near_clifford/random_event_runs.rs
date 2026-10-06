use super::*;

#[derive(Clone, Copy, Debug)]
enum PreparedRandom {
    Independent,
    Active,
    Noise {
        probability: f64,
        choices: usize,
    },
    Sparse {
        probability_bits: u64,
        log_failure: f64,
    },
}

impl PreparedRandom {
    fn from_kind(kind: RandomKind) -> Self {
        match kind {
            RandomKind::Independent => Self::Independent,
            RandomKind::Active => Self::Active,
            RandomKind::Noise { probability, .. } if (1e-12..=0.01).contains(&probability) => {
                Self::Sparse {
                    probability_bits: probability.to_bits(),
                    log_failure: (-probability).ln_1p(),
                }
            }
            RandomKind::Noise {
                probability,
                choices,
            } => Self::Noise {
                probability,
                choices,
            },
        }
    }

    fn matches(self, kind: RandomKind) -> bool {
        match (self, kind) {
            (Self::Independent, RandomKind::Independent) | (Self::Active, RandomKind::Active) => {
                true
            }
            (
                Self::Sparse {
                    probability_bits, ..
                },
                RandomKind::Noise { probability, .. },
            ) => probability_bits == probability.to_bits(),
            (
                Self::Noise {
                    probability,
                    choices,
                },
                RandomKind::Noise {
                    probability: other,
                    choices: other_choices,
                },
            ) => probability.to_bits() == other.to_bits() && choices == other_choices,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
struct RandomRun {
    kind: PreparedRandom,
    count: usize,
}

#[derive(Clone, Debug)]
pub(super) struct RandomRunPlan {
    runs: Vec<RandomRun>,
    event_count: usize,
}

impl RandomRunPlan {
    // Optional metadata reservation; this is not RSS or the existing tape budget.
    const BYTE_BUDGET: usize = 8 * 1024 * 1024;

    pub(super) fn build(kinds: &[RandomKind], remaining_bytes: usize) -> Option<Self> {
        let budget = remaining_bytes.min(Self::BYTE_BUDGET);
        // Count first, so alternating events cannot trigger an unaccounted Vec
        // growth allocation. Compilation is two linear passes, sampling is not.
        let mut previous = None;
        let mut run_count = 0usize;
        for &kind in kinds {
            let matches = match (previous, kind) {
                (Some(RandomKind::Independent), RandomKind::Independent)
                | (Some(RandomKind::Active), RandomKind::Active) => true,
                (
                    Some(RandomKind::Noise {
                        probability: a,
                        choices: x,
                    }),
                    RandomKind::Noise {
                        probability: b,
                        choices: y,
                    },
                ) => a.to_bits() == b.to_bits() && ((1e-12..=0.01).contains(&a) || x == y),
                _ => false,
            };
            if !matches {
                run_count = run_count.checked_add(1)?;
                if run_count.checked_mul(std::mem::size_of::<RandomRun>())? > budget {
                    return None;
                }
            }
            previous = Some(kind);
        }
        let mut runs: Vec<RandomRun> = Vec::new();
        runs.try_reserve_exact(run_count).ok()?;
        if runs
            .capacity()
            .checked_mul(std::mem::size_of::<RandomRun>())?
            > budget
        {
            return None;
        }
        for &kind in kinds {
            if let Some(last) = runs.last_mut() {
                if last.kind.matches(kind) {
                    last.count = last.count.checked_add(1)?;
                    continue;
                }
            }
            runs.push(RandomRun {
                kind: PreparedRandom::from_kind(kind),
                count: 1,
            });
        }
        Some(Self {
            runs,
            event_count: kinds.len(),
        })
    }

    #[cfg(test)]
    pub(super) fn fill_row<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        output: &mut [u64],
    ) {
        assert_eq!(output.len(), self.event_count);
        assert_eq!(kinds.len(), self.event_count);
        let mut offset = 0;
        for run in &self.runs {
            let end = offset + run.count;
            random.fill_prepared_impl(
                run.kind,
                &mut output[offset..end],
                None,
                Some(&kinds[offset..end]),
            );
            offset = end;
        }
        debug_assert_eq!(offset, output.len());
    }
}

impl RandomRunPlan {
    pub(super) fn fill_row_with_noise_masks<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        output: &mut [u64],
        noise_masks: &mut [u64],
        lane_bit: u64,
    ) {
        assert_eq!(output.len(), self.event_count);
        assert_eq!(kinds.len(), self.event_count);
        debug_assert!(lane_bit.is_power_of_two());
        let mut event = 0;
        let mut noise = 0;
        for run in &self.runs {
            let end = event + run.count;
            match run.kind {
                PreparedRandom::Noise { .. } | PreparedRandom::Sparse { .. } => {
                    let noise_end = noise + run.count;
                    random.fill_prepared_impl(
                        run.kind,
                        &mut output[event..end],
                        Some((&mut noise_masks[noise..noise_end], lane_bit)),
                        matches!(run.kind, PreparedRandom::Sparse { .. })
                            .then_some(&kinds[event..end]),
                    );
                    noise = noise_end;
                }
                _ => random.fill_prepared_impl(run.kind, &mut output[event..end], None, None),
            }
            event = end;
        }
        debug_assert_eq!(event, output.len());
        debug_assert_eq!(noise, noise_masks.len());
    }
}

// Short homogeneous runs are common between unlike events. Fixed-size stores
// avoid a platform memset call while larger runs retain the bulk implementation.
#[inline]
fn clear_events(output: &mut [u64]) {
    match output {
        [] => {}
        [a] => *a = 0,
        [a, b] => {
            *a = 0;
            *b = 0;
        }
        [a, b, c] => {
            *a = 0;
            *b = 0;
            *c = 0;
        }
        [a, b, c, d] => {
            *a = 0;
            *b = 0;
            *c = 0;
            *d = 0;
        }
        _ => output.fill(0),
    }
}

impl<R: Rng> RowRandom<'_, R> {
    #[cfg(test)]
    fn fill_prepared(&mut self, kind: PreparedRandom, output: &mut [u64], kinds: &[RandomKind]) {
        self.fill_prepared_impl(kind, output, None, Some(kinds));
    }

    fn fill_prepared_impl(
        &mut self,
        kind: PreparedRandom,
        output: &mut [u64],
        mut noise_masks: Option<(&mut [u64], u64)>,
        sparse_kinds: Option<&[RandomKind]>,
    ) {
        if output.is_empty() {
            return; // Empty runs cannot alter pending probability or prefetch RNG.
        }
        if let Some(tape) = self.tape {
            let end = self.cursor + output.len();
            output.copy_from_slice(&tape[self.cursor..end]);
            self.cursor = end;
            if let Some((masks, lane)) = noise_masks {
                for (&value, mask) in output.iter().zip(masks) {
                    if value != 0 {
                        *mask |= lane;
                    }
                }
            }
            return; // Same replay values, no RNG or bit/geometric state updates.
        }
        match kind {
            PreparedRandom::Independent => {
                let mut offset = 0;
                while offset < output.len() {
                    if self.bits_left == 0 {
                        self.bit_word = self.rng.r#gen::<u64>();
                        self.bits_left = 64;
                    }
                    let take = (output.len() - offset).min(self.bits_left as usize);
                    let word = self.bit_word;
                    for (bit, value) in output[offset..offset + take].iter_mut().enumerate() {
                        *value = (word >> bit) & 1;
                    }
                    self.bit_word = if take == 64 { 0 } else { word >> take };
                    self.bits_left -= take as u8;
                    offset += take;
                    // Do not load another word when offset reaches output.len().
                }
            }
            PreparedRandom::Active => {
                for value in output {
                    *value = self.rng.r#gen::<f64>().to_bits();
                }
            }
            PreparedRandom::Noise {
                probability,
                choices,
            } => {
                if probability == 0. {
                    clear_events(output);
                } else if probability == 1. && choices == 1 {
                    output.fill(1);
                    if let Some((masks, lane)) = noise_masks {
                        for mask in masks {
                            *mask |= lane;
                        }
                    }
                } else if probability == 1. {
                    for value in output {
                        *value = (self.rng.gen_range(0..choices) + 1) as u64;
                    }
                    if let Some((masks, lane)) = noise_masks {
                        for mask in masks {
                            *mask |= lane;
                        }
                    }
                } else {
                    // This preserves the original direct-noise draw expression.
                    // In particular, direct noise does NOT change the pending
                    // sparse probability/skip, even when its probability differs.
                    for (index, value) in output.iter_mut().enumerate() {
                        *value = if !near_noise_occurs(probability, self.rng) {
                            0
                        } else if choices == 1 {
                            1
                        } else {
                            (self.rng.gen_range(0..choices) + 1) as u64
                        };
                        if *value != 0 {
                            if let Some((masks, lane)) = &mut noise_masks {
                                masks[index] |= *lane;
                            }
                        }
                    }
                }
            }
            PreparedRandom::Sparse {
                probability_bits,
                log_failure,
            } => {
                if self.noise_probability != probability_bits {
                    self.noise_skip = None;
                    self.noise_probability = probability_bits;
                }
                let mut offset = 0;
                while offset < output.len() {
                    let skip = self.noise_skip.get_or_insert_with(|| {
                        let u = self.rng.r#gen::<f64>();
                        ((-u).ln_1p() / log_failure).floor() as usize
                    });
                    if *skip != 0 {
                        let failures = (*skip).min(output.len() - offset);
                        clear_events(&mut output[offset..offset + failures]);
                        *skip -= failures;
                        offset += failures;
                        // Some(0) must survive an exact run boundary: the next
                        // same-p sparse event succeeds without renewing a uniform.
                        continue;
                    }
                    // All skipped events are same-p sparse Noise; their choice
                    // count cannot affect Bernoulli failures or pending carry.
                    // Read the original event ONLY at a success, before drawing
                    // that event's category with the original usize range type.
                    let RandomKind::Noise { choices, .. } = sparse_kinds
                        .expect("sparse runs borrow their original typed event slice")[offset]
                    else {
                        unreachable!("sparse runs contain only noise events")
                    };
                    output[offset] = if choices == 1 {
                        1
                    } else {
                        (self.rng.gen_range(0..choices) + 1) as u64
                    };
                    self.noise_skip = None;
                    if let Some((masks, lane)) = &mut noise_masks {
                        masks[offset] |= *lane;
                    }
                    offset += 1;
                    // A success at the final event must NOT renew the next run.
                }
            }
        }
    }
}

#[cfg(test)]
mod random_event_runs_tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn sparse_run_borrows_success_choices_without_changing_rng_or_masks() {
        let probability = 0.001f64;
        let kinds = [1, 3, 15, 1].map(|choices| RandomKind::Noise {
            probability,
            choices,
        });
        let plan = RandomRunPlan::build(&kinds, size_of::<RandomRun>()).unwrap();
        assert_eq!(plan.runs.len(), 1);
        assert_eq!(plan.runs[0].count, kinds.len());
        let mut a = StdRng::seed_from_u64(2415);
        let mut b = a.clone();
        {
            let mut scalar = RowRandom::live(&mut a);
            let mut runs = RowRandom::live(&mut b);
            scalar.noise_probability = probability.to_bits();
            runs.noise_probability = probability.to_bits();
            // Two failures with unlike choice counts, then guaranteed choice15.
            scalar.noise_skip = Some(2);
            runs.noise_skip = Some(2);
            let expected = kinds
                .iter()
                .map(|&kind| scalar.draw(kind))
                .collect::<Vec<_>>();
            let mut actual = [99; 4];
            let mut masks = [0; 4];
            plan.fill_row_with_noise_masks(&mut runs, &kinds, &mut actual, &mut masks, 1u64 << 63);
            assert_eq!(expected, actual);
            assert_ne!(actual[2], 0);
            assert_eq!(
                masks,
                actual.map(|value| if value == 0 { 0 } else { 1u64 << 63 })
            );
            assert_eq!(scalar.noise_probability, runs.noise_probability);
            assert_eq!(scalar.noise_skip, runs.noise_skip);
            assert_eq!(scalar.bits_left, runs.bits_left);
            assert_eq!(scalar.bit_word, runs.bit_word);
        }
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn sparse_run_count_pass_and_emission_agree_but_direct_choices_split() {
        let kinds = [
            RandomKind::Noise {
                probability: 0.001,
                choices: 1,
            },
            RandomKind::Noise {
                probability: 0.001,
                choices: 3,
            },
            RandomKind::Noise {
                probability: 0.001,
                choices: 15,
            },
            RandomKind::Noise {
                probability: 0.001,
                choices: 1,
            },
            RandomKind::Noise {
                probability: 0.37,
                choices: 1,
            },
            RandomKind::Noise {
                probability: 0.37,
                choices: 15,
            },
            RandomKind::Noise {
                probability: 0.001,
                choices: 3,
            },
        ];
        let bytes = size_of::<RandomRun>();
        assert!(RandomRunPlan::build(&kinds, 3 * bytes).is_none());
        let plan = RandomRunPlan::build(&kinds, 4 * bytes).unwrap();
        assert_eq!(plan.runs.len(), 4);
        assert_eq!(
            plan.runs.iter().map(|run| run.count).collect::<Vec<_>>(),
            [4, 1, 1, 1]
        );
        for seed in [0, 1, 15, 2401] {
            compare(&kinds, seed);
        }
    }

    fn compare(kinds: &[RandomKind], seed: u64) {
        let plan = RandomRunPlan::build(kinds, PLAN_BYTE_BUDGET).unwrap();
        let mut a = StdRng::seed_from_u64(seed);
        let mut b = a.clone();
        for _ in 0..8 {
            let mut scalar = RowRandom::live(&mut a);
            let mut runs = RowRandom::live(&mut b);
            let expected = kinds
                .iter()
                .map(|&kind| scalar.draw(kind))
                .collect::<Vec<_>>();
            let mut actual = vec![0; kinds.len()];
            plan.fill_row(&mut runs, kinds, &mut actual);
            assert_eq!(expected, actual);
            assert_eq!(scalar.bit_word, runs.bit_word);
            assert_eq!(scalar.bits_left, runs.bits_left);
            assert_eq!(scalar.noise_probability, runs.noise_probability);
            assert_eq!(scalar.noise_skip, runs.noise_skip);
        }
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn prepared_random_runs_keep_every_typed_event_and_continuation() {
        let mut kinds = Vec::new();
        for (kind, count) in [
            (RandomKind::Independent, 65),
            (
                RandomKind::Noise {
                    probability: 0.01,
                    choices: 1,
                },
                129,
            ),
            (RandomKind::Active, 3),
            (
                RandomKind::Noise {
                    probability: 0.01,
                    choices: 15,
                },
                100,
            ),
            (
                RandomKind::Noise {
                    probability: 0.37,
                    choices: 3,
                },
                7,
            ),
            (
                RandomKind::Noise {
                    probability: 0.,
                    choices: 15,
                },
                17,
            ),
            (
                RandomKind::Noise {
                    probability: 1.,
                    choices: 1,
                },
                17,
            ),
            (
                RandomKind::Noise {
                    probability: 1.,
                    choices: 15,
                },
                17,
            ),
            (
                RandomKind::Noise {
                    probability: 0.01,
                    choices: 1,
                },
                64,
            ),
            (RandomKind::Independent, 70),
            (
                RandomKind::Noise {
                    probability: 1e-12,
                    choices: 3,
                },
                65,
            ),
            (
                RandomKind::Noise {
                    probability: 0.001,
                    choices: 1,
                },
                31,
            ),
            (
                RandomKind::Noise {
                    probability: 0.01,
                    choices: 15,
                },
                32,
            ),
            (
                RandomKind::Noise {
                    probability: 0.01000001,
                    choices: 1,
                },
                32,
            ),
            (
                RandomKind::Noise {
                    probability: 0.01,
                    choices: 1,
                },
                32,
            ),
        ] {
            kinds.extend(std::iter::repeat_n(kind, count));
        }
        for seed in [0, 1, 21, 970, 2026100703] {
            compare(&kinds, seed);
        }
    }

    #[test]
    fn noise_masks_match_typed_events_and_clear_tail_packet_high_lanes() {
        let mut kinds = vec![RandomKind::Independent; 70];
        for (probability, choices) in [
            (0., 1),
            (1e-12, 3),
            (0.001, 15),
            (0.001, 1),
            (0.01, 3),
            (0.37, 15),
            (1., 1),
            (1., 15),
            (0.001, 3),
        ] {
            kinds.extend(std::iter::repeat_n(
                RandomKind::Noise {
                    probability,
                    choices,
                },
                17,
            ));
            kinds.push(RandomKind::Active);
        }
        let plan = RandomRunPlan::build(&kinds, PLAN_BYTE_BUDGET).unwrap();
        let noise_count = kinds
            .iter()
            .filter(|k| matches!(k, RandomKind::Noise { .. }))
            .count();
        let mut a = StdRng::seed_from_u64(2181);
        let mut b = a.clone();
        let mut c = a.clone();
        let mut masks = vec![u64::MAX; noise_count];
        for lanes in [64, 1, 31, 32, 63, 64, 1] {
            masks.fill(0);
            let mut reference_masks = vec![0; noise_count];
            let mut fallback_masks = vec![0; noise_count];
            for lane in 0..lanes {
                let mut reference = RowRandom::live(&mut a);
                let expected = kinds.iter().map(|&k| reference.draw(k)).collect::<Vec<_>>();
                let mut actual = vec![0; kinds.len()];
                let mut fallback = vec![0; kinds.len()];
                let mut random = RowRandom::live(&mut b);
                plan.fill_row_with_noise_masks(
                    &mut random,
                    &kinds,
                    &mut actual,
                    &mut masks,
                    1u64 << lane,
                );
                let mut original = RowRandom::live(&mut c);
                fill_original_row_with_noise_masks(
                    &mut original,
                    &kinds,
                    &mut fallback,
                    &mut fallback_masks,
                    1u64 << lane,
                );
                assert_eq!(expected, actual);
                assert_eq!(expected, fallback);
                assert_eq!(reference.bit_word, random.bit_word);
                assert_eq!(reference.bits_left, random.bits_left);
                assert_eq!(reference.noise_probability, random.noise_probability);
                assert_eq!(reference.noise_skip, random.noise_skip);
                let mut slot = 0;
                for (&kind, &value) in kinds.iter().zip(&expected) {
                    if matches!(kind, RandomKind::Noise { .. }) {
                        if value != 0 {
                            reference_masks[slot] |= 1u64 << lane;
                        }
                        slot += 1;
                    }
                }
            }
            assert_eq!(masks, reference_masks);
            assert_eq!(masks, fallback_masks);
            let continuation = a.next_u64();
            assert_eq!(continuation, b.next_u64());
            assert_eq!(continuation, c.next_u64());
        }
    }

    #[test]
    fn random_run_metadata_rejection_keeps_the_original_event_path() {
        let kinds = [RandomKind::Independent, RandomKind::Active];
        assert!(RandomRunPlan::build(&kinds, 0).is_none());
        assert!(RandomRunPlan::build(&kinds, size_of::<RandomRun>()).is_none());
        assert!(RandomRunPlan::build(&kinds, 2 * size_of::<RandomRun>()).is_some());
        let mut plan =
            CompiledNearCliffordExecutor::compile_text("H 0 1 2 3\nT 0 1 2 3\nMX 0 1 2 3\n")
                .unwrap();
        assert!(plan.random_runs.is_some());
        let mut a = StdRng::seed_from_u64(118);
        let mut b = a.clone();
        let actual = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8(129, &mut a)
            .unwrap();
        plan.random_runs = None;
        let expected = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8(129, &mut b)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn prepared_random_runs_retain_a_zero_skip_at_boundaries_without_prefetch() {
        let p = 0.01f64;
        let kind = PreparedRandom::from_kind(RandomKind::Noise {
            probability: p,
            choices: 1,
        });
        let mut a = StdRng::seed_from_u64(81);
        let mut b = a.clone();
        let mut scalar = RowRandom::live(&mut a);
        let mut runs = RowRandom::live(&mut b);
        scalar.noise_probability = p.to_bits();
        runs.noise_probability = p.to_bits();
        scalar.noise_skip = Some(3);
        runs.noise_skip = Some(3);
        let expected = (0..3)
            .map(|_| {
                scalar.draw(RandomKind::Noise {
                    probability: p,
                    choices: 1,
                })
            })
            .collect::<Vec<_>>();
        let mut actual = [9; 3];
        runs.fill_prepared(
            kind,
            &mut actual,
            &[RandomKind::Noise {
                probability: p,
                choices: 1,
            }; 3],
        );
        assert_eq!(expected, actual);
        assert_eq!(runs.noise_skip, Some(0));
        runs.fill_prepared(
            PreparedRandom::from_kind(RandomKind::Noise {
                probability: 0.001,
                choices: 1,
            }),
            &mut [],
            &[],
        );
        assert_eq!(runs.noise_probability, p.to_bits());
        assert_eq!(scalar.draw(RandomKind::Active), {
            let mut active = [0];
            runs.fill_prepared(PreparedRandom::Active, &mut active, &[RandomKind::Active]);
            active[0]
        });
        let mut success = [0];
        runs.fill_prepared(
            kind,
            &mut success,
            &[RandomKind::Noise {
                probability: p,
                choices: 1,
            }],
        );
        assert_eq!(success, [1]);
        assert_eq!(
            scalar.draw(RandomKind::Noise {
                probability: p,
                choices: 1
            }),
            1
        );
        assert_eq!(scalar.noise_skip, runs.noise_skip);
        drop(scalar);
        drop(runs);
        assert_eq!(a.next_u64(), b.next_u64());
    }
}
