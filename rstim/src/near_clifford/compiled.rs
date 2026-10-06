//! Offline Clifford-frame plan; runtime carries a virtual Pauli and compact amplitudes.
use super::*;
#[path = "compile_frame.rs"]
mod compile_frame;
use compile_frame::CompileFrame;
#[path = "coherent_packet.rs"]
mod coherent_packet;
use coherent_packet::CoherentPacket;
#[path = "random_event_runs.rs"]
mod random_event_runs;
use random_event_runs::RandomRunPlan;

const PLAN_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const COEFFICIENT_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const DEFAULT_CACHE_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const PACKET_BYTE_BUDGET: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
enum RandomKind {
    Independent,
    Active,
    Noise { probability: f64, choices: usize },
}
impl RandomKind {
    fn draw(self, rng: &mut impl Rng) -> u64 {
        match self {
            Self::Independent => u64::from(rng.r#gen::<bool>()),
            Self::Active => rng.r#gen::<f64>().to_bits(),
            Self::Noise {
                probability,
                choices,
            } => {
                if !near_noise_occurs(probability, rng) {
                    0
                } else if choices == 1 {
                    1
                } else {
                    (rng.gen_range(0..choices) + 1) as u64
                }
            }
        }
    }
}
struct RowRandom<'a, R> {
    tape: Option<&'a [u64]>,
    cursor: usize,
    rng: &'a mut R,
    bit_word: u64,
    bits_left: u8,
    noise_probability: u64,
    noise_skip: Option<usize>,
}
impl<'a, R: Rng> RowRandom<'a, R> {
    fn live(rng: &'a mut R) -> Self {
        Self {
            tape: None,
            cursor: 0,
            rng,
            bit_word: 0,
            bits_left: 0,
            noise_probability: 0,
            noise_skip: None,
        }
    }
    fn recorded(tape: &'a [u64], rng: &'a mut R) -> Self {
        Self {
            tape: Some(tape),
            ..Self::live(rng)
        }
    }
    #[inline]
    fn draw(&mut self, kind: RandomKind) -> u64 {
        if let Some(tape) = self.tape {
            let value = tape[self.cursor];
            self.cursor += 1;
            value
        } else {
            match kind {
                RandomKind::Independent => {
                    if self.bits_left == 0 {
                        self.bit_word = self.rng.r#gen::<u64>();
                        self.bits_left = 64;
                    }
                    let bit = self.bit_word & 1;
                    self.bit_word >>= 1;
                    self.bits_left -= 1;
                    bit
                }
                RandomKind::Noise {
                    probability,
                    choices,
                } if (1e-12..=0.01).contains(&probability) => {
                    // IID Bernoulli failures before the next success follow a geometric
                    // law. Renewing this run avoids a uniform draw at each sparse site.
                    // The run and unused bit pool end at each row, making call splits
                    // independent of execution strategy. All channels remain Pauli.
                    if self.noise_probability != probability.to_bits() {
                        self.noise_skip = None;
                        self.noise_probability = probability.to_bits();
                    }
                    let skip = self.noise_skip.get_or_insert_with(|| {
                        let u = self.rng.r#gen::<f64>();
                        ((-u).ln_1p() / (-probability).ln_1p()).floor() as usize
                    });
                    if *skip != 0 {
                        *skip -= 1;
                        0
                    } else {
                        self.noise_skip = None;
                        if choices == 1 {
                            1
                        } else {
                            (self.rng.gen_range(0..choices) + 1) as u64
                        }
                    }
                }
                _ => kind.draw(self.rng),
            }
        }
    }
}
fn for_support(words: &[u64], mut visit: impl FnMut(usize)) {
    for (word, &bits) in words.iter().enumerate() {
        let mut bits = bits;
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            visit(word * 64 + bit);
            bits &= bits - 1;
        }
    }
}

#[derive(Clone, Copy)]
struct CachedMeasurement {
    probability_zero: f64,
    next: [Option<usize>; 2],
}
#[derive(Clone, Copy)]
enum CachedOp {
    None,
    Rotate([Option<usize>; 2]),
    Measure(CachedMeasurement),
}
struct CachedState {
    coefficients: Arc<Vec<ComplexAmp>>,
    next_node: Option<usize>,
    transition: CachedOp,
}
struct CoefficientCache {
    // Scalar state 0 is shared across positions; other IDs are never deduplicated.
    nodes: Vec<CachedOp>,
    states: Vec<CachedState>,
    start: usize,
    reserved: usize,
    budget: usize,
}
impl CoefficientCache {
    fn new(plan: &CompiledNearCliffordExecutor, budget: usize) -> Option<Self> {
        let bytes = plan
            .operations
            .len()
            .checked_mul(size_of::<CachedOp>())?
            .checked_add(
                plan.initial_coefficients
                    .len()
                    .checked_mul(size_of::<ComplexAmp>())?,
            )?
            .checked_add(512)?;
        if bytes > budget {
            return None;
        }
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(plan.operations.len()).ok()?;
        nodes.resize(plan.operations.len(), CachedOp::None);
        let mut states = Vec::new();
        states.try_reserve_exact(2).ok()?;
        states.push(CachedState {
            coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            next_node: None,
            transition: CachedOp::None,
        });
        let start = if plan.initial_coefficients.len() == 1 {
            0
        } else {
            states.push(CachedState {
                coefficients: plan.initial_coefficients.clone(),
                next_node: None,
                transition: CachedOp::None,
            });
            1
        };
        Some(Self {
            nodes,
            states,
            start,
            reserved: bytes,
            budget,
        })
    }
    fn entry(&self, id: usize, node: usize) -> Option<CachedOp> {
        if id == 0 {
            Some(self.nodes[node])
        } else {
            let state = &self.states[id];
            match state.next_node {
                Some(expected) if expected == node => Some(state.transition),
                None => Some(CachedOp::None),
                // Safely refuse cache reuse if a future optimizer changes this invariant.
                Some(_) => None,
            }
        }
    }
    fn set_entry(&mut self, id: usize, node: usize, entry: CachedOp) -> bool {
        if id == 0 {
            self.nodes[node] = entry;
            true
        } else {
            let state = &mut self.states[id];
            if state.next_node.is_some_and(|expected| expected != node) {
                return false;
            }
            state.next_node = Some(node);
            state.transition = entry;
            true
        }
    }
    fn state_charge(coefficients: &[ComplexAmp]) -> Option<usize> {
        if coefficients.len() == 1 {
            Some(0)
        } else {
            coefficients
                .len()
                .checked_mul(size_of::<ComplexAmp>())?
                .checked_add(256)
        }
    }
    fn fits(&self, bytes: usize) -> bool {
        self.reserved
            .checked_add(bytes)
            .is_some_and(|sum| sum <= self.budget)
    }
    fn store(&mut self, coefficients: &[ComplexAmp]) -> Option<usize> {
        if coefficients.len() == 1 {
            return Some(0);
        }
        let charge = Self::state_charge(coefficients)?;
        if !self.fits(charge) {
            return None;
        }
        let mut state = Vec::new();
        state.try_reserve_exact(coefficients.len()).ok()?;
        state.extend_from_slice(coefficients);
        self.states.try_reserve(1).ok()?;
        let id = self.states.len();
        self.states.push(CachedState {
            coefficients: Arc::new(state),
            next_node: None,
            transition: CachedOp::None,
        });
        self.reserved += charge;
        Some(id)
    }
}

#[derive(Clone, Debug)]
struct PackedPauli {
    x: Vec<u64>,
    z: Vec<u64>,
    phase: u8,
}
impl PackedPauli {
    fn new(p: &Pauli) -> Self {
        let pack = |bits: &[bool]| {
            let mut words = vec![0; bits.len().div_ceil(64)];
            for (i, &bit) in bits.iter().enumerate() {
                words[i / 64] |= u64::from(bit) << (i % 64);
            }
            words
        };
        Self {
            x: pack(&p.x),
            z: pack(&p.z),
            phase: p.phase,
        }
    }
    fn packet_anti(&self, x: &[u64], z: &[u64]) -> u64 {
        let mut parity = 0;
        for_support(&self.x, |q| parity ^= z[q]);
        for_support(&self.z, |q| parity ^= x[q]);
        parity
    }
    fn packet_apply(&self, lanes: u64, x: &mut [u64], z: &mut [u64]) {
        for_support(&self.x, |q| x[q] ^= lanes);
        for_support(&self.z, |q| z[q] ^= lanes);
    }
    fn anticommutes(&self, x: &[u64], z: &[u64]) -> bool {
        self.x
            .iter()
            .zip(&self.z)
            .zip(x.iter().zip(z))
            .fold(0, |parity, ((px, pz), (fx, fz))| {
                parity ^ ((px & fz) ^ (pz & fx))
            })
            .count_ones()
            % 2
            != 0
    }
    fn apply(&self, x: &mut [u64], z: &mut [u64]) {
        for ((fx, fz), (px, pz)) in x.iter_mut().zip(z).zip(self.x.iter().zip(&self.z)) {
            *fx ^= px;
            *fz ^= pz;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum BasisGate {
    H(usize),
    S(usize),
    CX(usize, usize),
    CZ(usize, usize),
}
fn get(bits: &[u64], q: usize) -> bool {
    bits[q / 64] >> (q % 64) & 1 != 0
}
fn flip(bits: &mut [u64], q: usize) {
    bits[q / 64] ^= 1 << (q % 64);
}
impl BasisGate {
    fn packet_conjugate(self, x: &mut [u64], z: &mut [u64]) {
        match self {
            Self::H(q) => std::mem::swap(&mut x[q], &mut z[q]),
            Self::S(q) => z[q] ^= x[q],
            Self::CX(a, b) => {
                x[b] ^= x[a];
                z[a] ^= z[b];
            }
            Self::CZ(a, b) => {
                z[a] ^= x[b];
                z[b] ^= x[a];
            }
        }
    }
    fn conjugate(self, x: &mut [u64], z: &mut [u64]) {
        match self {
            Self::H(q) => {
                if get(x, q) != get(z, q) {
                    flip(x, q);
                    flip(z, q);
                }
            }
            Self::S(q) => {
                if get(x, q) {
                    flip(z, q);
                }
            }
            Self::CX(a, b) => {
                if get(x, a) {
                    flip(x, b);
                }
                if get(z, b) {
                    flip(z, a);
                }
            }
            Self::CZ(a, b) => {
                if get(x, a) {
                    flip(z, b);
                }
                if get(x, b) {
                    flip(z, a);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct CompactPauli {
    physical: PackedPauli,
    x: usize,
    z: usize,
}
#[derive(Clone, Debug)]
enum Projection {
    Constant(bool),
    Independent {
        pivot: usize,
    },
    Active {
        pivot: usize,
        index: usize,
        y: bool,
        offset: bool,
    },
}
#[derive(Clone, Debug)]
struct Measurement {
    pauli: CompactPauli,
    projection: Projection,
    basis: Vec<BasisGate>,
    record: Option<usize>,
    inverted: bool,
    readout: f64,
    reset: Option<PackedPauli>,
}
#[derive(Clone, Debug)]
enum Condition {
    Record(usize),
    Sweep(u32),
}
// Every original channel outcome is retained, even if its effective Pauli
// becomes identity or matches another outcome: its record corrections can differ.
// Pauli phases are irrelevant for this classical mixture of unitary branches.
#[derive(Clone, Debug)]
struct NoiseChoice {
    pauli: PackedPauli,
    record_flips: Vec<usize>,
}
impl NoiseChoice {
    fn new(pauli: PackedPauli) -> Self {
        Self {
            pauli,
            record_flips: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
enum PlanOp {
    Basis(Vec<BasisGate>),
    Rotate {
        pauli: CompactPauli,
        expand: bool,
        dagger: bool,
    },
    Measure(Measurement),
    Noise {
        probability: f64,
        choices: Vec<NoiseChoice>,
    },
    Feedback {
        condition: Condition,
        pauli: PackedPauli,
    },
    Annotation {
        offsets: Vec<usize>,
        observable: Option<u32>,
    },
}

struct Product {
    terms: Vec<(usize, MeasurementBasis)>,
    inverted: bool,
}
fn mpp_products(targets: &[StimTarget]) -> Result<Vec<Product>, String> {
    use crate::ir::PauliBasis;
    if targets.is_empty() {
        return Err("compiled MPP requires a Pauli product".into());
    }
    let mut products = Vec::new();
    let mut terms = Vec::new();
    let mut inverted = false;
    let mut combined = false;
    let mut epoch = 1u32;
    let mut seen = [0u32; 4096];
    for target in targets {
        match target {
            StimTarget::Pauli {
                qubit,
                basis,
                inverted: inv,
            } => {
                if !combined && !terms.is_empty() {
                    products
                        .try_reserve(1)
                        .map_err(|e| format!("compiled product allocation failed: {e}"))?;
                    products.push(Product {
                        terms: std::mem::take(&mut terms),
                        inverted,
                    });
                    inverted = false;
                    epoch = epoch
                        .checked_add(1)
                        .ok_or("compiled product count overflow")?;
                }
                let q = *qubit as usize;
                if q >= seen.len() {
                    return Err("compiled MPP qubit exceeds width limit".into());
                }
                if seen[q] == epoch {
                    return Err("compiled MPP product requires distinct qubits".into());
                }
                seen[q] = epoch;
                terms
                    .try_reserve(1)
                    .map_err(|e| format!("compiled product allocation failed: {e}"))?;
                terms.push((
                    q,
                    match basis {
                        PauliBasis::X => MeasurementBasis::X,
                        PauliBasis::Y => MeasurementBasis::Y,
                        PauliBasis::Z => MeasurementBasis::Z,
                    },
                ));
                inverted ^= *inv;
                combined = false;
            }
            StimTarget::Combiner if !terms.is_empty() && !combined => combined = true,
            _ => return Err("compiled MPP requires well-formed Pauli products".into()),
        }
    }
    if combined {
        return Err("compiled MPP ends with a combiner".into());
    }
    products
        .try_reserve(1)
        .map_err(|e| format!("compiled product allocation failed: {e}"))?;
    products.push(Product { terms, inverted });
    Ok(products)
}
fn readout_probability(args: &[f64]) -> Result<f64, String> {
    if args.len() > 1
        || args
            .first()
            .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        return Err("compiled measurement requires zero or one probability in [0,1]".into());
    }
    Ok(args.first().copied().unwrap_or(0.))
}
#[derive(Default)]
struct ValidationBudget {
    instructions: usize,
    bytes: usize,
}
impl ValidationBudget {
    fn reserve(&mut self, bytes: usize) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or("compiled validation size overflow")?;
        if self.bytes > PLAN_BYTE_BUDGET {
            return Err("compiled validation reservation exceeds 64 MiB".into());
        }
        Ok(())
    }
}
// Only validate contracts/width/record dependencies with this view. The actual
// Pauli tape MUST be built from the untouched original instructions.
fn validation_view(
    instructions: &[StimInstr],
    budget: &mut ValidationBudget,
) -> Result<Vec<StimInstr>, String> {
    budget.instructions = budget
        .instructions
        .checked_add(instructions.len())
        .ok_or("compiled validation count overflow")?;
    if budget.instructions > 1_000_000 {
        return Err("compiled validation instruction limit exceeded".into());
    }
    budget.reserve(
        instructions
            .len()
            .checked_mul(2 * size_of::<StimInstr>())
            .ok_or("compiled validation size overflow")?,
    )?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(2 * instructions.len())
        .map_err(|e| format!("compiled validation allocation failed: {e}"))?;
    for instruction in instructions {
        match instruction {
            StimInstr::Repeat { count, body } => output.push(StimInstr::Repeat {
                count: *count,
                body: validation_view(body, budget)?,
            }),
            StimInstr::Op {
                name,
                args,
                targets,
                tag,
            } => {
                budget.instructions = budget
                    .instructions
                    .checked_add(targets.len())
                    .ok_or("compiled validation count overflow")?;
                if budget.instructions > 1_000_000 {
                    return Err("compiled validation target limit exceeded".into());
                }
                let payload = targets
                    .len()
                    .checked_mul(4 * size_of::<StimTarget>())
                    .and_then(|n| n.checked_add(args.len().checked_mul(2 * size_of::<f64>())?))
                    .and_then(|n| {
                        n.checked_add(2 * name.len() + tag.as_ref().map_or(0, |s| 2 * s.len()))
                    })
                    .ok_or("compiled validation size overflow")?;
                budget.reserve(payload)?;
                if name == "MPP" {
                    readout_probability(args)?;
                    let products = mpp_products(targets)?;
                    let all = products
                        .iter()
                        .flat_map(|p| p.terms.iter().map(|(q, _)| StimTarget::Qubit(*q as u32)))
                        .collect();
                    let measured = products
                        .iter()
                        .map(|p| StimTarget::Qubit(p.terms[0].0 as u32))
                        .collect();
                    output.push(StimInstr::new("I", Vec::new(), all));
                    output.push(StimInstr::new("M", Vec::new(), measured));
                } else {
                    let mut item = instruction.clone();
                    if matches!(
                        name.as_str(),
                        "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY"
                    ) {
                        readout_probability(args)?;
                        let StimInstr::Op { args, .. } = &mut item else {
                            unreachable!()
                        };
                        args.clear();
                    }
                    output.push(item);
                }
            }
        }
    }
    Ok(output)
}

// Operators are expressed in the initial physical coordinate system;
// all fixed physical Cliffords have been pulled into the offline unitary.
enum TapeOp {
    Rotate {
        pauli: PackedPauli,
        dagger: bool,
    },
    Measure {
        pauli: PackedPauli,
        record: Option<usize>,
        inverted: bool,
        readout: f64,
        reset: Option<PackedPauli>,
    },
    Noise {
        probability: f64,
        choices: Vec<NoiseChoice>,
    },
    Feedback {
        condition: Condition,
        pauli: PackedPauli,
    },
    Annotation {
        offsets: Vec<usize>,
        observable: Option<u32>,
    },
}
fn commute(a: &PackedPauli, b: &PackedPauli) -> bool {
    !a.anticommutes(&b.x, &b.z)
}
fn remove_unobservable_rotations(tape: &mut Vec<TapeOp>) -> Result<(), String> {
    // Sampling-only: move R(P) to the unobserved end of each trajectory.
    // Pauli noise, feedback, and reset corrections can flip its angle; commuting
    // projectors and retained rotations preserve every record probability.
    let mut removed = Vec::new();
    removed
        .try_reserve_exact(tape.len())
        .map_err(|e| format!("compiled tape allocation failed: {e}"))?;
    removed.resize(tape.len(), false);
    // A suffix scan would be quadratic even when it visits only removed items.
    // This constraint list makes non-comparison work linear in tape length.
    let mut future = Vec::new();
    future
        .try_reserve_exact(tape.len())
        .map_err(|e| format!("compiled tape allocation failed: {e}"))?;
    let mut remaining = 4_000_000usize;
    for index in (0..tape.len()).rev() {
        match &tape[index] {
            TapeOp::Measure { .. } => future.push(index),
            TapeOp::Rotate { pauli, .. } => {
                let mut keep = false;
                for &next in &future {
                    let other = match &tape[next] {
                        TapeOp::Rotate { pauli, .. } | TapeOp::Measure { pauli, .. } => pauli,
                        _ => unreachable!("future quantum constraint"),
                    };
                    let Some(budget) = remaining.checked_sub(pauli.x.len().max(1)) else {
                        keep = true;
                        break;
                    };
                    remaining = budget;
                    if !commute(pauli, other) {
                        keep = true;
                        break;
                    }
                }
                if keep {
                    future.push(index);
                } else {
                    removed[index] = true;
                }
            }
            _ => {}
        }
    }
    let mut index = 0;
    tape.retain(|_| {
        let keep = !removed[index];
        index += 1;
        keep
    });
    Ok(())
}

fn schedule_measurements(tape: &mut [TapeOp], reserved_bytes: &mut usize) -> Result<(), String> {
    // Rotations and other instruments use conservative all-P/Q commutation.
    // A Pauli noise outcome can always cross a measurement instrument:
    // Q^a Pi_a E = phase (E Q^c) Q^b Pi_b, c=[E,P], b=a xor c.
    // The moved instrument uses its early ideal b, and the deferred noise XORs
    // c into the reported record (after independent inversion/readout noise).
    // Full P/Q commutation permits measurement exchange; feedback/annotations
    // remain hard barriers while deferred noise restores original record labels.
    let reorder_measurements = true;
    let mut remaining = 4_000_000usize;
    for start in 0..tape.len() {
        let TapeOp::Measure {
            pauli,
            reset,
            record,
            ..
        } = &tape[start]
        else {
            continue;
        };
        let pauli = pauli.clone();
        let reset = reset.clone();
        let record = *record;
        let operators = std::iter::once(&pauli).chain(reset.iter());
        let mut position = start;
        while position > 0 {
            let other_count = match &tape[position - 1] {
                TapeOp::Noise { choices, .. } => choices.len().max(1),
                TapeOp::Measure { reset: Some(_), .. } => 2,
                _ => 1,
            };
            let cost = pauli
                .x
                .len()
                .max(1)
                .saturating_mul(other_count)
                .saturating_mul(1 + usize::from(reset.is_some()));
            let Some(next) = remaining.checked_sub(cost) else {
                return Ok(());
            };
            remaining = next;
            let movable = match &mut tape[position - 1] {
                TapeOp::Rotate { pauli: other, .. } => operators.clone().all(|p| commute(p, other)),
                TapeOp::Noise { choices, .. } => {
                    // Stage A emits at most the fifteen DEPOLARIZE2 choices.
                    // Bound temporary storage without any per-swap heap allocation.
                    if choices.is_empty() || choices.len() > 15 {
                        return Err("compiled noise choice count out of range".into());
                    }
                    let mut anti = [false; 15];
                    let mut additional = 0usize;
                    for (index, choice) in choices.iter().enumerate() {
                        // Repeated swaps must use the already transformed E.
                        anti[index] = !commute(&pauli, &choice.pauli);
                        additional += usize::from(anti[index] && record.is_some());
                    }
                    // Capture and final plan have separate 64 MiB reservations.
                    // Two slots per added index conservatively cover Vec capacity.
                    let charge = additional
                        .checked_mul(2 * size_of::<usize>())
                        .ok_or("compiled scheduled record byte overflow")?;
                    let total = reserved_bytes
                        .checked_add(charge)
                        .ok_or("compiled scheduled plan byte overflow")?;
                    if total > PLAN_BYTE_BUDGET {
                        return Err("compiled scheduled plan reservation exceeds 64 MiB".into());
                    }
                    // Reserve for the whole channel before changing any Pauli.
                    // Allocation failure aborts compilation, never partially falls back.
                    if record.is_some() {
                        for (index, choice) in choices.iter_mut().enumerate() {
                            if anti[index] {
                                choice.record_flips.try_reserve_exact(1).map_err(|e| {
                                    format!("compiled scheduled record allocation failed: {e}")
                                })?;
                            }
                        }
                    }
                    *reserved_bytes = total;
                    for (index, choice) in choices.iter_mut().enumerate() {
                        if !anti[index] {
                            continue;
                        }
                        if let Some(correction) = &reset {
                            // Only x/z matter for a virtual Pauli frame. Its phase
                            // stays irrelevant even when E Q is anti-Hermitian.
                            for (x, qx) in choice.pauli.x.iter_mut().zip(&correction.x) {
                                *x ^= qx;
                            }
                            for (z, qz) in choice.pauli.z.iter_mut().zip(&correction.z) {
                                *z ^= qz;
                            }
                        }
                        if let Some(record) = record {
                            // Keep earlier corrections; repeated indices XOR at runtime.
                            choice.record_flips.push(record);
                        }
                    }
                    true
                }
                TapeOp::Measure {
                    pauli: other,
                    reset: other_reset,
                    ..
                } => {
                    reorder_measurements
                        && operators.clone().all(|p| {
                            std::iter::once(&*other)
                                .chain(other_reset.iter())
                                .all(|other| commute(p, other))
                        })
                }
                _ => false,
            };
            if !movable {
                break;
            }
            tape.swap(position, position - 1);
            position -= 1;
        }
    }
    Ok(())
}

/// Opt-in offline plan. Its own reproducible RNG policy is distinct from the
/// adaptive incumbent sampler; physical Cliffords are evaluated only at compile time.
/// Structural rank is independent of amplitudes. Signed commuting measurements
/// are scheduled across compatible operators, then scattered into original record order.
/// Rotations that cannot affect any future record are omitted; no final state is exposed.
/// Native MPP accepts products with distinct qubit factors. Factor inversions
/// are XORed into the reported bit. M/MR/MPP readout noise changes only records;
/// projection and reset always use the ideal physical branch.
/// Compilation reserves at most 64 MiB each for the validation view, transient Pauli tape, emitted
/// plan, and live/reduced coefficients. Tape and plan can coexist during compilation.
/// The packed compilation frame reserves at most 9 MiB. Optional transitions
/// reserve at most 64 MiB and packet event/frame work buffers 16 MiB.
/// Optional homogeneous random-event runs use at most 8 MiB within the plan budget; rejection
/// retains the event-by-event generator with identical RNG and records.
/// Batched coherent arithmetic separately reserves at most 64 MiB; larger
/// structural ranks retain the scalar/cache path.
/// These are conservative reservations, not RSS limits. Limits fail before sampling.
#[derive(Clone, Debug)]
pub struct CompiledNearCliffordExecutor {
    operations: Vec<PlanOp>,
    num_qubits: usize,
    measurement_count: usize,
    peak_active_rank: usize,
    prefix_len: usize,
    initial_coefficients: Arc<Vec<ComplexAmp>>,
    random_kinds: Vec<RandomKind>,
    random_runs: Option<RandomRunPlan>,
    noise_event_count: usize,
}
struct Planner {
    state: CompileFrame,
    axes: Vec<usize>,
    limit: usize,
    peak: usize,
    operations: Vec<PlanOp>,
    expanded: usize,
    reserved_bytes: usize,
    tape: Option<Vec<TapeOp>>,
    record_count: usize,
}
impl Planner {
    fn apply_pair(&mut self, name: &str, a: usize, b: usize) -> Result<(), String> {
        match name {
            "CX" | "CNOT" | "ZCX" => self.state.apply_clifford(CliffordGate::CX(a, b)),
            "CZ" | "ZCZ" => self.state.apply_clifford(CliffordGate::CZ(a, b)),
            "SWAP" => self.state.apply_clifford(CliffordGate::Swap(a, b)),
            "CY" | "ZCY" => {
                self.state.apply_clifford(CliffordGate::SDag(b))?;
                self.state.apply_clifford(CliffordGate::CX(a, b))?;
                self.state.apply_clifford(CliffordGate::S(b))
            }
            _ => unreachable!("validated compiled pair"),
        }
    }
    fn pauli(&self, q: usize, basis: MeasurementBasis) -> Result<Pauli, String> {
        self.state.pauli(q, basis)
    }
    fn compact(&self, p: &Pauli) -> CompactPauli {
        let mask = |bits: &[bool]| {
            self.axes
                .iter()
                .enumerate()
                .fold(0, |m, (i, &q)| m | (usize::from(bits[q]) << i))
        };
        CompactPauli {
            physical: PackedPauli::new(p),
            x: mask(&p.x),
            z: mask(&p.z),
        }
    }
    fn right(&mut self, gate: BasisGate, gates: &mut Vec<BasisGate>) {
        self.state.right(gate);
        gates.push(gate);
    }
    fn rotate(&mut self, q: usize, dagger: bool) -> Result<(), String> {
        let pauli = PackedPauli::new(&self.pauli(q, MeasurementBasis::Z)?);
        if self.tape.is_some() {
            return self.emit(TapeOp::Rotate { pauli, dagger });
        }
        // Test-only direct planners retain the single physical coordinate API.
        let mut original = Pauli::identity(self.state.num_qubits);
        original.z[q] = true;
        self.rotate_pauli(&PackedPauli::new(&original), dagger)
    }
    fn reexpress(&self, input: &PackedPauli) -> Result<Pauli, String> {
        self.state.reexpress(input)
    }
    fn rotate_pauli(&mut self, original: &PackedPauli, dagger: bool) -> Result<(), String> {
        let mut p = self.reexpress(original)?;
        let pivot = (0..self.state.num_qubits).find(|q| p.x[*q] && !self.axes.contains(q));
        let expand = pivot.is_some();
        if let Some(pivot) = pivot {
            if self.axes.len() >= self.limit {
                return Err("compiled near-Clifford structural active-rank limit exceeded".into());
            }
            let count = 1usize
                .checked_shl((self.axes.len() + 1) as u32)
                .ok_or("compiled coefficient size overflow")?;
            // Immutable prefix and both retained work buffers can coexist.
            let bytes = count
                .checked_mul(3 * size_of::<ComplexAmp>())
                .ok_or("compiled coefficient byte overflow")?;
            if bytes > COEFFICIENT_BYTE_BUDGET {
                return Err("compiled coefficient reservation exceeds 64 MiB".into());
            }
            let mut gates = Vec::new();
            for bit in 0..self.state.num_qubits {
                if bit != pivot && p.x[bit] {
                    self.right(BasisGate::CX(pivot, bit), &mut gates);
                }
            }
            self.axes.push(pivot);
            self.peak = self.peak.max(self.axes.len());
            if !gates.is_empty() {
                self.operations.push(PlanOp::Basis(gates));
            }
            p = self.reexpress(original)?;
        }
        self.operations.push(PlanOp::Rotate {
            pauli: self.compact(&p),
            expand,
            dagger,
        });
        Ok(())
    }
    #[cfg(test)]
    fn measure(
        &mut self,
        q: usize,
        basis: MeasurementBasis,
        record: bool,
        inverted: bool,
        reset: bool,
    ) -> Result<(), String> {
        self.measure_with_error(q, basis, record, inverted, reset, 0.)
    }
    fn measure_with_error(
        &mut self,
        q: usize,
        basis: MeasurementBasis,
        record: bool,
        inverted: bool,
        reset: bool,
        readout: f64,
    ) -> Result<(), String> {
        let record_index = if record {
            let index = self.record_count;
            self.record_count += 1;
            Some(index)
        } else {
            None
        };
        if self.tape.is_some() {
            let pauli = PackedPauli::new(&self.pauli(q, basis)?);
            let reset = if reset {
                Some(PackedPauli::new(&self.pauli(
                    q,
                    if basis == MeasurementBasis::X {
                        MeasurementBasis::Z
                    } else {
                        MeasurementBasis::X
                    },
                )?))
            } else {
                None
            };
            return self.emit(TapeOp::Measure {
                pauli,
                record: record_index,
                inverted,
                readout,
                reset,
            });
        }
        let mut pauli = Pauli::identity(self.state.num_qubits);
        pauli.x[q] = basis != MeasurementBasis::Z;
        pauli.z[q] = basis != MeasurementBasis::X;
        pauli.phase = u8::from(basis == MeasurementBasis::Y);
        let correction = if reset {
            let mut correction = Pauli::identity(self.state.num_qubits);
            if basis == MeasurementBasis::X {
                correction.z[q] = true;
            } else {
                correction.x[q] = true;
            }
            Some(PackedPauli::new(&correction))
        } else {
            None
        };
        self.measure_pauli(
            &PackedPauli::new(&pauli),
            record_index,
            inverted,
            correction.as_ref(),
            readout,
        )
    }
    fn measure_pauli(
        &mut self,
        original: &PackedPauli,
        record: Option<usize>,
        inverted: bool,
        reset: Option<&PackedPauli>,
        readout: f64,
    ) -> Result<(), String> {
        let p = self.reexpress(original)?;
        let compact = self.compact(&p);
        let mut gates = Vec::new();
        let projection;
        if let Some(pivot) =
            (0..self.state.num_qubits).find(|bit| p.x[*bit] && !self.axes.contains(bit))
        {
            for bit in 0..self.state.num_qubits {
                if bit != pivot && p.x[bit] {
                    self.right(BasisGate::CX(pivot, bit), &mut gates);
                }
            }
            for bit in self.axes.clone() {
                if p.z[bit] {
                    self.right(BasisGate::CZ(pivot, bit), &mut gates);
                }
            }
            for _ in 0..p.phase {
                self.right(BasisGate::S(pivot), &mut gates);
            }
            self.right(BasisGate::H(pivot), &mut gates);
            projection = Projection::Independent { pivot };
        } else if compact.x == 0 && compact.z == 0 {
            projection = Projection::Constant(p.phase == 2);
        } else {
            let index = if compact.x == 0 {
                compact.z.trailing_zeros()
            } else {
                compact.x.trailing_zeros()
            } as usize;
            let pivot = self.axes[index];
            let y = (compact.x & compact.z).count_ones() % 2 != 0;
            let phase = (p.phase + 4 - u8::from(y)) % 4;
            if phase % 2 != 0 {
                return Err("non-Hermitian compiled measurement".into());
            }
            if compact.x == 0 {
                for (i, bit) in self.axes.clone().into_iter().enumerate() {
                    if i != index && compact.z & (1 << i) != 0 {
                        self.right(BasisGate::CX(bit, pivot), &mut gates);
                    }
                }
            } else {
                for (i, bit) in self.axes.clone().into_iter().enumerate() {
                    if i != index && compact.x & (1 << i) != 0 {
                        self.right(BasisGate::CX(pivot, bit), &mut gates);
                    }
                }
                for (i, bit) in self.axes.clone().into_iter().enumerate() {
                    if i != index && compact.z & (1 << i) != 0 {
                        self.right(BasisGate::CZ(pivot, bit), &mut gates);
                    }
                }
                if y {
                    self.right(BasisGate::S(pivot), &mut gates);
                }
                self.right(BasisGate::H(pivot), &mut gates);
            }
            self.axes.remove(index);
            projection = Projection::Active {
                pivot,
                index,
                y,
                offset: phase == 2,
            };
        }
        // The correction must use the post-projection basis V_after.
        let correction = reset
            .map(|p| self.reexpress(p).map(|p| PackedPauli::new(&p)))
            .transpose()?;
        self.operations.push(PlanOp::Measure(Measurement {
            pauli: compact,
            projection,
            basis: gates,
            record,
            inverted,
            readout,
            reset: correction,
        }));
        Ok(())
    }
}

impl CompiledNearCliffordExecutor {
    pub fn compile_text(text: &str) -> Result<Self, String> {
        Self::compile_with_limit(crate::parser::parse_lines(text)?, 16)
    }
    pub fn compile_with_limit(instructions: Vec<StimInstr>, limit: usize) -> Result<Self, String> {
        if limit >= usize::BITS as usize - 1 {
            return Err("compiled near-Clifford rank exceeds platform capacity".into());
        }
        let normalized = validation_view(&instructions, &mut ValidationBudget::default())?;
        let incumbent = NearCliffordExecutor::compile_with_limit(normalized, limit)?;
        let mut planner = Planner {
            state: CompileFrame::identity(incumbent.num_qubits)?,
            axes: Vec::new(),
            limit,
            peak: 0,
            operations: Vec::new(),
            expanded: 0,
            reserved_bytes: 0,
            tape: None,
            record_count: 0,
        };
        planner.tape = Some(Vec::new());
        planner.block(&instructions)?;
        let mut tape = planner.tape.take().unwrap();
        remove_unobservable_rotations(&mut tape)?;
        schedule_measurements(&mut tape, &mut planner.reserved_bytes)?;
        planner.state.reset_identity();
        planner.expanded = 0;
        planner.reserved_bytes = 0;
        for op in tape {
            planner.finish_op(op)?;
        }
        let mut plan = Self {
            operations: planner.operations,
            num_qubits: incumbent.num_qubits,
            measurement_count: incumbent.measurement_count,
            peak_active_rank: planner.peak,
            prefix_len: 0,
            initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            random_kinds: Vec::new(),
            random_runs: None,
            noise_event_count: 0,
        };
        let mut filled = Vec::new();
        filled
            .try_reserve_exact(plan.measurement_count)
            .map_err(|e| format!("compiled record validation allocation failed: {e}"))?;
        filled.resize(plan.measurement_count, false);
        for op in &plan.operations {
            match op {
                PlanOp::Measure(Measurement {
                    record: Some(index),
                    ..
                }) => {
                    let bit = filled
                        .get_mut(*index)
                        .ok_or("compiled record index out of range")?;
                    if *bit {
                        return Err("compiled duplicate record index".into());
                    }
                    *bit = true;
                }
                PlanOp::Noise { choices, .. } => {
                    if choices.iter().any(|choice| {
                        choice
                            .record_flips
                            .iter()
                            .any(|index| !filled.get(*index).copied().unwrap_or(false))
                    }) {
                        return Err("compiled scheduled noise precedes its record".into());
                    }
                }
                PlanOp::Feedback {
                    condition: Condition::Record(index),
                    ..
                } => {
                    if !filled.get(*index).copied().unwrap_or(false) {
                        return Err("compiled feedback precedes its record".into());
                    }
                }
                PlanOp::Annotation { offsets, .. } => {
                    if offsets
                        .iter()
                        .any(|index| !filled.get(*index).copied().unwrap_or(false))
                    {
                        return Err("compiled annotation precedes its record".into());
                    }
                }
                _ => {}
            }
        }
        if filled.iter().any(|bit| !*bit) {
            return Err("compiled missing record index".into());
        }
        let prefix = plan
            .operations
            .iter()
            .take_while(|op| matches!(op, PlanOp::Basis(_) | PlanOp::Rotate { .. }))
            .count();
        let mut sampler = plan.prepare_sampler_with_cache_budget(0)?;
        for op in &plan.operations[..prefix] {
            if let PlanOp::Rotate {
                pauli,
                expand,
                dagger,
            } = op
            {
                sampler.rotate(pauli, *expand, *dagger)?;
            }
            // The virtual frame is identity throughout this deterministic prefix.
        }
        let coefficients = std::mem::take(&mut sampler.coefficients);
        drop(sampler);
        plan.prefix_len = prefix;
        plan.initial_coefficients = Arc::new(coefficients);
        for op in &plan.operations[plan.prefix_len..] {
            let kind = match op {
                PlanOp::Noise {
                    probability,
                    choices,
                } => Some(RandomKind::Noise {
                    probability: *probability,
                    choices: choices.len(),
                }),
                PlanOp::Measure(Measurement {
                    projection: Projection::Independent { .. },
                    ..
                }) => Some(RandomKind::Independent),
                PlanOp::Measure(Measurement {
                    projection: Projection::Active { .. },
                    ..
                }) => Some(RandomKind::Active),
                _ => None,
            };
            if let Some(kind) = kind {
                plan.random_kinds
                    .try_reserve(1)
                    .map_err(|e| format!("compiled random plan allocation failed: {e}"))?;
                plan.random_kinds.push(kind);
            }
            if let PlanOp::Measure(Measurement {
                record: Some(_),
                readout,
                ..
            }) = op
            {
                if *readout > 0. {
                    plan.random_kinds
                        .try_reserve(1)
                        .map_err(|e| format!("compiled random plan allocation failed: {e}"))?;
                    plan.random_kinds.push(RandomKind::Noise {
                        probability: *readout,
                        choices: 1,
                    });
                }
            }
        }
        plan.noise_event_count = plan
            .random_kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Noise { .. }))
            .count();
        plan.random_runs = RandomRunPlan::build(
            &plan.random_kinds,
            PLAN_BYTE_BUDGET.saturating_sub(planner.reserved_bytes),
        );
        Ok(plan)
    }
    pub fn peak_active_rank(&self) -> usize {
        self.peak_active_rank
    }
    pub fn prepare_sampler(&self) -> Result<CompiledNearCliffordSampler<'_>, String> {
        self.prepare_sampler_with_cache_budget(DEFAULT_CACHE_BYTE_BUDGET)
    }
    /// Optional coefficient transitions use a conservative reservation, not RSS.
    /// Zero disables caching. Admission/allocation failure falls back to arithmetic.
    pub fn prepare_sampler_with_cache_budget(
        &self,
        cache_bytes: usize,
    ) -> Result<CompiledNearCliffordSampler<'_>, String> {
        if cache_bytes > DEFAULT_CACHE_BYTE_BUDGET {
            return Err("compiled cache budget exceeds 64 MiB".into());
        }
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(self.initial_coefficients.len())
            .map_err(|e| format!("compiled sampler allocation failed: {e}"))?;
        coefficients.extend_from_slice(&self.initial_coefficients);
        Ok(CompiledNearCliffordSampler {
            plan: self,
            x: vec![0; self.num_qubits.div_ceil(64)],
            z: vec![0; self.num_qubits.div_ceil(64)],
            coefficients,
            reduced_coefficients: Vec::new(),
            cache: CoefficientCache::new(self, cache_bytes),
            pack_enabled: true,
            #[cfg(test)]
            last_packet_live: 0,
            packet_x: Vec::new(),
            packet_z: Vec::new(),
            packet_tape: Vec::new(),
            packet_records: Vec::new(),
            packet_noise_masks: Vec::new(),
            coherent: CoherentPacket::default(),
        })
    }
    pub fn sample(
        &self,
        shots: usize,
        rng: &mut impl Rng,
    ) -> Result<Vec<NearCliffordShot>, String> {
        self.prepare_sampler()?.sample(shots, rng)
    }
}

/// Retains allocations, with identical streams for structured and flat APIs.
pub struct CompiledNearCliffordSampler<'a> {
    plan: &'a CompiledNearCliffordExecutor,
    x: Vec<u64>,
    z: Vec<u64>,
    coefficients: Vec<ComplexAmp>,
    reduced_coefficients: Vec<ComplexAmp>,
    cache: Option<CoefficientCache>,
    pack_enabled: bool,
    #[cfg(test)]
    last_packet_live: usize,
    packet_x: Vec<u64>,
    packet_z: Vec<u64>,
    packet_tape: Vec<u64>,
    packet_records: Vec<u64>,
    packet_noise_masks: Vec<u64>,
    coherent: CoherentPacket,
}

impl Planner {
    fn block(&mut self, instructions: &[StimInstr]) -> Result<(), String> {
        for instruction in instructions {
            self.expanded = self
                .expanded
                .checked_add(1)
                .ok_or("compiled plan size overflow")?;
            if self.expanded > 1_000_000 {
                return Err(
                    "compiled plan exceeds one million expanded instructions/targets".into(),
                );
            }
            match instruction {
                StimInstr::Repeat { count, body } => {
                    if *count > 1_000_000 {
                        return Err("compiled REPEAT expansion limit exceeded".into());
                    }
                    for _ in 0..*count {
                        self.block(body)?;
                    }
                }
                StimInstr::Op {
                    name,
                    args,
                    targets,
                    ..
                } => {
                    self.expanded = self
                        .expanded
                        .checked_add(targets.len())
                        .ok_or("compiled plan size overflow")?;
                    if self.expanded > 1_000_000 {
                        return Err("compiled plan target limit exceeded".into());
                    }
                    self.reserve_instruction(name, targets)?;
                    if matches!(
                        name.as_str(),
                        "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ" | "SWAP"
                    ) {
                        for pair in targets.chunks_exact(2) {
                            let b = pair[1].qubit_index().unwrap() as usize;
                            match pair[0] {
                                StimTarget::Qubit(a) => self.apply_pair(name, a as usize, b)?,
                                StimTarget::Rec(_) | StimTarget::Sweep(_) => {
                                    let condition = match pair[0] {
                                        StimTarget::Rec(offset) => {
                                            Condition::Record(self.absolute_record(offset)?)
                                        }
                                        StimTarget::Sweep(k) => Condition::Sweep(k),
                                        _ => unreachable!(),
                                    };
                                    let basis = match name.as_str() {
                                        "CY" | "ZCY" => MeasurementBasis::Y,
                                        "CZ" | "ZCZ" => MeasurementBasis::Z,
                                        _ => MeasurementBasis::X,
                                    };
                                    let pauli = PackedPauli::new(&self.pauli(b, basis)?);
                                    self.emit(TapeOp::Feedback { condition, pauli })?;
                                }
                                _ => unreachable!("validated feedback"),
                            }
                        }
                        continue;
                    }
                    if name == "DEPOLARIZE2" {
                        for pair in targets.chunks_exact(2) {
                            let a = pair[0].qubit_index().unwrap() as usize;
                            let b = pair[1].qubit_index().unwrap() as usize;
                            let mut choices = Vec::new();
                            for branch in 1..16 {
                                let mut p = Pauli::identity(self.state.num_qubits);
                                for (q, branch) in [(a, branch / 4), (b, branch % 4)] {
                                    if branch != 0 {
                                        let other = self.pauli(
                                            q,
                                            match branch {
                                                1 => MeasurementBasis::X,
                                                2 => MeasurementBasis::Y,
                                                _ => MeasurementBasis::Z,
                                            },
                                        )?;
                                        xor(&mut p.x, &other.x);
                                        xor(&mut p.z, &other.z);
                                    }
                                }
                                choices.push(NoiseChoice::new(PackedPauli::new(&p)));
                            }
                            self.emit(TapeOp::Noise {
                                probability: args[0],
                                choices,
                            })?;
                        }
                        continue;
                    }
                    if matches!(name.as_str(), "DETECTOR" | "OBSERVABLE_INCLUDE") {
                        self.emit(TapeOp::Annotation {
                            offsets: targets
                                .iter()
                                .map(|t| match t {
                                    StimTarget::Rec(offset) => self.absolute_record(*offset),
                                    _ => unreachable!(),
                                })
                                .collect::<Result<Vec<_>, String>>()?,
                            observable: if name == "OBSERVABLE_INCLUDE" {
                                Some(args[0] as u32)
                            } else {
                                None
                            },
                        })?;
                        continue;
                    }
                    if matches!(name.as_str(), "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS") {
                        continue;
                    }
                    if name == "MPP" {
                        let readout = readout_probability(args)?;
                        for product in mpp_products(targets)? {
                            let mut physical = Pauli::identity(self.state.num_qubits);
                            for (q, basis) in product.terms {
                                physical.x[q] = basis != MeasurementBasis::Z;
                                physical.z[q] = basis != MeasurementBasis::X;
                                physical.phase =
                                    (physical.phase + u8::from(basis == MeasurementBasis::Y)) % 4;
                            }
                            let pauli =
                                PackedPauli::new(&self.reexpress(&PackedPauli::new(&physical))?);
                            let index = self.record_count;
                            self.record_count += 1;
                            self.emit(TapeOp::Measure {
                                pauli,
                                record: Some(index),
                                inverted: product.inverted,
                                readout,
                                reset: None,
                            })?;
                        }
                        continue;
                    }
                    for target in targets {
                        let q = target.qubit_index().unwrap() as usize;
                        match name.as_str() {
                            "I" => {}
                            "H" => self.state.apply_clifford(CliffordGate::H(q))?,
                            "S" | "SQRT_Z" => self.state.apply_clifford(CliffordGate::S(q))?,
                            "S_DAG" | "SQRT_Z_DAG" => {
                                self.state.apply_clifford(CliffordGate::SDag(q))?
                            }
                            "X" => self.state.apply_clifford(CliffordGate::X(q))?,
                            "Y" => self.state.apply_clifford(CliffordGate::Y(q))?,
                            "Z" => self.state.apply_clifford(CliffordGate::Z(q))?,
                            "T" | "T_DAG" => self.rotate(q, name == "T_DAG")?,
                            "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" => {
                                let bases = match name.as_str() {
                                    "X_ERROR" => vec![MeasurementBasis::X],
                                    "Y_ERROR" => vec![MeasurementBasis::Y],
                                    "Z_ERROR" => vec![MeasurementBasis::Z],
                                    _ => vec![
                                        MeasurementBasis::X,
                                        MeasurementBasis::Y,
                                        MeasurementBasis::Z,
                                    ],
                                };
                                let choices = bases
                                    .into_iter()
                                    .map(|basis| {
                                        self.pauli(q, basis)
                                            .map(|p| NoiseChoice::new(PackedPauli::new(&p)))
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                                self.emit(TapeOp::Noise {
                                    probability: args[0],
                                    choices,
                                })?;
                            }
                            "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "R"
                            | "RZ" | "RX" | "RY" => {
                                let basis = if name.ends_with('X') {
                                    MeasurementBasis::X
                                } else if name.ends_with('Y') {
                                    MeasurementBasis::Y
                                } else {
                                    MeasurementBasis::Z
                                };
                                self.measure_with_error(
                                    q,
                                    basis,
                                    name.starts_with('M'),
                                    matches!(target, StimTarget::QubitInv(_)),
                                    name.starts_with('R') || name.starts_with("MR"),
                                    args.first().copied().unwrap_or(0.),
                                )?;
                            }
                            _ => unreachable!("validated compiled near-Clifford operation"),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn absolute_record(&self, offset: i32) -> Result<usize, String> {
        let distance = offset.unsigned_abs() as usize;
        if offset >= 0 || distance > self.record_count {
            return Err("compiled rec out of range".into());
        }
        Ok(self.record_count - distance)
    }
    fn emit(&mut self, op: TapeOp) -> Result<(), String> {
        if let Some(tape) = &mut self.tape {
            tape.try_reserve(1)
                .map_err(|e| format!("compiled tape allocation failed: {e}"))?;
            tape.push(op);
            Ok(())
        } else {
            self.finish_op(op)
        }
    }
    fn finish_op(&mut self, op: TapeOp) -> Result<(), String> {
        let target = [StimTarget::Qubit(0)];
        match op {
            TapeOp::Rotate { pauli, dagger } => {
                self.reserve_instruction("T", &target)?;
                self.rotate_pauli(&pauli, dagger)?;
            }
            TapeOp::Measure {
                pauli,
                record,
                inverted,
                reset,
                readout,
            } => {
                self.reserve_instruction("M", &target)?;
                self.measure_pauli(&pauli, record, inverted, reset.as_ref(), readout)?;
            }
            TapeOp::Noise {
                probability,
                choices,
            } => {
                self.reserve_instruction(
                    match choices.len() {
                        15 => "DEPOLARIZE2",
                        3 => "DEPOLARIZE1",
                        _ => "X_ERROR",
                    },
                    &target,
                )?;
                // The moved record-index buffers are owned by this final plan;
                // reserve their full capacities before allocating the new choices.
                let flip_capacity = choices
                    .iter()
                    .try_fold(0usize, |total, choice| {
                        total.checked_add(choice.record_flips.capacity())
                    })
                    .ok_or("compiled scheduled record capacity overflow")?;
                let charge = flip_capacity
                    .checked_mul(2 * size_of::<usize>())
                    .ok_or("compiled scheduled record byte overflow")?;
                let total = self
                    .reserved_bytes
                    .checked_add(charge)
                    .ok_or("compiled scheduled plan byte overflow")?;
                if total > PLAN_BYTE_BUDGET {
                    return Err("compiled scheduled plan reservation exceeds 64 MiB".into());
                }
                self.reserved_bytes = total;
                let mut planned = Vec::new();
                planned
                    .try_reserve_exact(choices.len())
                    .map_err(|e| format!("compiled scheduled noise allocation failed: {e}"))?;
                for choice in choices {
                    planned.push(NoiseChoice {
                        pauli: PackedPauli::new(&self.reexpress(&choice.pauli)?),
                        record_flips: choice.record_flips,
                    });
                }
                self.operations.push(PlanOp::Noise {
                    probability,
                    choices: planned,
                });
            }
            TapeOp::Feedback { condition, pauli } => {
                self.reserve_instruction("X_ERROR", &target)?;
                let pauli = PackedPauli::new(&self.reexpress(&pauli)?);
                self.operations.push(PlanOp::Feedback { condition, pauli });
            }
            TapeOp::Annotation {
                offsets,
                observable,
            } => {
                // Reserve capacity including every absolute-record index.
                let count = offsets.len().max(1);
                let charge = count
                    .checked_mul(4 * size_of::<PlanOp>() + 2 * size_of::<usize>())
                    .ok_or("compiled annotation overflow")?;
                self.reserved_bytes = self
                    .reserved_bytes
                    .checked_add(charge)
                    .ok_or("compiled plan byte overflow")?;
                if self.reserved_bytes > PLAN_BYTE_BUDGET {
                    return Err("compiled plan reservation exceeds 64 MiB".into());
                }
                self.operations
                    .try_reserve(1)
                    .map_err(|e| format!("compiled plan allocation failed: {e}"))?;
                self.operations.push(PlanOp::Annotation {
                    offsets,
                    observable,
                });
            }
        }
        Ok(())
    }

    fn reserve_instruction(&mut self, name: &str, targets: &[StimTarget]) -> Result<(), String> {
        let n = self.state.num_qubits;
        let packed = 2 * (size_of::<PackedPauli>() + 2 * n.div_ceil(64) * size_of::<u64>());
        // Empty flip-vector metadata belongs to each original noise outcome.
        // Its dynamic indices are charged separately by scheduler and Stage B.
        let noise_packed = 2 * (size_of::<NoiseChoice>() + 2 * n.div_ceil(64) * size_of::<u64>());
        let basis = 2 * (2 * n + 4) * size_of::<BasisGate>();
        // Two emitted nodes, up to two typed events, and Vec growth headroom.
        let base = 4 * (size_of::<PlanOp>() + size_of::<RandomKind>());
        let per_target = match name {
            "T" | "T_DAG" => base + packed + basis,
            "MPP" | "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "R" | "RZ" | "RX"
            | "RY" => base + 2 * packed + basis,
            "DEPOLARIZE2" => base + 15 * noise_packed,
            "DEPOLARIZE1" => base + 3 * noise_packed,
            "X_ERROR" | "Y_ERROR" | "Z_ERROR" => base + noise_packed,
            "DETECTOR" | "OBSERVABLE_INCLUDE" => base + size_of::<i32>(),
            "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ" => base + packed,
            _ => 0,
        };
        if per_target == 0 {
            return Ok(());
        }
        let count = if matches!(name, "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ") {
            targets
                .chunks_exact(2)
                .filter(|pair| !matches!(pair[0], StimTarget::Qubit(_)))
                .count()
        } else {
            targets.len().max(1)
        };
        let charge = per_target
            .checked_mul(count)
            .ok_or("compiled plan byte overflow")?;
        let total = self
            .reserved_bytes
            .checked_add(charge)
            .ok_or("compiled plan byte overflow")?;
        if total > PLAN_BYTE_BUDGET {
            return Err("compiled plan reservation exceeds 64 MiB".into());
        }
        self.operations
            .try_reserve(count)
            .map_err(|e| format!("compiled plan allocation failed: {e}"))?;
        self.reserved_bytes = total;
        Ok(())
    }
}

// Snapshot the input state before broadcasting a transition. A diverse packet
// takes the existing lane path; shared immutable states need one lookup per sign.
fn uniform_packet_state(states: &[Option<usize>; 64], mut live: u64) -> Option<usize> {
    if live == 0 {
        return None;
    }
    let id = states[live.trailing_zeros() as usize]?;
    while live != 0 {
        let lane = live.trailing_zeros() as usize;
        live &= live - 1;
        if states[lane] != Some(id) {
            return None;
        }
    }
    Some(id)
}
fn broadcast_packet_state(states: &mut [Option<usize>; 64], mut mask: u64, id: Option<usize>) {
    while mask != 0 {
        let lane = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        states[lane] = id;
    }
}

fn fill_original_row_with_noise_masks<R: Rng>(
    random: &mut RowRandom<'_, R>,
    kinds: &[RandomKind],
    output: &mut [u64],
    noise_masks: &mut [u64],
    lane_bit: u64,
) {
    debug_assert_eq!(kinds.len(), output.len());
    let mut noise = 0;
    for (&kind, value) in kinds.iter().zip(output) {
        *value = random.draw(kind);
        if matches!(kind, RandomKind::Noise { .. }) {
            if *value != 0 {
                noise_masks[noise] |= lane_bit;
            }
            noise += 1;
        }
    }
    debug_assert_eq!(noise, noise_masks.len());
}

// This function belongs in compiled.rs, used at PlanOp::Noise. `nonzero` is
// exactly the populated lane set; full tape is still retained for replay.
// The single-choice fast path is valid for any channel with one outcome.
#[inline]
fn packet_choice_masks(
    nonzero: u64,
    choices: usize,
    tape: &[u64],
    event: usize,
    random_count: usize,
) -> [u64; 15] {
    let mut result = [0; 15];
    if choices == 1 {
        result[0] = nonzero;
        return result;
    }
    debug_assert!((1..=15).contains(&choices));
    let mut hits = nonzero;
    while hits != 0 {
        let lane = hits.trailing_zeros() as usize;
        hits &= hits - 1;
        let choice = tape[lane * random_count + event] as usize;
        debug_assert!((1..=choices).contains(&choice));
        result[choice - 1] |= 1u64 << lane;
    }
    result
}

fn resize_packet(buffer: &mut Vec<u64>, len: usize) -> Result<(), String> {
    if buffer.capacity() < len {
        buffer
            .try_reserve_exact(len - buffer.len())
            .map_err(|e| format!("compiled packet allocation failed: {e}"))?;
    }
    // Every used frame/record cell is cleared by the packet and every tape
    // event is filled by its row generator. Retained cells need no second fill.
    buffer.resize(len, 0);
    Ok(())
}
impl CompiledNearCliffordSampler<'_> {
    /// Conservative admitted coefficient-cache bytes, excluding the static plan
    /// and arithmetic work buffers. This is not process memory usage.
    pub fn coefficient_cache_reserved_bytes(&self) -> usize {
        self.cache.as_ref().map_or(0, |cache| cache.reserved)
    }
    fn load_state(&mut self, id: usize) -> Result<(), String> {
        let state = self
            .cache
            .as_ref()
            .expect("admitted state belongs to a cache")
            .states[id]
            .coefficients
            .clone();
        self.coefficients.clear();
        self.coefficients
            .try_reserve_exact(state.len())
            .map_err(|e| format!("compiled coefficient allocation failed: {e}"))?;
        self.coefficients.extend_from_slice(&state);
        Ok(())
    }
    fn cached_rotate(
        &mut self,
        node: usize,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        state: &mut Option<usize>,
    ) -> Result<(), String> {
        let sign = p.physical.anticommutes(&self.x, &self.z);
        self.cached_rotate_signed(node, p, expand, dagger, sign, state)
    }
    fn cached_rotate_signed(
        &mut self,
        node: usize,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        sign: bool,
        state: &mut Option<usize>,
    ) -> Result<(), String> {
        // This operator is a common phase on the coherent base, even when F flips it.
        if p.x == 0 && p.z == 0 {
            return Ok(());
        }
        if let (Some(id), Some(cache)) = (*state, self.cache.as_ref()) {
            if let Some(CachedOp::Rotate(next)) = cache.entry(id, node) {
                if let Some(next) = next[usize::from(sign)] {
                    *state = Some(next);
                    return Ok(());
                }
            }
        }
        let input = *state;
        if let Some(id) = input {
            self.load_state(id)?;
        }
        self.rotate_signed(p, expand, dagger, sign)?;
        *state = None;
        if let (Some(id), Some(cache)) = (input, self.cache.as_mut()) {
            if let Some(entry) = cache.entry(id, node) {
                let mut children = match entry {
                    CachedOp::Rotate(next) => next,
                    CachedOp::None => [None; 2],
                    _ => return Ok(()),
                };
                if let Some(next) = cache.store(&self.coefficients) {
                    children[usize::from(sign)] = Some(next);
                    if cache.set_entry(id, node, CachedOp::Rotate(children)) {
                        *state = Some(next);
                    }
                }
            }
        }
        Ok(())
    }
    fn cached_probability_zero(
        &mut self,
        node: usize,
        p: &CompactPauli,
        state: Option<usize>,
    ) -> Result<f64, String> {
        if let (Some(id), Some(cache)) = (state, self.cache.as_ref()) {
            if let Some(CachedOp::Measure(entry)) = cache.entry(id, node) {
                return Ok(entry.probability_zero);
            }
        }
        if let Some(id) = state {
            self.load_state(id)?;
        }
        let probability = self.probability_zero(p);
        if let (Some(id), Some(cache)) = (state, self.cache.as_mut()) {
            cache.set_entry(
                id,
                node,
                CachedOp::Measure(CachedMeasurement {
                    probability_zero: probability,
                    next: [None; 2],
                }),
            );
        }
        Ok(probability)
    }
    fn cached_project(
        &mut self,
        node: usize,
        p: &CompactPauli,
        index: usize,
        y: bool,
        fixed: bool,
        branch: bool,
        state: &mut Option<usize>,
    ) -> Result<(), String> {
        if let (Some(id), Some(cache)) = (*state, self.cache.as_ref()) {
            if let Some(CachedOp::Measure(entry)) = cache.entry(id, node) {
                if let Some(next) = entry.next[usize::from(branch)] {
                    *state = Some(next);
                    return Ok(());
                }
            }
        }
        let input = *state;
        if let Some(id) = input {
            self.load_state(id)?;
        }
        self.project(p, index, y, fixed)?;
        *state = None;
        if let (Some(id), Some(cache)) = (input, self.cache.as_mut()) {
            if let Some(CachedOp::Measure(mut entry)) = cache.entry(id, node) {
                if let Some(next) = cache.store(&self.coefficients) {
                    entry.next[usize::from(branch)] = Some(next);
                    if cache.set_entry(id, node, CachedOp::Measure(entry)) {
                        *state = Some(next);
                    }
                }
            } else if self.coefficients.len() == 1 {
                *state = Some(0);
            }
        } else if self.coefficients.len() == 1 && self.cache.is_some() {
            *state = Some(0);
        }
        Ok(())
    }

    fn rotate(&mut self, p: &CompactPauli, expand: bool, dagger: bool) -> Result<(), String> {
        let flip = p.physical.anticommutes(&self.x, &self.z);
        self.rotate_signed(p, expand, dagger, flip)
    }
    fn rotate_signed(
        &mut self,
        p: &CompactPauli,
        expand: bool,
        dagger: bool,
        flip: bool,
    ) -> Result<(), String> {
        if p.x == 0 && p.z == 0 {
            return Ok(());
        }
        if expand {
            let len = self
                .coefficients
                .len()
                .checked_mul(2)
                .ok_or("compiled coefficient size overflow")?;
            self.coefficients
                .try_reserve_exact(len - self.coefficients.len())
                .map_err(|e| format!("compiled coefficient allocation failed: {e}"))?;
            self.coefficients.resize(len, ComplexAmp::default());
        }
        let c = (std::f64::consts::PI / 8.).cos();
        let s = (std::f64::consts::PI / 8.).sin();
        let imaginary = p.physical.phase % 2 == 0;
        let mut factor = if matches!(p.physical.phase, 0 | 3) {
            -s
        } else {
            s
        };
        if dagger ^ flip {
            factor = -factor;
        }
        let term = |index: usize, amp: ComplexAmp| {
            let factor = if (index & p.z).count_ones() % 2 != 0 {
                -factor
            } else {
                factor
            };
            if imaginary {
                ComplexAmp::new(-amp.im * factor, amp.re * factor)
            } else {
                amp * factor
            }
        };
        if p.x == 0 {
            for (i, amp) in self.coefficients.iter_mut().enumerate() {
                *amp = *amp * c + term(i, *amp);
            }
        } else {
            let pivot = 1 << p.x.trailing_zeros();
            for block in (0..self.coefficients.len()).step_by(pivot * 2) {
                let other = block ^ p.x;
                // The chosen pivot bit is zero in block and one in other, so
                // these equally sized ranges are disjoint. Borrow both once;
                // the iterator removes repeated bounds checks inside the kernel.
                let (a, b) = if block < other {
                    let (left, right) = self.coefficients.split_at_mut(other);
                    (&mut left[block..block + pivot], &mut right[..pivot])
                } else {
                    let (left, right) = self.coefficients.split_at_mut(block);
                    (&mut right[..pivot], &mut left[other..other + pivot])
                };
                for (offset, (a, b)) in a.iter_mut().zip(b).enumerate() {
                    let old_a = *a;
                    let old_b = *b;
                    *a = old_a * c + term(other + offset, old_b);
                    *b = old_b * c + term(block + offset, old_a);
                }
            }
        }
        Ok(())
    }
    fn probability_zero(&self, p: &CompactPauli) -> f64 {
        let mut expectation = 0.;
        let mut norm = 0.;
        for (i, &amp) in self.coefficients.iter().enumerate() {
            let sign = if (i & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            expectation +=
                (self.coefficients[i ^ p.x].conj() * i_pow(p.physical.phase) * (amp * sign)).re;
            norm += amp.norm_sqr();
        }
        ((1. + expectation / norm) * 0.5).clamp(0., 1.)
    }
    fn project(
        &mut self,
        p: &CompactPauli,
        index: usize,
        y: bool,
        fixed: bool,
    ) -> Result<(), String> {
        let pivot = 1 << index;
        let low = pivot - 1;
        let other = p.z & !pivot;
        self.reduced_coefficients.clear();
        let reduced = &mut self.reduced_coefficients;
        reduced
            .try_reserve_exact(self.coefficients.len() / 2)
            .map_err(|e| format!("compiled projection allocation failed: {e}"))?;
        let mut norm = 0.;
        for i in 0..self.coefficients.len() / 2 {
            let without = (i & low) | ((i & !low) << 1);
            let value = if p.x == 0 {
                let bit = fixed ^ ((without & other).count_ones() % 2 != 0);
                self.coefficients[without | ((bit as usize) << index)]
            } else {
                let a = self.coefficients[without];
                let mut b = self.coefficients[without ^ p.x];
                if y {
                    b = ComplexAmp::new(b.im, -b.re);
                }
                let sign = if fixed ^ ((without & other).count_ones() % 2 != 0) {
                    -1.
                } else {
                    1.
                };
                (a + b * sign) * std::f64::consts::FRAC_1_SQRT_2
            };
            norm += value.norm_sqr();
            reduced.push(value);
        }
        if !norm.is_finite() || norm <= 0. {
            return Err("compiled near-Clifford zero-probability measurement".into());
        }
        let scale = 1. / norm.sqrt();
        for amp in reduced {
            *amp = *amp * scale;
        }
        std::mem::swap(&mut self.coefficients, &mut self.reduced_coefficients);
        // A one-dimensional state has no observable relative phase.
        if self.coefficients.len() == 1 {
            self.coefficients[0] = ComplexAmp::new(1., 0.);
        }
        Ok(())
    }
    fn row(&mut self, sweep: &[bool], rng: &mut impl Rng) -> Result<NearCliffordShot, String> {
        self.row_with_random(sweep, &mut RowRandom::live(rng))
    }
    fn row_with_random(
        &mut self,
        sweep: &[bool],
        random: &mut RowRandom<'_, impl Rng>,
    ) -> Result<NearCliffordShot, String> {
        self.x.fill(0);
        self.z.fill(0);
        let mut state = self.cache.as_ref().map(|cache| cache.start);
        if state.is_none() {
            self.coefficients.clear();
            self.coefficients
                .try_reserve_exact(self.plan.initial_coefficients.len())
                .map_err(|e| format!("compiled sampler allocation failed: {e}"))?;
            self.coefficients
                .extend_from_slice(&self.plan.initial_coefficients);
        }
        let mut shot = NearCliffordShot {
            measurements: vec![false; self.plan.measurement_count],
            detectors: Vec::new(),
            observables: Vec::new(),
        };
        let plan = self.plan;
        for (node, op) in plan.operations.iter().enumerate().skip(plan.prefix_len) {
            match op {
                PlanOp::Basis(gates) => {
                    for gate in gates {
                        gate.conjugate(&mut self.x, &mut self.z);
                    }
                }
                PlanOp::Rotate {
                    pauli,
                    expand,
                    dagger,
                } => self.cached_rotate(node, pauli, *expand, *dagger, &mut state)?,
                PlanOp::Noise {
                    probability,
                    choices,
                } => {
                    let value = random.draw(RandomKind::Noise {
                        probability: *probability,
                        choices: choices.len(),
                    });
                    if value != 0 {
                        let choice = &choices[value as usize - 1];
                        choice.pauli.apply(&mut self.x, &mut self.z);
                        for index in &choice.record_flips {
                            shot.measurements[*index] ^= true;
                        }
                    }
                }
                PlanOp::Feedback { condition, pauli } => {
                    let bit = match condition {
                        Condition::Record(index) => shot.measurements[*index],
                        Condition::Sweep(k) => sweep.get(*k as usize).copied().unwrap_or(false),
                    };
                    if bit {
                        pauli.apply(&mut self.x, &mut self.z);
                    }
                }
                PlanOp::Annotation {
                    offsets,
                    observable,
                } => {
                    let mut bit = false;
                    for offset in offsets {
                        bit ^= shot.measurements[*offset];
                    }
                    match observable {
                        Some(k) => shot.observables.push((*k, bit)),
                        None => shot.detectors.push(bit),
                    }
                }
                PlanOp::Measure(m) => {
                    let anti = m.pauli.physical.anticommutes(&self.x, &self.z);
                    let branch = match m.projection {
                        Projection::Constant(bit) => bit,
                        Projection::Independent { .. } => random.draw(RandomKind::Independent) != 0,
                        Projection::Active { .. } => {
                            f64::from_bits(random.draw(RandomKind::Active))
                                >= self.cached_probability_zero(node, &m.pauli, state)?
                        }
                    };
                    let physical = branch ^ anti;
                    if let Projection::Active {
                        index, y, offset, ..
                    } = m.projection
                    {
                        self.cached_project(
                            node,
                            &m.pauli,
                            index,
                            y,
                            branch ^ offset,
                            branch,
                            &mut state,
                        )?;
                    }
                    for gate in &m.basis {
                        gate.conjugate(&mut self.x, &mut self.z);
                    }
                    match m.projection {
                        Projection::Constant(_) => {}
                        Projection::Independent { pivot } => {
                            if branch {
                                flip(&mut self.x, pivot);
                            }
                        }
                        Projection::Active { pivot, offset, .. } => {
                            if branch ^ offset {
                                flip(&mut self.x, pivot);
                            }
                        }
                    }
                    if let Some(correction) = &m.reset {
                        if physical {
                            correction.apply(&mut self.x, &mut self.z);
                        }
                    }
                    if let Some(index) = m.record {
                        let error = m.readout > 0.
                            && random.draw(RandomKind::Noise {
                                probability: m.readout,
                                choices: 1,
                            }) != 0;
                        shot.measurements[index] = physical ^ m.inverted ^ error;
                    }
                }
            }
        }
        Ok(shot)
    }
    // All random draws depend only on the fixed plan, not the coherent state.
    // Draw them in original row order, so packet size and fallback never change RNG.
    fn packet(
        &mut self,
        lanes: usize,
        sweep: &[bool],
        rng: &mut impl Rng,
        output: &mut Vec<u8>,
        coherent: bool,
    ) -> Result<(), String> {
        let mut x = std::mem::take(&mut self.packet_x);
        let mut z = std::mem::take(&mut self.packet_z);
        let mut tape = std::mem::take(&mut self.packet_tape);
        let mut records = std::mem::take(&mut self.packet_records);
        let mut noise_masks = std::mem::take(&mut self.packet_noise_masks);
        let result = (|| {
            let plan = self.plan;
            let random_count = plan.random_kinds.len();
            if coherent {
                self.coherent
                    .reset(&plan.initial_coefficients, lanes, plan.peak_active_rank)?;
            }
            noise_masks.fill(0);
            for lane in 0..lanes {
                let mut random = RowRandom::live(&mut *rng);
                let row = &mut tape[lane * random_count..(lane + 1) * random_count];
                if let Some(runs) = &plan.random_runs {
                    runs.fill_row_with_noise_masks(
                        &mut random,
                        &plan.random_kinds,
                        row,
                        &mut noise_masks,
                        1u64 << lane,
                    );
                } else {
                    fill_original_row_with_noise_masks(
                        &mut random,
                        &plan.random_kinds,
                        row,
                        &mut noise_masks,
                        1u64 << lane,
                    );
                }
            }
            x.fill(0);
            z.fill(0);
            records.fill(0);
            let start = self.cache.as_ref().map_or(0, |cache| cache.start);
            let mut states = [Some(start); 64];
            let all = if lanes == 64 {
                u64::MAX
            } else {
                (1u64 << lanes) - 1
            };
            let mut live = all;
            let mut event = 0;
            let mut noise_event = 0;
            let mut record = 0;
            for (node, op) in plan.operations.iter().enumerate().skip(plan.prefix_len) {
                match op {
                    PlanOp::Basis(gates) => {
                        for gate in gates {
                            gate.packet_conjugate(&mut x, &mut z);
                        }
                    }
                    PlanOp::Rotate {
                        pauli,
                        expand,
                        dagger,
                    } => {
                        let anti = pauli.physical.packet_anti(&x, &z);
                        if coherent {
                            self.coherent.rotate(pauli, *expand, *dagger, anti)?;
                        } else if let Some(id) = uniform_packet_state(&states, live) {
                            let masks = [live & !anti, live & anti];
                            for (sign, mask) in masks.into_iter().enumerate() {
                                if mask == 0 {
                                    continue;
                                }
                                let mut next = Some(id);
                                self.cached_rotate_signed(
                                    node,
                                    pauli,
                                    *expand,
                                    *dagger,
                                    sign != 0,
                                    &mut next,
                                )?;
                                broadcast_packet_state(&mut states, mask, next);
                                if next.is_none() {
                                    live &= !mask;
                                }
                            }
                        } else {
                            let mut remaining = live;
                            while remaining != 0 {
                                let lane = remaining.trailing_zeros() as usize;
                                remaining &= remaining - 1;
                                self.cached_rotate_signed(
                                    node,
                                    pauli,
                                    *expand,
                                    *dagger,
                                    anti >> lane & 1 != 0,
                                    &mut states[lane],
                                )?;
                                if states[lane].is_none() {
                                    live &= !(1 << lane);
                                }
                            }
                        }
                    }
                    PlanOp::Noise { choices, .. } => {
                        // No-hit channels have no effect on any frame or record.
                        // Typed draws remain in the complete tape for replay.
                        let selected = noise_masks[noise_event];
                        noise_event += 1;
                        event += 1;
                        if selected == 0 {
                            continue;
                        }
                        if choices.len() == 1 {
                            let choice = &choices[0];
                            choice.pauli.packet_apply(selected, &mut x, &mut z);
                            for index in &choice.record_flips {
                                records[*index] ^= selected;
                            }
                        } else {
                            let masks = packet_choice_masks(
                                selected,
                                choices.len(),
                                &tape,
                                event - 1,
                                random_count,
                            );
                            for (choice, mask) in choices.iter().zip(masks) {
                                if mask != 0 {
                                    choice.pauli.packet_apply(mask, &mut x, &mut z);
                                    for index in &choice.record_flips {
                                        records[*index] ^= mask;
                                    }
                                }
                            }
                        }
                    }
                    PlanOp::Feedback { condition, pauli } => {
                        let mask = match condition {
                            Condition::Record(index) => records[*index],
                            Condition::Sweep(k) => {
                                if sweep.get(*k as usize).copied().unwrap_or(false) {
                                    all
                                } else {
                                    0
                                }
                            }
                        };
                        pauli.packet_apply(mask, &mut x, &mut z);
                    }
                    PlanOp::Annotation { .. } => {} // Flat output has no annotation fields.
                    PlanOp::Measure(m) => {
                        let anti = m.pauli.physical.packet_anti(&x, &z);
                        let mut branch = 0;
                        match m.projection {
                            Projection::Constant(bit) => {
                                if bit {
                                    branch = all;
                                }
                            }
                            Projection::Independent { .. } => {
                                for lane in 0..lanes {
                                    if tape[lane * random_count + event] != 0 {
                                        branch |= 1 << lane;
                                    }
                                }
                                event += 1;
                            }
                            Projection::Active {
                                index, y, offset, ..
                            } => {
                                if coherent {
                                    let probabilities = self.coherent.probability_zero(&m.pauli);
                                    for lane in 0..lanes {
                                        if f64::from_bits(tape[lane * random_count + event])
                                            >= probabilities[lane]
                                        {
                                            branch |= 1 << lane;
                                        }
                                    }
                                    self.coherent.project(
                                        &m.pauli,
                                        index,
                                        y,
                                        if offset { branch ^ all } else { branch },
                                    )?;
                                } else if let Some(id) = uniform_packet_state(&states, live) {
                                    let probability =
                                        self.cached_probability_zero(node, &m.pauli, Some(id))?;
                                    let mut remaining = live;
                                    while remaining != 0 {
                                        let lane = remaining.trailing_zeros() as usize;
                                        remaining &= remaining - 1;
                                        if f64::from_bits(tape[lane * random_count + event])
                                            >= probability
                                        {
                                            branch |= 1 << lane;
                                        }
                                    }
                                    let masks = [live & !branch, live & branch];
                                    for (bit, mask) in masks.into_iter().enumerate() {
                                        if mask == 0 {
                                            continue;
                                        }
                                        let mut next = Some(id);
                                        self.cached_project(
                                            node,
                                            &m.pauli,
                                            index,
                                            y,
                                            (bit != 0) ^ offset,
                                            bit != 0,
                                            &mut next,
                                        )?;
                                        broadcast_packet_state(&mut states, mask, next);
                                        if next.is_none() {
                                            live &= !mask;
                                        }
                                    }
                                } else {
                                    let mut remaining = live;
                                    while remaining != 0 {
                                        let lane = remaining.trailing_zeros() as usize;
                                        remaining &= remaining - 1;
                                        let bit = f64::from_bits(tape[lane * random_count + event])
                                            >= self.cached_probability_zero(
                                                node,
                                                &m.pauli,
                                                states[lane],
                                            )?;
                                        if bit {
                                            branch |= 1 << lane;
                                        }
                                        self.cached_project(
                                            node,
                                            &m.pauli,
                                            index,
                                            y,
                                            bit ^ offset,
                                            bit,
                                            &mut states[lane],
                                        )?;
                                        if states[lane].is_none() {
                                            live &= !(1 << lane);
                                        }
                                    }
                                }
                                event += 1;
                            }
                        }
                        let physical = branch ^ anti;
                        for gate in &m.basis {
                            gate.packet_conjugate(&mut x, &mut z);
                        }
                        match m.projection {
                            Projection::Constant(_) => {}
                            Projection::Independent { pivot } => x[pivot] ^= branch,
                            Projection::Active { pivot, offset, .. } => {
                                x[pivot] ^= if offset { branch ^ all } else { branch }
                            }
                        }
                        if let Some(correction) = &m.reset {
                            correction.packet_apply(physical, &mut x, &mut z);
                        }
                        if let Some(index) = m.record {
                            let mut reported = if m.inverted { physical ^ all } else { physical };
                            if m.readout > 0. {
                                reported ^= noise_masks[noise_event];
                                noise_event += 1;
                                event += 1;
                            }
                            records[index] = reported;
                            record += 1;
                        }
                    }
                }
            }
            debug_assert_eq!(event, random_count);
            debug_assert_eq!(noise_event, plan.noise_event_count);
            debug_assert_eq!(record, plan.measurement_count);
            // When almost every row replays, scalar arithmetic avoids duplicated work.
            // This affects only execution strategy, never random-event or record order.
            #[cfg(test)]
            {
                self.last_packet_live = (live & all).count_ones() as usize;
            }
            if (live & all).count_ones() as usize <= lanes / 4 {
                self.pack_enabled = false;
            }
            for lane in 0..lanes {
                if live >> lane & 1 != 0 {
                    output.extend(records.iter().map(|bits| ((bits >> lane) & 1) as u8));
                } else {
                    // Admission failed. Replay the same row's already drawn events;
                    // no quantum result is resampled and the caller RNG is untouched.
                    let mut random = RowRandom::recorded(
                        &tape[lane * random_count..(lane + 1) * random_count],
                        &mut *rng,
                    );
                    output.extend(
                        self.row_with_random(sweep, &mut random)?
                            .measurements
                            .into_iter()
                            .map(u8::from),
                    );
                    debug_assert_eq!(random.cursor, random_count);
                }
            }
            Ok(())
        })();
        self.packet_x = x;
        self.packet_z = z;
        self.packet_tape = tape;
        self.packet_records = records;
        self.packet_noise_masks = noise_masks;
        result
    }
    pub fn sample(
        &mut self,
        shots: usize,
        rng: &mut impl Rng,
    ) -> Result<Vec<NearCliffordShot>, String> {
        self.sample_with_sweep(shots, &[], rng)
    }
    pub fn sample_with_sweep(
        &mut self,
        shots: usize,
        sweep: &[bool],
        rng: &mut impl Rng,
    ) -> Result<Vec<NearCliffordShot>, String> {
        let mut output = Vec::new();
        output.try_reserve_exact(shots).map_err(|e| e.to_string())?;
        for _ in 0..shots {
            output.push(self.row(sweep, rng)?);
        }
        Ok(output)
    }
    pub fn sample_measurements_u8(
        &mut self,
        shots: usize,
        rng: &mut impl Rng,
    ) -> Result<NearCliffordMeasurementBatch, String> {
        self.sample_measurements_u8_with_sweep(shots, &[], rng)
    }
    pub fn sample_measurements_u8_with_sweep(
        &mut self,
        shots: usize,
        sweep: &[bool],
        rng: &mut impl Rng,
    ) -> Result<NearCliffordMeasurementBatch, String> {
        let len = shots
            .checked_mul(self.plan.measurement_count)
            .ok_or("compiled measurement batch size overflow")?;
        let mut measurements = Vec::new();
        measurements
            .try_reserve_exact(len)
            .map_err(|e| e.to_string())?;
        let packet_bytes = self
            .plan
            .random_kinds
            .len()
            .checked_mul(64 * size_of::<u64>())
            .and_then(|n| {
                self.plan
                    .noise_event_count
                    .checked_mul(size_of::<u64>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                n.checked_add(
                    (2 * self.plan.num_qubits + self.plan.measurement_count) * size_of::<u64>(),
                )
            });
        let coherent_eligible = self.plan.peak_active_rank >= 4
            && (1usize << self.plan.peak_active_rank)
                .checked_mul(64 * 4 * size_of::<f64>())
                .is_some_and(|n| n <= COEFFICIENT_BYTE_BUDGET);
        let packed = shots >= 32
            && packet_bytes.is_some_and(|n| n <= PACKET_BYTE_BUDGET)
            && (coherent_eligible || (self.pack_enabled && self.cache.is_some()));
        if packed {
            resize_packet(&mut self.packet_x, self.plan.num_qubits)?;
            resize_packet(&mut self.packet_z, self.plan.num_qubits)?;
            resize_packet(&mut self.packet_records, self.plan.measurement_count)?;
            resize_packet(&mut self.packet_noise_masks, self.plan.noise_event_count)?;
            resize_packet(&mut self.packet_tape, 64 * self.plan.random_kinds.len())?;
            for offset in (0..shots).step_by(64) {
                let lanes = (shots - offset).min(64);
                let coherent = coherent_eligible && (!self.pack_enabled || self.cache.is_none());
                if coherent || (self.pack_enabled && self.cache.is_some()) {
                    self.packet(lanes, sweep, rng, &mut measurements, coherent)?;
                } else {
                    for _ in 0..lanes {
                        measurements
                            .extend(self.row(sweep, rng)?.measurements.into_iter().map(u8::from));
                    }
                }
            }
        } else {
            for _ in 0..shots {
                measurements.extend(self.row(sweep, rng)?.measurements.into_iter().map(u8::from));
            }
        }
        Ok(NearCliffordMeasurementBatch {
            shots,
            measurements_per_shot: self.plan.measurement_count,
            measurements,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn single(v: &mut [ComplexAmp], q: usize, m: [[ComplexAmp; 2]; 2]) {
        let bit = 1 << q;
        for i in 0..v.len() {
            if i & bit == 0 {
                let j = i | bit;
                let a = v[i];
                let b = v[j];
                v[i] = m[0][0] * a + m[0][1] * b;
                v[j] = m[1][0] * a + m[1][1] * b;
            }
        }
    }
    fn dense_gate(v: &mut [ComplexAmp], gate: CliffordGate) {
        let z = ComplexAmp::default();
        let one = ComplexAmp::new(1., 0.);
        let i = ComplexAmp::new(0., 1.);
        match gate {
            CliffordGate::H(q) => {
                let s = one * std::f64::consts::FRAC_1_SQRT_2;
                single(v, q, [[s, s], [s, s * (-1.)]]);
            }
            CliffordGate::S(q) => single(v, q, [[one, z], [z, i]]),
            CliffordGate::SDag(q) => single(v, q, [[one, z], [z, i * (-1.)]]),
            CliffordGate::X(q) => single(v, q, [[z, one], [one, z]]),
            CliffordGate::Y(q) => single(v, q, [[z, i * (-1.)], [i, z]]),
            CliffordGate::Z(q) => single(v, q, [[one, z], [z, one * (-1.)]]),
            CliffordGate::CX(a, b) => {
                for k in 0..v.len() {
                    if k & (1 << a) != 0 && k & (1 << b) == 0 {
                        v.swap(k, k ^ (1 << b));
                    }
                }
            }
            CliffordGate::CZ(a, b) => {
                for (k, amp) in v.iter_mut().enumerate() {
                    if k & (1 << a) != 0 && k & (1 << b) != 0 {
                        *amp = *amp * (-1.);
                    }
                }
            }
            CliffordGate::Swap(a, b) => {
                for k in 0..v.len() {
                    if k & (1 << a) == 0 && k & (1 << b) != 0 {
                        v.swap(k, k ^ (1 << a) ^ (1 << b));
                    }
                }
            }
        }
    }
    fn dense_basis(v: &mut [ComplexAmp], gate: BasisGate) {
        dense_gate(
            v,
            match gate {
                BasisGate::H(q) => CliffordGate::H(q),
                BasisGate::S(q) => CliffordGate::S(q),
                BasisGate::CX(a, b) => CliffordGate::CX(a, b),
                BasisGate::CZ(a, b) => CliffordGate::CZ(a, b),
            },
        );
    }
    fn dense_frame(v: &mut [ComplexAmp], x: u64, z: u64) {
        for q in 0..3 {
            if z >> q & 1 != 0 {
                dense_gate(v, CliffordGate::Z(q));
            }
        }
        for q in 0..3 {
            if x >> q & 1 != 0 {
                dense_gate(v, CliffordGate::X(q));
            }
        }
    }
    fn embed(
        coefficients: &[ComplexAmp],
        axes: &[usize],
        fixed: Option<(usize, bool)>,
    ) -> Vec<ComplexAmp> {
        let mut v = vec![ComplexAmp::default(); 8];
        for (i, &amp) in coefficients.iter().enumerate() {
            let mut index = axes
                .iter()
                .enumerate()
                .fold(0, |bits, (j, &q)| bits | (((i >> j) & 1) << q));
            if let Some((q, true)) = fixed {
                index |= 1 << q;
            }
            v[index] = amp;
        }
        v
    }
    fn normalize(v: &mut [ComplexAmp]) -> f64 {
        let norm = v.iter().map(|a| a.norm_sqr()).sum::<f64>();
        if norm > 0. {
            for amp in v {
                *amp = *amp * (1. / norm.sqrt());
            }
        }
        norm
    }

    #[test]
    fn compiled_forced_projectors_preserve_full_conditional_complex_density() {
        let mut rng = StdRng::seed_from_u64(20261007);
        let mut flags = [false; 4];
        for _ in 0..64 {
            let gates = (0..12)
                .map(|_| {
                    let a = rng.gen_range(0..3);
                    let b = (a + rng.gen_range(1..3)) % 3;
                    match rng.gen_range(0..9) {
                        0 => CliffordGate::H(a),
                        1 => CliffordGate::S(a),
                        2 => CliffordGate::SDag(a),
                        3 => CliffordGate::X(a),
                        4 => CliffordGate::Y(a),
                        5 => CliffordGate::Z(a),
                        6 => CliffordGate::CX(a, b),
                        7 => CliffordGate::CZ(a, b),
                        _ => CliffordGate::Swap(a, b),
                    }
                })
                .collect::<Vec<_>>();
            for axes in [vec![0, 1, 2], vec![0, 2]] {
                let mut coeff = (0..1 << axes.len())
                    .map(|_| ComplexAmp::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)))
                    .collect::<Vec<_>>();
                normalize(&mut coeff);
                for q in 0..3 {
                    for basis in [
                        MeasurementBasis::X,
                        MeasurementBasis::Y,
                        MeasurementBasis::Z,
                    ] {
                        let mut planner = Planner {
                            state: CompileFrame::identity(3).unwrap(),
                            axes: axes.clone(),
                            limit: 3,
                            peak: axes.len(),
                            operations: Vec::new(),
                            expanded: 0,
                            reserved_bytes: 0,
                            tape: None,
                            record_count: 0,
                        };
                        for gate in &gates {
                            planner.state.apply_clifford(*gate).unwrap();
                        }
                        planner.measure(q, basis, true, false, false).unwrap();
                        let PlanOp::Measure(m) = &planner.operations[0] else {
                            panic!("measurement plan");
                        };
                        flags[0] |= matches!(m.projection, Projection::Independent { .. });
                        flags[1] |= matches!(m.projection, Projection::Active { y: true, .. });
                        flags[2] |= m.pauli.x.count_ones() > 1;
                        flags[3] |= matches!(m.projection, Projection::Active { offset: true, .. });
                        let frame_x = rng.gen_range(0u64..8);
                        let frame_z = rng.gen_range(0u64..8);
                        let mut before = embed(&coeff, &axes, None);
                        dense_frame(&mut before, frame_x, frame_z);
                        for gate in &gates {
                            dense_gate(&mut before, *gate);
                        }
                        for branch in [false, true] {
                            let mut image = before.clone();
                            dense_gate(
                                &mut image,
                                match basis {
                                    MeasurementBasis::X => CliffordGate::X(q),
                                    MeasurementBasis::Y => CliffordGate::Y(q),
                                    MeasurementBasis::Z => CliffordGate::Z(q),
                                },
                            );
                            let mut expected = before
                                .iter()
                                .zip(&image)
                                .map(|(&a, &b)| (a + b * if branch { -1. } else { 1. }) * 0.5)
                                .collect::<Vec<_>>();
                            if normalize(&mut expected) < 1e-24 {
                                continue;
                            }
                            let plan = CompiledNearCliffordExecutor {
                                operations: Vec::new(),
                                num_qubits: 3,
                                measurement_count: 1,
                                peak_active_rank: axes.len(),
                                prefix_len: 0,
                                initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
                                random_kinds: Vec::new(),
                                random_runs: None,
                                noise_event_count: 0,
                            };
                            let mut sampler = plan.prepare_sampler().unwrap();
                            sampler.coefficients = coeff.clone();
                            sampler.x[0] = frame_x;
                            sampler.z[0] = frame_z;
                            let base_branch =
                                branch ^ m.pauli.physical.anticommutes(&sampler.x, &sampler.z);
                            let fixed = match m.projection {
                                Projection::Constant(bit) => {
                                    assert_eq!(bit, base_branch);
                                    None
                                }
                                Projection::Independent { pivot } => Some((pivot, base_branch)),
                                Projection::Active {
                                    pivot,
                                    index,
                                    y,
                                    offset,
                                } => {
                                    sampler
                                        .project(&m.pauli, index, y, base_branch ^ offset)
                                        .unwrap();
                                    Some((pivot, base_branch ^ offset))
                                }
                            };
                            for gate in &m.basis {
                                gate.conjugate(&mut sampler.x, &mut sampler.z);
                            }
                            if let Some((pivot, true)) = fixed {
                                flip(&mut sampler.x, pivot);
                            }
                            let mut actual = embed(&sampler.coefficients, &planner.axes, None);
                            dense_frame(&mut actual, sampler.x[0], sampler.z[0]);
                            // U'=UC: C's right-composition list acts in reverse order.
                            for gate in m.basis.iter().rev() {
                                dense_basis(&mut actual, *gate);
                            }
                            for gate in &gates {
                                dense_gate(&mut actual, *gate);
                            }
                            for i in 0..8 {
                                for j in 0..8 {
                                    let a = actual[i] * actual[j].conj();
                                    let e = expected[i] * expected[j].conj();
                                    assert!(
                                        (a.re - e.re).abs() < 3e-12 && (a.im - e.im).abs() < 3e-12,
                                        "q={q}, basis={basis:?}, branch={branch}, projection={:?}",
                                        m.projection
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            flags, [true; 4],
            "must exercise independent, Y, multi-X and negative-offset cases"
        );
    }

    #[test]
    fn compiled_pauli_rotation_kernel_matches_dense_complex_density_and_expansion() {
        let plan = CompiledNearCliffordExecutor::compile_text("I 2\n").unwrap();
        let mut rng = StdRng::seed_from_u64(748);
        for expand in [false, true] {
            for _ in 0..128 {
                let x = if expand {
                    rng.gen_range(0usize..4) | 4
                } else {
                    rng.gen_range(0usize..8)
                };
                let z = rng.gen_range(0usize..8);
                let negative = rng.r#gen::<bool>();
                let dagger = rng.r#gen::<bool>();
                let frame_x = rng.gen_range(0u64..8);
                let frame_z = rng.gen_range(0u64..8);
                let mut pauli = Pauli::identity(3);
                for q in 0..3 {
                    pauli.x[q] = x >> q & 1 != 0;
                    pauli.z[q] = z >> q & 1 != 0;
                }
                pauli.phase = ((x & z).count_ones() as u8 + 2 * u8::from(negative)) % 4;
                let p = CompactPauli {
                    physical: PackedPauli::new(&pauli),
                    x,
                    z,
                };
                let mut coefficients = (0..if expand { 4 } else { 8 })
                    .map(|_| ComplexAmp::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)))
                    .collect::<Vec<_>>();
                normalize(&mut coefficients);
                let mut before = coefficients.clone();
                before.resize(8, ComplexAmp::default());
                dense_frame(&mut before, frame_x, frame_z);
                let mut image = before.clone();
                for q in 0..3 {
                    match (pauli.x[q], pauli.z[q]) {
                        (true, true) => dense_gate(&mut image, CliffordGate::Y(q)),
                        (true, false) => dense_gate(&mut image, CliffordGate::X(q)),
                        (false, true) => dense_gate(&mut image, CliffordGate::Z(q)),
                        _ => {}
                    }
                }
                if negative {
                    for amp in &mut image {
                        *amp = *amp * -1.;
                    }
                }
                let c = (std::f64::consts::PI / 8.).cos();
                let s = (std::f64::consts::PI / 8.).sin();
                let rotation = ComplexAmp::new(0., if dagger { s } else { -s });
                let expected = before
                    .iter()
                    .zip(&image)
                    .map(|(&a, &b)| a * c + b * rotation)
                    .collect::<Vec<_>>();
                let mut sampler = plan.prepare_sampler_with_cache_budget(0).unwrap();
                sampler.coefficients = coefficients;
                sampler.x[0] = frame_x;
                sampler.z[0] = frame_z;
                sampler.rotate(&p, expand, dagger).unwrap();
                let mut actual = sampler.coefficients.clone();
                dense_frame(&mut actual, frame_x, frame_z);
                for i in 0..8 {
                    for j in 0..8 {
                        let a = actual[i] * actual[j].conj();
                        let e = expected[i] * expected[j].conj();
                        assert!((a.re - e.re).abs() < 3e-12 && (a.im - e.im).abs() < 3e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn compiled_packet_partially_replays_admission_failures_without_redrawing() {
        let plan = CompiledNearCliffordExecutor::compile_text("H 0 1\nT 0 1\nCX 0 1\nMY 0\nMX 1\n")
            .unwrap();
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        let mut packed = plan
            .prepare_sampler_with_cache_budget(initial + 288)
            .unwrap();
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut a = StdRng::seed_from_u64(583);
        let mut b = a.clone();
        for _ in 0..4 {
            let expected = scalar.sample_measurements_u8(64, &mut a).unwrap();
            let actual = packed.sample_measurements_u8(64, &mut b).unwrap();
            assert_eq!(expected, actual);
            assert!(packed.last_packet_live > 0 && packed.last_packet_live < 64);
            assert_eq!(packed.coefficient_cache_reserved_bytes(), initial + 288);
        }
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn compiled_scheduled_noise_rejects_whole_channel_before_mutating_choices() {
        let mut x = Pauli::identity(1);
        x.x[0] = true;
        let mut y = x.clone();
        y.z[0] = true;
        y.phase = 1;
        let mut z = Pauli::identity(1);
        z.z[0] = true;
        let mut tape = vec![
            TapeOp::Noise {
                probability: 1.,
                choices: vec![
                    NoiseChoice::new(PackedPauli::new(&x)),
                    NoiseChoice::new(PackedPauli::new(&y)),
                ],
            },
            TapeOp::Measure {
                pauli: PackedPauli::new(&z),
                record: Some(0),
                inverted: false,
                readout: 0.,
                reset: Some(PackedPauli::new(&x)),
            },
        ];
        let mut reserved = PLAN_BYTE_BUDGET - 2 * size_of::<usize>();
        let initial = reserved;
        assert!(schedule_measurements(&mut tape, &mut reserved).is_err());
        assert_eq!(reserved, initial);
        let TapeOp::Noise { choices, .. } = &tape[0] else {
            panic!("failed swap must retain original channel position")
        };
        for (choice, original) in choices.iter().zip([x, y]) {
            let expected = PackedPauli::new(&original);
            assert_eq!(choice.pauli.x, expected.x);
            assert_eq!(choice.pauli.z, expected.z);
            assert_eq!(choice.pauli.phase, expected.phase);
            assert!(choice.record_flips.is_empty());
        }
    }

    #[test]
    fn compiled_positive_tiny_projection_is_normalized_without_a_cutoff() {
        let plan = CompiledNearCliffordExecutor {
            operations: Vec::new(),
            num_qubits: 1,
            measurement_count: 0,
            peak_active_rank: 1,
            prefix_len: 0,
            initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            random_kinds: Vec::new(),
            random_runs: None,
            noise_event_count: 0,
        };
        let mut sampler = plan.prepare_sampler().unwrap();
        sampler.coefficients = vec![ComplexAmp::new(1., 0.), ComplexAmp::new(1e-16, 0.)];
        let p = CompactPauli {
            physical: PackedPauli {
                x: vec![0],
                z: vec![1],
                phase: 0,
            },
            x: 0,
            z: 1,
        };
        sampler.project(&p, 0, false, true).unwrap();
        assert_eq!(sampler.coefficients, vec![ComplexAmp::new(1., 0.)]);
    }
}
