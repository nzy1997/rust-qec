//! Optional Noise storage; full event tape remains allocated for dead-row replay.
use super::*;

pub(super) struct NoisePacket {
    planes: Vec<[u64; 4]>,
    needs_clear: bool,
}

impl NoisePacket {
    #[cfg(test)]
    pub(super) fn lazy_error_reserved_bytes(&self) -> usize {
        self.planes
            .capacity()
            .checked_mul(size_of::<[u64; 4]>())
            .unwrap()
            .checked_add(size_of::<Self>())
            .unwrap()
    }
    pub(super) fn new(kinds: &[RandomKind], count: usize, remaining_bytes: usize) -> Option<Self> {
        if count == 0
            || kinds.iter().any(|kind| {
                matches!(kind, RandomKind::Noise { choices, .. } if !(1..=15).contains(choices))
            })
        {
            return None;
        }
        let requested = count
            .checked_mul(size_of::<[u64; 4]>())?
            .checked_add(size_of::<Self>())?;
        if requested > remaining_bytes {
            return None;
        }
        let mut planes = Vec::new();
        planes.try_reserve_exact(count).ok()?;
        let allocated = planes
            .capacity()
            .checked_mul(size_of::<[u64; 4]>())?
            .checked_add(size_of::<Self>())?;
        if allocated > remaining_bytes {
            return None; // Optional allocation drops before any row draws.
        }
        planes.resize(count, [0; 4]);
        Some(Self {
            planes,
            needs_clear: false,
        })
    }

    pub(super) fn clear(&mut self) {
        // Construction has already zeroed the first packet's storage.
        if self.needs_clear {
            self.planes.fill([0; 4]);
        }
        self.needs_clear = true;
    }

    pub(super) fn planes(&mut self) -> &mut [[u64; 4]] {
        &mut self.planes
    }

    #[inline]
    fn value(&self, noise: usize, lane: usize) -> u64 {
        let [a, b, c, d] = self.planes[noise];
        ((a >> lane) & 1)
            | (((b >> lane) & 1) << 1)
            | (((c >> lane) & 1) << 2)
            | (((d >> lane) & 1) << 3)
    }

    pub(super) fn choice_masks(&self, noise: usize, selected: u64, choices: usize) -> [u64; 15] {
        let mut result = [0; 15];
        let mut hits = selected;
        while hits != 0 {
            let lane = hits.trailing_zeros() as usize;
            hits &= hits - 1;
            let choice = self.value(noise, lane) as usize;
            debug_assert!((1..=choices).contains(&choice));
            result[choice - 1] |= 1u64 << lane;
        }
        result
    }

    pub(super) fn restore_row(&self, kinds: &[RandomKind], row: &mut [u64], lane: usize) {
        debug_assert_eq!(kinds.len(), row.len());
        let mut noise = 0;
        for (&kind, value) in kinds.iter().zip(row) {
            if matches!(kind, RandomKind::Noise { .. }) {
                // Includes every zero; retained tape cells can belong to a prior packet.
                *value = self.value(noise, lane);
                noise += 1;
            }
        }
        debug_assert_eq!(noise, self.planes.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn compact_noise_restores_original_typed_rows_and_carry_with_both_independent_producers() {
        let sparse = |choices| RandomKind::Noise {
            probability: 0.01,
            choices,
        };
        // Choice changes stay in one sparse run; unlike events retain pending
        // carry, and different sparse p resets it. Include all category widths.
        let mut kinds = vec![sparse(1), sparse(3), sparse(15)];
        kinds.extend(std::iter::repeat_n(RandomKind::Independent, 65));
        kinds.extend([
            RandomKind::Active,
            RandomKind::Noise {
                probability: 0.,
                choices: 15,
            },
            RandomKind::Noise {
                probability: 1.,
                choices: 1,
            },
            RandomKind::Noise {
                probability: 1.,
                choices: 15,
            },
            RandomKind::Noise {
                probability: 0.37,
                choices: 3,
            },
            sparse(15),
            sparse(1),
            RandomKind::Active,
        ]);
        kinds.extend((0..131).map(|i| sparse([1, 3, 15][i % 3])));
        kinds.extend([
            RandomKind::Noise {
                probability: 0.001,
                choices: 15,
            },
            RandomKind::Noise {
                probability: 0.001,
                choices: 1,
            },
            sparse(3),
        ]);
        let count = kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Noise { .. }))
            .count();
        let runs = RandomRunPlan::build(&kinds, PLAN_BYTE_BUDGET).unwrap();
        for compact_independent in [false, true] {
            let mut packet = NoisePacket::new(&kinds, count, PACKET_BYTE_BUDGET).unwrap();
            let mut independent = compact_independent
                .then(|| IndependentPacket::new(65, PACKET_BYTE_BUDGET).unwrap());
            let mut a = StdRng::seed_from_u64(1603);
            let mut b = a.clone();
            // Clear/reuse includes a short packet between complete packets.
            for lanes in [64, 7, 64] {
                packet.clear();
                let mut masks = vec![0; count];
                let mut expected_rows = Vec::new();
                let mut rows = vec![99; lanes * kinds.len()];
                for lane in 0..lanes {
                    let mut scalar = RowRandom::live(&mut a);
                    let mut compact = RowRandom::live(&mut b);
                    // An exact run boundary leaves Some(0), not a renewed draw.
                    scalar.noise_probability = 0.01f64.to_bits();
                    compact.noise_probability = 0.01f64.to_bits();
                    scalar.noise_skip = Some(3);
                    compact.noise_skip = Some(3);
                    expected_rows.push(
                        kinds
                            .iter()
                            .map(|&kind| scalar.draw(kind))
                            .collect::<Vec<_>>(),
                    );
                    let row = &mut rows[lane * kinds.len()..(lane + 1) * kinds.len()];
                    runs.fill_row_with_compact_noise(
                        &mut compact,
                        &kinds,
                        row,
                        &mut masks,
                        1u64 << lane,
                        packet.planes(),
                        independent.as_mut().map(|packet| packet.row(lane)),
                    );
                    for (&kind, &value) in kinds.iter().zip(row.iter()) {
                        if matches!(kind, RandomKind::Noise { .. }) {
                            assert_eq!(value, 99, "Noise tape must not be filled or cleared");
                        }
                    }
                    assert_eq!(scalar.noise_probability, compact.noise_probability);
                    assert_eq!(scalar.noise_skip, compact.noise_skip);
                    assert_eq!(scalar.bit_word, compact.bit_word);
                    assert_eq!(scalar.bits_left, compact.bits_left);
                    // Transition back to the scalar producer with pending carry.
                    for kind in [sparse(15), RandomKind::Active, sparse(1)] {
                        assert_eq!(scalar.draw(kind), compact.draw(kind));
                    }
                }
                if let Some(independent) = &mut independent {
                    independent.transpose(lanes);
                }
                let mut noise = 0;
                for (event, &kind) in kinds.iter().enumerate() {
                    let RandomKind::Noise { choices, .. } = kind else {
                        continue;
                    };
                    let mut expected_masks = [0; 15];
                    let mut selected = 0;
                    for (lane, row) in expected_rows.iter().enumerate() {
                        let value = row[event] as usize;
                        if value != 0 {
                            selected |= 1u64 << lane;
                            expected_masks[value - 1] |= 1u64 << lane;
                        }
                    }
                    assert_eq!(masks[noise], selected);
                    assert_eq!(
                        packet.choice_masks(noise, selected, choices),
                        expected_masks
                    );
                    noise += 1;
                }
                for (lane, expected) in expected_rows.iter().enumerate() {
                    let row = &mut rows[lane * kinds.len()..(lane + 1) * kinds.len()];
                    packet.restore_row(&kinds, row, lane);
                    if let Some(independent) = &independent {
                        independent.restore_row(&kinds, row, lane);
                    }
                    assert_eq!(&*row, expected.as_slice());
                }
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }

    #[test]
    fn compact_noise_rejects_before_drawing_when_width_or_capacity_is_unavailable() {
        let kinds = [RandomKind::Noise {
            probability: 0.001,
            choices: 15,
        }];
        let requested = size_of::<[u64; 4]>() + size_of::<NoisePacket>();
        assert!(NoisePacket::new(&kinds, 1, requested - 1).is_none());
        assert!(NoisePacket::new(&kinds, usize::MAX, PACKET_BYTE_BUDGET).is_none());
        assert!(NoisePacket::new(&kinds, 0, PACKET_BYTE_BUDGET).is_none());
        let invalid = [RandomKind::Noise {
            probability: 0.001,
            choices: 16,
        }];
        assert!(NoisePacket::new(&invalid, 1, PACKET_BYTE_BUDGET).is_none());
        let packet = NoisePacket::new(&kinds, 1, PACKET_BYTE_BUDGET).unwrap();
        assert!(
            packet.planes.capacity() * size_of::<[u64; 4]>() + size_of::<NoisePacket>()
                <= PACKET_BYTE_BUDGET
        );
    }
}
