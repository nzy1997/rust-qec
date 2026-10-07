//! Borrow compact packet events only as scalar counts replay visits them.
use super::*;

pub(super) trait RowDraw {
    fn draw(&mut self, kind: RandomKind) -> u64;
    fn discard_remaining(&mut self, kinds: &[RandomKind]);
}

impl<R: Rng> RowDraw for RowRandom<'_, R> {
    #[inline]
    fn draw(&mut self, kind: RandomKind) -> u64 {
        RowRandom::draw(self, kind)
    }
    fn discard_remaining(&mut self, kinds: &[RandomKind]) {
        RowRandom::discard_remaining(self, kinds);
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
