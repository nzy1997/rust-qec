//! Counts-only restart points at failed coefficient-cache transitions.

#[derive(Clone, Copy)]
pub(super) struct ReplayPosition {
    pub(super) parent: usize,
    pub(super) node: usize,
    pub(super) event: usize,
    pub(super) noise_event: usize,
    pub(super) independent_event: usize,
    pub(super) logical: bool,
}

pub(super) struct PacketPrefix<'a> {
    pub(super) x: &'a [u64],
    pub(super) z: &'a [u64],
    pub(super) records: &'a [u64],
    pub(super) logical: u64,
}

pub(super) struct CountsResume<'a> {
    pub(super) position: ReplayPosition,
    pub(super) x: &'a [u64],
    pub(super) z: &'a [u64],
    pub(super) records: &'a [bool],
    pub(super) observable: u32,
}

pub(super) struct CountsReplay {
    positions: Vec<Option<ReplayPosition>>,
    frames: Vec<u64>,
    records: Vec<bool>,
    words: usize,
    record_count: usize,
}

// Both counts strategies use one out-of-line owner. Ordinary sampling keeps
// the original sampler header without a second counts-specific Vec field.
enum CountsStorage {
    Affine(Vec<u64>),
    Replay(CountsReplay),
}

#[derive(Default)]
pub(super) struct CountsWorkspace {
    storage: Vec<CountsStorage>,
}

impl CountsWorkspace {
    pub(super) fn as_ref(&self) -> Option<&CountsReplay> {
        match self.storage.first() {
            Some(CountsStorage::Replay(buffer)) => Some(buffer),
            _ => None,
        }
    }

    pub(super) fn as_mut(&mut self) -> Option<&mut CountsReplay> {
        match self.storage.first_mut() {
            Some(CountsStorage::Replay(buffer)) => Some(buffer),
            _ => None,
        }
    }

    pub(super) fn is_none(&self) -> bool {
        self.as_ref().is_none()
    }

    #[cfg(test)]
    pub(super) fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub(super) fn clear_replay(&mut self) {
        if self.as_ref().is_some() {
            *self = Self::default();
        }
    }

    // Optional affine admission counts actual owner and inner capacities.
    // A failed switch leaves any admitted replay snapshot untouched.
    pub(super) fn affine_outputs(&mut self, count: usize, budget: usize) -> Option<&mut Vec<u64>> {
        if matches!(self.storage.first(), Some(CountsStorage::Affine(_))) {
            let overhead = self
                .storage
                .capacity()
                .checked_mul(size_of::<CountsStorage>())?
                .checked_add(size_of::<Self>())?;
            let Some(CountsStorage::Affine(outputs)) = self.storage.first_mut() else {
                unreachable!()
            };
            if count.checked_mul(size_of::<u64>())?.checked_add(overhead)? > budget {
                return None;
            }
            if outputs.capacity() < count {
                outputs.try_reserve_exact(count - outputs.len()).ok()?;
            }
            if outputs
                .capacity()
                .checked_mul(size_of::<u64>())?
                .checked_add(overhead)?
                > budget
            {
                return None;
            }
            outputs.resize(count, 0);
            Some(outputs)
        } else {
            let requested = count
                .checked_mul(size_of::<u64>())?
                .checked_add(size_of::<CountsStorage>())?
                .checked_add(size_of::<Self>())?;
            if requested > budget {
                return None;
            }
            let mut holder = Vec::new();
            holder.try_reserve_exact(1).ok()?;
            let overhead = holder
                .capacity()
                .checked_mul(size_of::<CountsStorage>())?
                .checked_add(size_of::<Self>())?;
            if count.checked_mul(size_of::<u64>())?.checked_add(overhead)? > budget {
                return None;
            }
            let mut outputs = Vec::new();
            outputs.try_reserve_exact(count).ok()?;
            if outputs
                .capacity()
                .checked_mul(size_of::<u64>())?
                .checked_add(overhead)?
                > budget
            {
                return None;
            }
            outputs.resize(count, 0);
            holder.push(CountsStorage::Affine(outputs));
            self.storage = holder;
            let Some(CountsStorage::Affine(outputs)) = self.storage.first_mut() else {
                unreachable!()
            };
            Some(outputs)
        }
    }

    pub(super) fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        let Some(buffer) = self.as_ref() else {
            return Some(0);
        };
        self.storage
            .capacity()
            .checked_mul(size_of::<CountsStorage>())?
            .checked_add(size_of::<Self>())?
            .checked_add(
                buffer
                    .reserved_bytes()?
                    .checked_sub(size_of::<CountsReplay>())?,
            )
    }
}

impl CountsReplay {
    fn new(qubits: usize, record_count: usize, budget: usize) -> Option<Self> {
        let words = qubits.div_ceil(64);
        let frame_len = words.checked_mul(128)?;
        let record_len = record_count.checked_mul(64)?;
        let requested = frame_len
            .checked_mul(size_of::<u64>())?
            .checked_add(record_len.checked_mul(size_of::<bool>())?)?
            .checked_add(64usize.checked_mul(size_of::<Option<ReplayPosition>>())?)?
            .checked_add(size_of::<Self>())?;
        if requested > budget {
            return None;
        }
        let mut positions = Vec::new();
        positions.try_reserve_exact(64).ok()?;
        let mut frames = Vec::new();
        frames.try_reserve_exact(frame_len).ok()?;
        let mut records = Vec::new();
        records.try_reserve_exact(record_len).ok()?;
        let mut result = Self {
            positions,
            frames,
            records,
            words,
            record_count,
        };
        if result.reserved_bytes()? > budget {
            return None;
        }
        result.positions.resize(64, None);
        result.frames.resize(frame_len, 0);
        result.records.resize(record_len, false);
        Some(result)
    }

    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        self.frames
            .capacity()
            .checked_mul(size_of::<u64>())?
            .checked_add(self.records.capacity().checked_mul(size_of::<bool>())?)?
            .checked_add(
                self.positions
                    .capacity()
                    .checked_mul(size_of::<Option<ReplayPosition>>())?,
            )?
            .checked_add(size_of::<Self>())
    }

    pub(super) fn reset(&mut self) {
        self.positions.fill(None);
    }

    // Allocation is optional and precedes snapshot mutation. On rejection the
    // already drawn packet still has its complete original replay contract.
    pub(super) fn capture_into(
        storage: &mut CountsWorkspace,
        mask: u64,
        position: ReplayPosition,
        prefix: PacketPrefix<'_>,
        budget: usize,
    ) -> bool {
        if storage.is_none() {
            let mut holder = Vec::new();
            if holder.try_reserve_exact(1).is_err() {
                return false;
            }
            // The inner admission includes one CountsReplay header already.
            // Count the owner and any rounded outer capacity before allocating
            // the snapshot arrays; failure leaves the original replay intact.
            let overhead = holder
                .capacity()
                .checked_mul(size_of::<CountsStorage>())
                .and_then(|bytes| bytes.checked_sub(size_of::<Self>()))
                .and_then(|bytes| bytes.checked_add(size_of::<CountsWorkspace>()));
            let Some(inner_budget) = overhead.and_then(|bytes| budget.checked_sub(bytes)) else {
                return false;
            };
            let Some(buffer) = Self::new(prefix.x.len(), prefix.records.len(), inner_budget) else {
                return false;
            };
            holder.push(CountsStorage::Replay(buffer));
            storage.storage = holder;
        }
        if storage.reserved_bytes().is_none_or(|bytes| bytes > budget) {
            return false;
        }
        let Some(buffer) = storage.as_mut() else {
            return false;
        };
        debug_assert_eq!(prefix.x.len(), prefix.z.len());
        debug_assert_eq!(buffer.words, prefix.x.len().div_ceil(64));
        debug_assert_eq!(buffer.record_count, prefix.records.len());
        let mut remaining = mask;
        while remaining != 0 {
            let lane = remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;
            debug_assert!(buffer.positions[lane].is_none());
            let frame_start = lane * 2 * buffer.words;
            let frame = &mut buffer.frames[frame_start..frame_start + 2 * buffer.words];
            frame.fill(0);
            for (qubit, (&x, &z)) in prefix.x.iter().zip(prefix.z).enumerate() {
                frame[qubit / 64] |= ((x >> lane) & 1) << (qubit % 64);
                frame[buffer.words + qubit / 64] |= ((z >> lane) & 1) << (qubit % 64);
            }
            let record_start = lane * buffer.record_count;
            for (destination, &bits) in buffer.records
                [record_start..record_start + buffer.record_count]
                .iter_mut()
                .zip(prefix.records)
            {
                *destination = bits >> lane & 1 != 0;
            }
            buffer.positions[lane] = Some(ReplayPosition {
                logical: prefix.logical >> lane & 1 != 0,
                ..position
            });
        }
        true
    }

    pub(super) fn resume(&self, lane: usize, observable: u32) -> Option<CountsResume<'_>> {
        let position = self.positions[lane]?;
        let frame_start = lane * 2 * self.words;
        let record_start = lane * self.record_count;
        Some(CountsResume {
            position,
            x: &self.frames[frame_start..frame_start + self.words],
            z: &self.frames[frame_start + self.words..frame_start + 2 * self.words],
            records: &self.records[record_start..record_start + self.record_count],
            observable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn counts_from_raw_records(
        rows: &[NearCliffordShot],
        observable: u32,
    ) -> NearCliffordPostselectedCounts {
        let accepted: Vec<_> = rows
            .iter()
            .filter(|row| row.detectors.iter().all(|&bit| !bit))
            .collect();
        NearCliffordPostselectedCounts {
            attempted: rows.len(),
            accepted: accepted.len(),
            logical_errors: accepted
                .iter()
                .filter(|row| {
                    row.observables
                        .iter()
                        .filter(|(index, _)| *index == observable)
                        .fold(false, |parity, (_, bit)| parity ^ bit)
                })
                .count(),
        }
    }

    #[test]
    fn conditional_noise_restart_keeps_absolute_events_and_multiword_physical_frames() {
        let text = conditional_fixture::circuit(8, 3, true)
            .replace("M 0\nR 0\n", "M 0\nR 0\nCX rec[-1] 1\nCZ rec[-1] 2\n");
        for offset in [0, 65, 129] {
            let mut instructions = crate::parser::parse_lines(&text).unwrap();
            for instruction in &mut instructions {
                let StimInstr::Op { targets, .. } = instruction else {
                    unreachable!()
                };
                for target in targets {
                    match target {
                        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => *q += offset,
                        StimTarget::Pauli { qubit, .. } => *qubit += offset,
                        _ => {}
                    }
                }
            }
            for arithmetic in [
                CompiledRotationArithmetic::Strict,
                CompiledRotationArithmetic::Fused,
            ] {
                let plan = CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
                    instructions.clone(),
                    16,
                    arithmetic,
                )
                .unwrap();
                assert!(
                    plan.noise_signs.is_some(),
                    "must exercise absolute signed-noise lookups"
                );
                let initial = plan
                    .prepare_sampler()
                    .unwrap()
                    .coefficient_cache_reserved_bytes();
                for shots in [32, 63, 64, 65, 129] {
                    let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                    let mut candidate = plan.prepare_sampler_with_cache_budget(initial).unwrap();
                    let mut a = StdRng::seed_from_u64(1082741);
                    let mut b = a.clone();
                    let rows = reference.sample(shots, &mut a).unwrap();
                    assert_eq!(
                        candidate
                            .sample_postselected_counts(shots, 0, &mut b)
                            .unwrap(),
                        counts_from_raw_records(&rows, 0),
                        "{arithmetic:?} offset={offset} shots={shots}"
                    );
                    assert!(candidate.checkpoint_replay_rows > 0);
                    if shots <= 64 {
                        let saved = candidate.counts_workspace.as_ref().unwrap();
                        assert!(
                            saved.frames.iter().any(|&bits| bits != 0),
                            "restart must preserve a nonzero physical frame"
                        );
                    }
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }

    #[test]
    fn snapshots_preserve_scalar_frames_records_and_logical_prefix_at_word_seams() {
        let position = ReplayPosition {
            parent: 17,
            node: 123,
            event: 89,
            noise_event: 41,
            independent_event: 37,
            logical: false,
        };
        for qubits in [0, 1, 63, 64, 65, 129] {
            for records in [0, 1, 63, 64, 65, 129] {
                let x: Vec<_> = (0..qubits)
                    .map(|q| 0x8421_0842_1084_2108u64.rotate_left(q as u32))
                    .collect();
                let z: Vec<_> = x.iter().map(|bits| !bits).collect();
                let r: Vec<_> = (0..records)
                    .map(|n| 0xa55a_a55a_a55a_a55au64.rotate_left(n as u32))
                    .collect();
                let prefix = || PacketPrefix {
                    x: &x,
                    z: &z,
                    records: &r,
                    logical: 1 | (1 << 32),
                };
                let mut storage = CountsWorkspace::default();
                assert!(!CountsReplay::capture_into(
                    &mut storage,
                    u64::MAX,
                    position,
                    prefix(),
                    0
                ));
                assert!(storage.is_none());
                let lanes = [0, 31, 32, 63];
                let mask = lanes.iter().fold(0, |bits, &lane| bits | (1u64 << lane));
                assert!(CountsReplay::capture_into(
                    &mut storage,
                    mask,
                    position,
                    prefix(),
                    PACKET_BYTE_BUDGET
                ));
                let buffer = storage.as_mut().unwrap();
                assert!(buffer.reserved_bytes().unwrap() <= PACKET_BYTE_BUDGET);
                assert!(buffer.resume(1, 7).is_none());
                for lane in lanes {
                    let saved = buffer.resume(lane, 7).unwrap();
                    assert_eq!(
                        (
                            saved.position.parent,
                            saved.position.node,
                            saved.position.event,
                            saved.position.noise_event,
                            saved.position.independent_event
                        ),
                        (17, 123, 89, 41, 37)
                    );
                    assert_eq!(saved.observable, 7);
                    assert_eq!(saved.position.logical, lane == 0 || lane == 32);
                    for q in 0..qubits {
                        assert_eq!((saved.x[q / 64] >> (q % 64)) & 1, (x[q] >> lane) & 1);
                        assert_eq!((saved.z[q / 64] >> (q % 64)) & 1, (z[q] >> lane) & 1);
                    }
                    assert_eq!(
                        saved.records,
                        r.iter()
                            .map(|bits| bits >> lane & 1 != 0)
                            .collect::<Vec<_>>()
                    );
                }
                buffer.reset();
                assert!((0..64).all(|lane| buffer.resume(lane, 7).is_none()));
                assert!(!CountsReplay::capture_into(
                    &mut storage,
                    mask,
                    position,
                    prefix(),
                    0
                ));
                assert!(storage.as_ref().unwrap().resume(0, 7).is_none());
            }
        }
        assert!(CountsReplay::new(usize::MAX, 0, usize::MAX).is_none());
        assert!(CountsReplay::new(0, usize::MAX, usize::MAX).is_none());
    }

    #[test]
    fn affine_admission_rejects_before_replacing_an_existing_restart() {
        let mut workspace = CountsWorkspace::default();
        let position = ReplayPosition {
            parent: 7,
            node: 17,
            event: 23,
            noise_event: 11,
            independent_event: 5,
            logical: false,
        };
        assert!(CountsReplay::capture_into(
            &mut workspace,
            1,
            position,
            PacketPrefix {
                x: &[1, 0, 1],
                z: &[0, 1, 1],
                records: &[1, 0, 1],
                logical: 1,
            },
            PACKET_BYTE_BUDGET,
        ));
        let reserved = workspace.reserved_bytes();
        let required = size_of::<CountsWorkspace>() + size_of::<CountsStorage>() + 8 * 8;
        assert!(workspace.affine_outputs(8, required - 1).is_none());
        assert!(workspace.affine_outputs(usize::MAX, usize::MAX).is_none());
        assert_eq!(workspace.reserved_bytes(), reserved);
        let restart = workspace.as_ref().unwrap().resume(0, 0).unwrap();
        assert_eq!(restart.position.node, 17);
        assert_eq!(restart.x, &[5]);
        assert_eq!(restart.z, &[6]);
        assert_eq!(restart.records, &[true, false, true]);
        assert!(restart.position.logical);
        let outputs = workspace.affine_outputs(8, PACKET_BYTE_BUDGET).unwrap();
        outputs[0] = u64::MAX;
        let pointer = outputs.as_ptr();
        let capacity = outputs.capacity();
        let actual = size_of::<CountsWorkspace>()
            + workspace.storage.capacity() * size_of::<CountsStorage>()
            + capacity * size_of::<u64>();
        assert!(workspace.affine_outputs(8, actual - 1).is_none());
        workspace.clear_replay();
        let outputs = workspace.affine_outputs(8, actual).unwrap();
        assert_eq!(outputs.as_ptr(), pointer);
        assert_eq!(outputs[0], u64::MAX);
        assert!(workspace.is_none());
    }

    #[test]
    fn affine_workspace_survives_flat_records_and_preserves_counts_rng() {
        let plan = CompiledNearCliffordExecutor::compile_text(
            "H 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        )
        .unwrap();
        let mut candidate = plan.prepare_sampler().unwrap();
        let mut reference = plan.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(39519);
        let mut b = a.clone();
        let expected = reference.sample(64, &mut a).unwrap();
        assert_eq!(
            candidate.sample_postselected_counts(64, 0, &mut b).unwrap(),
            counts_from_raw_records(&expected, 0)
        );
        let Some(CountsStorage::Affine(outputs)) = candidate.counts_workspace.storage.first()
        else {
            panic!("Clifford counts must use the affine workspace");
        };
        let pointer = outputs.as_ptr();
        let capacity = outputs.capacity();
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let rows = reference.sample(65, &mut a).unwrap();
        assert_eq!(
            candidate
                .sample_measurements_u8(65, &mut b)
                .unwrap()
                .measurements,
            rows.iter()
                .flat_map(|row| row.measurements.iter().copied().map(u8::from))
                .collect::<Vec<_>>()
        );
        let Some(CountsStorage::Affine(outputs)) = candidate.counts_workspace.storage.first()
        else {
            panic!("flat records must preserve affine capacity");
        };
        assert_eq!((outputs.as_ptr(), outputs.capacity()), (pointer, capacity));
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let rows = reference.sample(65, &mut a).unwrap();
        assert_eq!(
            candidate.sample_postselected_counts(65, 0, &mut b).unwrap(),
            counts_from_raw_records(&rows, 0)
        );
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn owner_capacity_admission_precedes_snapshot_mutation() {
        let x = [1u64, 2, 3];
        let z = [3u64, 2, 1];
        let records = [1u64, 2, 3, 0];
        let position = ReplayPosition {
            parent: 7,
            node: 17,
            event: 23,
            noise_event: 11,
            independent_event: 5,
            logical: false,
        };
        let prefix = || PacketPrefix {
            x: &x,
            z: &z,
            records: &records,
            logical: 1,
        };
        let mut storage = CountsWorkspace::default();
        assert_eq!(storage.reserved_bytes(), Some(0));
        assert!(CountsReplay::capture_into(
            &mut storage,
            1,
            position,
            prefix(),
            PACKET_BYTE_BUDGET
        ));
        // Exercise actual retained outer capacity, including vacant slots.
        storage.storage.try_reserve_exact(3).unwrap();
        let inner = storage.as_ref().unwrap().reserved_bytes().unwrap();
        let reserved = storage.reserved_bytes().unwrap();
        assert_eq!(
            reserved,
            inner - size_of::<CountsReplay>()
                + size_of::<CountsWorkspace>()
                + storage.storage.capacity() * size_of::<CountsStorage>()
        );
        assert!(!CountsReplay::capture_into(
            &mut storage,
            2,
            position,
            prefix(),
            reserved - 1
        ));
        let buffer = storage.as_ref().unwrap();
        let first = buffer.resume(0, 0).unwrap();
        assert_eq!(first.position.node, 17);
        assert_eq!(first.x, &[5]);
        assert_eq!(first.z, &[5]);
        assert_eq!(first.records, &[true, false, true, false]);
        assert!(first.position.logical);
        assert!(buffer.resume(1, 0).is_none());
        assert!(CountsReplay::capture_into(
            &mut storage,
            2,
            position,
            prefix(),
            reserved
        ));
        assert_eq!(storage.reserved_bytes(), Some(reserved));
        let second = storage.as_ref().unwrap().resume(1, 0).unwrap();
        assert_eq!(second.x, &[6]);
        assert_eq!(second.z, &[3]);
        assert_eq!(second.records, &[false, true, true, false]);
        assert!(!second.position.logical);
    }

    #[test]
    fn restart_at_rotate_and_project_preserves_counts_feedback_and_rng_continuation() {
        // Random prefix ends compilation before the two rotations. Its
        // measurements and repeated selected annotation must survive a restart.
        let text = "H 3\nM 3\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\nDETECTOR rec[-1] rec[-1]\nH 5\nM 5\nDETECTOR rec[-1]\nCX rec[-2] 0\nCX sweep[0] 1\nH 0 1\nT 0 1\nCX 0 1\nMY(0.37) 0\nCX rec[-1] 2\nMRX(0.001) 1\nREPEAT 129 {\nR 4\nH 4\nM 4\nX_ERROR(0) 2\nDEPOLARIZE1(0.01) 2\n}\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\nM 2\nOBSERVABLE_INCLUDE(7) rec[-1]\n";
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            let mut observed_rotate = false;
            let mut observed_project = false;
            for extra in [0, 608, 896] {
                for shots in [32, 63, 64, 65, 129] {
                    for budget in [0, PACKET_BYTE_BUDGET] {
                        let mut candidate = plan
                            .prepare_sampler_with_cache_budget(initial + extra)
                            .unwrap();
                        candidate.replay_checkpoint_budget = budget;
                        let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                        let mut a = StdRng::seed_from_u64(1606);
                        let mut b = a.clone();
                        let rows = reference.sample_with_sweep(shots, &[true], &mut a).unwrap();
                        let counts = candidate
                            .sample_postselected_counts_with_sweep(shots, 7, &[true], &mut b)
                            .unwrap();
                        assert_eq!(
                            counts,
                            counts_from_raw_records(&rows, 7),
                            "{arithmetic:?} extra={extra} shots={shots} budget={budget}"
                        );
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                        if budget == 0 {
                            assert_eq!(candidate.checkpoint_replay_rows, 0);
                            assert!(candidate.counts_workspace.is_none());
                        } else {
                            assert!(
                                candidate.checkpoint_replay_rows > 0,
                                "restart must be exercised: extra={extra} shots={shots}"
                            );
                            let buffer = candidate.counts_workspace.as_ref().unwrap();
                            for saved in buffer.positions.iter().flatten() {
                                match plan.operations[saved.node] {
                                    PlanOp::Rotate { .. } => observed_rotate = true,
                                    PlanOp::Measure(_) => observed_project = true,
                                    _ => panic!("restart must precede a coefficient transition"),
                                }
                            }
                        }
                        // Reusing the sampler must preserve its retained state
                        // and optional storage contracts on the next call too.
                        let rows = reference.sample_with_sweep(65, &[false], &mut a).unwrap();
                        assert_eq!(
                            candidate
                                .sample_postselected_counts_with_sweep(65, 7, &[false], &mut b)
                                .unwrap(),
                            counts_from_raw_records(&rows, 7)
                        );
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                        let rows = reference.sample_with_sweep(65, &[true], &mut a).unwrap();
                        let flat = candidate
                            .sample_measurements_u8_with_sweep(65, &[true], &mut b)
                            .unwrap();
                        assert_eq!(
                            flat.measurements,
                            rows.iter()
                                .flat_map(|row| row.measurements.iter().copied().map(u8::from))
                                .collect::<Vec<_>>()
                        );
                        assert!(candidate.counts_workspace.is_none());
                        for _ in 0..16 {
                            assert_eq!(a.next_u64(), b.next_u64());
                        }
                    }
                }
            }
            assert!(
                observed_rotate && observed_project,
                "both transition failures must be observed"
            );
        }
    }

    #[test]
    fn failed_restart_restores_optional_buffers_without_retrying_the_packet_draws() {
        let mut plan = CompiledNearCliffordExecutor::compile_text(
            "H 0\nT 0\nH 1\nM 1\nT 1\nMX 0\nMX 1\nOBSERVABLE_INCLUDE(7) rec[-1]\n",
        )
        .unwrap();
        plan.initial_coefficients =
            Arc::new(vec![ComplexAmp::default(); plan.initial_coefficients.len()]);
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        let mut candidate = plan.prepare_sampler_with_cache_budget(initial).unwrap();
        let mut fallback = plan.prepare_sampler_with_cache_budget(initial).unwrap();
        fallback.replay_checkpoint_budget = 0;
        // The intentionally zero-norm cached parent has the original vector
        // shape. Canonical state zero otherwise normalizes one-cell inputs.
        for sampler in [&mut candidate, &mut fallback] {
            let cache = sampler.cache.as_mut().unwrap();
            let start = cache.start;
            let count = cache.states[start].coefficients.len();
            cache.states[start].coefficients = Arc::new(vec![ComplexAmp::default(); count]);
        }
        let mut a = StdRng::seed_from_u64(3722);
        let mut b = a.clone();
        let mut expected = a.clone();
        for _ in 0..64 {
            let mut row = RowRandom::live(&mut expected);
            for &kind in &plan.random_kinds {
                row.draw(kind);
            }
        }
        let error = candidate
            .sample_postselected_counts(64, 7, &mut a)
            .unwrap_err();
        assert_eq!(
            error,
            fallback
                .sample_postselected_counts(64, 7, &mut b)
                .unwrap_err()
        );
        assert!(candidate.checkpoint_replay_rows > 0);
        assert!(candidate.counts_workspace.is_some());
        assert_eq!(candidate.packet_x.len(), plan.num_qubits);
        assert_eq!(candidate.packet_records.len(), plan.measurement_count);
        assert_eq!(candidate.packet_tape.len(), 64 * plan.random_kinds.len());
        for _ in 0..16 {
            // All draws were produced once, including unvisited suffix events.
            let value = expected.next_u64();
            assert_eq!(a.next_u64(), value);
            assert_eq!(b.next_u64(), value);
        }
    }
}
