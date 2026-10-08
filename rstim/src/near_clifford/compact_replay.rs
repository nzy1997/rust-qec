//! Borrow compact packet events only as scalar counts replay visits them.
use super::*;

pub(super) trait RowDraw {
    fn draw(&mut self, kind: RandomKind) -> u64;
    fn discard_remaining(&mut self, kinds: &[RandomKind]);
    fn skip_zero_noise(&mut self, _maximum: usize) -> usize {
        0 // Live and compact-sidecar consumers retain their original path.
    }
}

impl<R: Rng> RowDraw for RowRandom<'_, R> {
    #[inline]
    fn draw(&mut self, kind: RandomKind) -> u64 {
        RowRandom::draw(self, kind)
    }
    fn discard_remaining(&mut self, kinds: &[RandomKind]) {
        RowRandom::discard_remaining(self, kinds);
    }
    #[inline]
    fn skip_zero_noise(&mut self, maximum: usize) -> usize {
        let Some(tape) = self.tape else {
            return 0;
        };
        let end = self
            .cursor
            .checked_add(maximum)
            .expect("random tape cursor overflow");
        let values = tape
            .get(self.cursor..end)
            .expect("recorded random tape is incomplete");
        let skipped = values
            .iter()
            .position(|&value| value != 0)
            .unwrap_or(maximum);
        self.cursor += skipped;
        skipped
    }
}

// All caller RNG events were already drawn when the packet was produced.
// Active values remain in the tape; compact Noise and Independent cells may
// contain retained values from an unrelated prior packet and must not be read.
pub(super) struct CompactReplay<'a> {
    tape: &'a [u64],
    noise: Option<&'a NoisePacket>,
    independent: Option<&'a IndependentPacket>,
    lane: usize,
    cursor: usize,
    noise_event: usize,
    independent_event: usize,
}
impl<'a> CompactReplay<'a> {
    pub(super) fn new(
        tape: &'a [u64],
        noise: Option<&'a NoisePacket>,
        independent: Option<&'a IndependentPacket>,
        lane: usize,
    ) -> Self {
        debug_assert!(lane < 64);
        Self {
            tape,
            noise,
            independent,
            lane,
            cursor: 0,
            noise_event: 0,
            independent_event: 0,
        }
    }
    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }
}
impl RowDraw for CompactReplay<'_> {
    #[inline]
    fn draw(&mut self, kind: RandomKind) -> u64 {
        assert!(
            self.cursor < self.tape.len(),
            "recorded random tape is incomplete"
        );
        let value = match kind {
            RandomKind::Noise { .. } if self.noise.is_some() => {
                let value = self.noise.unwrap().value(self.noise_event, self.lane);
                self.noise_event += 1;
                value
            }
            RandomKind::Independent if self.independent.is_some() => {
                let value =
                    (self.independent.unwrap().mask(self.independent_event) >> self.lane) & 1;
                self.independent_event += 1;
                value
            }
            _ => self.tape[self.cursor],
        };
        self.cursor += 1;
        value
    }
    fn discard_remaining(&mut self, kinds: &[RandomKind]) {
        let end = self
            .cursor
            .checked_add(kinds.len())
            .expect("random tape cursor overflow");
        assert!(end <= self.tape.len(), "recorded random tape is incomplete");
        self.cursor = end;
        // This retires the row. No subsequent event is read from its sidecars.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn each_optional_sidecar_replays_literal_typed_events_without_reading_stale_cells() {
        let mut kinds = vec![RandomKind::Active];
        kinds.extend(std::iter::repeat_n(RandomKind::Independent, 131));
        for probability in [0., 0.001, 0.01, 0.37, 1.] {
            for choices in [1, 3, 15] {
                kinds.push(RandomKind::Noise {
                    probability,
                    choices,
                });
            }
            kinds.push(RandomKind::Active);
        }
        for seed in [173, 917] {
            let mut rng = StdRng::seed_from_u64(seed);
            let expected: Vec<_> = {
                let mut live = RowRandom::live(&mut rng);
                kinds.iter().map(|&kind| live.draw(kind)).collect()
            };
            let mut untouched = rng.clone();
            let mut noise = NoisePacket::new(&kinds, 15, PACKET_BYTE_BUDGET).unwrap();
            let mut independent = IndependentPacket::new(131, PACKET_BYTE_BUDGET).unwrap();
            let (mut n, mut i) = (0, 0);
            for (&kind, &value) in kinds.iter().zip(&expected) {
                match kind {
                    RandomKind::Noise { .. } => {
                        for bit in 0..4 {
                            noise.planes()[n][bit] = ((value >> bit) & 1) << 63;
                        }
                        n += 1;
                    }
                    RandomKind::Independent => {
                        i += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(i, 131);
            let words = independent.row(63);
            i = 0;
            for (&kind, &value) in kinds.iter().zip(&expected) {
                if matches!(kind, RandomKind::Independent) {
                    words[i / 64] |= value << (i % 64);
                    i += 1;
                }
            }
            independent.transpose(64);
            for use_noise in [false, true] {
                for use_independent in [false, true] {
                    let tape: Vec<_> = kinds
                        .iter()
                        .zip(&expected)
                        .map(|(&kind, &value)| {
                            if (use_noise && matches!(kind, RandomKind::Noise { .. }))
                                || (use_independent && matches!(kind, RandomKind::Independent))
                            {
                                u64::MAX
                            } else {
                                value
                            }
                        })
                        .collect();
                    let mut replay = CompactReplay::new(
                        &tape,
                        use_noise.then_some(&noise),
                        use_independent.then_some(&independent),
                        63,
                    );
                    for (&kind, &value) in kinds.iter().zip(&expected) {
                        assert_eq!(
                            replay.draw(kind),
                            value,
                            "seed={seed} noise={use_noise} independent={use_independent}"
                        );
                    }
                    assert_eq!(replay.cursor(), kinds.len());
                }
            }
            for _ in 0..16 {
                assert_eq!(rng.next_u64(), untouched.next_u64());
            }
        }
    }
}
