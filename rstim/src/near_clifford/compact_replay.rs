//! Borrow compact packet events only as scalar counts replay visits them.
use super::*;

pub(super) trait RowDraw {
    fn draw(&mut self, kind: RandomKind) -> u64;
    fn discard_remaining(&mut self, kinds: &[RandomKind]);
    fn skip_zero_noise(&mut self, _maximum: usize) -> usize {
        0 // Live consumers retain their original draw path.
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
    // The producer already records nonzero lanes for every Noise event.
    // Borrow those masks with the decoder, without recomputing four-plane ORs.
    noise: Option<(&'a NoisePacket, &'a [u64])>,
    independent: Option<&'a IndependentPacket>,
    lane: usize,
    cursor: usize,
    noise_event: usize,
    independent_event: usize,
}
impl<'a> CompactReplay<'a> {
    pub(super) fn new(
        tape: &'a [u64],
        noise: Option<(&'a NoisePacket, &'a [u64])>,
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
    fn skip_zero_noise(&mut self, maximum: usize) -> usize {
        // The prepared span contains only Noise nodes, each with one event.
        // Compact cells in the full tape may be stale: consult the sidecar
        // when present, then advance both cursors only past literal zeroes.
        let end = self
            .cursor
            .checked_add(maximum)
            .expect("random tape cursor overflow");
        let values = self
            .tape
            .get(self.cursor..end)
            .expect("recorded random tape is incomplete");
        let skipped = if let Some((_, hits)) = self.noise {
            let end = self
                .noise_event
                .checked_add(maximum)
                .expect("noise cursor overflow");
            let masks = hits
                .get(self.noise_event..end)
                .expect("compact noise is incomplete");
            let lane = 1u64 << self.lane;
            masks
                .iter()
                .position(|&mask| mask & lane != 0)
                .unwrap_or(maximum)
        } else {
            values
                .iter()
                .position(|&value| value != 0)
                .unwrap_or(maximum)
        };
        self.cursor += skipped;
        if self.noise.is_some() {
            self.noise_event += skipped;
        }
        skipped
    }

    #[inline]
    fn draw(&mut self, kind: RandomKind) -> u64 {
        assert!(
            self.cursor < self.tape.len(),
            "recorded random tape is incomplete"
        );
        let value = match kind {
            RandomKind::Noise { .. } if self.noise.is_some() => {
                let value = self.noise.unwrap().0.value(self.noise_event, self.lane);
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
    fn zero_noise_spans_stop_at_hits_without_reading_stale_optional_cells() {
        let noise_kind = RandomKind::Noise {
            probability: 0.001,
            choices: 15,
        };
        let mut kinds = vec![noise_kind; 10];
        kinds.extend([RandomKind::Independent, RandomKind::Active]);
        kinds.extend([noise_kind; 4]);
        kinds.push(RandomKind::Independent);
        kinds.extend(std::iter::repeat_n(RandomKind::Independent, 127));
        let mut expected = vec![0, 0, 0, 15, 0, 0, 0, 0, 8, 0, 1, 0xabcdef, 0, 0, 1, 0, 0];
        expected.extend((0..127).map(|i| (i % 2) as u64));
        for lane in [0, 1, 31, 32, 63] {
            let mut noise = NoisePacket::new(&kinds, 14, usize::MAX).unwrap();
            let mask = 1u64 << lane;
            // Other lanes hit every event; only this lane follows literal values.
            let mut noise_hits = vec![!mask; 14];
            let mut n = 0;
            for (&kind, &value) in kinds.iter().zip(&expected) {
                if matches!(kind, RandomKind::Noise { .. }) {
                    for bit in 0..4 {
                        noise.planes()[n][bit] = !mask | (((value >> bit) & 1) << lane);
                    }
                    if value != 0 {
                        noise_hits[n] |= mask;
                    }
                    n += 1;
                }
            }
            let mut independent = IndependentPacket::new(129, usize::MAX).unwrap();
            let words = independent.row(lane);
            let mut i = 0;
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
                        use_noise.then_some((&noise, noise_hits.as_slice())),
                        use_independent.then_some(&independent),
                        lane,
                    );
                    assert_eq!(replay.skip_zero_noise(0), 0);
                    assert_eq!(replay.skip_zero_noise(3), 3);
                    assert_eq!(replay.draw(noise_kind), 15);
                    assert_eq!(replay.skip_zero_noise(6), 4);
                    assert_eq!(replay.draw(noise_kind), 8);
                    assert_eq!(replay.skip_zero_noise(1), 1);
                    assert_eq!(replay.draw(RandomKind::Independent), 1);
                    assert_eq!(replay.draw(RandomKind::Active), 0xabcdef);
                    assert_eq!(replay.skip_zero_noise(4), 2);
                    assert_eq!(replay.draw(noise_kind), 1);
                    assert_eq!(replay.skip_zero_noise(1), 1);
                    assert_eq!(replay.draw(RandomKind::Independent), 0);
                    for &value in &expected[17..] {
                        assert_eq!(replay.draw(RandomKind::Independent), value);
                    }
                    assert_eq!(replay.cursor(), expected.len());
                    assert_eq!(replay.noise_event, if use_noise { 14 } else { 0 });
                    assert_eq!(
                        replay.independent_event,
                        if use_independent { 129 } else { 0 }
                    );
                }
            }
        }
    }

    #[test]
    fn prepared_compact_counts_skip_zero_noise_with_literal_records_and_continuation() {
        struct Observed<'a> {
            row: CompactReplay<'a>,
            skipped: usize,
        }
        impl RowDraw for Observed<'_> {
            fn draw(&mut self, kind: RandomKind) -> u64 {
                self.row.draw(kind)
            }
            fn discard_remaining(&mut self, kinds: &[RandomKind]) {
                self.row.discard_remaining(kinds);
            }
            fn skip_zero_noise(&mut self, maximum: usize) -> usize {
                let n = self.row.skip_zero_noise(maximum);
                self.skipped += n;
                n
            }
        }
        let text = "H 3\nM 3\nH 0 1 2\nT 0 1 2\nREPEAT 40 {\nDEPOLARIZE1(0) 0 1 2\nX_ERROR(0) 1\n}\nDEPOLARIZE1(1) 3\nMY(1) 0\nCX rec[-1] 2\nCX sweep[1] 1\nT_DAG 2\nMPP X1*Y2\nM 0 1 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\n";
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
            assert!(plan.noise_spans.is_some());
            for seed in [739, 1739, 9173, 1667] {
                let mut caller = StdRng::seed_from_u64(seed);
                let tape: Vec<_> = {
                    let mut live = RowRandom::live(&mut caller);
                    plan.random_kinds
                        .iter()
                        .map(|&kind| live.draw(kind))
                        .collect()
                };
                let mut untouched = caller.clone();
                let mut noise =
                    NoisePacket::new(&plan.random_kinds, plan.noise_event_count, usize::MAX)
                        .unwrap();
                let mut noise_hits = vec![0; plan.noise_event_count];
                let mut n = 0;
                for (&kind, &value) in plan.random_kinds.iter().zip(&tape) {
                    if matches!(kind, RandomKind::Noise { .. }) {
                        for bit in 0..4 {
                            noise.planes()[n][bit] = ((value >> bit) & 1) << 63;
                        }
                        noise_hits[n] = u64::from(value != 0) << 63;
                        n += 1;
                    }
                }
                for sidecar in [None, Some(&noise)] {
                    let stale: Vec<_> = plan
                        .random_kinds
                        .iter()
                        .zip(&tape)
                        .map(|(&kind, &value)| {
                            if sidecar.is_some() && matches!(kind, RandomKind::Noise { .. }) {
                                u64::MAX
                            } else {
                                value
                            }
                        })
                        .collect();
                    let mut replay = Observed {
                        row: CompactReplay::new(
                            &stale,
                            sidecar.map(|packet| (packet, noise_hits.as_slice())),
                            None,
                            63,
                        ),
                        skipped: 0,
                    };
                    let selected = plan
                        .prepare_sampler()
                        .unwrap()
                        .row_with_random_kernel::<true, true>(&[true, false], &mut replay)
                        .unwrap();
                    assert!(
                        replay.skipped >= 160,
                        "must actually retire eligible compact zero Noise nodes"
                    );
                    assert_eq!(replay.row.cursor(), tape.len());
                    let expected = plan
                        .prepare_sampler()
                        .unwrap()
                        .row_with_random_mode::<false>(
                            &[true, false],
                            &mut RowRandom::recorded(&tape, &mut caller),
                        )
                        .unwrap();
                    if selected.detectors.iter().all(|&bit| !bit) {
                        assert_eq!(selected, expected);
                    } else {
                        assert!(expected.detectors.iter().any(|&bit| bit));
                    }
                }
                for _ in 0..16 {
                    assert_eq!(caller.next_u64(), untouched.next_u64());
                }
            }
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            let mut original = plan.clone();
            original.random_runs = None;
            original.noise_spans = None;
            for extra in [0, 288] {
                let mut native = plan
                    .prepare_sampler_with_cache_budget(initial + extra)
                    .unwrap();
                let mut reference = original.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(1739);
                let mut b = a.clone();
                for shots in [64, 65, 129] {
                    let rows = reference
                        .sample_with_sweep(shots, &[true, false], &mut a)
                        .unwrap();
                    let accepted: Vec<_> = rows
                        .iter()
                        .filter(|row| row.detectors.iter().all(|&bit| !bit))
                        .collect();
                    let logical_errors = accepted
                        .iter()
                        .filter(|row| {
                            row.observables
                                .iter()
                                .filter(|(index, _)| *index == 7)
                                .fold(false, |parity, (_, bit)| parity ^ bit)
                        })
                        .count();
                    let counts = native
                        .sample_postselected_counts_with_sweep(shots, 7, &[true, false], &mut b)
                        .unwrap();
                    assert_eq!(
                        counts,
                        NearCliffordPostselectedCounts {
                            attempted: shots,
                            accepted: accepted.len(),
                            logical_errors
                        }
                    );
                    assert_eq!(
                        native.last_packet_live, 0,
                        "exercise admission-failure replay"
                    );
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }

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
            let mut noise_hits = vec![0; 15];
            let mut independent = IndependentPacket::new(131, PACKET_BYTE_BUDGET).unwrap();
            let (mut n, mut i) = (0, 0);
            for (&kind, &value) in kinds.iter().zip(&expected) {
                match kind {
                    RandomKind::Noise { .. } => {
                        for bit in 0..4 {
                            noise.planes()[n][bit] = ((value >> bit) & 1) << 63;
                        }
                        noise_hits[n] = u64::from(value != 0) << 63;
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
                        use_noise.then_some((&noise, noise_hits.as_slice())),
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
