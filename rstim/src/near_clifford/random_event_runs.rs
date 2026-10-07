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

    pub(super) fn reserved_bytes(&self) -> usize {
        self.runs.capacity() * size_of::<RandomRun>()
    }
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

    pub(super) fn fill_row_with_compact_independent<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        output: &mut [u64],
        noise_masks: &mut [u64],
        lane_bit: u64,
        independent_words: &mut [u64],
    ) {
        assert_eq!(output.len(), self.event_count);
        assert_eq!(kinds.len(), self.event_count);
        debug_assert!(lane_bit.is_power_of_two());
        let mut event = 0;
        let mut noise = 0;
        let mut independent = 0;
        for run in &self.runs {
            let end = event + run.count;
            match run.kind {
                PreparedRandom::Independent => {
                    random.fill_independent_words(run.count, independent_words, &mut independent);
                }
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
        debug_assert_eq!(independent.div_ceil(64), independent_words.len());
    }

    pub(super) fn fill_row_with_compact_noise<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        output: &mut [u64],
        noise_masks: &mut [u64],
        lane_bit: u64,
        planes: &mut [[u64; 4]],
        mut independent_words: Option<&mut [u64]>,
    ) {
        assert_eq!(output.len(), self.event_count);
        assert_eq!(kinds.len(), self.event_count);
        assert_eq!(planes.len(), noise_masks.len());
        debug_assert!(lane_bit.is_power_of_two());
        let mut event = 0;
        let mut noise = 0;
        let mut independent = 0;
        for run in &self.runs {
            let end = event + run.count;
            match run.kind {
                PreparedRandom::Independent if independent_words.is_some() => {
                    random.fill_independent_words(
                        run.count,
                        independent_words.as_deref_mut().unwrap(),
                        &mut independent,
                    );
                }
                PreparedRandom::Noise { .. } | PreparedRandom::Sparse { .. } => {
                    let noise_end = noise + run.count;
                    random.fill_prepared_storage::<true>(
                        run.kind,
                        &mut output[event..end],
                        Some((&mut noise_masks[noise..noise_end], lane_bit)),
                        matches!(run.kind, PreparedRandom::Sparse { .. })
                            .then_some(&kinds[event..end]),
                        Some(&mut planes[noise..noise_end]),
                    );
                    noise = noise_end;
                }
                _ => random.fill_prepared_impl(run.kind, &mut output[event..end], None, None),
            }
            event = end;
        }
        debug_assert_eq!(event, output.len());
        debug_assert_eq!(noise, noise_masks.len());
        if let Some(words) = independent_words {
            debug_assert_eq!(independent.div_ceil(64), words.len());
        }
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
        noise_masks: Option<(&mut [u64], u64)>,
        sparse_kinds: Option<&[RandomKind]>,
    ) {
        self.fill_prepared_storage::<false>(kind, output, noise_masks, sparse_kinds, None);
    }

    // Both destinations use one typed draw/carry implementation. The const flag
    // removes Noise tape stores/clears from the compact specialization only.
    fn fill_prepared_storage<const COMPACT: bool>(
        &mut self,
        kind: PreparedRandom,
        output: &mut [u64],
        mut noise_masks: Option<(&mut [u64], u64)>,
        sparse_kinds: Option<&[RandomKind]>,
        mut planes: Option<&mut [[u64; 4]]>,
    ) {
        if output.is_empty() {
            return; // Empty runs cannot alter pending probability or prefetch RNG.
        }
        if let Some(tape) = self.tape {
            let end = self.cursor + output.len();
            if COMPACT {
                let lane = noise_masks.as_ref().unwrap().1;
                for (index, &value) in tape[self.cursor..end].iter().enumerate() {
                    write_noise_event::<true>(output, index, value, &mut planes, lane);
                }
            } else {
                output.copy_from_slice(&tape[self.cursor..end]);
            }
            self.cursor = end;
            if let Some((masks, lane)) = noise_masks {
                for (&value, mask) in tape[end - output.len()..end].iter().zip(masks) {
                    if value != 0 {
                        *mask |= lane;
                    }
                }
            }
            return; // Same replay values, no RNG or bit/geometric state updates.
        }
        let lane_bit = noise_masks.as_ref().map_or(0, |(_, lane)| *lane);
        match kind {
            PreparedRandom::Independent => {
                let mut offset = 0;
                while offset < output.len() {
                    let (word, take) = self.take_independent(output.len() - offset);
                    for (bit, value) in output[offset..offset + take].iter_mut().enumerate() {
                        *value = (word >> bit) & 1;
                    }
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
                    if !COMPACT {
                        clear_events(output);
                    }
                } else if probability == 1. && choices == 1 {
                    if COMPACT {
                        for index in 0..output.len() {
                            write_noise_event::<true>(output, index, 1, &mut planes, lane_bit);
                        }
                    } else {
                        output.fill(1);
                    }
                    if let Some((masks, lane)) = noise_masks {
                        for mask in masks {
                            *mask |= lane;
                        }
                    }
                } else if probability == 1. {
                    for index in 0..output.len() {
                        let value = (self.rng.gen_range(0..choices) + 1) as u64;
                        write_noise_event::<COMPACT>(output, index, value, &mut planes, lane_bit);
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
                    for index in 0..output.len() {
                        let value = if !near_noise_occurs(probability, self.rng) {
                            0
                        } else if choices == 1 {
                            1
                        } else {
                            (self.rng.gen_range(0..choices) + 1) as u64
                        };
                        write_noise_event::<COMPACT>(output, index, value, &mut planes, lane_bit);
                        if value != 0 {
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
                // Live, nonempty prepared runs already own this exact denominator.
                // Populate it even at the same p, so prepared -> scalar reuses it.
                self.noise_log_failure = log_failure;
                let mut offset = 0;
                while offset < output.len() {
                    let remaining = output.len() - offset;
                    let Some(failures) = self.sparse_hit(remaining) else {
                        if !COMPACT {
                            clear_events(&mut output[offset..]);
                        }
                        break;
                    };
                    if !COMPACT {
                        clear_events(&mut output[offset..offset + failures]);
                    }
                    offset += failures;
                    // All skipped events are same-p sparse Noise; their choice
                    // count cannot affect Bernoulli failures or pending carry.
                    // Read the original event ONLY at a success, before drawing
                    // that event's category with the original usize range type.
                    let RandomKind::Noise { choices, .. } = sparse_kinds
                        .expect("sparse runs borrow their original typed event slice")[offset]
                    else {
                        unreachable!("sparse runs contain only noise events")
                    };
                    let value = self.noise_choice(choices);
                    write_noise_event::<COMPACT>(output, offset, value, &mut planes, lane_bit);
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

#[inline]
fn write_noise_event<const COMPACT: bool>(
    output: &mut [u64],
    index: usize,
    value: u64,
    planes: &mut Option<&mut [[u64; 4]]>,
    lane_bit: u64,
) {
    if COMPACT {
        if value != 0 {
            debug_assert!(value <= 15);
            let plane = &mut planes.as_deref_mut().unwrap()[index];
            for (bit, word) in plane.iter_mut().enumerate() {
                if value >> bit & 1 != 0 {
                    *word |= lane_bit;
                }
            }
        }
    } else {
        output[index] = value;
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
        let mut c = a.clone();
        let mut d = a.clone();
        let mut active = vec![0u64; kinds.len().div_ceil(64)];
        for event in 0..kinds.len() {
            if (event * 7 + event / 31) % 11 < 5 {
                active[event / 64] |= 1u64 << (event % 64);
            }
        }
        for _ in 0..8 {
            let mut scalar = RowRandom::live(&mut a);
            let mut runs = RowRandom::live(&mut b);
            let mut visitor = RowRandom::live(&mut c);
            let mut selected = RowRandom::live(&mut d);
            let expected = kinds
                .iter()
                .map(|&kind| scalar.draw(kind))
                .collect::<Vec<_>>();
            let mut actual = vec![0; kinds.len()];
            plan.fill_row(&mut runs, kinds, &mut actual);
            assert_eq!(expected, actual);
            let mut scattered = vec![0; kinds.len()];
            plan.visit_nonzero(&mut visitor, kinds, |event, value| scattered[event] = value);
            assert_eq!(expected, scattered);
            assert_eq!(scalar.bit_word, visitor.bit_word);
            assert_eq!(scalar.bits_left, visitor.bits_left);
            assert_eq!(scalar.noise_probability, visitor.noise_probability);
            assert_eq!(scalar.noise_skip, visitor.noise_skip);
            scattered.fill(0);
            plan.visit_selected(&mut selected, kinds, &active, |event, value| {
                scattered[event] = value
            });
            for (event, (&expected, &actual)) in expected.iter().zip(&scattered).enumerate() {
                assert_eq!(
                    actual,
                    if active[event / 64] >> (event % 64) & 1 != 0 {
                        expected
                    } else {
                        0
                    }
                );
            }
            assert_eq!(scalar.bit_word, selected.bit_word);
            assert_eq!(scalar.bits_left, selected.bits_left);
            assert_eq!(scalar.noise_probability, selected.noise_probability);
            assert_eq!(scalar.noise_skip, selected.noise_skip);
            let mut replay_rng = StdRng::seed_from_u64(99);
            let before = replay_rng.clone().next_u64();
            let mut replay = RowRandom::recorded(&expected, &mut replay_rng);
            scattered.fill(0);
            plan.visit_nonzero(&mut replay, kinds, |event, value| scattered[event] = value);
            assert_eq!(expected, scattered);
            assert_eq!(replay.cursor, kinds.len());
            assert_eq!(before, replay_rng.next_u64());
            assert_eq!(scalar.bit_word, runs.bit_word);
            assert_eq!(scalar.bits_left, runs.bits_left);
            assert_eq!(scalar.noise_probability, runs.noise_probability);
            assert_eq!(scalar.noise_skip, runs.noise_skip);
        }
        for _ in 0..16 {
            let expected = a.next_u64();
            assert_eq!(expected, b.next_u64());
            assert_eq!(expected, c.next_u64());
            assert_eq!(expected, d.next_u64());
        }
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
        let before_empty_log = runs.noise_log_failure.to_bits();
        runs.fill_prepared(
            PreparedRandom::from_kind(RandomKind::Noise {
                probability: 0.001,
                choices: 1,
            }),
            &mut [],
            &[],
        );
        assert_eq!(runs.noise_probability, p.to_bits());
        assert_eq!(runs.noise_log_failure.to_bits(), before_empty_log);
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

impl RandomRunPlan {
    #[cfg(test)]
    pub(super) fn visit_nonzero<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        emit: impl FnMut(usize, u64),
    ) {
        self.visit_impl(random, kinds, None, emit);
    }
    pub(super) fn visit_selected<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        active: &[u64],
        emit: impl FnMut(usize, u64),
    ) {
        debug_assert_eq!(active.len(), kinds.len().div_ceil(64));
        self.visit_impl(random, kinds, Some(active), emit);
    }
    fn visit_impl<R: Rng>(
        &self,
        random: &mut RowRandom<'_, R>,
        kinds: &[RandomKind],
        active: Option<&[u64]>,
        mut emit: impl FnMut(usize, u64),
    ) {
        let needed =
            |event: usize| active.is_none_or(|words| words[event / 64] >> (event % 64) & 1 != 0);
        debug_assert_eq!(kinds.len(), self.event_count);
        if let Some(tape) = random.tape {
            for (event, &value) in tape[random.cursor..random.cursor + self.event_count]
                .iter()
                .enumerate()
            {
                if value != 0 && needed(event) {
                    emit(event, value);
                }
            }
            random.cursor += self.event_count;
            return;
        }
        let mut event = 0;
        for run in &self.runs {
            match run.kind {
                PreparedRandom::Independent => {
                    let mut offset = 0;
                    while offset < run.count {
                        let (mut bits, take) = random.take_independent(run.count - offset);
                        if let Some(words) = active {
                            let start = event + offset;
                            let shift = start % 64;
                            let mut mask = words[start / 64] >> shift;
                            if shift != 0 && take > 64 - shift {
                                mask |= words[start / 64 + 1] << (64 - shift);
                            }
                            bits &= mask;
                        }
                        while bits != 0 {
                            let bit = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            emit(event + offset + bit, 1);
                        }
                        offset += take;
                    }
                }
                PreparedRandom::Sparse {
                    probability_bits,
                    log_failure,
                } => {
                    if random.noise_probability != probability_bits {
                        random.noise_skip = None;
                        random.noise_probability = probability_bits;
                    }
                    random.noise_log_failure = log_failure;
                    let mut offset = 0;
                    while offset < run.count {
                        let Some(failures) = random.sparse_hit(run.count - offset) else {
                            break;
                        };
                        offset += failures;
                        let RandomKind::Noise { choices, .. } = kinds[event + offset] else {
                            unreachable!("sparse run kinds")
                        };
                        let value = random.noise_choice(choices);
                        if needed(event + offset) {
                            emit(event + offset, value);
                        }
                        offset += 1;
                    }
                }
                _ => {
                    for offset in 0..run.count {
                        let value = random.draw(kinds[event + offset]);
                        if value != 0 && needed(event + offset) {
                            emit(event + offset, value);
                        }
                    }
                }
            }
            event += run.count;
        }
        debug_assert_eq!(event, kinds.len());
    }
}
