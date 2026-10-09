//! Optional spans of Noise operations, separated by every other operation.
use super::*;

const MIN_SPAN: usize = 4;
const BYTE_BUDGET: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NoiseSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Clone, Debug)]
pub(super) struct NoiseSpans {
    spans: Vec<NoiseSpan>,
}

fn visit(operations: &[PlanOp], prefix: usize, mut emit: impl FnMut(NoiseSpan)) {
    let mut start = prefix;
    for node in prefix..=operations.len() {
        if node < operations.len() && matches!(operations[node], PlanOp::Noise { .. }) {
            continue;
        }
        if node - start >= MIN_SPAN {
            emit(NoiseSpan { start, end: node });
        }
        start = node + 1;
    }
}

impl NoiseSpans {
    pub(super) fn build(operations: &[PlanOp], prefix: usize, remaining: usize) -> Option<Self> {
        let budget = remaining.min(BYTE_BUDGET);
        let mut count = 0usize;
        visit(operations, prefix, |_| count += 1);
        if count == 0
            || count
                .checked_mul(size_of::<NoiseSpan>())?
                .checked_add(size_of::<Self>())?
                > budget
        {
            return None;
        }
        let mut spans = Vec::new();
        spans.try_reserve_exact(count).ok()?;
        if spans
            .capacity()
            .checked_mul(size_of::<NoiseSpan>())?
            .checked_add(size_of::<Self>())?
            > budget
        {
            return None;
        }
        visit(operations, prefix, |span| spans.push(span));
        Some(Self { spans })
    }

    pub(super) fn reserved_bytes(&self) -> usize {
        self.spans.capacity() * size_of::<NoiseSpan>() + size_of::<Self>()
    }

    pub(super) fn spans(&self) -> &[NoiseSpan] {
        &self.spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise() -> PlanOp {
        PlanOp::Noise {
            probability: 0.001,
            choices: Vec::new(),
        }
    }

    #[test]
    fn spans_preserve_all_non_noise_boundaries_and_optional_budget() {
        let boundaries = vec![
            PlanOp::Basis(Vec::new()),
            PlanOp::Rotate {
                pauli: CompactPauli {
                    physical: PackedPauli {
                        x: vec![0],
                        z: vec![0],
                        phase: 0,
                    },
                    x: 0,
                    z: 0,
                },
                expand: true,
                dagger: false,
            },
            PlanOp::Annotation {
                offsets: Vec::new(),
                observable: None,
            },
            PlanOp::Feedback {
                condition: Condition::Sweep(1),
                pauli: PackedPauli {
                    x: vec![1],
                    z: vec![0],
                    phase: 0,
                },
            },
            PlanOp::Measure(Measurement {
                pauli: CompactPauli {
                    physical: PackedPauli {
                        x: vec![0],
                        z: vec![1],
                        phase: 0,
                    },
                    x: 0,
                    z: 0,
                },
                projection: Projection::Independent { pivot: 0 },
                basis: Vec::new(),
                record: Some(0),
                inverted: false,
                readout: 0.03,
                reset: None,
            }),
        ];
        let mut operations = vec![noise(), noise(), noise()];
        let mut expected = Vec::new();
        for boundary in boundaries {
            operations.push(boundary);
            let start = operations.len();
            operations.extend((0..7).map(|_| noise()));
            expected.push(NoiseSpan {
                start,
                end: start + 7,
            });
        }
        let spans = NoiseSpans::build(&operations, 3, usize::MAX).unwrap();
        assert_eq!(spans.spans(), expected);
        for span in spans.spans() {
            assert!(
                operations[span.start..span.end]
                    .iter()
                    .all(|op| matches!(op, PlanOp::Noise { .. }))
            );
        }
        let bytes = spans.reserved_bytes();
        assert!(bytes <= BYTE_BUDGET);
        assert!(NoiseSpans::build(&operations, 3, bytes - 1).is_none());
        assert_eq!(
            NoiseSpans::build(&operations, 3, bytes).unwrap().spans(),
            expected
        );
        assert!(NoiseSpans::build(&operations[..3], 0, usize::MAX).is_none());
        assert!(NoiseSpans::build(&operations, operations.len(), usize::MAX).is_none());
    }

    #[test]
    fn recorded_zero_prefix_stops_at_each_hit_and_live_rows_keep_their_draws() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        let tape = [0, 0, 15, 0, 0, 0, 1, 0];
        let kind = RandomKind::Noise {
            probability: 0.001,
            choices: 15,
        };
        let mut a = StdRng::seed_from_u64(739);
        let mut b = a.clone();
        let mut recorded = RowRandom::recorded(&tape, &mut a);
        assert_eq!(recorded.skip_zero_noise(0), 0);
        assert_eq!(recorded.skip_zero_noise(3), 2);
        assert_eq!(recorded.draw(kind), 15);
        assert_eq!(recorded.skip_zero_noise(2), 2);
        assert_eq!(recorded.skip_zero_noise(3), 1);
        assert_eq!(recorded.draw(kind), 1);
        assert_eq!(recorded.skip_zero_noise(1), 1);
        assert_eq!(recorded.cursor, tape.len());
        assert_eq!(a.next_u64(), b.next_u64());
        let mut live = RowRandom::live(&mut a);
        assert_eq!(live.skip_zero_noise(128), 0);
        let mut literal = RowRandom::live(&mut b);
        for _ in 0..80 {
            assert_eq!(live.draw(kind), literal.draw(kind));
        }
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn prepared_counts_retire_actual_zero_noise_nodes_with_literal_records_and_rng() {
        use rand::{RngCore, SeedableRng, rngs::StdRng};
        struct Observed<'a, R> {
            row: RowRandom<'a, R>,
            skipped: usize,
        }
        impl<R: Rng> RowDraw for Observed<'_, R> {
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
        let text = "H 0 1 2\nT 0 1 2\nREPEAT 40 {\nDEPOLARIZE1(0) 0 1 2\nX_ERROR(0) 1\n}\nMY(0.03) 0\nCX rec[-1] 2\nT_DAG 2\nMPP X1*Y2\nM 0 1 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\n";
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
            assert!(plan.noise_spans.is_some());
            let mut a = StdRng::seed_from_u64(1739);
            let mut b = a.clone();
            let tape: Vec<_> = {
                let mut live = RowRandom::live(&mut a);
                plan.random_kinds
                    .iter()
                    .map(|&kind| live.draw(kind))
                    .collect()
            };
            let mut producer = RowRandom::live(&mut b);
            for (&kind, &expected) in plan.random_kinds.iter().zip(&tape) {
                assert_eq!(producer.draw(kind), expected);
            }
            let mut replay = Observed {
                row: RowRandom::recorded(&tape, &mut a),
                skipped: 0,
            };
            let selected = plan
                .prepare_sampler()
                .unwrap()
                .row_with_random_kernel::<true, true, false>(&[], &mut replay)
                .unwrap();
            assert!(
                replay.skipped >= 128,
                "must actually retire eligible zero Noise operations"
            );
            assert_eq!(replay.row.cursor, tape.len());
            let expected = plan
                .prepare_sampler()
                .unwrap()
                .row_with_random_mode::<false, false>(&[], &mut RowRandom::recorded(&tape, &mut b))
                .unwrap();
            if selected.detectors.iter().all(|&bit| !bit) {
                assert_eq!(selected, expected);
            } else {
                assert!(expected.detectors.iter().any(|&bit| bit));
            }
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}
