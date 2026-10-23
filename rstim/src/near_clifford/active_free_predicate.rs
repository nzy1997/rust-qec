//! Bounded one-pass affine detector predicates for explicit eager postselection.
use super::super::*;

#[derive(Clone, Copy, Debug, Default)]
struct Input {
    first: usize,
    count: usize,
    active: bool,
}

struct Quota {
    bytes: usize,
    limit: usize,
    work: usize,
}
impl Quota {
    // Monotone accounting includes all allocated vector payloads and headers,
    // including dropped scratch. Allocator metadata/reallocation transients are
    // not measured; every vector here has just one fallible exact reservation.
    fn zeros<T: Default + Clone>(&mut self, n: usize) -> Option<Vec<T>> {
        self.tick(n)?;
        let header = size_of::<Vec<T>>();
        if self
            .bytes
            .checked_add(header)?
            .checked_add(n.checked_mul(size_of::<T>())?)?
            > self.limit
        {
            return None;
        }
        let mut v = Vec::new();
        v.try_reserve_exact(n).ok()?;
        self.bytes = self
            .bytes
            .checked_add(header)?
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
}

struct Matrix {
    data: Vec<u64>,
    words: usize,
    rows: usize,
}
impl Matrix {
    fn new(rows: usize, words: usize, q: &mut Quota) -> Option<Self> {
        Some(Self {
            data: q.zeros(rows.checked_mul(words)?)?,
            words,
            rows,
        })
    }
    fn range(&self, row: usize) -> Option<std::ops::Range<usize>> {
        if row >= self.rows {
            return None;
        }
        let start = row.checked_mul(self.words)?;
        Some(start..start.checked_add(self.words)?)
    }
    fn row(&self, row: usize) -> Option<&[u64]> {
        self.data.get(self.range(row)?)
    }
    fn row_mut(&mut self, row: usize) -> Option<&mut [u64]> {
        let range = self.range(row)?;
        self.data.get_mut(range)
    }
    fn toggle(&mut self, row: usize, variable: usize, q: &mut Quota) -> Option<()> {
        q.tick(1)?;
        let words = self.words;
        let r = self.row_mut(row)?;
        if variable / 64 >= words {
            return None;
        }
        r[variable / 64] ^= 1u64 << (variable % 64);
        Some(())
    }
    fn xor_rows(&mut self, to: usize, from: usize, q: &mut Quota) -> Option<()> {
        let a = self.range(to)?;
        let b = self.range(from)?;
        q.tick(self.words)?;
        for k in 0..self.words {
            let value = self.data[b.start + k];
            self.data[a.start + k] ^= value;
        }
        Some(())
    }
}
fn xor(to: &mut [u64], from: &[u64], q: &mut Quota) -> Option<()> {
    if to.len() != from.len() {
        return None;
    }
    q.tick(to.len())?;
    for (a, b) in to.iter_mut().zip(from) {
        *a ^= b;
    }
    Some(())
}
fn support(
    bits: &[u64],
    qubits: usize,
    q: &mut Quota,
    mut visit: impl FnMut(usize, &mut Quota) -> Option<()>,
) -> Option<()> {
    if bits.len() != qubits.checked_add(63)? / 64 {
        return None;
    }
    q.tick(bits.len())?;
    for (word, &bits) in bits.iter().enumerate() {
        let mut bits = bits;
        while bits != 0 {
            q.tick(1)?;
            let i = word
                .checked_mul(64)?
                .checked_add(bits.trailing_zeros() as usize)?;
            if i >= qubits {
                return None;
            }
            visit(i, q)?;
            bits &= bits - 1;
        }
    }
    Some(())
}
fn conjugate(g: BasisGate, x: &mut Matrix, z: &mut Matrix, q: &mut Quota) -> Option<()> {
    q.tick(1)?;
    match g {
        BasisGate::H(a) => {
            q.tick(x.words)?;
            for (a, b) in x.row_mut(a)?.iter_mut().zip(z.row_mut(a)?) {
                std::mem::swap(a, b);
            }
        }
        BasisGate::S(a) => xor(z.row_mut(a)?, x.row(a)?, q)?,
        BasisGate::CX(a, b) => {
            if a == b {
                return None;
            }
            x.xor_rows(b, a, q)?;
            z.xor_rows(a, b, q)?;
        }
        BasisGate::CZ(a, b) => {
            if a == b {
                return None;
            }
            xor(z.row_mut(a)?, x.row(b)?, q)?;
            xor(z.row_mut(b)?, x.row(a)?, q)?;
        }
    }
    Some(())
}
fn apply(
    p: &PackedPauli,
    value: &[u64],
    x: &mut Matrix,
    z: &mut Matrix,
    q: &mut Quota,
) -> Option<()> {
    if p.phase >= 4 {
        return None;
    }
    support(&p.x, x.rows, q, |i, q| xor(x.row_mut(i)?, value, q))?;
    support(&p.z, z.rows, q, |i, q| xor(z.row_mut(i)?, value, q))
}

#[derive(Debug)]
pub(in super::super) struct ActiveFreePredicate {
    eligible: Vec<usize>,
    inputs: Vec<Input>,
    sweeps: Vec<u32>,
    sweep_first: usize,
    variables: usize,
    words: usize,
    columns: Vec<u64>,
}
impl ActiveFreePredicate {
    pub(in super::super) fn build(
        plan: &CompiledNearCliffordExecutor,
        byte_limit: usize,
        work_limit: usize,
    ) -> Option<Self> {
        let mut quota = Quota {
            bytes: size_of::<Self>(),
            limit: byte_limit,
            work: work_limit,
        };
        quota.tick(plan.operations.len().checked_add(plan.random_kinds.len())?)?;
        let detector_count = plan
            .operations
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    PlanOp::Annotation {
                        observable: None,
                        ..
                    }
                )
            })
            .count();
        if detector_count == 0 {
            return None;
        }
        let sweep_count = plan
            .operations
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    PlanOp::Feedback {
                        condition: Condition::Sweep(_),
                        ..
                    }
                )
            })
            .count();
        let mut sweeps = quota.zeros::<u32>(sweep_count)?;
        let mut s = 0;
        for op in &plan.operations {
            if let PlanOp::Feedback {
                condition: Condition::Sweep(index),
                ..
            } = op
            {
                sweeps[s] = *index;
                s += 1;
            }
        }
        // Bound sort comparisons conservatively as n*(ceil(log2(n))+1).
        let sort_depth = usize::BITS as usize - sweep_count.leading_zeros() as usize + 1;
        quota.tick(sweep_count.checked_mul(sort_depth)?)?;
        sweeps.sort_unstable();
        sweeps.dedup();
        let mut inputs = quota.zeros::<Input>(plan.random_kinds.len())?;
        let mut variables = 1usize; // Variable zero is the affine constant one.
        for (input, kind) in inputs.iter_mut().zip(&plan.random_kinds) {
            let (count, active) = match *kind {
                RandomKind::Independent => (1, false),
                RandomKind::Active => (1, true),
                RandomKind::Noise {
                    probability,
                    choices,
                } => {
                    if !probability.is_finite()
                        || !(0. ..=1.).contains(&probability)
                        || choices == 0
                        || choices > 15
                    {
                        return None;
                    }
                    (choices, false)
                }
            };
            *input = Input {
                first: variables,
                count,
                active,
            };
            variables = variables.checked_add(count)?;
        }
        let sweep_first = variables;
        variables = variables.checked_add(sweeps.len())?;
        let words = variables.checked_add(63)? / 64;
        let mut active = quota.zeros::<u64>(words)?;
        for input in &inputs {
            if input.active {
                active[input.first / 64] |= 1 << (input.first % 64);
            }
        }
        let mut x = Matrix::new(plan.num_qubits, words, &mut quota)?;
        let mut z = Matrix::new(plan.num_qubits, words, &mut quota)?;
        let mut records = Matrix::new(plan.measurement_count, words, &mut quota)?;
        let mut expressions = Matrix::new(detector_count, words, &mut quota)?;
        let mut eligible = quota.zeros::<usize>(detector_count)?;
        let mut anti = quota.zeros::<u64>(words)?;
        let mut branch = quota.zeros::<u64>(words)?;
        let mut physical = quota.zeros::<u64>(words)?;
        let mut value = quota.zeros::<u64>(words)?;
        if let Some(signs) = &plan.noise_signs {
            quota.tick(signs.offsets.len())?;
            if signs.offsets.len() != plan.operations.len().checked_add(1)?
                || signs.offsets.first() != Some(&0)
                || signs.offsets.last() != Some(&signs.refs.len())
                || signs.offsets.windows(2).any(|r| r[0] > r[1])
            {
                return None;
            }
        }
        let (mut event, mut detector, mut selected) = (0, 0, 0);
        for (node, op) in plan.operations.iter().enumerate().skip(plan.prefix_len) {
            quota.tick(1)?;
            match op {
                PlanOp::Rotate { .. } => {} // Changes only the opaque branch probabilities.
                PlanOp::Basis(gates) => {
                    for &g in gates {
                        conjugate(g, &mut x, &mut z, &mut quota)?;
                    }
                }
                PlanOp::Noise {
                    probability,
                    choices,
                } => {
                    if !matches!(plan.random_kinds.get(event),Some(RandomKind::Noise { probability:p,choices:c }) if p.to_bits()==probability.to_bits() && *c==choices.len())
                    {
                        return None;
                    }
                    let input = inputs[event];
                    for (category, choice) in choices.iter().enumerate() {
                        let variable = input.first.checked_add(category)?;
                        support(&choice.pauli.x, plan.num_qubits, &mut quota, |i, q| {
                            x.toggle(i, variable, q)
                        })?;
                        support(&choice.pauli.z, plan.num_qubits, &mut quota, |i, q| {
                            z.toggle(i, variable, q)
                        })?;
                        for &r in &choice.record_flips {
                            records.toggle(r, variable, &mut quota)?;
                        }
                    }
                    event += 1;
                }
                PlanOp::Feedback { condition, pauli } => {
                    quota.tick(words)?;
                    value.fill(0);
                    match condition {
                        Condition::Record(r) => xor(&mut value, records.row(*r)?, &mut quota)?,
                        Condition::Sweep(index) => {
                            quota.tick(sort_depth)?;
                            let variable =
                                sweep_first.checked_add(sweeps.binary_search(index).ok()?)?;
                            value[variable / 64] = 1 << (variable % 64);
                        }
                    }
                    apply(pauli, &value, &mut x, &mut z, &mut quota)?;
                }
                PlanOp::Measure(m) => {
                    quota.tick(words.checked_mul(2)?)?;
                    anti.fill(0);
                    branch.fill(0);
                    support(&m.pauli.physical.z, plan.num_qubits, &mut quota, |i, q| {
                        xor(&mut anti, x.row(i)?, q)
                    })?;
                    support(&m.pauli.physical.x, plan.num_qubits, &mut quota, |i, q| {
                        xor(&mut anti, z.row(i)?, q)
                    })?;
                    if let Some(signs) = &plan.noise_signs {
                        for reference in &signs.refs[signs.offsets[node]..signs.offsets[node + 1]] {
                            quota.tick(1)?;
                            let input = *inputs.get(reference.event)?;
                            if !matches!(plan.random_kinds.get(reference.event),Some(RandomKind::Noise { choices,.. }) if *choices==usize::from(reference.choices))
                            {
                                return None;
                            }
                            for category in 0..input.count {
                                quota.tick(1)?;
                                if reference.mask >> category & 1 != 0 {
                                    let variable = input.first + category;
                                    anti[variable / 64] ^= 1 << (variable % 64);
                                }
                            }
                        }
                    }
                    match m.projection {
                        Projection::Constant(bit) => branch[0] = u64::from(bit),
                        Projection::Independent { .. } | Projection::Active { .. } => {
                            let input = *inputs.get(event)?;
                            if !matches!(
                                (&m.projection, plan.random_kinds.get(event)),
                                (
                                    Projection::Independent { .. },
                                    Some(RandomKind::Independent)
                                ) | (Projection::Active { .. }, Some(RandomKind::Active))
                            ) {
                                return None;
                            }
                            branch[input.first / 64] = 1 << (input.first % 64);
                            event += 1;
                        }
                    }
                    quota.tick(words)?;
                    physical.copy_from_slice(&anti);
                    xor(&mut physical, &branch, &mut quota)?;
                    for &g in &m.basis {
                        conjugate(g, &mut x, &mut z, &mut quota)?;
                    }
                    match m.projection {
                        Projection::Constant(_) => {}
                        Projection::Independent { pivot } => {
                            xor(x.row_mut(pivot)?, &branch, &mut quota)?
                        }
                        Projection::Active { pivot, offset, .. } => {
                            xor(x.row_mut(pivot)?, &branch, &mut quota)?;
                            if offset {
                                x.toggle(pivot, 0, &mut quota)?;
                            }
                        }
                    }
                    if let Some(reset) = &m.reset {
                        apply(reset, &physical, &mut x, &mut z, &mut quota)?;
                    }
                    if let Some(r) = m.record {
                        quota.tick(words)?;
                        let dest = records.row_mut(r)?;
                        dest.copy_from_slice(&physical);
                        if m.inverted {
                            dest[0] ^= 1;
                        }
                        if m.readout > 0. {
                            if !matches!(plan.random_kinds.get(event),Some(RandomKind::Noise{probability,choices:1}) if probability.to_bits()==m.readout.to_bits())
                            {
                                return None;
                            }
                            records.toggle(r, inputs.get(event)?.first, &mut quota)?;
                            event += 1;
                        }
                    }
                }
                PlanOp::Annotation {
                    offsets,
                    observable: None,
                } => {
                    quota.tick(words)?;
                    value.fill(0);
                    for &r in offsets {
                        xor(&mut value, records.row(r)?, &mut quota)?;
                    }
                    quota.tick(words)?;
                    if !value.iter().zip(&active).any(|(v, a)| v & a != 0) {
                        quota.tick(words)?;
                        expressions.row_mut(selected)?.copy_from_slice(&value);
                        eligible[selected] = detector;
                        selected += 1;
                    }
                    detector += 1;
                }
                PlanOp::Annotation {
                    observable: Some(_),
                    ..
                } => {}
            }
        }
        if event != inputs.len() || detector != detector_count || selected == 0 {
            return None;
        }
        eligible.truncate(selected);
        let output_words = selected.checked_add(63)? / 64;
        let mut columns = quota.zeros::<u64>(variables.checked_mul(output_words)?)?;
        for output in 0..selected {
            let row = expressions.row(output)?;
            quota.tick(words)?;
            for (word, &bits) in row.iter().enumerate() {
                let mut bits = bits;
                while bits != 0 {
                    quota.tick(1)?;
                    let variable = word * 64 + bits.trailing_zeros() as usize;
                    if variable >= variables {
                        return None;
                    }
                    columns[variable * output_words + output / 64] ^= 1 << (output % 64);
                    bits &= bits - 1;
                }
            }
        }
        Some(Self {
            eligible,
            inputs,
            sweeps,
            sweep_first,
            variables,
            words: output_words,
            columns,
        })
    }
    fn column(&self, variable: usize) -> Option<&[u64]> {
        if variable >= self.variables {
            return None;
        }
        self.columns
            .get(variable * self.words..(variable + 1) * self.words)
    }
}

fn packet_fail_mask<'a>(
    lanes: usize,
    detectors: usize,
    base: &[u64],
    columns: impl IntoIterator<Item = (&'a [u64], u64)>,
    scratch: &mut Vec<u64>,
    byte_budget: usize,
    work_budget: usize,
) -> Option<u64> {
    if !(1..=64).contains(&lanes) {
        return None;
    }
    let words = detectors.checked_add(63)? / 64;
    if base.len() != words {
        return None;
    }
    let valid_tail = if detectors % 64 == 0 {
        u64::MAX
    } else {
        (1u64 << (detectors % 64)) - 1
    };
    if base.last().is_some_and(|bits| bits & !valid_tail != 0) {
        return None;
    }
    let maximum_words = byte_budget.checked_sub(size_of::<Vec<u64>>())? / size_of::<u64>();
    if detectors > maximum_words || scratch.capacity() > maximum_words {
        return None;
    }
    let mut remaining = work_budget.checked_sub(detectors.checked_mul(2)?.checked_add(words)?)?;
    if scratch.capacity() < detectors {
        scratch.try_reserve_exact(detectors - scratch.len()).ok()?;
        if scratch.capacity() > maximum_words {
            *scratch = Vec::new();
            return None;
        }
    }
    scratch.resize(detectors, 0);
    scratch.fill(0);
    let all = if lanes == 64 {
        u64::MAX
    } else {
        (1u64 << lanes) - 1
    };
    for (word, &bits) in base.iter().enumerate() {
        let mut bits = bits;
        while bits != 0 {
            remaining = remaining.checked_sub(1)?;
            let bit = bits.trailing_zeros() as usize;
            scratch[word * 64 + bit] = all;
            bits &= bits - 1;
        }
    }
    // Caller supplies read-only category/Independent lane masks from the existing producer.
    // This kernel never consumes a random event and never changes a sampler/cache/output.
    // Scratch may be changed on a late refusal and must not be interpreted unless Some.
    for (effect, selected) in columns {
        remaining = remaining.checked_sub(words.checked_add(1)?)?;
        if effect.len() != words || effect.last().is_some_and(|bits| bits & !valid_tail != 0) {
            return None;
        }
        let selected = selected & all;
        if selected == 0 {
            continue;
        }
        for (word, &bits) in effect.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                remaining = remaining.checked_sub(1)?;
                let bit = bits.trailing_zeros() as usize;
                scratch[word * 64 + bit] ^= selected;
                bits &= bits - 1;
            }
        }
    }
    let _ = remaining;
    Some(scratch.iter().fold(0, |rejected, parity| rejected | parity) & all)
}

// A read-only view of the existing producer. Category masks are computed once
// per typed event; no row is redrawn, rewritten, or compacted by this iterator.
struct PacketColumns<'a> {
    model: &'a ActiveFreePredicate,
    plan: &'a CompiledNearCliffordExecutor,
    tape: &'a [u64],
    hits: &'a [u64],
    noise: Option<&'a NoisePacket>,
    independent: Option<&'a IndependentPacket>,
    lanes: usize,
    all: u64,
    event: usize,
    noise_event: usize,
    independent_event: usize,
    first: usize,
    count: usize,
    category: usize,
    masks: [u64; 15],
}
impl<'a> Iterator for PacketColumns<'a> {
    type Item = (&'a [u64], u64);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while self.category < self.count {
                let category = self.category;
                self.category += 1;
                let mask = self.masks[category] & self.all;
                if mask == 0 {
                    continue;
                }
                let model = self.model;
                let column = model.column(self.first + category)?;
                if column.iter().all(|word| *word == 0) {
                    continue;
                }
                return Some((column, mask));
            }
            let event = self.event;
            let kind = *self.plan.random_kinds.get(event)?;
            self.event += 1;
            let input = self.model.inputs[event];
            self.first = input.first;
            self.count = input.count;
            self.category = 0;
            self.masks.fill(0);
            match kind {
                RandomKind::Active => {
                    // These columns are zero by the compiler eligibility proof.
                    self.count = 0;
                }
                RandomKind::Independent => {
                    self.masks[0] = if let Some(packet) = self.independent {
                        packet.mask(self.independent_event)
                    } else {
                        let mut selected = 0;
                        for lane in 0..self.lanes {
                            if self.tape[lane * self.plan.random_kinds.len() + event] != 0 {
                                selected |= 1u64 << lane;
                            }
                        }
                        selected
                    };
                    self.independent_event += 1;
                }
                RandomKind::Noise { choices, .. } => {
                    let selected = self.hits[self.noise_event] & self.all;
                    self.masks = if let Some(packet) = self.noise {
                        packet.choice_masks(self.noise_event, selected, choices)
                    } else {
                        packet_choice_masks(
                            selected,
                            choices,
                            self.tape,
                            event,
                            self.plan.random_kinds.len(),
                        )
                    };
                    self.noise_event += 1;
                }
            }
        }
    }
}

impl ActiveFreePredicate {
    // Only a model built from this same immutable plan may be used. The caller
    // supplies completed producer storage and preserves original resource/RNG
    // ordering. A None result permits the unchanged physical path; no caller
    // sampler/cache/tape/output state has been modified by this method.
    pub(in super::super) fn packet_rejection_mask(
        &self,
        plan: &CompiledNearCliffordExecutor,
        lanes: usize,
        sweep: &[bool],
        tape: &[u64],
        hits: &[u64],
        noise: Option<&NoisePacket>,
        independent: Option<&IndependentPacket>,
        scratch: &mut Vec<u64>,
        byte_budget: usize,
        work_budget: usize,
    ) -> Option<u64> {
        if !(1..=64).contains(&lanes)
            || self.inputs.len() != plan.random_kinds.len()
            || tape.len() < lanes.checked_mul(plan.random_kinds.len())?
            || hits.len() != plan.noise_event_count
        {
            return None;
        }
        let mut noise_count = 0usize;
        let mut independent_count = 0usize;
        for (kind, input) in plan.random_kinds.iter().zip(&self.inputs) {
            match *kind {
                RandomKind::Noise { choices, .. } => {
                    if !(1..=15).contains(&choices) || choices != input.count || input.active {
                        return None;
                    }
                    noise_count = noise_count.checked_add(1)?;
                }
                RandomKind::Independent => {
                    if input.count != 1 || input.active {
                        return None;
                    }
                    independent_count = independent_count.checked_add(1)?;
                }
                RandomKind::Active => {
                    if input.count != 1 || !input.active {
                        return None;
                    }
                }
            }
        }
        if noise_count != plan.noise_event_count
            || independent_count != plan.independent_event_count
        {
            return None;
        }
        // Include the bounded iterator/decoder visits even when all columns are
        // skipped. The generic kernel's work counter alone cannot see those.
        // At most 64 lanes and four compact Noise bit planes are read per event.
        let iterator_work = self
            .inputs
            .len()
            .checked_mul(64 * 16)?
            .checked_add(self.variables.checked_mul(self.words)?)?
            .checked_add(self.sweeps.len().checked_mul(self.words.checked_add(1)?)?)?;
        let kernel_work = work_budget.checked_sub(iterator_work)?;
        let all = if lanes == 64 {
            u64::MAX
        } else {
            (1u64 << lanes) - 1
        };
        let columns = PacketColumns {
            model: self,
            plan,
            tape,
            hits,
            noise,
            independent,
            lanes,
            all,
            event: 0,
            noise_event: 0,
            independent_event: 0,
            first: 0,
            count: 0,
            category: 0,
            masks: [0; 15],
        };
        let sweeps = self.sweeps.iter().enumerate().filter_map(|(i, index)| {
            sweep
                .get(*index as usize)
                .copied()
                .unwrap_or(false)
                .then(|| {
                    (
                        self.column(self.sweep_first + i)
                            .expect("compiled sweep variable"),
                        all,
                    )
                })
        });
        packet_fail_mask(
            lanes,
            self.eligible.len(),
            self.column(0)?,
            columns.chain(sweeps),
            scratch,
            byte_budget,
            kernel_work,
        )
    }
}
