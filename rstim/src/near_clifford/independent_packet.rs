//! Optional compact Independent producer; full event tape remains the replay contract.
use super::*;
use crate::sim::bit_transpose::transpose_64x64;

// Small independent runs amortize packing at eight events in the application scouts.
const MIN_INDEPENDENT_COUNT: usize = 8;

pub(super) struct IndependentPacket {
    rows: Vec<u64>,
    masks: Vec<u64>,
    words_per_row: usize,
}

impl IndependentPacket {
    #[cfg(test)]
    pub(super) fn snapshot(&self) -> (Vec<u64>, usize, Vec<u64>, usize, usize) {
        (
            self.rows.clone(),
            self.rows.capacity(),
            self.masks.clone(),
            self.masks.capacity(),
            self.words_per_row,
        )
    }
    pub(super) fn new(count: usize, remaining_bytes: usize) -> Option<Self> {
        // Small independent populations retain the existing fill/gather path.
        if count < MIN_INDEPENDENT_COUNT {
            return None;
        }
        let words_per_row = count.div_ceil(64);
        let row_len = words_per_row.checked_mul(64)?;
        let requested = row_len
            .checked_add(count)?
            .checked_mul(size_of::<u64>())?
            .checked_add(size_of::<Self>())?
            .checked_add(size_of::<[u64; 64]>())?;
        if requested > remaining_bytes {
            return None;
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(row_len).ok()?;
        let mut masks = Vec::new();
        masks.try_reserve_exact(count).ok()?;
        let allocated = rows
            .capacity()
            .checked_add(masks.capacity())?
            .checked_mul(size_of::<u64>())?
            .checked_add(size_of::<Self>())?
            .checked_add(size_of::<[u64; 64]>())?;
        if allocated > remaining_bytes {
            return None; // Both temporary allocations drop before existing fill runs.
        }
        rows.resize(row_len, 0);
        masks.resize(count, 0);
        Some(Self {
            rows,
            masks,
            words_per_row,
        })
    }

    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        self.rows
            .capacity()
            .checked_add(self.masks.capacity())?
            .checked_mul(size_of::<u64>())?
            .checked_add(size_of::<Self>())?
            .checked_add(size_of::<[u64; 64]>())
    }

    pub(super) fn row(&mut self, lane: usize) -> &mut [u64] {
        let start = lane * self.words_per_row;
        let row = &mut self.rows[start..start + self.words_per_row];
        row.fill(0); // Clears unused high bits even after an unlike prior packet.
        row
    }

    pub(super) fn transpose(&mut self, lanes: usize) {
        debug_assert!((1..=64).contains(&lanes));
        for word in 0..self.words_per_row {
            let mut tile = [0u64; 64]; // Absent tail lanes never read retained rows.
            for (lane, value) in tile[..lanes].iter_mut().enumerate() {
                *value = self.rows[lane * self.words_per_row + word];
            }
            transpose_64x64(&mut tile);
            let start = word * 64;
            let count = (self.masks.len() - start).min(64);
            self.masks[start..start + count].copy_from_slice(&tile[..count]);
        }
    }

    pub(super) fn mask(&self, independent: usize) -> u64 {
        self.masks[independent]
    }

    pub(super) fn restore_row(&self, kinds: &[RandomKind], row: &mut [u64], lane: usize) {
        debug_assert_eq!(kinds.len(), row.len());
        let mut independent = 0;
        for (&kind, value) in kinds.iter().zip(row) {
            if matches!(kind, RandomKind::Independent) {
                *value = (self.masks[independent] >> lane) & 1;
                independent += 1;
            }
        }
        debug_assert_eq!(independent, self.masks.len());
    }
}

impl<R: Rng> RowRandom<'_, R> {
    pub(super) fn fill_independent_words(
        &mut self,
        count: usize,
        words: &mut [u64],
        independent: &mut usize,
    ) {
        debug_assert!(self.tape.is_none());
        let mut remaining = count;
        while remaining != 0 {
            let bit = *independent % 64;
            let (value, take) = self.take_independent(remaining.min(64 - bit));
            words[*independent / 64] |= value << bit;
            *independent += take;
            remaining -= take;
            // Exact boundaries do not prefetch. Other typed runs keep this pool.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn transpose_matches_each_coordinate_and_clears_every_tail() {
        for count in [
            MIN_INDEPENDENT_COUNT,
            MIN_INDEPENDENT_COUNT + 1,
            64,
            65,
            127,
            128,
            129,
            193,
        ] {
            let mut packet = IndependentPacket::new(count, PACKET_BYTE_BUDGET).unwrap();
            // Coordinate oracle: no call to another transpose implementation.
            for lanes in (1..=64).rev() {
                for lane in 0..lanes {
                    let row = packet.row(lane);
                    for independent in 0..count {
                        let bit = ((lane * 13 + independent * 7 + lane * independent) % 11) < 5;
                        row[independent / 64] |= u64::from(bit) << (independent % 64);
                    }
                }
                packet.transpose(lanes);
                for independent in 0..count {
                    for lane in 0..64 {
                        let expected = lane < lanes
                            && ((lane * 13 + independent * 7 + lane * independent) % 11) < 5;
                        assert_eq!(packet.mask(independent) >> lane & 1, u64::from(expected));
                    }
                }
            }
        }
    }

    #[test]
    fn independent_single_coordinates_preserve_lane_and_bit_orientation() {
        let mut packet = IndependentPacket::new(129, PACKET_BYTE_BUDGET).unwrap();
        for lane in [0, 1, 31, 32, 63] {
            for independent in [0, 1, 31, 32, 63, 64, 65, 127, 128] {
                for reset_lane in 0..64 {
                    packet.row(reset_lane);
                }
                packet.row(lane)[independent / 64] = 1u64 << (independent % 64);
                packet.transpose(64);
                for index in 0..129 {
                    assert_eq!(
                        packet.mask(index),
                        if index == independent {
                            1u64 << lane
                        } else {
                            0
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn mixed_runs_preserve_typed_events_pool_and_rng_and_restore_only_independent_slots() {
        let mut kinds = Vec::new();
        for (kind, count) in [
            (RandomKind::Independent, 63),
            (RandomKind::Active, 2),
            (RandomKind::Independent, 2),
            (
                RandomKind::Noise {
                    probability: 0.001,
                    choices: 15,
                },
                71,
            ),
            (RandomKind::Independent, 64),
            (
                RandomKind::Noise {
                    probability: 0.001,
                    choices: 3,
                },
                1,
            ),
            (
                RandomKind::Noise {
                    probability: 0.37,
                    choices: 3,
                },
                3,
            ),
            (RandomKind::Active, 1),
            (RandomKind::Independent, 65),
            (
                RandomKind::Noise {
                    probability: 1.,
                    choices: 1,
                },
                1,
            ),
        ] {
            kinds.extend(std::iter::repeat_n(kind, count));
        }
        let independent_count = kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Independent))
            .count();
        let noise_count = kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Noise { .. }))
            .count();
        let runs = RandomRunPlan::build(&kinds, PLAN_BYTE_BUDGET).unwrap();
        let mut packet = IndependentPacket::new(independent_count, PACKET_BYTE_BUDGET).unwrap();
        for seed in [0, 1, 63, 64, 65, 2415] {
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            let mut masks = vec![0; noise_count];
            let mut reference_masks = vec![0; noise_count];
            for lane in 0..64 {
                let mut reference = RowRandom::live(&mut a);
                let mut compact = RowRandom::live(&mut b);
                expected.push(
                    kinds
                        .iter()
                        .map(|&kind| reference.draw(kind))
                        .collect::<Vec<_>>(),
                );
                let mut noise = 0;
                for (&kind, &value) in kinds.iter().zip(&expected[lane]) {
                    if matches!(kind, RandomKind::Noise { .. }) {
                        if value != 0 {
                            reference_masks[noise] |= 1u64 << lane;
                        }
                        noise += 1;
                    }
                }
                let mut row = vec![u64::MAX; kinds.len()];
                runs.fill_row_with_compact_independent(
                    &mut compact,
                    &kinds,
                    &mut row,
                    &mut masks,
                    1u64 << lane,
                    packet.row(lane),
                );
                for (&kind, &value) in kinds.iter().zip(&row) {
                    if matches!(kind, RandomKind::Independent) {
                        assert_eq!(value, u64::MAX);
                    }
                }
                assert_eq!(reference.bit_word, compact.bit_word);
                assert_eq!(reference.bits_left, compact.bits_left);
                assert_eq!(reference.noise_probability, compact.noise_probability);
                assert_eq!(reference.noise_skip, compact.noise_skip);
                actual.push(row);
            }
            assert_eq!(masks, reference_masks);
            packet.transpose(64);
            for lane in 0..64 {
                packet.restore_row(&kinds, &mut actual[lane], lane);
                assert_eq!(actual[lane], expected[lane]);
                let mut replay = RowRandom::recorded(&actual[lane], &mut b);
                for (&kind, &value) in kinds.iter().zip(&expected[lane]) {
                    assert_eq!(replay.draw(kind), value);
                }
                assert_eq!(replay.cursor, kinds.len());
            }
            assert_eq!(a.next_u64(), b.next_u64()); // Includes replay: zero redraws.
        }
    }

    #[test]
    fn optional_workspace_rejection_keeps_original_typed_events_and_continuation() {
        let mut kinds = vec![RandomKind::Independent; 63];
        kinds.push(RandomKind::Active);
        kinds.extend(std::iter::repeat_n(RandomKind::Independent, 66));
        kinds.push(RandomKind::Noise {
            probability: 0.37,
            choices: 15,
        });
        kinds.push(RandomKind::Noise {
            probability: 0.001,
            choices: 3,
        });
        kinds.push(RandomKind::Active);
        let runs = RandomRunPlan::build(&kinds, PLAN_BYTE_BUDGET).unwrap();
        let mut optional = IndependentPacket::new(129, 0);
        assert!(optional.is_none());
        for seed in [0, 1, 63, 65, 2415] {
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            for lane in 0..64 {
                let mut reference = RowRandom::live(&mut a);
                let mut generator = RowRandom::live(&mut b);
                let expected = kinds
                    .iter()
                    .map(|&kind| reference.draw(kind))
                    .collect::<Vec<_>>();
                let mut actual = vec![u64::MAX; kinds.len()];
                let mut masks = [0; 2];
                if let Some(packet) = &mut optional {
                    runs.fill_row_with_compact_independent(
                        &mut generator,
                        &kinds,
                        &mut actual,
                        &mut masks,
                        1u64 << lane,
                        packet.row(lane),
                    );
                } else {
                    runs.fill_row_with_noise_masks(
                        &mut generator,
                        &kinds,
                        &mut actual,
                        &mut masks,
                        1u64 << lane,
                    );
                }
                assert_eq!(expected, actual);
                assert_eq!(reference.bit_word, generator.bit_word);
                assert_eq!(reference.bits_left, generator.bits_left);
                assert_eq!(reference.noise_probability, generator.noise_probability);
                assert_eq!(reference.noise_skip, generator.noise_skip);
                for (slot, event) in [130, 131].into_iter().enumerate() {
                    assert_eq!(
                        masks[slot],
                        if expected[event] != 0 {
                            1u64 << lane
                        } else {
                            0
                        }
                    );
                }
            }
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn compact_workspace_rejects_budget_before_using_live_generator() {
        let count: usize = 129;
        let minimum = (64 * count.div_ceil(64) + count) * size_of::<u64>()
            + size_of::<IndependentPacket>()
            + size_of::<[u64; 64]>();
        assert!(IndependentPacket::new(count, minimum - 1).is_none());
        assert!(IndependentPacket::new(MIN_INDEPENDENT_COUNT - 1, PACKET_BYTE_BUDGET).is_none());
        assert!(IndependentPacket::new(usize::MAX, PACKET_BYTE_BUDGET).is_none());
        let packet = IndependentPacket::new(count, PACKET_BYTE_BUDGET).unwrap();
        let actual = (packet.rows.capacity() + packet.masks.capacity()) * size_of::<u64>()
            + size_of::<IndependentPacket>()
            + size_of::<[u64; 64]>();
        assert!(IndependentPacket::new(count, actual).is_some());
    }
}
