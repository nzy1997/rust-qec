//! Bounded scheduling in original physical coordinates. Pauli channels keep
//! their order; crossing them changes rotation angles or logical measurement signs.
use super::*;

const MAX_OPS: usize = 2048;
const WORK_LIMIT: usize = 16_000_000;
// Bound temporary coordinate Paulis, basis gates and the cleared rank-plan
// buffers at the existing 4096-qubit maximum. The large frame is borrowed.
const RANK_SCRATCH_BYTES: usize = 2 * 1024 * 1024;

struct Budget {
    bytes: usize,
    limit: usize,
    work: usize,
}
impl Budget {
    fn zeros<T: Default + Clone>(&mut self, n: usize) -> Option<Vec<T>> {
        if self.bytes.checked_add(n.checked_mul(size_of::<T>())?)? > self.limit {
            return None;
        }
        let mut v = Vec::new();
        v.try_reserve_exact(n).ok()?;
        self.bytes = self
            .bytes
            .checked_add(v.capacity().checked_mul(size_of::<T>())?)?;
        if self.bytes > self.limit {
            return None;
        }
        v.resize(n, T::default());
        Some(v)
    }
    fn tick(&mut self, n: usize) -> Option<()> {
        self.work = self.work.checked_sub(n)?;
        Some(())
    }
    fn push<T>(&mut self, v: &mut Vec<T>, value: T) -> Option<()> {
        if v.len() == v.capacity() {
            let old = v.capacity();
            let add = 64;
            if self.bytes.checked_add(add * size_of::<T>())? > self.limit {
                return None;
            }
            v.try_reserve_exact(add).ok()?;
            self.bytes = self
                .bytes
                .checked_add((v.capacity() - old).checked_mul(size_of::<T>())?)?;
            if self.bytes > self.limit {
                return None;
            }
        }
        v.push(value);
        Some(())
    }
}
fn operator(op: &TapeOp, i: usize) -> Option<&PackedPauli> {
    match op {
        TapeOp::Rotate { pauli, .. } | TapeOp::Feedback { pauli, .. } => (i == 0).then_some(pauli),
        TapeOp::Measure { pauli, reset, .. } => match i {
            0 => Some(pauli),
            1 => reset.as_ref(),
            _ => None,
        },
        TapeOp::Noise { choices, .. } => choices.get(i).map(|c| &c.pauli),
        TapeOp::Annotation { .. } => None,
    }
}
fn quantum_commutes(a: &TapeOp, b: &TapeOp, budget: &mut Budget) -> Option<bool> {
    for i in 0..15 {
        let Some(p) = operator(a, i) else {
            break;
        };
        for j in 0..15 {
            let Some(q) = operator(b, j) else {
                break;
            };
            budget.tick(p.x.len().max(1))?;
            if !commute(p, q) {
                return Some(false);
            }
        }
    }
    Some(true)
}
fn reads(op: &TapeOp, r: usize, budget: &mut Budget) -> Option<bool> {
    Some(match op {
        TapeOp::Feedback {
            condition: Condition::Record(i),
            ..
        } => *i == r,
        TapeOp::Annotation { offsets, .. } => {
            budget.tick(offsets.len().max(1))?;
            offsets.contains(&r)
        }
        TapeOp::Noise { choices, .. } => {
            for c in choices {
                budget.tick(c.record_flips.len().max(1))?;
                if c.record_flips.contains(&r) {
                    return Some(true);
                }
            }
            false
        }
        _ => false,
    })
}
fn independent(a: &TapeOp, b: &TapeOp, budget: &mut Budget) -> Option<bool> {
    budget.tick(1)?;
    let movable = |o: &TapeOp| matches!(o, TapeOp::Rotate { .. } | TapeOp::Measure { .. });
    if !movable(a) && !movable(b) {
        return Some(false);
    }
    if let TapeOp::Measure {
        record: Some(r), ..
    } = a
    {
        if reads(b, *r, budget)? {
            return Some(false);
        }
    }
    if let TapeOp::Measure {
        record: Some(r), ..
    } = b
    {
        if reads(a, *r, budget)? {
            return Some(false);
        }
    }
    if matches!(
        (a, b),
        (
            TapeOp::Rotate { .. } | TapeOp::Measure { reset: None, .. },
            TapeOp::Noise { .. }
        ) | (
            TapeOp::Noise { .. },
            TapeOp::Rotate { .. } | TapeOp::Measure { reset: None, .. }
        )
    ) {
        return Some(true);
    }
    quantum_commutes(a, b, budget)
}
fn coordinate_work(p: &Planner, pauli: &PackedPauli) -> Option<usize> {
    let support = pauli
        .x
        .iter()
        .chain(&pauli.z)
        .try_fold(0usize, |n, w| n.checked_add(w.count_ones() as usize))?;
    let words = p.state.num_qubits.div_ceil(64).max(1);
    support
        .checked_mul(words)?
        .checked_mul(32)?
        .checked_add(p.state.num_qubits.checked_mul(p.axes.len() + 2)?)
}
fn effect(p: &Planner, o: &TapeOp, budget: &mut Budget) -> Option<i32> {
    let physical = match o {
        TapeOp::Rotate { pauli, .. } | TapeOp::Measure { pauli, .. } => pauli,
        _ => {
            budget.tick(1)?;
            return Some(0);
        }
    };
    budget.tick(coordinate_work(p, physical)?)?;
    let v = p.reexpress(physical).ok()?;
    let expands = (0..p.state.num_qubits).any(|q| v.x[q] && !p.axes.contains(&q));
    Some(match o {
        TapeOp::Rotate { .. } => i32::from(expands),
        _ => {
            if expands {
                0
            } else {
                let c = p.compact(&v);
                if c.x == 0 && c.z == 0 { 0 } else { -1 }
            }
        }
    })
}
fn advance(p: &mut Planner, o: &TapeOp, budget: &mut Budget) -> Option<()> {
    let physical = match o {
        TapeOp::Rotate { pauli, .. } | TapeOp::Measure { pauli, .. } => pauli,
        _ => {
            budget.tick(1)?;
            return Some(());
        }
    };
    let coordinates = coordinate_work(p, physical)?.checked_mul(4)?;
    // Preflight the worst basis update before touching a potentially very wide
    // frame, then charge the emitted gates. Rejection is still before any tape
    // mutation or caller RNG event.
    let worst = p
        .state
        .num_qubits
        .checked_mul(p.state.num_qubits)?
        .checked_mul(96)?
        .checked_add(coordinates)?;
    if worst > budget.work {
        return None;
    }
    match o {
        TapeOp::Rotate { pauli, dagger } => p.rotate_pauli(pauli, *dagger).ok()?,
        TapeOp::Measure {
            pauli,
            record,
            inverted,
            readout,
            reset,
        } => p
            .measure_pauli(pauli, *record, *inverted, reset.as_ref(), *readout)
            .ok()?,
        _ => unreachable!(),
    }
    let gates = p.operations.iter().try_fold(0usize, |n, op| {
        n.checked_add(match op {
            PlanOp::Basis(g) => g.len(),
            PlanOp::Measure(m) => m.basis.len(),
            _ => 0,
        })
    })?;
    budget
        .tick(coordinates.checked_add(gates.checked_mul(p.state.num_qubits)?.checked_mul(32)?)?)?;
    p.operations.clear();
    Some(())
}
// Evaluate the existing measurement-only pass without cloning its noise buffers.
// That pass changes only Noise choices/record flips; projector and reset Paulis
// stay fixed, and its Noise crossing is unconditional. Those noise mutations
// cannot affect structural rank. The comparison is tested against the real pass.
fn legacy_order(tape: &[TapeOp], budget: &mut Budget) -> Option<Vec<usize>> {
    let mut order = budget.zeros::<usize>(tape.len())?;
    for (i, id) in order.iter_mut().enumerate() {
        *id = i;
    }
    for start in 0..order.len() {
        let current = &tape[order[start]];
        let TapeOp::Measure { record, .. } = current else {
            continue;
        };
        let mut pos = start;
        while pos > 0 {
            let other = &tape[order[pos - 1]];
            budget.tick(1)?;
            let movable = match other {
                TapeOp::Noise { .. } => true,
                TapeOp::Feedback { condition, .. } => {
                    let earlier = match condition {
                        Condition::Record(i) => record.is_none_or(|r| *i < r),
                        Condition::Sweep(_) => true,
                    };
                    earlier && quantum_commutes(current, other, budget)?
                }
                TapeOp::Annotation { offsets, .. } => {
                    budget.tick(offsets.len().max(1))?;
                    record.is_none_or(|r| offsets.iter().all(|i| *i < r))
                }
                _ => quantum_commutes(current, other, budget)?,
            };
            if !movable {
                break;
            }
            order.swap(pos, pos - 1);
            pos -= 1;
        }
    }
    Some(order)
}
#[derive(Clone, Copy, Default)]
struct LogicalRef {
    noise: usize,
    mask: u16,
}
pub(super) struct Schedule {
    pub order: Vec<usize>,
    offsets: Vec<usize>,
    refs: Vec<LogicalRef>,
}
impl Schedule {
    pub(super) fn has_refs(&self, id: usize) -> bool {
        self.offsets[id] != self.offsets[id + 1]
    }
    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        self.order
            .capacity()
            .checked_add(self.offsets.capacity())?
            .checked_mul(size_of::<usize>())?
            .checked_add(self.refs.capacity().checked_mul(size_of::<LogicalRef>())?)?
            .checked_add(size_of::<Self>())
    }
    pub(super) fn apply(&self, tape: &mut [TapeOp]) -> Option<()> {
        let mut inverse = Vec::new();
        inverse.try_reserve_exact(tape.len()).ok()?;
        inverse.resize(tape.len(), 0);
        for (pos, &id) in self.order.iter().enumerate() {
            inverse[id] = pos;
        }
        for i in 0..inverse.len() {
            while inverse[i] != i {
                let j = inverse[i];
                tape.swap(i, j);
                inverse.swap(i, j);
            }
        }
        Some(())
    }
    pub(super) fn bind(
        &self,
        ops: &[PlanOp],
        ids: &[usize],
        prefix: usize,
        limit: usize,
    ) -> Option<NoiseSigns> {
        let mut budget = Budget {
            bytes: 0,
            limit,
            work: WORK_LIMIT,
        };
        let mut bindings = budget.zeros::<NoiseRef>(self.order.len())?;
        let mut event = 0;
        let mut ordinal = 0;
        for (node, op) in ops.iter().enumerate().skip(prefix) {
            match op {
                PlanOp::Noise { choices, .. } => {
                    bindings[ids[node]] = NoiseRef {
                        event,
                        ordinal,
                        choices: choices.len() as u8,
                        mask: 0,
                    };
                    event += 1;
                    ordinal += 1;
                }
                PlanOp::Measure(m) => {
                    if !matches!(m.projection, Projection::Constant(_)) {
                        event += 1;
                    }
                    if m.record.is_some() && m.readout > 0. {
                        event += 1;
                        ordinal += 1;
                    }
                }
                _ => {}
            }
        }
        let mut offsets = budget.zeros::<usize>(ops.len() + 1)?;
        let mut refs = Vec::new();
        for (node, &id) in ids.iter().enumerate() {
            offsets[node] = refs.len();
            if matches!(ops[node], PlanOp::Rotate { .. } | PlanOp::Measure(_)) {
                if node < prefix && self.has_refs(id) {
                    return None;
                }
                for r in &self.refs[self.offsets[id]..self.offsets[id + 1]] {
                    let b = bindings[r.noise];
                    if b.choices == 0 || b.choices > 15 {
                        return None;
                    }
                    budget.push(&mut refs, NoiseRef { mask: r.mask, ..b })?;
                }
            }
        }
        offsets[ops.len()] = refs.len();
        Some(NoiseSigns { offsets, refs })
    }
}
struct PreviewFrontier {
    done: Vec<bool>,
    preds: Vec<usize>,
    order: Vec<usize>,
    // Noise/feedback/annotations do not change the rank-only frame. Reuse
    // readiness effects until a quantum step or a copied parent changes it.
    effects: Vec<i32>,
    dense_work: u128,
}
impl PreviewFrontier {
    fn new(n: usize, graph: &[u64], words: usize, budget: &mut Budget) -> Option<Self> {
        budget.tick(graph.len())?;
        let edges = graph
            .iter()
            .try_fold(0usize, |n, bits| n.checked_add(bits.count_ones() as usize))?;
        budget.tick(
            graph
                .len()
                .checked_add(edges)?
                .checked_add(n.checked_mul(4)?)?,
        )?;
        let done = budget.zeros::<bool>(n)?;
        let mut preds = budget.zeros::<usize>(n)?;
        let mut order = budget.zeros::<usize>(n)?;
        order.clear();
        let mut effects = budget.zeros::<i32>(n)?;
        effects.fill(i32::MAX);
        for id in 0..n {
            for (w, &value) in graph[id * words..(id + 1) * words].iter().enumerate() {
                let mut value = value;
                while value != 0 {
                    let bit = value.trailing_zeros() as usize;
                    value &= value - 1;
                    preds[w * 64 + bit] += 1;
                }
            }
        }
        Some(Self {
            done,
            preds,
            order,
            effects,
            dense_work: 0,
        })
    }
    fn step(
        &mut self,
        p: &mut Planner,
        id: usize,
        tape: &[TapeOp],
        graph: &[u64],
        words: usize,
        budget: &mut Budget,
    ) -> Option<()> {
        if self.done[id] || self.preds[id] != 0 {
            return None;
        }
        // Charge the two bitset scans and the actual successor updates, rather
        // than charging every tape node for a sparse outgoing edge set.
        budget.tick(words)?;
        let row = &graph[id * words..(id + 1) * words];
        let edges = row
            .iter()
            .try_fold(0usize, |n, bits| n.checked_add(bits.count_ones() as usize))?;
        budget.tick(words.checked_add(edges)?.checked_add(6)?)?;
        let before = p.axes.len();
        let quantum = matches!(tape[id], TapeOp::Rotate { .. } | TapeOp::Measure { .. });
        if quantum {
            budget.tick(self.effects.len())?;
        }
        advance(p, &tape[id], budget)?;
        if quantum {
            self.effects.fill(i32::MAX);
            self.dense_work = self
                .dense_work
                .checked_add(1u128.checked_shl(before.max(p.axes.len()).try_into().ok()?)?)?;
        }
        self.done[id] = true;
        // Each id occurs once; storage for the full tape was reserved up front.
        self.order.push(id);
        for (w, &value) in graph[id * words..(id + 1) * words].iter().enumerate() {
            let mut value = value;
            while value != 0 {
                let bit = value.trailing_zeros() as usize;
                value &= value - 1;
                self.preds[w * 64 + bit] -= 1;
            }
        }
        Some(())
    }
    fn close(
        &mut self,
        p: &mut Planner,
        tape: &[TapeOp],
        graph: &[u64],
        words: usize,
        budget: &mut Budget,
    ) -> Option<()> {
        loop {
            let mut best = None;
            for id in 0..tape.len() {
                budget.tick(1)?;
                if !self.done[id] && self.preds[id] == 0 {
                    if self.effects[id] == i32::MAX {
                        self.effects[id] = effect(p, &tape[id], budget)?;
                    }
                    let score = (self.effects[id], id);
                    if score.0 <= 0 && best.is_none_or(|b| score < b) {
                        best = Some(score);
                    }
                }
            }
            let Some((_, id)) = best else {
                return Some(());
            };
            self.step(p, id, tape, graph, words, budget)?;
        }
    }
    fn copy_from(&mut self, parent: &Self, budget: &mut Budget) -> Option<()> {
        budget.tick(parent.done.len().checked_mul(3)?)?;
        self.done.copy_from_slice(&parent.done);
        self.preds.copy_from_slice(&parent.preds);
        self.order.clear();
        self.effects.fill(i32::MAX);
        self.dense_work = parent.dense_work;
        Some(())
    }
}

// A width-one lookahead previews every ready expanding op followed by the
// nonexpanding closure. Keep one frame and one winning prefix, not a beam of
// cloned tapes/frames. Any incomplete search returns the already complete
// greedy schedule; its legality and references were validated beforehand.
fn preview_order(
    tape: &[TapeOp],
    p: &mut Planner,
    graph: &[u64],
    words: usize,
    budget: &mut Budget,
) -> Option<Vec<usize>> {
    let n = tape.len();
    let q = p.state.num_qubits;
    let frame_bytes = p.state.reserved_bytes()?;
    if budget.bytes.checked_add(frame_bytes)? > budget.limit {
        return None;
    }
    let copy_work = q
        .checked_mul(2)?
        .checked_mul(q.div_ceil(64).checked_mul(2)?.checked_add(2)?)?;
    budget.tick(copy_work)?;
    let trial_frame = CompileFrame::identity(q).ok()?;
    budget.bytes = budget.bytes.checked_add(trial_frame.reserved_bytes()?)?;
    if budget.bytes > budget.limit {
        return None;
    }
    let mut axes = budget.zeros::<usize>(q)?;
    axes.clear();
    let mut trial_p = Planner {
        state: trial_frame,
        axes,
        limit: p.limit,
        peak: 0,
        operations: Vec::new(),
        expanded: 0,
        reserved_bytes: 0,
        tape: None,
        record_count: 0,
    };
    let mut parent = PreviewFrontier::new(n, graph, words, budget)?;
    let mut trial = PreviewFrontier::new(n, graph, words, budget)?;
    let mut prefix = budget.zeros::<usize>(n)?;
    prefix.clear();
    p.state.reset_identity();
    p.axes.clear();
    p.peak = 0;
    p.expanded = 0;
    p.reserved_bytes = 0;
    p.record_count = 0;
    parent.close(p, tape, graph, words, budget)?;
    while parent.order.len() != n {
        let mut best = None;
        for id in 0..n {
            budget.tick(1)?;
            if parent.done[id] || parent.preds[id] != 0 {
                continue;
            }
            budget.tick(copy_work.checked_add(p.axes.len())?)?;
            trial_p.state.copy_from(&p.state)?;
            trial_p.axes.clear();
            trial_p.axes.extend_from_slice(&p.axes);
            trial_p.peak = p.peak;
            trial_p.expanded = 0;
            trial_p.reserved_bytes = 0;
            trial_p.record_count = 0;
            trial.copy_from(&parent, budget)?;
            trial.step(&mut trial_p, id, tape, graph, words, budget)?;
            trial.close(&mut trial_p, tape, graph, words, budget)?;
            // Siblings share the parent order and start with distinct ids, so
            // id is their exact lexicographic tie-break without copying paths.
            let score = (
                trial_p.peak,
                trial_p.axes.len(),
                std::cmp::Reverse(parent.order.len() + trial.order.len()),
                trial.dense_work,
                id,
            );
            if best.is_none_or(|b| score < b) {
                budget.tick(trial.order.len())?;
                prefix.clear();
                prefix.extend_from_slice(&trial.order);
                best = Some(score);
            }
        }
        best?;
        for &id in &prefix {
            parent.step(p, id, tape, graph, words, budget)?;
        }
    }
    Some(parent.order)
}

pub(super) fn build(
    tape: &[TapeOp],
    state: &mut CompileFrame,
    limit: usize,
    remaining: usize,
) -> Option<Schedule> {
    build_bounded(tape, state, limit, remaining, WORK_LIMIT)
}

// No tape is mutated until all search/admission/order checks have succeeded.
// The caller's existing frame is borrowed by value and restored on every return.
fn build_bounded(
    tape: &[TapeOp],
    state: &mut CompileFrame,
    limit: usize,
    remaining: usize,
    work_limit: usize,
) -> Option<Schedule> {
    let n = tape.len();
    if n == 0 || n > MAX_OPS {
        return None;
    }
    if tape.iter().any(|o|matches!(o,TapeOp::Noise{choices,..}if choices.is_empty()||choices.len()>15||choices.iter().any(|c|!c.record_flips.is_empty()))){return None;}
    if remaining < RANK_SCRATCH_BYTES {
        return None;
    }
    let mut budget = Budget {
        bytes: RANK_SCRATCH_BYTES,
        limit: remaining,
        work: work_limit,
    };
    let words = n.div_ceil(64);
    let mut successors = budget.zeros::<u64>(n.checked_mul(words)?)?;
    let mut preds = budget.zeros::<usize>(n)?;
    for i in 0..n {
        for j in i + 1..n {
            if !independent(&tape[i], &tape[j], &mut budget)? {
                successors[i * words + j / 64] |= 1 << (j % 64);
                preds[j] += 1;
            }
        }
    }
    let old = legacy_order(tape, &mut budget)?;
    let mut p = Planner {
        state: std::mem::replace(state, CompileFrame::identity(0).ok()?),
        axes: Vec::new(),
        limit,
        peak: 0,
        operations: Vec::new(),
        expanded: 0,
        reserved_bytes: 0,
        tape: None,
        record_count: 0,
    };
    let result = (|| {
        p.state.reset_identity();
        for &id in &old {
            advance(&mut p, &tape[id], &mut budget)?;
        }
        let old_peak = p.peak;
        if old_peak < 4 {
            return None;
        }
        p.state.reset_identity();
        p.axes.clear();
        p.peak = 0;
        let mut done = budget.zeros::<bool>(n)?;
        let mut order = budget.zeros::<usize>(n)?;
        for step in 0..n {
            let mut best = None;
            for id in 0..n {
                budget.tick(1)?;
                if !done[id] && preds[id] == 0 {
                    let score = (effect(&p, &tape[id], &mut budget)?, id);
                    if best.is_none_or(|b| score < b) {
                        best = Some(score);
                    }
                }
            }
            let id = best?.1;
            advance(&mut p, &tape[id], &mut budget)?;
            done[id] = true;
            order[step] = id;
            for (w, &bits) in successors[id * words..(id + 1) * words].iter().enumerate() {
                let mut bits = bits;
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    preds[w * 64 + bit] -= 1;
                }
            }
        }
        if p.peak >= old_peak {
            return None;
        }
        let greedy_peak = p.peak;
        let fallback = refs_for_order(tape, order, &successors, words, &mut budget)?;
        if greedy_peak >= 4 {
            if let Some(order) = preview_order(tape, &mut p, &successors, words, &mut budget) {
                if p.peak < greedy_peak {
                    if let Some(selected) =
                        refs_for_order(tape, order, &successors, words, &mut budget)
                    {
                        return Some(selected);
                    }
                }
            }
        }
        Some(fallback)
    })();
    *state = p.state;
    result
}
fn refs_for_order(
    tape: &[TapeOp],
    order: Vec<usize>,
    successors: &[u64],
    words: usize,
    budget: &mut Budget,
) -> Option<Schedule> {
    let n = tape.len();
    let mut at = budget.zeros::<usize>(n)?;
    let mut seen = budget.zeros::<bool>(n)?;
    for (pos, &id) in order.iter().enumerate() {
        if id >= n || seen[id] {
            return None;
        }
        seen[id] = true;
        at[id] = pos;
    }
    if seen.iter().any(|v| !*v) {
        return None;
    }
    // Check every forbidden inversion, including resets and classical readers.
    for i in 0..n {
        for j in i + 1..n {
            budget.tick(1)?;
            if successors[i * words + j / 64] >> (j % 64) & 1 != 0 && at[i] >= at[j] {
                return None;
            }
        }
    }
    let mut offsets = budget.zeros::<usize>(n + 1)?;
    let mut refs = Vec::new();
    for (id, op) in tape.iter().enumerate() {
        offsets[id] = refs.len();
        let pauli = match op {
            TapeOp::Rotate { pauli, .. }
            | TapeOp::Measure {
                pauli, reset: None, ..
            } => pauli,
            _ => continue,
        };
        for (noise, o) in tape.iter().enumerate() {
            budget.tick(1)?;
            let TapeOp::Noise { choices, .. } = o else {
                continue;
            };
            if (noise < id) == (at[noise] < at[id]) {
                continue;
            }
            let mut mask = 0;
            for (i, c) in choices.iter().enumerate() {
                budget.tick(pauli.x.len().max(1))?;
                if !commute(pauli, &c.pauli) {
                    mask |= 1 << i;
                }
            }
            if mask != 0 {
                budget.push(&mut refs, LogicalRef { noise, mask })?;
            }
        }
    }
    offsets[n] = refs.len();
    Some(Schedule {
        order,
        offsets,
        refs,
    })
}
#[derive(Clone, Copy, Debug, Default)]
struct NoiseRef {
    event: usize,
    ordinal: usize,
    mask: u16,
    choices: u8,
}
#[derive(Clone, Debug)]
pub(super) struct NoiseSigns {
    offsets: Vec<usize>,
    refs: Vec<NoiseRef>,
}
impl NoiseSigns {
    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        self.offsets
            .capacity()
            .checked_mul(size_of::<usize>())?
            .checked_add(self.refs.capacity().checked_mul(size_of::<NoiseRef>())?)?
            .checked_add(size_of::<Self>())
    }
    pub(super) fn scalar(&self, node: usize, draw: &impl RowDraw) -> bool {
        if draw.noise_is_zero() {
            return false;
        }
        self.refs[self.offsets[node]..self.offsets[node + 1]]
            .iter()
            .fold(false, |p, r| {
                let value = draw.noise_value(r.event, r.ordinal);
                assert!(
                    value <= u64::from(r.choices),
                    "scheduled noise outcome out of range"
                );
                p ^ (value != 0 && (r.mask >> (value - 1)) & 1 != 0)
            })
    }
    pub(super) fn packet(
        &self,
        node: usize,
        tape: &[u64],
        count: usize,
        hits: &[u64],
        packet: Option<&NoisePacket>,
    ) -> u64 {
        let mut parity = 0;
        for r in &self.refs[self.offsets[node]..self.offsets[node + 1]] {
            let masks = if let Some(p) = packet {
                p.choice_masks(r.ordinal, hits[r.ordinal], r.choices as usize)
            } else {
                packet_choice_masks(hits[r.ordinal], r.choices as usize, tape, r.event, count)
            };
            for (i, &mask) in masks.iter().enumerate() {
                if r.mask >> i & 1 != 0 {
                    parity ^= mask;
                }
            }
        }
        parity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn zero_noise_summary_skips_sign_refs_but_unknown_rows_still_scan() {
        struct Observed {
            zero: bool,
            values: [u64; 2],
            reads: std::cell::Cell<usize>,
        }
        impl RowDraw for Observed {
            fn draw(&mut self, _: RandomKind) -> u64 {
                unreachable!()
            }
            fn discard_remaining(&mut self, _: &[RandomKind]) {
                unreachable!()
            }
            fn noise_is_zero(&self) -> bool {
                self.zero
            }
            fn noise_value(&self, event: usize, _: usize) -> u64 {
                assert!(!self.zero, "a certified zero row must not read sign refs");
                self.reads.set(self.reads.get() + 1);
                self.values[event]
            }
        }
        let signs = NoiseSigns {
            offsets: vec![0, 1, 3],
            refs: vec![
                NoiseRef {
                    event: 0,
                    ordinal: 0,
                    mask: 1,
                    choices: 3,
                },
                NoiseRef {
                    event: 0,
                    ordinal: 0,
                    mask: 2,
                    choices: 3,
                },
                NoiseRef {
                    event: 1,
                    ordinal: 1,
                    mask: 1,
                    choices: 3,
                },
            ],
        };
        for (zero, values, expected, reads) in [
            (true, [0, 0], [false, false], 0),
            (false, [0, 0], [false, false], 3),
            (false, [1, 2], [true, false], 3),
            (false, [2, 1], [false, false], 3),
        ] {
            let draw = Observed {
                zero,
                values,
                reads: std::cell::Cell::new(0),
            };
            assert_eq!([signs.scalar(0, &draw), signs.scalar(1, &draw)], expected);
            assert_eq!(draw.reads.get(), reads);
        }
        let mut rng = StdRng::seed_from_u64(739);
        assert!(!RowRandom::live(&mut rng).noise_is_zero());
        assert!(!RowRandom::recorded(&[0, 0], &mut rng).noise_is_zero());
    }

    #[test]
    fn zero_noise_summary_preserves_scheduled_records_counts_and_carry() {
        let text = conditional_fixture::circuit(8, 3, true);
        assert!(text.contains("DEPOLARIZE1(0.001)"));
        for probability in ["0", "0.001", "0.1", "1"] {
            let text = text.replace("DEPOLARIZE1(0.001)", &format!("DEPOLARIZE1({probability})"));
            for policy in [
                CompiledRotationArithmetic::Strict,
                CompiledRotationArithmetic::Fused,
            ] {
                let plan =
                    CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy)
                        .unwrap();
                assert!(plan.noise_signs.is_some());
                let mut fallback = plan.clone();
                fallback.random_runs = None;
                for selected in [&plan, &fallback] {
                    for budget in [0, DEFAULT_CACHE_BYTE_BUDGET] {
                        let mut reference = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                        let mut rows = selected.prepare_sampler_with_cache_budget(budget).unwrap();
                        let mut counts =
                            selected.prepare_sampler_with_cache_budget(budget).unwrap();
                        let mut a = StdRng::seed_from_u64(739);
                        let mut b = a.clone();
                        let mut c = a.clone();
                        for _ in 0..64 {
                            let frozen = scalar_prepared_tape_tests::draw_frozen_packet_rows(
                                &plan.random_kinds,
                                1,
                                &mut a,
                            );
                            let mut replay = RowRandom::recorded(&frozen[0], &mut a);
                            let expected = reference
                                .row_with_random_mode::<false>(&[], &mut replay)
                                .unwrap();
                            assert_eq!(replay.cursor, plan.random_kinds.len());
                            assert_eq!(rows.row(&[], &mut b).unwrap(), expected);
                            let accepted = usize::from(expected.detectors.iter().all(|&bit| !bit));
                            let logical = expected
                                .observables
                                .iter()
                                .filter(|(index, _)| *index == 0)
                                .fold(false, |parity, (_, bit)| parity ^ bit);
                            assert_eq!(
                                counts.sample_postselected_counts(1, 0, &mut c).unwrap(),
                                NearCliffordPostselectedCounts {
                                    attempted: 1,
                                    accepted,
                                    logical_errors: accepted * usize::from(logical),
                                }
                            );
                            let (mut carry_a, mut carry_b, mut carry_c) =
                                (a.clone(), b.clone(), c.clone());
                            for _ in 0..16 {
                                let word = carry_a.next_u64();
                                assert_eq!(word, carry_b.next_u64());
                                assert_eq!(word, carry_c.next_u64());
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn reduced_conditional_rank_uses_coherent_packets_with_exact_records_and_carry() {
        let text = conditional_fixture::circuit(8, 3, true);
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let ordinary = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
                "H 0\nT 0\nMX 0\n",
                policy,
            )
            .unwrap();
            assert_eq!(ordinary.peak_active_rank(), 1);
            assert!(ordinary.noise_signs.is_none());
            let mut ordinary_sampler = ordinary.prepare_sampler_with_cache_budget(0).unwrap();
            ordinary_sampler.observe_lazy_error_producer = true;
            ordinary_sampler
                .sample_measurements_u8(64, &mut StdRng::seed_from_u64(718293))
                .unwrap();
            assert!(ordinary_sampler.last_packet_error_producer.is_none());
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy).unwrap();
            assert_eq!(plan.peak_active_rank(), 2);
            assert!(plan.noise_signs.is_some());
            for shots in [1, 31, 32, 64] {
                let mut scalar_rng = StdRng::seed_from_u64(718293);
                let expected = plan
                    .prepare_sampler_with_cache_budget(0)
                    .unwrap()
                    .sample(shots, &mut scalar_rng)
                    .unwrap();
                let mut actual_rng = StdRng::seed_from_u64(718293);
                let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
                sampler.observe_lazy_error_producer = true;
                let actual = sampler
                    .sample_measurements_u8(shots, &mut actual_rng)
                    .unwrap();
                let expected: Vec<u8> = expected
                    .iter()
                    .flat_map(|row| row.measurements.iter().map(|&value| u8::from(value)))
                    .collect();
                assert_eq!(
                    actual.measurements, expected,
                    "packet boundary records, shots={shots}"
                );
                assert_eq!(
                    sampler
                        .last_packet_error_producer
                        .as_ref()
                        .map(|producer| producer.coherent),
                    (shots >= 32).then_some(true),
                    "packet boundary route, shots={shots}"
                );
                for _ in 0..16 {
                    assert_eq!(actual_rng.next_u64(), scalar_rng.next_u64());
                }
            }
            let mut expected_rng = StdRng::seed_from_u64(718293);
            let rows = plan
                .prepare_sampler_with_cache_budget(0)
                .unwrap()
                .sample(129, &mut expected_rng)
                .unwrap();
            let expected_continuation = expected_rng.clone();
            let expected: Vec<u8> = rows
                .iter()
                .flat_map(|row| row.measurements.iter().copied().map(u8::from))
                .collect();
            let mut rng = StdRng::seed_from_u64(718293);
            let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
            sampler.observe_lazy_error_producer = true;
            let mut actual = Vec::new();
            for shots in [64, 0, 65] {
                actual.extend(
                    sampler
                        .sample_measurements_u8(shots, &mut rng)
                        .unwrap()
                        .measurements,
                );
            }
            assert_eq!(actual, expected);
            assert!(
                sampler
                    .last_packet_error_producer
                    .as_ref()
                    .unwrap()
                    .coherent
            );
            for _ in 0..16 {
                assert_eq!(rng.next_u64(), expected_rng.next_u64());
            }
            let accepted: Vec<_> = rows
                .iter()
                .filter(|row| row.detectors.iter().all(|bit| !bit))
                .collect();
            let expected_counts = NearCliffordPostselectedCounts {
                attempted: 129,
                accepted: accepted.len(),
                logical_errors: accepted
                    .iter()
                    .filter(|row| {
                        row.observables
                            .iter()
                            .filter(|(index, _)| *index == 0)
                            .fold(false, |parity, (_, bit)| parity ^ bit)
                    })
                    .count(),
            };
            let mut counts_rng = StdRng::seed_from_u64(718293);
            let mut counts_sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
            counts_sampler.observe_lazy_error_producer = true;
            assert_eq!(
                counts_sampler
                    .sample_postselected_counts(129, 0, &mut counts_rng)
                    .unwrap(),
                expected_counts,
            );
            assert!(
                counts_sampler
                    .last_packet_error_producer
                    .as_ref()
                    .unwrap()
                    .coherent
            );
            let mut carry = expected_continuation;
            for _ in 0..16 {
                assert_eq!(counts_rng.next_u64(), carry.next_u64());
            }
        }
    }
    fn captured(text: &str) -> (Planner, Vec<TapeOp>) {
        let instr = crate::parser::parse_lines(text).unwrap();
        let incumbent = NearCliffordExecutor::compile_with_limit(
            validation_view(&instr, &mut ValidationBudget::default()).unwrap(),
            16,
        )
        .unwrap();
        let mut p = Planner {
            state: CompileFrame::identity(incumbent.num_qubits).unwrap(),
            axes: Vec::new(),
            limit: 16,
            peak: 0,
            operations: Vec::new(),
            expanded: 0,
            reserved_bytes: 0,
            tape: Some(Vec::new()),
            record_count: 0,
        };
        p.block(&instr).unwrap();
        let mut tape = p.tape.take().unwrap();
        remove_unobservable_rotations(&mut tape).unwrap();
        (p, tape)
    }
    #[test]
    fn preview_canonical_structural_rank_is_five() {
        let (mut p, mut tape) = captured(&conditional_fixture::circuit(16, 8, false));
        let selected = build(&tape, &mut p.state, 16, PLAN_BYTE_BUDGET).unwrap();
        selected.apply(&mut tape).unwrap();
        p.state.reset_identity();
        for op in tape {
            p.finish_op(op).unwrap();
        }
        assert_eq!(p.peak, 5);
    }
    #[test]
    fn preview_resource_rejection_retains_complete_greedy_schedule() {
        let text = conditional_fixture::circuit(16, 8, false);
        for (work, bytes) in [
            (8_000_000, PLAN_BYTE_BUDGET),
            (WORK_LIMIT, RANK_SCRATCH_BYTES + 72 * 1024),
        ] {
            let (mut p, mut tape) = captured(&text);
            let before = tape_signature(&tape);
            let frame_bytes = p.state.reserved_bytes().unwrap();
            let selected = build_bounded(&tape, &mut p.state, 16, bytes, work)
                .unwrap_or_else(|| panic!("no complete schedule work={work} bytes={bytes}"));
            assert_eq!(tape_signature(&tape), before);
            assert_eq!(p.state.num_qubits, 32);
            assert_eq!(p.state.reserved_bytes().unwrap(), frame_bytes);
            assert_eq!(selected.order.len(), tape.len());
            selected.apply(&mut tape).unwrap();
            p.state.reset_identity();
            for op in tape {
                p.finish_op(op).unwrap();
            }
            assert_eq!(p.peak, 9, "work={work} bytes={bytes}");
        }
    }
    fn tape_signature(tape: &[TapeOp]) -> Vec<String> {
        tape.iter()
            .map(|op| match op {
                TapeOp::Rotate { pauli, dagger } => format!("rotate {:?}", (pauli, dagger)),
                TapeOp::Measure {
                    pauli,
                    record,
                    inverted,
                    readout,
                    reset,
                } => format!(
                    "measure {:?}",
                    (pauli, record, inverted, readout.to_bits(), reset)
                ),
                TapeOp::Noise {
                    probability,
                    choices,
                } => format!("noise {:?}", (probability.to_bits(), choices)),
                TapeOp::Feedback { condition, pauli } => {
                    format!("feedback {:?}", (condition, pauli))
                }
                TapeOp::Annotation {
                    offsets,
                    observable,
                } => format!("annotation {:?}", (offsets, observable)),
            })
            .collect()
    }
    fn same_quantum(a: &TapeOp, b: &TapeOp) -> bool {
        match (a, b) {
            (
                TapeOp::Rotate {
                    pauli: p,
                    dagger: d,
                },
                TapeOp::Rotate {
                    pauli: q,
                    dagger: e,
                },
            ) => p.x == q.x && p.z == q.z && p.phase == q.phase && d == e,
            (
                TapeOp::Measure {
                    pauli: p,
                    record: r,
                    ..
                },
                TapeOp::Measure {
                    pauli: q,
                    record: s,
                    ..
                },
            ) => p.x == q.x && p.z == q.z && p.phase == q.phase && r == s,
            _ => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }
    #[test]
    fn legacy_rank_prediction_matches_actual_measurement_pass() {
        for text in [
            conditional_fixture::circuit(4, 2, false),
            conditional_fixture::circuit(8, 3, true),
            "H 0 1 2\nT 0 1 2\nY_ERROR(0.23) 1\nMRX !1\nCX rec[-1] 2\nMPP !Y0*Z2\nT_DAG 2\nMY 2\n"
                .to_owned(),
        ] {
            let (mut p, tape) = captured(&text);
            let mut budget = Budget {
                bytes: 0,
                limit: PLAN_BYTE_BUDGET,
                work: WORK_LIMIT,
            };
            let order = legacy_order(&tape, &mut budget).unwrap();
            // Independent actual pass with owned original ops, including its
            // transformed outcome Paulis and record-correction buffers.
            let instr = crate::parser::parse_lines(&text).unwrap();
            p.state.reset_identity();
            p.tape = Some(Vec::new());
            p.record_count = 0;
            p.block(&instr).unwrap();
            let mut real = p.tape.take().unwrap();
            remove_unobservable_rotations(&mut real).unwrap();
            schedule_measurements(&mut real, &mut 0).unwrap();
            assert_eq!(order.len(), real.len());
            for (pos, &id) in order.iter().enumerate() {
                assert!(same_quantum(&tape[id], &real[pos]), "position{pos}");
            }
        }
    }
    #[test]
    fn missing_budget_or_illegal_permutation_rejects_before_tape_mutation() {
        let (mut p, tape) = captured(&conditional_fixture::circuit(4, 2, false));
        assert!(build(&tape, &mut p.state, 16, 0).is_none());
        let n = tape.len();
        let words = n.div_ceil(64);
        let mut b = Budget {
            bytes: 0,
            limit: PLAN_BYTE_BUDGET,
            work: WORK_LIMIT,
        };
        let mut graph = vec![0; n * words];
        for i in 0..n {
            for j in i + 1..n {
                if !independent(&tape[i], &tape[j], &mut b).unwrap() {
                    graph[i * words + j / 64] |= 1 << (j % 64);
                }
            }
        }
        let reverse = (0..n).rev().collect();
        assert!(refs_for_order(&tape, reverse, &graph, words, &mut b).is_none());
        let duplicate = vec![0; n];
        assert!(refs_for_order(&tape, duplicate, &graph, words, &mut b).is_none());
    }
    #[test]
    fn actual_conditional_plan_has_measurement_and_rotation_references_and_compact_lookups() {
        let text = conditional_fixture::circuit(16, 8, true)
            .replace("MPP ", "MPP(0.23) ")
            .replace("DEPOLARIZE1(0.001)", "DEPOLARIZE2(0.23)");
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        assert!(plan.peak_active_rank() < 16);
        let signs = plan
            .noise_signs
            .as_ref()
            .expect("must execute conditional schedule");
        let mut event = 0;
        let mut directions = [[0; 2]; 2];
        let mut signed_measurement = 0;
        for (node, op) in plan.operations.iter().enumerate().skip(plan.prefix_len) {
            if matches!(op, PlanOp::Rotate { .. } | PlanOp::Measure(_)) {
                let ty = usize::from(matches!(op, PlanOp::Measure(_)));
                for r in &signs.refs[signs.offsets[node]..signs.offsets[node + 1]] {
                    directions[ty][usize::from(r.event >= event)] += 1;
                    if let PlanOp::Measure(m) = op {
                        let canonical = m
                            .pauli
                            .physical
                            .x
                            .iter()
                            .zip(&m.pauli.physical.z)
                            .map(|(&x, &z)| (x & z).count_ones())
                            .sum::<u32>()
                            % 4;
                        signed_measurement += usize::from(
                            (u32::from(m.pauli.physical.phase) + 4 - canonical) % 4 == 2,
                        );
                    }
                }
            }
            match op {
                PlanOp::Noise { .. } => event += 1,
                PlanOp::Measure(m) => {
                    event += usize::from(!matches!(m.projection, Projection::Constant(_)));
                    event += usize::from(m.record.is_some() && m.readout > 0.);
                }
                _ => {}
            }
        }
        assert!(directions[0].iter().sum::<usize>() > 0);
        assert!(directions[1].iter().sum::<usize>() > 0);
        assert!(directions.iter().map(|v| v[0]).sum::<usize>() > 0);
        assert!(directions.iter().map(|v| v[1]).sum::<usize>() > 0);
        assert!(
            signed_measurement > 0,
            "must cover signed projectors, not only inverted reported bits"
        );
        let mut rng = StdRng::seed_from_u64(108739);
        let row: Vec<_> = {
            let mut live = RowRandom::live(&mut rng);
            plan.random_kinds.iter().map(|&k| live.draw(k)).collect()
        };
        let mut continuation = rng.clone();
        let mut noise = NoisePacket::new(
            &plan.random_kinds,
            plan.noise_event_count,
            PACKET_BYTE_BUDGET,
        )
        .unwrap();
        let mut ordinal = 0;
        for (&kind, &value) in plan.random_kinds.iter().zip(&row) {
            if matches!(kind, RandomKind::Noise { .. }) {
                for bit in 0..4 {
                    noise.planes()[ordinal][bit] = ((value >> bit) & 1) << 63;
                }
                ordinal += 1;
            }
        }
        let stale: Vec<_> = plan
            .random_kinds
            .iter()
            .zip(&row)
            .map(|(&k, &v)| {
                if matches!(k, RandomKind::Noise { .. }) {
                    u64::MAX
                } else {
                    v
                }
            })
            .collect();
        let compact = CompactReplay::new(&stale, Some(&noise), None, 63);
        let recorded = RowRandom::recorded(&row, &mut rng);
        for node in plan.prefix_len..plan.operations.len() {
            assert_eq!(signs.scalar(node, &compact), signs.scalar(node, &recorded));
        }
        println!(
            "ACTUAL_CONDITIONAL rank={} directions={directions:?} signed_measurement={signed_measurement}",
            plan.peak_active_rank()
        );
        for _ in 0..16 {
            assert_eq!(rng.next_u64(), continuation.next_u64());
        }
    }
    #[test]
    fn valid_external_order_covers_future_rotation_and_past_signed_measurement() {
        let pauli = |x, z, phase| PackedPauli {
            x: vec![x],
            z: vec![z],
            phase,
        };
        let tape = vec![
            TapeOp::Noise {
                probability: 0.37,
                choices: vec![NoiseChoice::new(pauli(0, 1, 0))],
            },
            TapeOp::Rotate {
                pauli: pauli(1, 0, 0),
                dagger: false,
            },
            TapeOp::Measure {
                pauli: pauli(1, 1, 3),
                record: Some(0),
                inverted: true,
                readout: 0.23,
                reset: None,
            },
            TapeOp::Noise {
                probability: 0.37,
                choices: vec![
                    NoiseChoice::new(pauli(1, 0, 0)),
                    NoiseChoice::new(pauli(1, 1, 1)),
                    NoiseChoice::new(pauli(0, 1, 0)),
                ],
            },
        ];
        let mut budget = Budget {
            bytes: 0,
            limit: PLAN_BYTE_BUDGET,
            work: WORK_LIMIT,
        };
        let mut graph = vec![0; 4];
        for i in 0..4 {
            for j in i + 1..4 {
                if !independent(&tape[i], &tape[j], &mut budget).unwrap() {
                    graph[i] |= 1 << j;
                }
            }
        }
        let selected = refs_for_order(&tape, vec![1, 0, 3, 2], &graph, 1, &mut budget).unwrap();
        assert_eq!(selected.refs[selected.offsets[1]].noise, 0);
        assert_eq!(selected.refs[selected.offsets[1]].mask, 1);
        assert_eq!(selected.refs[selected.offsets[2]].noise, 3);
        assert_eq!(selected.refs[selected.offsets[2]].mask, 5);
        let mut p = Planner {
            state: CompileFrame::identity(3).unwrap(),
            axes: Vec::new(),
            limit: 16,
            peak: 0,
            operations: Vec::new(),
            expanded: 0,
            reserved_bytes: 0,
            tape: None,
            record_count: 0,
        };
        let mut ids = Vec::new();
        let mut tape: Vec<_> = tape.into_iter().map(Some).collect();
        for &id in &selected.order {
            let begin = p.operations.len();
            p.finish_op(tape[id].take().unwrap()).unwrap();
            ids.extend(std::iter::repeat_n(id, p.operations.len() - begin));
        }
        let signs = selected
            .bind(&p.operations, &ids, 0, PLAN_BYTE_BUDGET)
            .unwrap();
        let plan = CompiledNearCliffordExecutor {
            operations: p.operations,
            annotation_positions: Vec::new(),
            num_qubits: 3,
            measurement_count: 1,
            peak_active_rank: p.peak,
            prefix_len: 0,
            rotation_arithmetic: CompiledRotationArithmetic::Strict,
            initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            random_kinds: vec![
                RandomKind::Noise {
                    probability: 0.37,
                    choices: 1,
                },
                RandomKind::Noise {
                    probability: 0.37,
                    choices: 3,
                },
                RandomKind::Active,
                RandomKind::Noise {
                    probability: 0.23,
                    choices: 1,
                },
            ],
            random_runs: None,
            noise_spans: None,
            noise_signs: Some(signs),
            noise_event_count: 3,
            independent_event_count: 0,
            scalar_basis: None,
            linear_counts: Arc::new(OnceLock::new()),
            counts_plan_budget: 0,
        };
        // Independent physical calculation: the original Z noise is applied to
        // |000>, so is a global phase; the final channel follows the projector.
        // R_X(pi/4) gives <-Y>=sin(pi/4), inversion/readout give this parity.
        let expected = 0.5 + 0.27 * std::f64::consts::FRAC_1_SQRT_2;
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let mut plan = plan.clone();
            plan.rotation_arithmetic = arithmetic;
            let mut rng = StdRng::seed_from_u64(1082739);
            let rows = plan
                .prepare_sampler_with_cache_budget(0)
                .unwrap()
                .sample(16384, &mut rng)
                .unwrap();
            let ones = rows.iter().filter(|s| s.measurements[0]).count() as f64 / 16384.;
            assert!(
                (ones - expected).abs()
                    < 7. * (expected * (1. - expected) / 16384.).sqrt() + 4. / 16384.
            );
            let mut other = StdRng::seed_from_u64(1082739);
            let flat = plan
                .prepare_sampler()
                .unwrap()
                .sample_measurements_u8(16384, &mut other)
                .unwrap();
            assert_eq!(
                flat.measurements,
                rows.iter()
                    .map(|s| u8::from(s.measurements[0]))
                    .collect::<Vec<_>>()
            );
            for _ in 0..16 {
                assert_eq!(rng.next_u64(), other.next_u64());
            }
        }
    }
    #[test]
    fn conditional_packet_capacity_rejection_preserves_rows_and_rng() {
        let text = conditional_fixture::circuit(8, 3, true);
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            assert!(plan.noise_signs.is_some());
            let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut candidate = plan.prepare_sampler().unwrap();
            // Simulate a retained allocation larger than the shared cap. The
            // packet request itself fits; admission must use actual capacities
            // and discard optional storage before drawing any random events.
            candidate
                .packet_tape
                .try_reserve_exact(PACKET_BYTE_BUDGET / size_of::<u64>() + 1)
                .unwrap();
            let row_capacity = candidate.conditional_tape.capacity();
            let mut a = StdRng::seed_from_u64(1082740);
            let mut b = a.clone();
            let rows = reference.sample(65, &mut a).unwrap();
            let flat = candidate.sample_measurements_u8(65, &mut b).unwrap();
            assert_eq!(
                flat.measurements,
                rows.iter()
                    .flat_map(|s| s.measurements.iter().map(|&v| u8::from(v)))
                    .collect::<Vec<_>>()
            );
            assert_eq!(candidate.packet_tape.capacity(), 0);
            assert_eq!(candidate.packet_x.capacity(), 0);
            assert!(candidate.packet_independent.is_none());
            assert_eq!(candidate.conditional_tape.capacity(), row_capacity);
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
            // The same sampler can resume packed operation after the fallback.
            let rows = reference.sample(129, &mut a).unwrap();
            let flat = candidate.sample_measurements_u8(129, &mut b).unwrap();
            assert_eq!(
                flat.measurements,
                rows.iter()
                    .flat_map(|s| s.measurements.iter().map(|&v| u8::from(v)))
                    .collect::<Vec<_>>()
            );
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
    #[test]
    fn conditional_signs_preserve_streams_across_packed_physical_word_boundary() {
        let mut instructions =
            crate::parser::parse_lines(&conditional_fixture::circuit(8, 3, true)).unwrap();
        for instruction in &mut instructions {
            let StimInstr::Op { targets, .. } = instruction else {
                unreachable!()
            };
            for target in targets {
                match target {
                    StimTarget::Qubit(q) | StimTarget::QubitInv(q) => *q += 65,
                    StimTarget::Pauli { qubit, .. } => *qubit += 65,
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
            assert!(plan.noise_signs.is_some());
            assert_eq!(plan.num_qubits, 81);
            let mut a = StdRng::seed_from_u64(1082741);
            let mut b = a.clone();
            let rows = plan
                .prepare_sampler_with_cache_budget(0)
                .unwrap()
                .sample(129, &mut a)
                .unwrap();
            let flat = plan
                .prepare_sampler()
                .unwrap()
                .sample_measurements_u8(129, &mut b)
                .unwrap();
            assert_eq!(
                flat.measurements,
                rows.iter()
                    .flat_map(|s| s.measurements.iter().map(|&v| u8::from(v)))
                    .collect::<Vec<_>>()
            );
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}
