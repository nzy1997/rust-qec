//! Offline Clifford-frame plan; runtime carries a virtual Pauli and compact amplitudes.
use super::*;
#[path = "compile_frame.rs"]
mod compile_frame;
use compile_frame::CompileFrame;
#[path = "coefficient_intern.rs"]
mod coefficient_intern;
use coefficient_intern::CoefficientIntern;
#[path = "coherent_packet.rs"]
mod coherent_packet;
use coherent_packet::CoherentPacket;
#[path = "random_event_runs.rs"]
mod random_event_runs;
use random_event_runs::RandomRunPlan;
#[path = "linear_counts.rs"]
mod linear_counts;
use linear_counts::LinearCountsPlan;
use std::sync::OnceLock;
#[path = "independent_packet.rs"]
mod independent_packet;
use independent_packet::IndependentPacket;
#[path = "noise_packet.rs"]
mod noise_packet;
use noise_packet::NoisePacket;
#[path = "compact_replay.rs"]
mod compact_replay;
use compact_replay::{CompactReplay, RowDraw};
#[path = "noise_spans.rs"]
mod noise_spans;
use noise_spans::NoiseSpans;
#[path = "noise_schedule.rs"]
mod noise_schedule;
use noise_schedule::NoiseSigns;
#[cfg(test)]
#[path = "../../tests/support/near_clifford_structural.rs"]
mod conditional_fixture;
#[path = "scalar_basis.rs"]
mod scalar_basis;
use scalar_basis::{ScalarBasisProgram, build_scalar_basis};
#[cfg(test)]
#[path = "lazy_uniform_reference_tests.rs"]
mod lazy_uniform_reference_tests;

const PLAN_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const COEFFICIENT_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const DEFAULT_CACHE_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const PACKET_BYTE_BUDGET: usize = 16 * 1024 * 1024;
// Conservative admission floor: noisy application scouts cover 128 or more events.
const MIN_SCALAR_PREPARED_NOISE_EVENTS: usize = 128;

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
    // Known only after producing a complete immutable row; live draws are unknown.
    noise_is_zero: bool,
    noise_probability: u64,
    // Zero is an uninitialized sentinel: every valid sparse log(1-p) is finite and negative.
    noise_log_failure: f64,
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
            noise_is_zero: false,
            noise_probability: 0,
            noise_log_failure: 0.,
            noise_skip: None,
        }
    }
    fn recorded(tape: &'a [u64], rng: &'a mut R) -> Self {
        Self {
            tape: Some(tape),
            ..Self::live(rng)
        }
    }
    fn recorded_with_zero_noise(tape: &'a [u64], rng: &'a mut R, noise_is_zero: bool) -> Self {
        Self {
            noise_is_zero,
            ..Self::recorded(tape, rng)
        }
    }
    #[inline]
    fn take_independent(&mut self, maximum: usize) -> (u64, usize) {
        debug_assert!(maximum != 0);
        if self.bits_left == 0 {
            self.bit_word = self.rng.r#gen::<u64>();
            self.bits_left = 64;
        }
        let take = maximum.min(self.bits_left as usize);
        let value = self.bit_word
            & if take == 64 {
                u64::MAX
            } else {
                (1u64 << take) - 1
            };
        self.bit_word = if take == 64 { 0 } else { self.bit_word >> take };
        self.bits_left -= take as u8;
        (value, take)
    }
    #[inline]
    fn noise_choice(&mut self, choices: usize) -> u64 {
        if choices == 1 {
            1
        } else {
            (self.rng.gen_range(0..choices) + 1) as u64
        }
    }
    // Return the next success inside this nonempty span, consuming all failures.
    // Some(0) survives an exact boundary; a final success never prefetches.
    #[inline]
    fn sparse_hit(&mut self, remaining: usize) -> Option<usize> {
        debug_assert!(remaining != 0);
        let skip = self.noise_skip.get_or_insert_with(|| {
            let u = self.rng.r#gen::<f64>();
            if self.noise_log_failure == 0. {
                self.noise_log_failure = (-f64::from_bits(self.noise_probability)).ln_1p();
            }
            ((-u).ln_1p() / self.noise_log_failure).floor() as usize
        });
        if *skip >= remaining {
            *skip -= remaining;
            None
        } else {
            let hit = *skip;
            self.noise_skip = None;
            Some(hit)
        }
    }
    // Recorded rows have already consumed every caller RNG event. Retiring a
    // rejected row only advances its tape cursor; live rows keep the literal draws.
    fn discard_remaining(&mut self, kinds: &[RandomKind]) {
        if let Some(tape) = self.tape {
            let end = self
                .cursor
                .checked_add(kinds.len())
                .expect("random tape cursor overflow");
            assert!(end <= tape.len(), "recorded random tape is incomplete");
            self.cursor = end;
        } else {
            for &kind in kinds {
                self.draw(kind);
            }
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
                RandomKind::Independent => self.take_independent(1).0,
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
                        self.noise_log_failure = 0.;
                    }
                    if self.sparse_hit(1).is_some() {
                        self.noise_choice(choices)
                    } else {
                        0
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
    // Scalar state 0 is shared across positions. Optional interning only shares
    // exact coefficient bits produced at the same coherent node.
    nodes: Vec<CachedOp>,
    states: Vec<CachedState>,
    intern: Option<CoefficientIntern>,
    intern_attempted: bool,
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
            intern: None,
            intern_attempted: false,
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
    fn store(&mut self, node: usize, coefficients: &[ComplexAmp]) -> Option<usize> {
        if coefficients.len() == 1 {
            return Some(0);
        }
        let charge = Self::state_charge(coefficients)?;
        // Once admission closes, avoid hashing a coefficient vector on every
        // scalar replay. Interning is optional construction work, not a lookup
        // route after the bounded cache has filled.
        if !self.fits(charge) {
            return None;
        }
        let internable = (coefficient_intern::MIN_COEFFICIENTS
            ..=coefficient_intern::MAX_COEFFICIENTS)
            .contains(&coefficients.len());
        // Defer optional index allocation until enough non-scalar states have
        // accumulated to amortize it. Small and cold caches retain the original
        // admission path; existing unindexed states remain valid cache entries.
        if !self.intern_attempted
            && self.states.len() >= coefficient_intern::MIN_STATES
            && internable
        {
            self.intern_attempted = true;
            let remaining = self.budget - self.reserved - charge;
            self.intern = CoefficientIntern::new(remaining);
            self.reserved += self
                .intern
                .as_ref()
                .map_or(0, CoefficientIntern::reserved_bytes);
        }
        let slot = if internable {
            if let Some(intern) = &self.intern {
                let (existing, slot) = intern.find_or_slot(node, coefficients, &self.states);
                if let Some(id) = existing {
                    return Some(id);
                }
                slot
            } else {
                None
            }
        } else {
            None
        };
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
        if let (Some(intern), Some((slot, fingerprint))) = (&mut self.intern, slot) {
            intern.insert(slot, fingerprint, node, id);
        }
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
    #[inline(always)]
    fn conjugate(self, x: &mut [u64], z: &mut [u64]) {
        match self {
            Self::H(q) => {
                let word = q / 64;
                let mask = 1u64 << (q % 64);
                let changed = (x[word] ^ z[word]) & mask;
                x[word] ^= changed;
                z[word] ^= changed;
            }
            Self::S(q) => {
                let word = q / 64;
                z[word] ^= x[word] & (1u64 << (q % 64));
            }
            Self::CX(a, b) => {
                let (aw, ab) = (a / 64, a % 64);
                let (bw, bb) = (b / 64, b % 64);
                let target_x = ((x[aw] >> ab) & 1) << bb;
                let control_z = ((z[bw] >> bb) & 1) << ab;
                x[bw] ^= target_x;
                z[aw] ^= control_z;
            }
            Self::CZ(a, b) => {
                let (aw, ab) = (a / 64, a % 64);
                let (bw, bb) = (b / 64, b % 64);
                z[aw] ^= ((x[bw] >> bb) & 1) << ab;
                z[bw] ^= ((x[aw] >> ab) & 1) << bb;
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

// Shared dispatcher sinks keep packet admission, fallback and RNG order identical.
enum BatchOutput<'a> {
    Measurements(&'a mut Vec<u8>),
    Counts {
        observable_index: u32,
        counts: &'a mut NearCliffordPostselectedCounts,
    },
}
impl BatchOutput<'_> {
    fn row(&mut self, shot: NearCliffordShot) {
        match self {
            Self::Measurements(output) => {
                output.extend(shot.measurements.into_iter().map(u8::from))
            }
            Self::Counts {
                observable_index,
                counts,
            } => {
                if shot.detectors.iter().any(|&bit| bit) {
                    return;
                }
                counts.accepted += 1;
                let parity = shot
                    .observables
                    .iter()
                    .filter(|(index, _)| index == observable_index)
                    .fold(false, |parity, (_, bit)| parity ^ bit);
                counts.logical_errors += usize::from(parity);
            }
        }
    }
    fn packet_counts_reduced(&mut self, live: u64, logical: u64) {
        if let Self::Counts { counts, .. } = self {
            counts.accepted += live.count_ones() as usize;
            counts.logical_errors += (live & logical).count_ones() as usize;
        }
    }
    fn packet_row(&mut self, records: &[u64], lane: usize) {
        if let Self::Measurements(output) = self {
            output.extend(records.iter().map(|bits| ((bits >> lane) & 1) as u8));
        }
    }
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
    // Full P/Q commutation also permits crossing feedback that reads only
    // earlier records. Annotations read earlier records and have no quantum action.
    // Only measurements move left: record producers cannot move past their readers,
    // and deferred noise stays before every original reader of its corrected record.
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
                TapeOp::Feedback { .. } => 2,
                TapeOp::Annotation { offsets, .. } => offsets.len().max(1),
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
                TapeOp::Feedback {
                    condition,
                    pauli: other,
                } => {
                    let reads_earlier_record = match condition {
                        Condition::Record(index) => record.is_none_or(|output| *index < output),
                        Condition::Sweep(_) => true,
                    };
                    reads_earlier_record && operators.clone().all(|p| commute(p, other))
                }
                TapeOp::Annotation { offsets, .. } => {
                    record.is_none_or(|output| offsets.iter().all(|index| *index < output))
                }
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

/// Arithmetic used by every compiled rotation, including the deterministic prefix.
/// The policy is immutable for a plan and its coefficient caches. It does not change
/// probability reduction order, projection arithmetic, or the typed RNG policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompiledRotationArithmetic {
    /// Preserve the original separately rounded multiply and add operations.
    #[default]
    Strict,
    /// Round the partner product and addition together with explicit FP64 FMA.
    /// Coefficient/CDF bits and sampled records may differ from Strict.
    Fused,
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
/// Optional noise-transparent scheduling borrows the compilation frame, has a
/// sixteen-million-work bound, and shares the 64 MiB tape/plan reservations. Its
/// one-time typed scalar row shares the 16 MiB packet buffer cap. Rejection
/// before scheduling retains the existing compiler.
/// The packed compilation frame reserves at most 9 MiB. Optional transitions
/// reserve at most 64 MiB and packet event/frame work buffers 16 MiB.
/// Optional homogeneous random-event runs use at most 8 MiB within the plan budget; rejection
/// retains the event-by-event generator with identical RNG and records.
/// On the first nonempty counts call, a rotation-free plan may lazily build an affine
/// detector/observable model within the remaining plan budget, including construction
/// scratch. Oversized models retain the original path. This first-call cost is not
/// hidden in compilation. Its reusable counts output workspace separately reserves
/// at most 16 MiB. Structured and flat calls keep the physical executor.
/// Batched coherent arithmetic separately reserves at most 64 MiB; larger
/// structural ranks retain the scalar/cache path.
/// These are conservative reservations, not RSS limits. Limits fail before sampling.
#[derive(Clone, Debug)]
pub struct CompiledNearCliffordExecutor {
    operations: Vec<PlanOp>,
    annotation_positions: Vec<usize>,
    num_qubits: usize,
    measurement_count: usize,
    peak_active_rank: usize,
    prefix_len: usize,
    rotation_arithmetic: CompiledRotationArithmetic,
    initial_coefficients: Arc<Vec<ComplexAmp>>,
    random_kinds: Vec<RandomKind>,
    random_runs: Option<RandomRunPlan>,
    noise_spans: Option<NoiseSpans>,
    noise_signs: Option<NoiseSigns>,
    noise_event_count: usize,
    independent_event_count: usize,
    scalar_basis: Option<Vec<Option<ScalarBasisProgram>>>,
    linear_counts: Arc<OnceLock<Option<LinearCountsPlan>>>,
    counts_plan_budget: usize,
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
        Self::compile_text_with_arithmetic(text, CompiledRotationArithmetic::Strict)
    }
    /// Compile with one rotation policy for the prefix, scalar, and packet paths.
    pub fn compile_text_with_arithmetic(
        text: &str,
        arithmetic: CompiledRotationArithmetic,
    ) -> Result<Self, String> {
        Self::compile_with_limit_and_arithmetic(crate::parser::parse_lines(text)?, 16, arithmetic)
    }
    pub fn compile_with_limit(instructions: Vec<StimInstr>, limit: usize) -> Result<Self, String> {
        Self::compile_with_limit_and_arithmetic(
            instructions,
            limit,
            CompiledRotationArithmetic::Strict,
        )
    }
    /// Fused uses explicit mul_add only in rotations; other arithmetic stays strict.
    pub fn compile_with_limit_and_arithmetic(
        instructions: Vec<StimInstr>,
        limit: usize,
        arithmetic: CompiledRotationArithmetic,
    ) -> Result<Self, String> {
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
        let capture_bytes = planner.reserved_bytes;
        let mut schedule = noise_schedule::build(
            &tape,
            &mut planner.state,
            limit,
            PLAN_BYTE_BUDGET.saturating_sub(capture_bytes),
        );
        // Bound both retained metadata representations and final construction
        // scratch before accepting an optional schedule. Rejection keeps the
        // existing compiler and its seeded stream.
        if schedule.as_ref().is_some_and(|s| {
            s.reserved_bytes()
                .and_then(|m| m.checked_mul(3))
                .and_then(|m| capture_bytes.checked_mul(2).and_then(|c| m.checked_add(c)))
                .is_none_or(|n| n > PLAN_BYTE_BUDGET)
        }) {
            schedule = None;
        }
        if let Some(selected) = &schedule {
            if selected.apply(&mut tape).is_none() {
                schedule = None;
            }
        }
        if schedule.is_none() {
            schedule_measurements(&mut tape, &mut planner.reserved_bytes)?;
        }
        planner.state.reset_identity();
        planner.expanded = 0;
        planner.reserved_bytes = schedule
            .as_ref()
            .and_then(|s| s.reserved_bytes())
            .unwrap_or(0);
        let mut logical_ids = Vec::new();
        if schedule.is_some() {
            logical_ids
                .try_reserve_exact(
                    tape.len()
                        .checked_mul(2)
                        .ok_or("scheduled node size overflow")?,
                )
                .map_err(|e| format!("scheduled node allocation failed: {e}"))?;
        }
        for (position, op) in tape.into_iter().enumerate() {
            let begin = planner.operations.len();
            planner.finish_op(op)?;
            if let Some(selected) = &schedule {
                logical_ids.extend(std::iter::repeat_n(
                    selected.order[position],
                    planner.operations.len() - begin,
                ));
            }
        }
        let mut plan = Self {
            operations: planner.operations,
            annotation_positions: Vec::new(),
            num_qubits: incumbent.num_qubits,
            measurement_count: incumbent.measurement_count,
            peak_active_rank: planner.peak,
            prefix_len: 0,
            rotation_arithmetic: arithmetic,
            initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            random_kinds: Vec::new(),
            random_runs: None,
            noise_spans: None,
            noise_signs: None,
            noise_event_count: 0,
            independent_event_count: 0,
            scalar_basis: None,
            linear_counts: Arc::new(OnceLock::new()),
            counts_plan_budget: 0,
        };
        let annotation_count = plan
            .operations
            .iter()
            .filter(|op| matches!(op, PlanOp::Annotation { .. }))
            .count();
        planner.reserved_bytes = planner
            .reserved_bytes
            .checked_add(
                annotation_count
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or("compiled annotation index size overflow")?,
            )
            .ok_or("compiled annotation index budget overflow")?;
        if planner.reserved_bytes > PLAN_BYTE_BUDGET {
            return Err("compiled annotation index exceeds plan byte budget".into());
        }
        plan.annotation_positions
            .try_reserve_exact(annotation_count)
            .map_err(|e| format!("compiled annotation index allocation failed: {e}"))?;
        plan.annotation_positions.extend(
            plan.operations
                .iter()
                .enumerate()
                .filter_map(|(index, op)| matches!(op, PlanOp::Annotation { .. }).then_some(index)),
        );
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
            .enumerate()
            .take_while(|(node, op)| {
                matches!(op, PlanOp::Basis(_) | PlanOp::Rotate { .. })
                    && schedule
                        .as_ref()
                        .is_none_or(|s| !s.has_refs(logical_ids[*node]))
            })
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
        if let Some(selected) = &schedule {
            let signs = selected
                .bind(
                    &plan.operations,
                    &logical_ids,
                    plan.prefix_len,
                    PLAN_BYTE_BUDGET.saturating_sub(planner.reserved_bytes),
                )
                .ok_or("scheduled noise metadata exceeds plan budget")?;
            planner.reserved_bytes = planner
                .reserved_bytes
                .checked_add(
                    signs
                        .reserved_bytes()
                        .ok_or("scheduled noise metadata size overflow")?,
                )
                .ok_or("scheduled plan size overflow")?;
            if planner.reserved_bytes > PLAN_BYTE_BUDGET {
                return Err("scheduled plan exceeds 64 MiB".into());
            }
            plan.noise_signs = Some(signs);
        }
        drop(schedule);
        drop(logical_ids);
        plan.independent_event_count = plan
            .random_kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Independent))
            .count();
        plan.noise_event_count = plan
            .random_kinds
            .iter()
            .filter(|kind| matches!(kind, RandomKind::Noise { .. }))
            .count();
        if let Some((programs, bytes)) = build_scalar_basis(
            &plan.operations,
            plan.num_qubits,
            PLAN_BYTE_BUDGET.saturating_sub(planner.reserved_bytes),
        ) {
            planner.reserved_bytes += bytes;
            plan.scalar_basis = Some(programs);
        }
        plan.random_runs = RandomRunPlan::build(
            &plan.random_kinds,
            PLAN_BYTE_BUDGET.saturating_sub(planner.reserved_bytes),
        );
        let run_bytes = plan
            .random_runs
            .as_ref()
            .map_or(0, RandomRunPlan::reserved_bytes);
        let remaining = PLAN_BYTE_BUDGET
            .saturating_sub(planner.reserved_bytes)
            .saturating_sub(run_bytes);
        if plan.noise_event_count >= MIN_SCALAR_PREPARED_NOISE_EVENTS {
            plan.noise_spans = NoiseSpans::build(&plan.operations, plan.prefix_len, remaining);
        }
        plan.counts_plan_budget = remaining.saturating_sub(
            plan.noise_spans
                .as_ref()
                .map_or(0, NoiseSpans::reserved_bytes),
        );
        Ok(plan)
    }
    // The packet and dense paths keep the original gates. Direct test-only
    // Planner plans have no optional table and retain sequential execution.
    #[inline(always)]
    fn conjugate_scalar_basis(
        &self,
        node: usize,
        gates: &[BasisGate],
        x: &mut [u64],
        z: &mut [u64],
    ) {
        if let Some(Some(program)) = self.scalar_basis.as_ref().and_then(|p| p.get(node)) {
            program.apply(gates, x, z);
        } else {
            for &gate in gates {
                gate.conjugate(x, z);
            }
        }
    }
    /// The fixed rotation policy of this plan and every sampler prepared from it.
    pub fn rotation_arithmetic(&self) -> CompiledRotationArithmetic {
        self.rotation_arithmetic
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
        let mut conditional_tape = Vec::new();
        if self.noise_signs.is_some() {
            let count = self.random_kinds.len();
            if count
                .checked_mul(size_of::<u64>())
                .is_none_or(|n| n > PACKET_BYTE_BUDGET)
            {
                return Err("scheduled row exceeds 16 MiB".into());
            }
            conditional_tape
                .try_reserve_exact(count)
                .map_err(|e| format!("scheduled row allocation failed: {e}"))?;
            if conditional_tape
                .capacity()
                .checked_mul(size_of::<u64>())
                .is_none_or(|n| n > PACKET_BYTE_BUDGET)
            {
                return Err("scheduled row capacity exceeds 16 MiB".into());
            }
            conditional_tape.resize(count, 0);
        }
        Ok(CompiledNearCliffordSampler {
            conditional_tape,
            plan: self,
            x: vec![0; self.num_qubits.div_ceil(64)],
            z: vec![0; self.num_qubits.div_ceil(64)],
            coefficients,
            reduced_coefficients: Vec::new(),
            cache: CoefficientCache::new(self, cache_bytes),
            pack_enabled: true,
            #[cfg(test)]
            last_packet_live: 0,
            #[cfg(test)]
            last_packet_live_mask: 0,
            #[cfg(test)]
            last_packet_error_producer: None,
            #[cfg(test)]
            observe_lazy_error_producer: false,
            #[cfg(test)]
            materialized_packet_reference: false,
            #[cfg(test)]
            last_scalar_prepared: false,
            packet_x: Vec::new(),
            packet_z: Vec::new(),
            packet_tape: Vec::new(),
            packet_records: Vec::new(),
            packet_noise_masks: Vec::new(),
            packet_independent: None,
            coherent: CoherentPacket::default(),
            counts_outputs: Vec::new(),
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

/// Retains allocations, with identical streams for successful structured and flat calls.
/// On an execution error, RNG events may already have been drawn for the current
/// row or packet; no exact failed-call RNG prefix is promised across strategies.
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
    #[cfg(test)]
    last_packet_live_mask: u64,
    #[cfg(test)]
    last_packet_error_producer: Option<PacketErrorProducerObservation>,
    #[cfg(test)]
    observe_lazy_error_producer: bool,
    #[cfg(test)]
    materialized_packet_reference: bool,
    #[cfg(test)]
    last_scalar_prepared: bool,
    packet_x: Vec<u64>,
    packet_z: Vec<u64>,
    packet_tape: Vec<u64>,
    conditional_tape: Vec<u64>,
    packet_records: Vec<u64>,
    packet_noise_masks: Vec<u64>,
    packet_independent: Option<IndependentPacket>,
    coherent: CoherentPacket,
    counts_outputs: Vec<u64>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct PacketErrorProducerObservation {
    lanes: usize,
    coherent: bool,
    noise_admitted: bool,
    noise_reserved_bytes: usize,
    rows: Vec<Vec<u64>>,
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

// Some(id) means every live lane has that exact ID; the array may be stale
// and must not be read. With None, every live lane is materialized in lanes.
struct PacketCacheStates {
    lanes: [Option<usize>; 64],
    uniform: Option<usize>,
    #[cfg(test)]
    materialized_reference: bool,
}
impl PacketCacheStates {
    #[cfg(test)]
    fn enable_materialized_reference(&mut self, start: usize) {
        self.materialized_reference = true;
        self.lanes = [Some(start); 64];
        self.uniform = None;
    }
    #[cfg(test)]
    fn reference_transition(&mut self, mask: u64, id: Option<usize>) {
        if self.materialized_reference {
            // Literal S2 writes, including newly dead lanes, before live removal.
            for lane in 0..64 {
                if mask & (1u64 << lane) != 0 {
                    self.lanes[lane] = id;
                }
            }
        }
    }
    fn new(start: usize) -> Self {
        Self {
            lanes: [None; 64],
            uniform: Some(start),
            #[cfg(test)]
            materialized_reference: false,
        }
    }
    fn uniform(&mut self, live: u64) -> Option<usize> {
        #[cfg(test)]
        if self.materialized_reference {
            // Frozen S2 representation: literal scan, never inspect the hint
            // or call a production scan helper.
            let first = (0..64).find(|&lane| live & (1u64 << lane) != 0)?;
            let id = self.lanes[first]?;
            for lane in 0..64 {
                if live & (1u64 << lane) != 0 && self.lanes[lane] != Some(id) {
                    return None;
                }
            }
            return Some(id);
        }
        if live == 0 {
            return None;
        }
        if self.uniform.is_none() {
            self.uniform = uniform_packet_state(&self.lanes, live);
        }
        self.uniform
    }
    // masks partition the pre-operation live set. live is the post-admission
    // subset after callers remove every mask whose next ID is None.
    fn finish_uniform(&mut self, masks: [u64; 2], next: [Option<usize>; 2], live: u64) {
        #[cfg(test)]
        if self.materialized_reference {
            return; // Each reference transition was already broadcast immediately.
        }
        match (masks[0] & live != 0, masks[1] & live != 0) {
            (false, false) => self.uniform = None,
            (true, false) => self.uniform = next[0],
            (false, true) => self.uniform = next[1],
            (true, true) if next[0] == next[1] => self.uniform = next[0],
            (true, true) => {
                self.uniform = None;
                broadcast_packet_state(&mut self.lanes, masks[0] & live, next[0]);
                broadcast_packet_state(&mut self.lanes, masks[1] & live, next[1]);
            }
        }
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
    // Used frame/record cells are cleared by the packet. Active events fill
    // their tape slots; compact Independent/Noise slots can remain untouched
    // until restoration makes the full tape complete for each replayed row.
    // Retained cells need no second fill here.
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
                if let Some(next) = cache.store(node, &self.coefficients) {
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
                if let Some(next) = cache.store(node, &self.coefficients) {
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
        if self.plan.rotation_arithmetic == CompiledRotationArithmetic::Fused {
            return self.rotate_signed_fused(p, dagger, flip);
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
        let term = |parity: bool, amp: ComplexAmp| {
            let factor = if parity { -factor } else { factor };
            if imaginary {
                ComplexAmp::new(-amp.im * factor, amp.re * factor)
            } else {
                amp * factor
            }
        };
        if p.z == 0 && p.x >= 2 && p.x & 7 != 0 {
            match (imaginary, p.x & 1 != 0) {
                (true, true) => Self::rotate_highest_z0::<false, true, true>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (true, false) => Self::rotate_highest_z0::<false, true, false>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (false, true) => Self::rotate_highest_z0::<false, false, true>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (false, false) => Self::rotate_highest_z0::<false, false, false>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
            }
            return Ok(());
        }
        if p.x == 0 {
            for (i, amp) in self.coefficients.iter_mut().enumerate() {
                *amp = *amp * c + term((i & p.z).count_ones() % 2 != 0, *amp);
            }
        } else {
            let pivot = 1 << p.x.trailing_zeros();
            let parity_swap = (p.x & p.z).count_ones() % 2 != 0;
            let uniform = p.z & (pivot - 1) == 0;
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
                if uniform {
                    // No Z coordinate below the chosen X pivot: every offset
                    // in this block has the same two signed rotation factors.
                    let parity_a = (block & p.z).count_ones() % 2 != 0;
                    let parity_b = parity_a ^ parity_swap;
                    for (a, b) in a.iter_mut().zip(b) {
                        let old_a = *a;
                        let old_b = *b;
                        *a = old_a * c + term(parity_b, old_b);
                        *b = old_b * c + term(parity_a, old_a);
                    }
                } else {
                    for (offset, (a, b)) in a.iter_mut().zip(b).enumerate() {
                        let old_a = *a;
                        let old_b = *b;
                        let parity_a = ((block + offset) & p.z).count_ones() % 2 != 0;
                        *a = old_a * c + term(parity_a ^ parity_swap, old_b);
                        *b = old_b * c + term(parity_a, old_a);
                    }
                }
            }
        }
        Ok(())
    }
    fn rotate_signed_fused(
        &mut self,
        p: &CompactPauli,
        dagger: bool,
        flip: bool,
    ) -> Result<(), String> {
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
        let update = |own: ComplexAmp, parity: bool, partner: ComplexAmp| {
            let factor = if parity { -factor } else { factor };
            if imaginary {
                ComplexAmp::new(
                    (-partner.im).mul_add(factor, own.re * c),
                    partner.re.mul_add(factor, own.im * c),
                )
            } else {
                ComplexAmp::new(
                    partner.re.mul_add(factor, own.re * c),
                    partner.im.mul_add(factor, own.im * c),
                )
            }
        };
        // With no compact Z signs, use the highest X bit to borrow two full
        // halves once. Any lower X mask permutes adjacent two-coefficient
        // groups, so even a lowest pivot of one can expose two FP64 lanes.
        // Larger lowest-pivot blocks already vectorize in the retained kernel.
        if p.z == 0 && p.x >= 2 && p.x & 7 != 0 {
            match (imaginary, p.x & 1 != 0) {
                (true, true) => Self::rotate_highest_z0::<true, true, true>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (true, false) => Self::rotate_highest_z0::<true, true, false>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (false, true) => Self::rotate_highest_z0::<true, false, true>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
                (false, false) => Self::rotate_highest_z0::<true, false, false>(
                    &mut self.coefficients,
                    p.x,
                    c,
                    factor,
                ),
            }
            return Ok(());
        }
        if p.x == 0 {
            for (i, amp) in self.coefficients.iter_mut().enumerate() {
                *amp = update(*amp, (i & p.z).count_ones() % 2 != 0, *amp);
            }
        } else {
            let pivot = 1 << p.x.trailing_zeros();
            let parity_swap = (p.x & p.z).count_ones() % 2 != 0;
            let uniform = p.z & (pivot - 1) == 0;
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
                if uniform {
                    // No Z coordinate below the chosen X pivot: every offset
                    // in this block has the same two signed rotation factors.
                    let parity_a = (block & p.z).count_ones() % 2 != 0;
                    let parity_b = parity_a ^ parity_swap;
                    for (a, b) in a.iter_mut().zip(b) {
                        let old_a = *a;
                        let old_b = *b;
                        *a = update(old_a, parity_b, old_b);
                        *b = update(old_b, parity_a, old_a);
                    }
                } else {
                    for (offset, (a, b)) in a.iter_mut().zip(b).enumerate() {
                        let old_a = *a;
                        let old_b = *b;
                        let parity_a = ((block + offset) & p.z).count_ones() % 2 != 0;
                        *a = update(old_a, parity_a ^ parity_swap, old_b);
                        *b = update(old_b, parity_a, old_a);
                    }
                }
            }
        }
        Ok(())
    }
    fn rotate_highest_z0<const FUSED: bool, const IMAGINARY: bool, const SWAP: bool>(
        coefficients: &mut [ComplexAmp],
        x: usize,
        c: f64,
        factor: f64,
    ) {
        #[cfg(target_arch = "x86_64")]
        if FUSED
            && IMAGINARY
            && std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
        {
            // SAFETY: Both CPU features are checked once before the loop.
            // The helper snapshots disjoint groups using local f64 arrays.
            unsafe {
                Self::rotate_highest_z0_avx2::<SWAP>(coefficients, x, c, factor);
            }
            return;
        }
        let pivot = 1usize << (usize::BITS - 1 - x.leading_zeros());
        let group_xor = (x ^ pivot) >> 1;
        debug_assert!(pivot >= 2);
        debug_assert_eq!(coefficients.len() % (pivot * 2), 0);
        for block in coefficients.chunks_exact_mut(pivot * 2) {
            let (left, right) = block.split_at_mut(pivot);
            for (group, a) in left.chunks_exact_mut(2).enumerate() {
                // XOR is a permutation of the pivot/2 adjacent groups.
                // The separate halves and distinct groups never overlap.
                let other = (group ^ group_xor) * 2;
                let a: &mut [ComplexAmp; 2] = a.try_into().unwrap();
                let b: &mut [ComplexAmp; 2] = (&mut right[other..other + 2]).try_into().unwrap();
                Self::rotate_adjacent_pair::<FUSED, IMAGINARY, SWAP>(a, b, c, factor);
            }
        }
    }

    #[inline(always)]
    fn rotate_adjacent_pair<const FUSED: bool, const IMAGINARY: bool, const SWAP: bool>(
        a: &mut [ComplexAmp; 2],
        b: &mut [ComplexAmp; 2],
        c: f64,
        factor: f64,
    ) {
        // Snapshot all four inputs before either group is written. Local
        // two-lane arrays expose SLP without changing the persistent AoS layout.
        let old_a = *a;
        let old_b = if SWAP { [b[1], b[0]] } else { *b };
        let ar = [old_a[0].re, old_a[1].re];
        let ai = [old_a[0].im, old_a[1].im];
        let br = [old_b[0].re, old_b[1].re];
        let bi = [old_b[0].im, old_b[1].im];
        // Strict keeps own*c + partner*factor in the original order; Fused
        // retains the original single-rounding mul_add. Const selection makes
        // the policy branch disappear from the arithmetic kernel.
        let accumulate = |partner: f64, own_scaled: f64| {
            if FUSED {
                partner.mul_add(factor, own_scaled)
            } else {
                own_scaled + partner * factor
            }
        };
        let (out_ar, out_ai, out_br, out_bi) = if IMAGINARY {
            (
                [accumulate(-bi[0], ar[0] * c), accumulate(-bi[1], ar[1] * c)],
                [accumulate(br[0], ai[0] * c), accumulate(br[1], ai[1] * c)],
                [accumulate(-ai[0], br[0] * c), accumulate(-ai[1], br[1] * c)],
                [accumulate(ar[0], bi[0] * c), accumulate(ar[1], bi[1] * c)],
            )
        } else {
            (
                [accumulate(br[0], ar[0] * c), accumulate(br[1], ar[1] * c)],
                [accumulate(bi[0], ai[0] * c), accumulate(bi[1], ai[1] * c)],
                [accumulate(ar[0], br[0] * c), accumulate(ar[1], br[1] * c)],
                [accumulate(ai[0], bi[0] * c), accumulate(ai[1], bi[1] * c)],
            )
        };
        a[0] = ComplexAmp::new(out_ar[0], out_ai[0]);
        a[1] = ComplexAmp::new(out_ar[1], out_ai[1]);
        b[usize::from(SWAP)] = ComplexAmp::new(out_br[0], out_bi[0]);
        b[usize::from(!SWAP)] = ComplexAmp::new(out_br[1], out_bi[1]);
    }
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn rotate_highest_z0_avx2<const SWAP: bool>(
        coefficients: &mut [ComplexAmp],
        x: usize,
        c: f64,
        factor: f64,
    ) {
        let pivot = 1usize << (usize::BITS - 1 - x.leading_zeros());
        let group_xor = (x ^ pivot) >> 1;
        debug_assert!(pivot >= 2);
        debug_assert_eq!(coefficients.len() % (pivot * 2), 0);
        for block in coefficients.chunks_exact_mut(pivot * 2) {
            let (left, right) = block.split_at_mut(pivot);
            for (group, a) in left.chunks_exact_mut(2).enumerate() {
                // XOR is a permutation of the pivot/2 adjacent groups.
                // The separate halves and distinct groups never overlap.
                let other = (group ^ group_xor) * 2;
                let a: &mut [ComplexAmp; 2] = a.try_into().unwrap();
                let b: &mut [ComplexAmp; 2] = (&mut right[other..other + 2]).try_into().unwrap();
                // SAFETY: This function has the same CPU feature precondition.
                unsafe {
                    Self::rotate_adjacent_pair_avx2::<SWAP>(a, b, c, factor);
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn rotate_adjacent_pair_avx2<const SWAP: bool>(
        a: &mut [ComplexAmp; 2],
        b: &mut [ComplexAmp; 2],
        c: f64,
        factor: f64,
    ) {
        use std::arch::x86_64::*;
        let old_a = *a;
        let old_b = if SWAP { [b[1], b[0]] } else { *b };
        let av = [old_a[0].re, old_a[0].im, old_a[1].re, old_a[1].im];
        let bv = [old_b[0].re, old_b[0].im, old_b[1].re, old_b[1].im];
        // SAFETY: Each local f64 array contains exactly four initialized
        // elements. Unaligned loads require no stronger alignment; no
        // ComplexAmp representation or field-layout assumption is made.
        let (va, vb) = unsafe { (_mm256_loadu_pd(av.as_ptr()), _mm256_loadu_pd(bv.as_ptr())) };
        let vc = _mm256_set1_pd(c);
        let vf = _mm256_set1_pd(factor);
        let sign = _mm256_set_pd(0., -0., 0., -0.);
        let pa = _mm256_xor_pd(_mm256_permute_pd::<5>(vb), sign);
        let pb = _mm256_xor_pd(_mm256_permute_pd::<5>(va), sign);
        // Match partner.mul_add(factor, own*c), including unary negation
        // before FMA. No reassociation or fused own*c multiply is permitted.
        let oa = _mm256_fmadd_pd(pa, vf, _mm256_mul_pd(va, vc));
        let ob = _mm256_fmadd_pd(pb, vf, _mm256_mul_pd(vb, vc));
        let (mut out_a, mut out_b) = ([0.; 4], [0.; 4]);
        // SAFETY: Both output arrays have four writable f64 elements.
        unsafe {
            _mm256_storeu_pd(out_a.as_mut_ptr(), oa);
            _mm256_storeu_pd(out_b.as_mut_ptr(), ob);
        }
        a[0] = ComplexAmp::new(out_a[0], out_a[1]);
        a[1] = ComplexAmp::new(out_a[2], out_a[3]);
        b[usize::from(SWAP)] = ComplexAmp::new(out_b[0], out_b[1]);
        b[usize::from(!SWAP)] = ComplexAmp::new(out_b[2], out_b[3]);
    }
    fn probability_zero(&self, p: &CompactPauli) -> f64 {
        // i_pow uses phase % 4; unsigned phase & 3 preserves every accepted u8 alias.
        match p.physical.phase & 3 {
            0 => self.probability_zero_phase::<0>(p),
            1 => self.probability_zero_phase::<1>(p),
            2 => self.probability_zero_phase::<2>(p),
            _ => self.probability_zero_phase::<3>(p),
        }
    }
    #[inline]
    fn probability_zero_phase<const PHASE: u8>(&self, p: &CompactPauli) -> f64 {
        let mut expectation = 0.;
        let mut norm = 0.;
        for (i, &amp) in self.coefficients.iter().enumerate() {
            let sign = if (i & p.z).count_ones() % 2 != 0 {
                -1.
            } else {
                1.
            };
            expectation += (self.coefficients[i ^ p.x].conj() * i_pow(PHASE) * (amp * sign)).re;
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
    // Packet frame/mask buffers and the optional Independent sidecar share the
    // existing cap with scalar tape. Count actual retained capacities, not len.
    fn scalar_tape_other_bytes(&self) -> Option<usize> {
        let vectors = self
            .packet_x
            .capacity()
            .checked_add(self.packet_z.capacity())?
            .checked_add(self.packet_records.capacity())?
            .checked_add(self.packet_noise_masks.capacity())?
            .checked_mul(size_of::<u64>())?;
        let independent = self
            .packet_independent
            .as_ref()
            .map_or(Some(0), IndependentPacket::reserved_bytes)?;
        vectors.checked_add(independent)?.checked_add(
            self.conditional_tape
                .capacity()
                .checked_mul(size_of::<u64>())?,
        )
    }

    fn try_prepare_scalar_tape(&mut self, byte_budget: usize) -> bool {
        let Some(remaining) = self
            .scalar_tape_other_bytes()
            .and_then(|occupied| byte_budget.checked_sub(occupied))
        else {
            return false;
        };
        let max_words = remaining / size_of::<u64>();
        let count = self.plan.random_kinds.len();
        if count > max_words || self.packet_tape.capacity() > max_words {
            return false; // Optional rejection precedes any RNG draw or growth.
        }
        if self.packet_tape.capacity() < count {
            if self
                .packet_tape
                .try_reserve_exact(count - self.packet_tape.len())
                .is_err()
            {
                return false;
            }
            if self.packet_tape.capacity() > max_words {
                // Allocators may round exact requests upward. Drop the optional
                // enlarged scratch before live fallback, rather than retain an
                // over-budget buffer. No quantum/RNG state has changed yet.
                self.packet_tape = Vec::new();
                return false;
            }
        }
        if self.packet_tape.len() < count {
            self.packet_tape.resize(count, 0);
        }
        true // A retained 64R packet tape keeps its original len/capacity.
    }

    fn row(&mut self, sweep: &[bool], rng: &mut impl Rng) -> Result<NearCliffordShot, String> {
        self.row_with_scalar_tape_budget(sweep, rng, PACKET_BYTE_BUDGET)
    }

    fn row_with_scalar_tape_budget(
        &mut self,
        sweep: &[bool],
        rng: &mut impl Rng,
        byte_budget: usize,
    ) -> Result<NearCliffordShot, String> {
        self.row_with_scalar_tape_budget_mode::<false>(sweep, rng, byte_budget)
    }

    fn row_for_output(
        &mut self,
        sweep: &[bool],
        rng: &mut impl Rng,
        output: &BatchOutput<'_>,
    ) -> Result<NearCliffordShot, String> {
        if matches!(output, BatchOutput::Counts { .. }) {
            self.row_with_scalar_tape_budget_mode::<true>(sweep, rng, PACKET_BYTE_BUDGET)
        } else {
            self.row(sweep, rng)
        }
    }

    fn row_with_scalar_tape_budget_mode<const POSTSELECT: bool>(
        &mut self,
        sweep: &[bool],
        rng: &mut impl Rng,
        byte_budget: usize,
    ) -> Result<NearCliffordShot, String> {
        #[cfg(test)]
        {
            self.last_scalar_prepared = false;
        }
        let plan = self.plan;
        if plan.noise_signs.is_some() {
            let mut tape = std::mem::take(&mut self.conditional_tape);
            let count = plan.random_kinds.len();
            assert_eq!(tape.len(), count, "scheduled row buffer is incomplete");
            let mut live = RowRandom::live(&mut *rng);
            let noise_is_zero = if let Some(runs) = &plan.random_runs {
                runs.fill_row_with_zero_noise(&mut live, &plan.random_kinds, &mut tape)
            } else {
                let mut noise_is_zero = true;
                for (&kind, value) in plan.random_kinds.iter().zip(&mut tape) {
                    *value = live.draw(kind);
                    if matches!(kind, RandomKind::Noise { .. }) && *value != 0 {
                        noise_is_zero = false;
                    }
                }
                noise_is_zero
            };
            let mut replay = RowRandom::recorded_with_zero_noise(&tape, &mut *rng, noise_is_zero);
            let result = self.row_with_random_kernel::<POSTSELECT, true>(sweep, &mut replay);
            debug_assert!(result.is_err() || replay.cursor == count);
            self.conditional_tape = tape;
            return result;
        }
        if plan.noise_event_count < MIN_SCALAR_PREPARED_NOISE_EVENTS || plan.random_kinds.is_empty()
        {
            return self.row_with_random_mode::<POSTSELECT>(sweep, &mut RowRandom::live(rng));
        }
        let Some(runs) = &plan.random_runs else {
            return self.row_with_random_mode::<POSTSELECT>(sweep, &mut RowRandom::live(rng));
        };
        if !self.try_prepare_scalar_tape(byte_budget) {
            return self.row_with_random_mode::<POSTSELECT>(sweep, &mut RowRandom::live(rng));
        }
        #[cfg(test)]
        {
            self.last_scalar_prepared = true;
        }
        let count = plan.random_kinds.len();
        let mut tape = std::mem::take(&mut self.packet_tape);
        let result = {
            runs.fill_row(
                &mut RowRandom::live(&mut *rng),
                &plan.random_kinds,
                &mut tape[..count],
            );
            let mut replay = RowRandom::recorded(&tape[..count], &mut *rng);
            let result = self.row_with_random_kernel::<POSTSELECT, true>(sweep, &mut replay);
            debug_assert!(result.is_err() || replay.cursor == count);
            result
        };
        // Restore even on execution Err; never retry after the row was drawn.
        self.packet_tape = tape;
        result
    }
    fn row_with_random(
        &mut self,
        sweep: &[bool],
        random: &mut RowRandom<'_, impl Rng>,
    ) -> Result<NearCliffordShot, String> {
        self.row_with_random_mode::<false>(sweep, random)
    }

    fn row_with_random_mode<const POSTSELECT: bool>(
        &mut self,
        sweep: &[bool],
        random: &mut impl RowDraw,
    ) -> Result<NearCliffordShot, String> {
        self.row_with_random_kernel::<POSTSELECT, false>(sweep, random)
    }

    fn row_with_random_kernel<const POSTSELECT: bool, const SKIP_NOISE: bool>(
        &mut self,
        sweep: &[bool],
        random: &mut impl RowDraw,
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
        let mut event = 0;
        let mut spans = plan
            .noise_spans
            .as_ref()
            .map_or(&[][..], NoiseSpans::spans)
            .iter()
            .peekable();
        let mut node = plan.prefix_len;
        while node < plan.operations.len() {
            if POSTSELECT && SKIP_NOISE {
                if let Some(span) = spans.peek() {
                    if node >= span.end {
                        spans.next();
                        continue;
                    }
                    if node >= span.start {
                        // The complete typed producer has already drawn this row.
                        // Only literal zero Noise values retire operations; no
                        // basis, measurement, feedback or annotation is crossed.
                        let skipped = random.skip_zero_noise(span.end - node);
                        node += skipped;
                        event += skipped;
                        if node == span.end {
                            spans.next();
                            continue;
                        }
                    }
                }
            }
            let op = &plan.operations[node];
            match op {
                PlanOp::Basis(gates) => {
                    plan.conjugate_scalar_basis(node, gates, &mut self.x, &mut self.z);
                }
                PlanOp::Rotate {
                    pauli,
                    expand,
                    dagger,
                } => {
                    if let Some(signs) = &plan.noise_signs {
                        let sign = pauli.physical.anticommutes(&self.x, &self.z)
                            ^ signs.scalar(node, random);
                        self.cached_rotate_signed(node, pauli, *expand, *dagger, sign, &mut state)?;
                    } else {
                        self.cached_rotate(node, pauli, *expand, *dagger, &mut state)?;
                    }
                }
                PlanOp::Noise {
                    probability,
                    choices,
                } => {
                    event += 1;
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
                        None => {
                            shot.detectors.push(bit);
                            if POSTSELECT && bit {
                                // Consume the remaining fixed typed draws, including sparse
                                // run renewal and unused-bit boundaries, in their original
                                // row order. Recorded fallback tapes require no new draws.
                                random.discard_remaining(&plan.random_kinds[event..]);
                                return Ok(shot);
                            }
                        }
                    }
                }
                PlanOp::Measure(m) => {
                    let anti = m.pauli.physical.anticommutes(&self.x, &self.z)
                        ^ plan
                            .noise_signs
                            .as_ref()
                            .is_some_and(|s| s.scalar(node, random));
                    let branch = match m.projection {
                        Projection::Constant(bit) => bit,
                        Projection::Independent { .. } => {
                            event += 1;
                            random.draw(RandomKind::Independent) != 0
                        }
                        Projection::Active { .. } => {
                            event += 1;
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
                    if !m.basis.is_empty() {
                        plan.conjugate_scalar_basis(node, &m.basis, &mut self.x, &mut self.z);
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
                        let error = if m.readout > 0. {
                            event += 1;
                            random.draw(RandomKind::Noise {
                                probability: m.readout,
                                choices: 1,
                            }) != 0
                        } else {
                            false
                        };
                        shot.measurements[index] = physical ^ m.inverted ^ error;
                    }
                }
            }
            node += 1;
        }
        debug_assert_eq!(event, plan.random_kinds.len());
        Ok(shot)
    }
    // All random draws depend only on the fixed plan, not the coherent state.
    // Draw them in original row order, so packet size and fallback never change RNG.
    fn packet(
        &mut self,
        lanes: usize,
        sweep: &[bool],
        rng: &mut impl Rng,
        output: &mut BatchOutput<'_>,
        coherent: bool,
        mut noise_packet: Option<&mut NoisePacket>,
    ) -> Result<(), String> {
        let mut x = std::mem::take(&mut self.packet_x);
        let mut z = std::mem::take(&mut self.packet_z);
        let mut tape = std::mem::take(&mut self.packet_tape);
        let mut records = std::mem::take(&mut self.packet_records);
        let mut noise_masks = std::mem::take(&mut self.packet_noise_masks);
        let mut independent_packet = self.packet_independent.take();
        let result = (|| {
            let plan = self.plan;
            let random_count = plan.random_kinds.len();
            if coherent {
                self.coherent
                    .reset(&plan.initial_coefficients, lanes, plan.peak_active_rank)?;
            }
            noise_masks.fill(0);
            if let Some(packet) = &mut noise_packet {
                packet.clear();
            }
            for lane in 0..lanes {
                let mut random = RowRandom::live(&mut *rng);
                let row = &mut tape[lane * random_count..(lane + 1) * random_count];
                if let Some(runs) = &plan.random_runs {
                    if let Some(packet) = &mut noise_packet {
                        runs.fill_row_with_compact_noise(
                            &mut random,
                            &plan.random_kinds,
                            row,
                            &mut noise_masks,
                            1u64 << lane,
                            packet.planes(),
                            independent_packet.as_mut().map(|packet| packet.row(lane)),
                        );
                    } else if let Some(packet) = &mut independent_packet {
                        runs.fill_row_with_compact_independent(
                            &mut random,
                            &plan.random_kinds,
                            row,
                            &mut noise_masks,
                            1u64 << lane,
                            packet.row(lane),
                        );
                    } else {
                        runs.fill_row_with_noise_masks(
                            &mut random,
                            &plan.random_kinds,
                            row,
                            &mut noise_masks,
                            1u64 << lane,
                        );
                    }
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
            if let Some(packet) = &mut independent_packet {
                packet.transpose(lanes);
            }
            #[cfg(test)]
            if self.observe_lazy_error_producer {
                // Observe completed producer storage before any physics/Err.
                // Decode into copies only: original tape/sidecars are untouched.
                let rows = (0..lanes)
                    .map(|lane| {
                        let mut row = tape[lane * random_count..(lane + 1) * random_count].to_vec();
                        if let Some(packet) = &noise_packet {
                            packet.restore_row(&plan.random_kinds, &mut row, lane);
                        }
                        if let Some(packet) = &independent_packet {
                            packet.restore_row(&plan.random_kinds, &mut row, lane);
                        }
                        row
                    })
                    .collect();
                self.last_packet_error_producer = Some(PacketErrorProducerObservation {
                    lanes,
                    coherent,
                    noise_admitted: noise_packet.is_some(),
                    noise_reserved_bytes: noise_packet
                        .as_ref()
                        .map_or(0, |packet| packet.lazy_error_reserved_bytes()),
                    rows,
                });
            }
            x.fill(0);
            z.fill(0);
            records.fill(0);
            let start = self.cache.as_ref().map_or(0, |cache| cache.start);
            let mut states = PacketCacheStates::new(start);
            #[cfg(test)]
            if self.materialized_packet_reference {
                states.enable_materialized_reference(start);
            }
            let all = if lanes == 64 {
                u64::MAX
            } else {
                (1u64 << lanes) - 1
            };
            let mut live = all;
            let mut event = 0;
            let mut noise_event = 0;
            let mut independent_event = 0;
            let mut record = 0;
            let selected_observable = match output {
                BatchOutput::Counts {
                    observable_index, ..
                } => Some(*observable_index),
                _ => None,
            };
            let postselect = selected_observable.is_some();
            let mut logical = 0u64;
            // Rejected lanes must not be mistaken for cache-admission failures:
            // their counts are already final and they need no scalar replay.
            let mut rejected = 0u64;
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
                        let anti = pauli.physical.packet_anti(&x, &z)
                            ^ plan.noise_signs.as_ref().map_or(0, |s| {
                                s.packet(
                                    node,
                                    &tape,
                                    random_count,
                                    &noise_masks,
                                    noise_packet.as_deref(),
                                )
                            });
                        if coherent {
                            self.coherent.rotate_with_arithmetic(
                                pauli,
                                *expand,
                                *dagger,
                                anti,
                                plan.rotation_arithmetic,
                            )?;
                        } else if let Some(id) = states.uniform(live) {
                            let masks = [live & !anti, live & anti];
                            let mut next_states = [None; 2];
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
                                next_states[sign] = next;
                                #[cfg(test)]
                                states.reference_transition(mask, next);
                                if next.is_none() {
                                    live &= !mask;
                                }
                            }
                            states.finish_uniform(masks, next_states, live);
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
                                    &mut states.lanes[lane],
                                )?;
                                if states.lanes[lane].is_none() {
                                    live &= !(1 << lane);
                                }
                            }
                        }
                    }
                    PlanOp::Noise { choices, .. } => {
                        // No-hit channels have no effect on any frame or record.
                        // Optional compact draws restore the complete tape before replay.
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
                            let masks = if let Some(packet) = &noise_packet {
                                packet.choice_masks(noise_event - 1, selected, choices.len())
                            } else {
                                packet_choice_masks(
                                    selected,
                                    choices.len(),
                                    &tape,
                                    event - 1,
                                    random_count,
                                )
                            };
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
                    PlanOp::Annotation {
                        offsets,
                        observable,
                    } => {
                        if let Some(selected) = selected_observable {
                            // Deferred record corrections precede every original
                            // reader. Reduce each final raw annotation only once.
                            match observable {
                                None => {
                                    let parity = offsets
                                        .iter()
                                        .fold(0, |bits, &index| bits ^ records[index]);
                                    let newly_rejected = live & parity;
                                    rejected |= newly_rejected;
                                    live &= !newly_rejected;
                                    if live == 0 {
                                        // Complete typed tape/sidecars were produced first.
                                        // Admission failures still replay their own rows.
                                        break;
                                    }
                                }
                                Some(index) if *index == selected => {
                                    logical ^= offsets
                                        .iter()
                                        .fold(0, |bits, &index| bits ^ records[index]);
                                }
                                _ => {}
                            }
                        }
                    }
                    PlanOp::Measure(m) => {
                        let anti = m.pauli.physical.packet_anti(&x, &z)
                            ^ plan.noise_signs.as_ref().map_or(0, |s| {
                                s.packet(
                                    node,
                                    &tape,
                                    random_count,
                                    &noise_masks,
                                    noise_packet.as_deref(),
                                )
                            });
                        let mut branch = 0;
                        match m.projection {
                            Projection::Constant(bit) => {
                                if bit {
                                    branch = all;
                                }
                            }
                            Projection::Independent { .. } => {
                                if let Some(packet) = &independent_packet {
                                    branch = packet.mask(independent_event);
                                } else {
                                    for lane in 0..lanes {
                                        if tape[lane * random_count + event] != 0 {
                                            branch |= 1 << lane;
                                        }
                                    }
                                }
                                independent_event += 1;
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
                                } else if let Some(id) = states.uniform(live) {
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
                                    let mut next_states = [None; 2];
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
                                        next_states[bit] = next;
                                        #[cfg(test)]
                                        states.reference_transition(mask, next);
                                        if next.is_none() {
                                            live &= !mask;
                                        }
                                    }
                                    states.finish_uniform(masks, next_states, live);
                                } else {
                                    let mut remaining = live;
                                    while remaining != 0 {
                                        let lane = remaining.trailing_zeros() as usize;
                                        remaining &= remaining - 1;
                                        let bit = f64::from_bits(tape[lane * random_count + event])
                                            >= self.cached_probability_zero(
                                                node,
                                                &m.pauli,
                                                states.lanes[lane],
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
                                            &mut states.lanes[lane],
                                        )?;
                                        if states.lanes[lane].is_none() {
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
            if !postselect || live != 0 {
                debug_assert_eq!(event, random_count);
                debug_assert_eq!(noise_event, plan.noise_event_count);
                debug_assert_eq!(independent_event, plan.independent_event_count);
                debug_assert_eq!(record, plan.measurement_count);
            }
            // When almost every row replays, scalar arithmetic avoids duplicated work.
            // This affects only execution strategy, never random-event or record order.
            #[cfg(test)]
            {
                self.last_packet_live = (live & all).count_ones() as usize;
                self.last_packet_live_mask = live & all;
            }
            if ((live | rejected) & all).count_ones() as usize <= lanes / 4 {
                self.pack_enabled = false;
            }
            output.packet_counts_reduced(live & all, logical);
            for lane in 0..lanes {
                if rejected >> lane & 1 != 0 {
                    continue;
                }
                if live >> lane & 1 != 0 {
                    output.packet_row(&records, lane);
                } else {
                    // Admission failed. Replay the same row's already drawn events;
                    // no quantum result is resampled and the caller RNG is untouched.
                    if matches!(output, BatchOutput::Counts { .. }) {
                        let mut random = CompactReplay::new(
                            &tape[lane * random_count..(lane + 1) * random_count],
                            noise_packet.as_deref(),
                            independent_packet.as_ref(),
                            lane,
                        );
                        let shot = self.row_with_random_kernel::<true, true>(sweep, &mut random)?;
                        output.row(shot);
                        debug_assert_eq!(random.cursor(), random_count);
                    } else {
                        if let Some(packet) = &noise_packet {
                            packet.restore_row(
                                &plan.random_kinds,
                                &mut tape[lane * random_count..(lane + 1) * random_count],
                                lane,
                            );
                        }
                        if let Some(packet) = &independent_packet {
                            packet.restore_row(
                                &plan.random_kinds,
                                &mut tape[lane * random_count..(lane + 1) * random_count],
                                lane,
                            );
                        }
                        let mut random = RowRandom::recorded(
                            &tape[lane * random_count..(lane + 1) * random_count],
                            &mut *rng,
                        );
                        let shot = self.row_with_random(sweep, &mut random)?;
                        output.row(shot);
                        debug_assert_eq!(random.cursor, random_count);
                    }
                }
            }
            Ok(())
        })();
        self.packet_x = x;
        self.packet_z = z;
        self.packet_tape = tape;
        self.packet_records = records;
        self.packet_noise_masks = noise_masks;
        self.packet_independent = independent_packet;
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
        self.sample_batch(
            shots,
            sweep,
            rng,
            &mut BatchOutput::Measurements(&mut measurements),
        )?;
        Ok(NearCliffordMeasurementBatch {
            shots,
            measurements_per_shot: self.plan.measurement_count,
            measurements,
        })
    }

    /// Counts all-zero raw detector shots and their XOR-folded raw observable parity.
    ///
    /// The observable index must occur in the compiled circuit, even for zero shots.
    /// No reference normalization is performed. Scalar and admission-fallback rows
    /// skip remaining physics after a nonzero detector, while consuming all remaining
    /// typed random events. Packed counts also reject lanes when a raw detector
    /// is nonzero, and stop suffix physics once no live lanes remain. Sampling
    /// preserves the random stream of [`Self::sample`] without batch output records.
    /// Rejected scalar suffixes do not perform physics or its fallible allocations.
    pub fn sample_postselected_counts(
        &mut self,
        shots: usize,
        observable_index: u32,
        rng: &mut impl Rng,
    ) -> Result<NearCliffordPostselectedCounts, String> {
        self.sample_postselected_counts_with_sweep(shots, observable_index, &[], rng)
    }

    /// The counts contract of [`Self::sample_postselected_counts`] with sweep controls.
    pub fn sample_postselected_counts_with_sweep(
        &mut self,
        shots: usize,
        observable_index: u32,
        sweep: &[bool],
        rng: &mut impl Rng,
    ) -> Result<NearCliffordPostselectedCounts, String> {
        if !self.plan.annotation_positions.iter().any(|&position| {
            matches!(&self.plan.operations[position],
            PlanOp::Annotation { observable: Some(index), .. } if *index == observable_index)
        }) {
            return Err(format!(
                "observable {observable_index} is absent from the compiled circuit"
            ));
        }
        // The optional immutable model is shared by cloned plans. Construction
        // occurs only on the first nonempty counts call, before drawing RNG. Its
        // cost belongs to that first call; a declined model keeps the original path.
        if shots != 0 {
            if let Some(model) = self
                .plan
                .linear_counts
                .get_or_init(|| LinearCountsPlan::build(self.plan, self.plan.counts_plan_budget))
            {
                if model
                    .output_count()
                    .checked_mul(size_of::<u64>())
                    .is_some_and(|bytes| bytes <= PACKET_BYTE_BUDGET)
                {
                    let admitted = self.counts_outputs.capacity() >= model.output_count()
                        || self
                            .counts_outputs
                            .try_reserve_exact(model.output_count() - self.counts_outputs.len())
                            .is_ok();
                    if admitted
                        && self
                            .counts_outputs
                            .capacity()
                            .checked_mul(size_of::<u64>())
                            .is_some_and(|bytes| bytes <= PACKET_BYTE_BUDGET)
                    {
                        self.counts_outputs.resize(model.output_count(), 0);
                        return Ok(model.sample(
                            self.plan,
                            &mut self.counts_outputs,
                            shots,
                            observable_index,
                            sweep,
                            rng,
                        ));
                    }
                }
            }
        }
        let mut counts = NearCliffordPostselectedCounts {
            attempted: shots,
            ..Default::default()
        };
        self.sample_batch(
            shots,
            sweep,
            rng,
            &mut BatchOutput::Counts {
                observable_index,
                counts: &mut counts,
            },
        )?;
        Ok(counts)
    }

    fn sample_batch(
        &mut self,
        shots: usize,
        sweep: &[bool],
        rng: &mut impl Rng,
        output: &mut BatchOutput<'_>,
    ) -> Result<(), String> {
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
        // Conditional scheduling can reduce the peak below four while its
        // sign-dependent coefficient transitions still exhaust the cache.
        let coherent_eligible = (self.plan.peak_active_rank >= 4
            || (shots >= 32 && self.plan.peak_active_rank != 0 && self.plan.noise_signs.is_some()))
            && (1usize << self.plan.peak_active_rank)
                .checked_mul(64 * 4 * size_of::<f64>())
                .is_some_and(|n| n <= COEFFICIENT_BYTE_BUDGET);
        let packed = shots >= 32
            && packet_bytes
                .and_then(|n| {
                    self.conditional_tape
                        .capacity()
                        .checked_mul(size_of::<u64>())
                        .and_then(|c| n.checked_add(c))
                })
                .is_some_and(|n| n <= PACKET_BYTE_BUDGET)
            && (coherent_eligible || (self.pack_enabled && self.cache.is_some()));
        if packed {
            resize_packet(&mut self.packet_x, self.plan.num_qubits)?;
            resize_packet(&mut self.packet_z, self.plan.num_qubits)?;
            resize_packet(&mut self.packet_records, self.plan.measurement_count)?;
            resize_packet(&mut self.packet_noise_masks, self.plan.noise_event_count)?;
            resize_packet(&mut self.packet_tape, 64 * self.plan.random_kinds.len())?;
            let retained = self.scalar_tape_other_bytes().and_then(|n| {
                self.packet_tape
                    .capacity()
                    .checked_mul(size_of::<u64>())
                    .and_then(|t| n.checked_add(t))
            });
            if retained.is_none_or(|n| n > PACKET_BYTE_BUDGET) {
                // Exact requests may round up. Drop optional buffers before
                // scalar fallback; the admitted conditional row stays available.
                self.packet_x = Vec::new();
                self.packet_z = Vec::new();
                self.packet_records = Vec::new();
                self.packet_noise_masks = Vec::new();
                self.packet_tape = Vec::new();
                self.packet_independent = None;
                for _ in 0..shots {
                    output.row(self.row_for_output(sweep, rng, output)?);
                }
                return Ok(());
            }
            if self.packet_independent.is_none() && self.plan.random_runs.is_some() {
                // Optional compact workspace counts actual retained capacities.
                // Overflow, admission or allocation failure leaves the original
                // run producer and complete-tape consumer available.
                let occupied = self
                    .packet_x
                    .capacity()
                    .checked_add(self.packet_z.capacity())
                    .and_then(|n| n.checked_add(self.packet_records.capacity()))
                    .and_then(|n| n.checked_add(self.packet_noise_masks.capacity()))
                    .and_then(|n| n.checked_add(self.packet_tape.capacity()))
                    .and_then(|n| n.checked_mul(size_of::<u64>()))
                    .and_then(|n| {
                        self.conditional_tape
                            .capacity()
                            .checked_mul(size_of::<u64>())
                            .and_then(|c| n.checked_add(c))
                    });
                self.packet_independent = occupied
                    .and_then(|n| PACKET_BYTE_BUDGET.checked_sub(n))
                    .and_then(|remaining| {
                        IndependentPacket::new(self.plan.independent_event_count, remaining)
                    });
            }
            // Preserve the existing full-tape admission and Independent priority.
            // This optional local buffer is never retained by the sampler, and is
            // dropped before scalar fallback can reconsider scalar tape admission.
            let mut noise_packet = if shots >= 64 && self.plan.random_runs.is_some() {
                self.scalar_tape_other_bytes()
                    .and_then(|n| {
                        self.packet_tape
                            .capacity()
                            .checked_mul(size_of::<u64>())
                            .and_then(|tape| n.checked_add(tape))
                    })
                    .and_then(|occupied| PACKET_BYTE_BUDGET.checked_sub(occupied))
                    .and_then(|remaining| {
                        NoisePacket::new(
                            &self.plan.random_kinds,
                            self.plan.noise_event_count,
                            remaining,
                        )
                    })
            } else {
                None
            };
            for offset in (0..shots).step_by(64) {
                let lanes = (shots - offset).min(64);
                let coherent = coherent_eligible && (!self.pack_enabled || self.cache.is_none());
                if coherent || (self.pack_enabled && self.cache.is_some()) {
                    // Tails retain the original fill/consumer without compact clearing.
                    let compact = if lanes == 64 {
                        noise_packet.as_mut()
                    } else {
                        None
                    };
                    self.packet(lanes, sweep, rng, output, coherent, compact)?;
                } else {
                    drop(noise_packet.take());
                    for _ in 0..lanes {
                        output.row(self.row_for_output(sweep, rng, output)?);
                    }
                }
            }
        } else {
            for _ in 0..shots {
                output.row(self.row_for_output(sweep, rng, output)?);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    // Literal S2 array bookkeeping. Keep these loops independent of the
    // production uniform scan/broadcast helpers and lazy-state hint.
    fn literal_uniform(lanes: &[Option<usize>; 64], live: u64) -> Option<usize> {
        let first = (0..64).find(|&lane| live & (1u64 << lane) != 0)?;
        let id = lanes[first]?;
        for lane in 0..64 {
            if live & (1u64 << lane) != 0 && lanes[lane] != Some(id) {
                return None;
            }
        }
        Some(id)
    }

    fn literal_transition(lanes: &mut [Option<usize>; 64], mask: u64, id: Option<usize>) {
        for lane in 0..64 {
            if mask & (1u64 << lane) != 0 {
                lanes[lane] = id;
            }
        }
    }

    fn check_lazy_against_literal(
        lazy: &mut PacketCacheStates,
        literal: &[Option<usize>; 64],
        live: u64,
    ) {
        // Decode before uniform() can install a reconvergence hint.
        for lane in 0..64 {
            if live & (1u64 << lane) != 0 {
                let actual = if let Some(id) = lazy.uniform {
                    Some(id)
                } else {
                    lazy.lanes[lane]
                };
                assert_eq!(actual, literal[lane], "lane={lane}; live={live:#x}");
            }
        }
        if lazy.uniform.is_none() {
            for lane in 0..64 {
                if live & (1u64 << lane) != 0 {
                    assert!(lazy.lanes[lane].is_some());
                }
            }
        }
        assert_eq!(lazy.uniform(live), literal_uniform(literal, live));
    }

    fn check_lazy_partition(pre_live: u64, first: u64, next: [Option<usize>; 2]) {
        assert_eq!(first & !pre_live, 0);
        let masks = [first, pre_live & !first];
        let mut lazy = PacketCacheStates::new(7);
        // Poison the lazy array to prove uniform hints hide stale entries.
        lazy.lanes.fill(Some(usize::MAX));
        let mut literal = [Some(7); 64];
        assert_eq!(lazy.uniform(pre_live), literal_uniform(&literal, pre_live));
        let mut live = pre_live;
        for bit in 0..2 {
            literal_transition(&mut literal, masks[bit], next[bit]);
            if next[bit].is_none() {
                live &= !masks[bit];
            }
        }
        lazy.finish_uniform(masks, next, live);
        check_lazy_against_literal(&mut lazy, &literal, live);
    }

    #[test]
    fn lazy_cached_states_match_literal_admission_and_tail_partitions() {
        // Exhaustive sparse live sets and their partitions for widths 0..=6.
        for width in 0..=6 {
            for pre_live in 0u64..(1u64 << width) {
                for first in 0u64..(1u64 << width) {
                    if first & !pre_live != 0 {
                        continue;
                    }
                    for next in [
                        [Some(0), Some(0)],
                        [Some(11), Some(11)],
                        [Some(11), Some(12)],
                        [Some(11), None],
                        [None, Some(12)],
                        [None, None],
                    ] {
                        check_lazy_partition(pre_live, first, next);
                    }
                }
            }
        }
        let tail63 = (1u64 << 63) - 1;
        let sparse = 1 | (1u64 << 31) | (1u64 << 63);
        for pre_live in [0, 1, 1u64 << 63, tail63, sparse, u64::MAX] {
            for first in [0, pre_live, pre_live & 0x5555_5555_5555_5555] {
                for next in [
                    [Some(0), Some(0)],
                    [Some(11), Some(11)],
                    [Some(11), Some(12)],
                    [Some(11), None],
                    [None, Some(12)],
                    [None, None],
                ] {
                    check_lazy_partition(pre_live, first, next);
                }
            }
        }
    }

    #[test]
    fn lazy_cached_states_reconverge_then_split_without_reading_stale_lanes() {
        let live = 1 | (1u64 << 17) | (1u64 << 63);
        let masks = [1 | (1u64 << 63), 1u64 << 17];
        let mut lazy = PacketCacheStates::new(7);
        let mut literal = [Some(7); 64];
        lazy.finish_uniform(masks, [Some(11), Some(12)], live);
        literal_transition(&mut literal, masks[0], Some(11));
        literal_transition(&mut literal, masks[1], Some(12));
        check_lazy_against_literal(&mut lazy, &literal, live);
        assert!(lazy.uniform.is_none());
        // Emulate unchanged diverse-lane transitions to one actual ID.
        for lane in [0, 17, 63] {
            lazy.lanes[lane] = Some(0);
            literal[lane] = Some(0);
        }
        check_lazy_against_literal(&mut lazy, &literal, live);
        assert_eq!(lazy.uniform, Some(0));
        lazy.lanes.fill(Some(usize::MAX));
        lazy.finish_uniform(masks, [Some(21), Some(22)], live);
        literal_transition(&mut literal, masks[0], Some(21));
        literal_transition(&mut literal, masks[1], Some(22));
        check_lazy_against_literal(&mut lazy, &literal, live);
        assert!(lazy.uniform.is_none());
        // One failed branch leaves a uniform surviving subset.
        lazy.finish_uniform(masks, [Some(23), Some(23)], live);
        literal_transition(&mut literal, live, Some(23));
        literal_transition(&mut literal, masks[0], None);
        literal_transition(&mut literal, masks[1], Some(24));
        lazy.finish_uniform(masks, [None, Some(24)], masks[1]);
        check_lazy_against_literal(&mut lazy, &literal, masks[1]);
        assert_eq!(lazy.uniform, Some(24));
        // Finally all admitted survivors die. No stale entry can revive them.
        lazy.finish_uniform([0, masks[1]], [None, None], 0);
        literal_transition(&mut literal, masks[1], None);
        check_lazy_against_literal(&mut lazy, &literal, 0);
        assert!(lazy.uniform.is_none());
    }

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
    fn both_arithmetic_policies_preserve_dense_conditional_complex_density() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
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
                        .map(|_| {
                            ComplexAmp::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0))
                        })
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
                            flags[3] |=
                                matches!(m.projection, Projection::Active { offset: true, .. });
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
                                    annotation_positions: Vec::new(),
                                    num_qubits: 3,
                                    measurement_count: 1,
                                    peak_active_rank: axes.len(),
                                    prefix_len: 0,
                                    initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
                                    random_kinds: Vec::new(),
                                    random_runs: None,
                                    noise_spans: None,
                                    noise_signs: None,
                                    noise_event_count: 0,
                                    independent_event_count: 0,
                                    scalar_basis: None,
                                    linear_counts: Arc::new(OnceLock::new()),
                                    counts_plan_budget: 0,
                                    rotation_arithmetic: policy,
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
                                            (a.re - e.re).abs() < 3e-12
                                                && (a.im - e.im).abs() < 3e-12,
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
    }

    #[test]
    fn both_arithmetic_policies_match_dense_pauli_rotation_and_expansion() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic("I 2\n", policy)
                .unwrap();
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
                        .map(|_| {
                            ComplexAmp::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0))
                        })
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
                    sampler.coefficients = coefficients.clone();
                    sampler.x[0] = frame_x;
                    sampler.z[0] = frame_z;
                    sampler.rotate(&p, expand, dagger).unwrap();
                    let anti = if p.physical.anticommutes(&sampler.x, &sampler.z) {
                        u64::MAX
                    } else {
                        0
                    };
                    // Packet lanes are compared directly to the physical dense oracle,
                    // rather than taking the scalar candidate as physics evidence.
                    let mut packet = CoherentPacket::default();
                    for lanes in [1, 7, 32, 64] {
                        packet.reset(&coefficients, lanes, 3).unwrap();
                        packet
                            .rotate_with_arithmetic(&p, expand, dagger, anti, policy)
                            .unwrap();
                        for lane in 0..lanes {
                            let mut lane_state = (0..packet.len)
                                .map(|i| {
                                    ComplexAmp::new(
                                        packet.re[i * lanes + lane],
                                        packet.im[i * lanes + lane],
                                    )
                                })
                                .collect::<Vec<_>>();
                            dense_frame(&mut lane_state, frame_x, frame_z);
                            for i in 0..8 {
                                for j in 0..8 {
                                    let a = lane_state[i] * lane_state[j].conj();
                                    let e = expected[i] * expected[j].conj();
                                    assert!(
                                        (a.re - e.re).abs() < 3e-12 && (a.im - e.im).abs() < 3e-12,
                                        "policy={policy:?}; lanes={lanes}; lane={lane}; expand={expand}"
                                    );
                                }
                            }
                        }
                    }
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
    fn coherent_compact_packets_keep_raw_rows_and_rng_across_tails() {
        let mut text = String::from(
            "H 0 1 2 3\nT 0 1 2 3\nDEPOLARIZE2(0.23) 0 1\nMPP(0.37) !X0*Y1*X2*Y3\nMRX(0.41) !0\nCX rec[-1] 3\nT_DAG 3\nMRY 1\nMY 2\nMX 3\nM 0 1 2 3\n",
        );
        text.push_str("REPEAT 129 {\nR 4\nH 4\nM 4\n}\n");
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        assert!(plan.peak_active_rank >= 4);
        assert!(plan.independent_event_count >= 129);
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut flat = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut a = StdRng::seed_from_u64(461);
        let mut b = a.clone();
        for shots in [64, 1, 31, 32, 63, 65, 129, 64] {
            let expected = scalar.sample(shots, &mut a).unwrap();
            let actual = flat.sample_measurements_u8(shots, &mut b).unwrap();
            assert_eq!(
                actual.measurements,
                expected
                    .iter()
                    .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                    .collect::<Vec<_>>()
            );
            assert!(flat.packet_independent.is_some());
            assert_eq!(flat.coefficient_cache_reserved_bytes(), 0);
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn compact_noise_restores_mixed_and_all_cache_dead_rows_before_scalar_fallback() {
        // Noise and Independent events on disjoint dormant qubits do not change
        // the two active projections used by the existing partial-replay witness.
        let mut text = String::from("H 0 1\nT 0 1\nCX 0 1\nMY 0\nMX 1\n");
        text.push_str("REPEAT 33 {\nX_ERROR(0.01) 2\nDEPOLARIZE1(0.01) 2\nDEPOLARIZE2(0.01) 2 3\nY_ERROR(0.37) 2\nMR(0.001) 2\nR 4\nH 4\nM 4\n}\nDEPOLARIZE2(0) 2 3\nDEPOLARIZE2(1) 2 3\nMR(0.37) 2\n");
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            assert!(plan.noise_event_count >= MIN_SCALAR_PREPARED_NOISE_EVENTS);
            // Scheduled independent events end the deterministic prefix before
            // the two T expansions. Their 2- and 4-coefficient states cost
            // 288 + 320 bytes, then one 2-coefficient projection costs 288.
            assert_eq!(plan.initial_coefficients.len(), 1);
            assert_eq!(
                CoefficientCache::state_charge(&[ComplexAmp::new(0., 0.); 2]),
                Some(288)
            );
            assert_eq!(
                CoefficientCache::state_charge(&[ComplexAmp::new(0., 0.); 4]),
                Some(320)
            );
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for extra in [0, 896] {
                let mut packed = plan
                    .prepare_sampler_with_cache_budget(initial + extra)
                    .unwrap();
                assert!(packed.cache.is_some());
                let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(1606);
                let mut b = a.clone();
                let expected = scalar.sample(64, &mut a).unwrap();
                let actual = packed.sample_measurements_u8(64, &mut b).unwrap();
                assert_eq!(
                    actual.measurements,
                    expected
                        .iter()
                        .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                        .collect::<Vec<_>>()
                );
                if extra == 0 {
                    assert_eq!(packed.last_packet_live, 0);
                    assert!(!packed.pack_enabled);
                } else {
                    assert!(
                        packed.last_packet_live > 0 && packed.last_packet_live < 64,
                        "{arithmetic:?} extra={extra} live={} initial={initial} reserved={}",
                        packed.last_packet_live,
                        packed.coefficient_cache_reserved_bytes()
                    );
                }
                assert_eq!(packed.coefficient_cache_reserved_bytes(), initial + extra);
                assert!(packed.packet_independent.is_some());
                assert_eq!(a.next_u64(), b.next_u64());
                // Subsequent calls keep the original prepared-scalar admission.
                for shots in [63, 64] {
                    let expected = scalar.sample(shots, &mut a).unwrap();
                    let actual = packed.sample_measurements_u8(shots, &mut b).unwrap();
                    assert_eq!(
                        actual.measurements,
                        expected
                            .iter()
                            .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                            .collect::<Vec<_>>()
                    );
                    if extra == 0 {
                        assert!(packed.last_scalar_prepared);
                    }
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            // ONE call must drop its local Noise allocation after the first
            // all-dead packet, before preparing scalar rows for 64 + 1 shots.
            let mut packed = plan.prepare_sampler_with_cache_budget(initial).unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(1607);
            let mut b = a.clone();
            let expected = scalar.sample(129, &mut a).unwrap();
            let actual = packed.sample_measurements_u8(129, &mut b).unwrap();
            assert_eq!(
                actual.measurements,
                expected
                    .iter()
                    .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                    .collect::<Vec<_>>()
            );
            assert_eq!(packed.last_packet_live, 0);
            assert!(!packed.pack_enabled);
            assert!(packed.last_scalar_prepared);
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn compact_packet_restores_cache_dead_rows_after_many_independent_events() {
        let mut text = String::from("H 0 1\nT 0 1\nCX 0 1\nMY 0\nMX 1\n");
        text.push_str("REPEAT 129 {\nR 2\nH 2\nM 2\n}\n");
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        assert!(plan.independent_event_count >= 129);
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        for shots in [64, 65, 127, 129] {
            let mut packed = plan
                .prepare_sampler_with_cache_budget(initial + 896)
                .unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(583);
            let mut b = a.clone();
            // Fresh first packet gives an actual mixed-live replay witness;
            // later tails may switch to scalar and must not reuse stale counts.
            let mut expected = scalar
                .sample_measurements_u8(64, &mut a)
                .unwrap()
                .measurements;
            let mut actual = packed
                .sample_measurements_u8(64, &mut b)
                .unwrap()
                .measurements;
            assert!(packed.packet_independent.is_some());
            assert!(
                packed.last_packet_live > 0 && packed.last_packet_live < 64,
                "live {} initial {}",
                packed.last_packet_live,
                initial
            );
            assert_eq!(packed.coefficient_cache_reserved_bytes(), initial + 896);
            expected.extend(
                scalar
                    .sample_measurements_u8(shots - 64, &mut a)
                    .unwrap()
                    .measurements,
            );
            actual.extend(
                packed
                    .sample_measurements_u8(shots - 64, &mut b)
                    .unwrap()
                    .measurements,
            );
            assert_eq!(expected, actual);
            assert_eq!(a.next_u64(), b.next_u64());
        }
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

    fn classical_schedule_pauli(width: usize, x: &[usize], z: &[usize]) -> PackedPauli {
        let mut p = Pauli::identity(width);
        for &q in x {
            p.x[q] = true;
        }
        for &q in z {
            p.z[q] = true;
        }
        p.phase = (x.iter().filter(|q| z.contains(q)).count() % 4) as u8;
        PackedPauli::new(&p)
    }

    fn classical_schedule_measure(
        pauli: PackedPauli,
        record: Option<usize>,
        reset: Option<PackedPauli>,
    ) -> TapeOp {
        TapeOp::Measure {
            pauli,
            record,
            inverted: record.is_some(),
            readout: if record.is_some() { 0.37 } else { 0. },
            reset,
        }
    }

    fn classical_schedule_record_position(tape: &[TapeOp], record: Option<usize>) -> usize {
        tape.iter()
            .position(|op| matches!(op, TapeOp::Measure { record: found, .. } if *found == record))
            .unwrap()
    }

    #[test]
    fn scheduled_commuting_record_and_sweep_feedback_keep_earlier_producer_and_metadata() {
        for condition in [Condition::Record(0), Condition::Sweep(3)] {
            let mut tape = vec![
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[1]), Some(0), None),
                TapeOp::Feedback {
                    condition,
                    pauli: classical_schedule_pauli(2, &[], &[1]),
                },
                classical_schedule_measure(
                    classical_schedule_pauli(2, &[], &[0]),
                    Some(1),
                    Some(classical_schedule_pauli(2, &[0], &[])),
                ),
            ];
            let mut reserved = 19;
            schedule_measurements(&mut tape, &mut reserved).unwrap();
            let reader = tape
                .iter()
                .position(|op| matches!(op, TapeOp::Feedback { .. }))
                .unwrap();
            assert!(classical_schedule_record_position(&tape, Some(0)) < reader);
            let later = classical_schedule_record_position(&tape, Some(1));
            assert!(later < reader);
            let TapeOp::Measure {
                inverted,
                readout,
                reset,
                ..
            } = &tape[later]
            else {
                unreachable!()
            };
            assert!(*inverted);
            assert_eq!(*readout, 0.37);
            assert_eq!(reset.as_ref().unwrap().x, [1]);
            assert_eq!(reserved, 19);
        }
    }

    #[test]
    fn scheduled_recordless_reset_can_cross_commuting_record_feedback() {
        let mut tape = vec![
            classical_schedule_measure(classical_schedule_pauli(2, &[], &[1]), Some(0), None),
            TapeOp::Feedback {
                condition: Condition::Record(0),
                pauli: classical_schedule_pauli(2, &[], &[1]),
            },
            classical_schedule_measure(
                classical_schedule_pauli(2, &[], &[0]),
                None,
                Some(classical_schedule_pauli(2, &[0], &[])),
            ),
        ];
        schedule_measurements(&mut tape, &mut 0).unwrap();
        let reader = tape
            .iter()
            .position(|op| matches!(op, TapeOp::Feedback { .. }))
            .unwrap();
        assert!(classical_schedule_record_position(&tape, Some(0)) < reader);
        assert!(classical_schedule_record_position(&tape, None) < reader);
    }

    #[test]
    fn scheduled_feedback_rejects_anticommuting_measurement_or_reset() {
        for (feedback, reset) in [
            (classical_schedule_pauli(2, &[0], &[]), None),
            (
                classical_schedule_pauli(2, &[], &[0]),
                Some(classical_schedule_pauli(2, &[0], &[])),
            ),
        ] {
            let mut tape = vec![
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[1]), Some(0), None),
                TapeOp::Feedback {
                    condition: Condition::Record(0),
                    pauli: feedback,
                },
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[0]), Some(1), reset),
            ];
            schedule_measurements(&mut tape, &mut 0).unwrap();
            assert!(matches!(&tape[1], TapeOp::Feedback { .. }));
            assert_eq!(classical_schedule_record_position(&tape, Some(1)), 2);
        }
    }

    #[test]
    fn scheduled_feedback_retains_equal_and_later_record_index_barriers() {
        // Hand-built invalid-source tapes exercise the defensive admission guard,
        // rather than relying only on the parser's earlier-record restriction.
        for input in [1, 2] {
            let mut tape = vec![
                TapeOp::Feedback {
                    condition: Condition::Record(input),
                    pauli: classical_schedule_pauli(2, &[], &[1]),
                },
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[0]), Some(1), None),
            ];
            schedule_measurements(&mut tape, &mut 0).unwrap();
            assert!(matches!(&tape[0], TapeOp::Feedback { .. }));
            assert_eq!(classical_schedule_record_position(&tape, Some(1)), 1);
        }
    }

    #[test]
    fn scheduled_annotations_allow_empty_duplicate_and_recordless_crossings() {
        for (offsets, record) in [
            (Vec::new(), Some(1)),
            (vec![0, 0], Some(1)),
            (vec![0, 0], None),
        ] {
            let expected_offsets = offsets.clone();
            let mut tape = vec![
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[1]), Some(0), None),
                TapeOp::Annotation {
                    offsets,
                    observable: Some(7),
                },
                classical_schedule_measure(
                    classical_schedule_pauli(2, &[], &[0]),
                    record,
                    Some(classical_schedule_pauli(2, &[0], &[])),
                ),
            ];
            schedule_measurements(&mut tape, &mut 0).unwrap();
            let reader = tape
                .iter()
                .position(|op| matches!(op, TapeOp::Annotation { .. }))
                .unwrap();
            assert!(classical_schedule_record_position(&tape, Some(0)) < reader);
            assert!(classical_schedule_record_position(&tape, record) < reader);
            let TapeOp::Annotation {
                offsets,
                observable,
            } = &tape[reader]
            else {
                unreachable!()
            };
            assert_eq!(offsets, &expected_offsets);
            assert_eq!(*observable, Some(7));
        }
    }

    #[test]
    fn scheduled_annotations_retain_equal_and_later_record_index_barriers() {
        for offsets in [vec![1], vec![2], vec![0, 2]] {
            let mut tape = vec![
                TapeOp::Annotation {
                    offsets,
                    observable: None,
                },
                classical_schedule_measure(classical_schedule_pauli(2, &[], &[0]), Some(1), None),
            ];
            schedule_measurements(&mut tape, &mut 0).unwrap();
            assert!(matches!(&tape[0], TapeOp::Annotation { .. }));
            assert_eq!(classical_schedule_record_position(&tape, Some(1)), 1);
        }
    }

    #[test]
    fn scheduled_annotation_scan_budget_accepts_exact_boundary_and_stops_next_attempt() {
        // 64 words * 62_500 offsets = exactly 4_000_000 units. The next
        // commuting producer comparison must stop; one extra offset stops
        // before the annotation swap. Each offsets Vec is only about 0.5 MiB.
        for (count, expected_position) in [(62_500, 1), (62_501, 2)] {
            let mut tape = vec![
                classical_schedule_measure(
                    classical_schedule_pauli(4096, &[], &[1]),
                    Some(0),
                    None,
                ),
                TapeOp::Annotation {
                    offsets: vec![0; count],
                    observable: None,
                },
                classical_schedule_measure(
                    classical_schedule_pauli(4096, &[], &[0]),
                    Some(1),
                    None,
                ),
            ];
            let mut reserved = 23;
            schedule_measurements(&mut tape, &mut reserved).unwrap();
            assert_eq!(classical_schedule_record_position(&tape, Some(0)), 0);
            assert_eq!(
                classical_schedule_record_position(&tape, Some(1)),
                expected_position
            );
            assert_eq!(reserved, 23);
        }
    }

    #[test]
    fn scheduled_feedback_index_cost_stops_safely_when_only_one_word_scan_remains() {
        // After the annotation swap, 64 or 128 units remain. Feedback costs
        // 2 * 64, including its index guard: only the second case may swap.
        for (count, expected_position) in [(62_499, 2), (62_498, 1)] {
            let mut tape = vec![
                classical_schedule_measure(
                    classical_schedule_pauli(4096, &[], &[1]),
                    Some(0),
                    None,
                ),
                TapeOp::Feedback {
                    condition: Condition::Record(0),
                    pauli: classical_schedule_pauli(4096, &[], &[1]),
                },
                TapeOp::Annotation {
                    offsets: vec![0; count],
                    observable: Some(2),
                },
                classical_schedule_measure(
                    classical_schedule_pauli(4096, &[], &[0]),
                    Some(1),
                    None,
                ),
            ];
            schedule_measurements(&mut tape, &mut 0).unwrap();
            assert_eq!(classical_schedule_record_position(&tape, Some(0)), 0);
            assert_eq!(
                classical_schedule_record_position(&tape, Some(1)),
                expected_position
            );
        }
    }

    #[test]
    fn compiled_positive_tiny_projection_is_normalized_without_a_cutoff() {
        let plan = CompiledNearCliffordExecutor {
            operations: Vec::new(),
            annotation_positions: Vec::new(),
            num_qubits: 1,
            measurement_count: 0,
            peak_active_rank: 1,
            prefix_len: 0,
            rotation_arithmetic: CompiledRotationArithmetic::Strict,
            initial_coefficients: Arc::new(vec![ComplexAmp::new(1., 0.)]),
            random_kinds: Vec::new(),
            random_runs: None,
            noise_spans: None,
            noise_signs: None,
            noise_event_count: 0,
            independent_event_count: 0,
            scalar_basis: None,
            linear_counts: Arc::new(OnceLock::new()),
            counts_plan_budget: 0,
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

#[cfg(test)]
mod row_random_log_cache_tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    // Frozen old scalar policy. It deliberately recomputes log(1-p) at every
    // renewal and contains no cache or calls to the new RowRandom implementation.
    struct LegacyRowRandom<'a, R> {
        tape: Option<&'a [u64]>,
        cursor: usize,
        rng: &'a mut R,
        bit_word: u64,
        bits_left: u8,
        noise_probability: u64,
        noise_skip: Option<usize>,
    }
    impl<'a, R: Rng> LegacyRowRandom<'a, R> {
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
                    RandomKind::Active => self.rng.r#gen::<f64>().to_bits(),
                    RandomKind::Noise {
                        probability,
                        choices,
                    } => {
                        if !near_noise_occurs(probability, self.rng) {
                            0
                        } else if choices == 1 {
                            1
                        } else {
                            (self.rng.gen_range(0..choices) + 1) as u64
                        }
                    }
                }
            }
        }
    }

    fn noise(probability: f64, choices: usize) -> RandomKind {
        RandomKind::Noise {
            probability,
            choices,
        }
    }

    fn assert_same_state(legacy: &LegacyRowRandom<'_, StdRng>, cached: &RowRandom<'_, StdRng>) {
        assert_eq!(legacy.cursor, cached.cursor);
        assert_eq!(legacy.bit_word, cached.bit_word);
        assert_eq!(legacy.bits_left, cached.bits_left);
        assert_eq!(legacy.noise_probability, cached.noise_probability);
        assert_eq!(legacy.noise_skip, cached.noise_skip);
        assert_eq!(
            (*legacy.rng).clone().next_u64(),
            (*cached.rng).clone().next_u64()
        );
        if cached.noise_log_failure != 0. {
            let probability = f64::from_bits(cached.noise_probability);
            assert_eq!(
                cached.noise_log_failure.to_bits(),
                (-probability).ln_1p().to_bits()
            );
        }
    }

    fn event_schedule() -> Vec<RandomKind> {
        let mut kinds = Vec::new();
        for (kind, count) in [
            (RandomKind::Independent, 63),
            (RandomKind::Active, 2),
            (RandomKind::Independent, 2),
            (noise(0.01, 1), 4097),
            (noise(0.01, 3), 129),
            (noise(0.37, 15), 3),
            (RandomKind::Active, 3),
            (noise(0., 1), 5),
            (noise(-0., 15), 2),
            (noise(1., 1), 3),
            (noise(1., 15), 9),
            (noise(0.01, 15), 129),
            (RandomKind::Independent, 65),
            (noise(1e-12, 3), 130),
            (noise(f64::from_bits(1e-12f64.to_bits() - 1), 15), 5),
            (noise(1e-12, 1), 65),
            (noise(0.001, 15), 2049),
            (noise(0.001, 1), 129),
            (noise(f64::from_bits(0.01f64.to_bits() + 1), 3), 7),
            (noise(0.01, 15), 1025),
            (RandomKind::Independent, 129),
        ] {
            kinds.extend(std::iter::repeat_n(kind, count));
        }
        kinds
    }

    #[test]
    fn scalar_cache_matches_frozen_old_policy_after_every_typed_call() {
        let kinds = event_schedule();
        for seed in [0, 1, 63, 64, 65, 2415] {
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            for _row in 0..3 {
                let mut legacy = LegacyRowRandom::live(&mut a);
                let mut cached = RowRandom::live(&mut b);
                for &kind in &kinds {
                    assert_eq!(legacy.draw(kind), cached.draw(kind));
                    assert_same_state(&legacy, &cached);
                }
            }
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn prepared_scalar_switches_match_frozen_old_typed_calls() {
        let kinds = event_schedule();
        let chunks = [1, 63, 2, 64, 65, 129, 7, 3];
        for seed in [0, 1, 65, 2415] {
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            let mut legacy = LegacyRowRandom::live(&mut a);
            let mut cached = RowRandom::live(&mut b);
            let mut offset = 0;
            let mut chunk = 0;
            while offset < kinds.len() {
                let end = (offset + chunks[chunk % chunks.len()]).min(kinds.len());
                let events = &kinds[offset..end];
                let expected = events
                    .iter()
                    .map(|&kind| legacy.draw(kind))
                    .collect::<Vec<_>>();
                let actual = if chunk % 2 == 0 {
                    let mut actual = vec![u64::MAX; events.len()];
                    RandomRunPlan::build(events, PLAN_BYTE_BUDGET)
                        .unwrap()
                        .fill_row(&mut cached, events, &mut actual);
                    actual
                } else {
                    events
                        .iter()
                        .map(|&kind| cached.draw(kind))
                        .collect::<Vec<_>>()
                };
                assert_eq!(expected, actual);
                assert_same_state(&legacy, &cached);
                offset = end;
                chunk += 1;
            }
            drop(legacy);
            drop(cached);
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn zero_skip_and_same_probability_survive_direct_noise_and_choice_changes() {
        let p: f64 = 0.01;
        let mut a = StdRng::seed_from_u64(81);
        let mut b = a.clone();
        let mut legacy = LegacyRowRandom::live(&mut a);
        let mut cached = RowRandom::live(&mut b);
        // Hand-seeded legacy state remains a supported test fixture: a scalar
        // renewal can initialize the cache lazily without altering the carry.
        legacy.noise_probability = p.to_bits();
        cached.noise_probability = p.to_bits();
        legacy.noise_skip = Some(3);
        cached.noise_skip = Some(3);
        let skipped = [noise(p, 1), noise(p, 3), noise(p, 15)];
        let expected = skipped.map(|kind| legacy.draw(kind));
        let mut actual = [u64::MAX; 3];
        RandomRunPlan::build(&skipped, PLAN_BYTE_BUDGET)
            .unwrap()
            .fill_row(&mut cached, &skipped, &mut actual);
        assert_eq!(expected, actual);
        assert_eq!(cached.noise_skip, Some(0));
        let log = cached.noise_log_failure.to_bits();
        for kind in [
            noise(0.37, 15),
            noise(0., 1),
            noise(1., 15),
            RandomKind::Active,
            RandomKind::Independent,
        ] {
            assert_eq!(legacy.draw(kind), cached.draw(kind));
            assert_same_state(&legacy, &cached);
            assert_eq!(cached.noise_skip, Some(0));
            assert_eq!(cached.noise_log_failure.to_bits(), log);
        }
        assert_eq!(legacy.draw(noise(p, 15)), cached.draw(noise(p, 15)));
        assert_eq!(cached.noise_skip, None); // Success renews only on the next event.
        assert_eq!(legacy.draw(noise(p, 3)), cached.draw(noise(p, 3)));
        assert_same_state(&legacy, &cached);
        assert_eq!(cached.noise_log_failure.to_bits(), log);
        assert_eq!(legacy.draw(noise(0.001, 15)), cached.draw(noise(0.001, 15)));
        assert_same_state(&legacy, &cached);
        assert_eq!(
            cached.noise_log_failure.to_bits(),
            (-0.001f64).ln_1p().to_bits()
        );
    }

    #[test]
    fn empty_prepared_and_recorded_replay_do_not_modify_cached_state_or_rng() {
        let kinds = [
            noise(0.01, 15),
            RandomKind::Active,
            RandomKind::Independent,
            noise(0.001, 3),
            noise(0.37, 15),
        ];
        let mut a = StdRng::seed_from_u64(2415);
        let mut b = a.clone();
        let row = {
            let mut legacy = LegacyRowRandom::live(&mut a);
            kinds.map(|kind| legacy.draw(kind))
        };
        {
            let mut legacy = LegacyRowRandom::live(&mut b);
            for &kind in &kinds {
                legacy.draw(kind);
            }
        }
        assert_eq!(a.clone().next_u64(), b.clone().next_u64());
        let mut legacy = LegacyRowRandom::recorded(&row, &mut a);
        let mut cached = RowRandom::recorded(&row, &mut b);
        cached.noise_probability = 0.01f64.to_bits();
        cached.noise_log_failure = (-0.01f64).ln_1p();
        cached.noise_skip = Some(0);
        legacy.noise_probability = cached.noise_probability;
        legacy.noise_skip = cached.noise_skip;
        let before = cached.noise_log_failure.to_bits();
        RandomRunPlan::build(&[], PLAN_BYTE_BUDGET)
            .unwrap()
            .fill_row(&mut cached, &[], &mut []);
        assert_eq!(cached.noise_log_failure.to_bits(), before);
        for kind in kinds {
            assert_eq!(legacy.draw(kind), cached.draw(kind));
            assert_same_state(&legacy, &cached);
            assert_eq!(cached.noise_log_failure.to_bits(), before);
            assert_eq!(cached.noise_skip, Some(0));
        }
        assert_eq!(cached.cursor, kinds.len());
        legacy.cursor = 0;
        cached.cursor = 0;
        let expected = kinds.map(|kind| legacy.draw(kind));
        let mut actual = [u64::MAX; 5];
        RandomRunPlan::build(&kinds, PLAN_BYTE_BUDGET)
            .unwrap()
            .fill_row(&mut cached, &kinds, &mut actual);
        assert_eq!(expected, actual);
        assert_same_state(&legacy, &cached);
        assert_eq!(cached.noise_log_failure.to_bits(), before);
        assert_eq!(cached.noise_skip, Some(0));
        drop(legacy);
        drop(cached);
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage() {
        assert!(
            size_of::<RowRandom<'static, StdRng>>()
                <= size_of::<LegacyRowRandom<'static, StdRng>>() + size_of::<f64>()
        );
    }
}

#[cfg(test)]
mod scalar_prepared_tape_tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    pub(super) fn draw_frozen_packet_rows(
        kinds: &[RandomKind],
        lanes: usize,
        rng: &mut impl Rng,
    ) -> Vec<Vec<u64>> {
        (0..lanes)
            .map(|_| {
                let mut row = FrozenScalarRowRandom::live(&mut *rng);
                kinds.iter().map(|&kind| row.draw(kind)).collect()
            })
            .collect()
    }

    // Exact baseline live typed policy, captured before the new row strategy.
    // It never calls production RowRandom or the prepared run implementation.
    struct FrozenScalarRowRandom<'a, R> {
        tape: Option<&'a [u64]>,
        cursor: usize,
        rng: &'a mut R,
        bit_word: u64,
        bits_left: u8,
        noise_probability: u64,
        // Zero is an uninitialized sentinel: every valid sparse log(1-p) is finite and negative.
        noise_log_failure: f64,
        noise_skip: Option<usize>,
    }
    impl<'a, R: Rng> FrozenScalarRowRandom<'a, R> {
        fn live(rng: &'a mut R) -> Self {
            Self {
                tape: None,
                cursor: 0,
                rng,
                bit_word: 0,
                bits_left: 0,
                noise_probability: 0,
                noise_log_failure: 0.,
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
                            self.noise_log_failure = 0.;
                        }
                        let skip = self.noise_skip.get_or_insert_with(|| {
                            let u = self.rng.r#gen::<f64>();
                            // Cache only the deterministic denominator. Draw expression,
                            // typed uniform and exact no-prefetch boundaries are unchanged.
                            if self.noise_log_failure == 0. {
                                self.noise_log_failure = (-probability).ln_1p();
                            }
                            ((-u).ln_1p() / self.noise_log_failure).floor() as usize
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
                    RandomKind::Active => self.rng.r#gen::<f64>().to_bits(),
                    RandomKind::Noise {
                        probability,
                        choices,
                    } => {
                        if !near_noise_occurs(probability, self.rng) {
                            0
                        } else if choices == 1 {
                            1
                        } else {
                            (self.rng.gen_range(0..choices) + 1) as u64
                        }
                    }
                }
            }
        }
    }

    fn mixed_plan() -> CompiledNearCliffordExecutor {
        let text = "H 0 1 2 3\nT 0 1 2 3\nREPEAT 129 {\nX_ERROR(0.001) 0 4 0 4\nDEPOLARIZE1(0.001) 4\nDEPOLARIZE2(0.001) 0 1\nR 4\nH 4\nMR(0.001) !4\nCX rec[-1] 0\n}\nDEPOLARIZE1(0) 1\nX_ERROR(1) 4\nDEPOLARIZE2(1) 0 1\nY_ERROR(0.37) 2\nMPP(0.007) !Y0*X1 X0*!Y1\nMRX(0.37) !0\nCX rec[-1] 3\nCX sweep[1] 2\nT_DAG 1\nMY 2\nMX 3\nM(1) 0 1 4\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(2) rec[-3]\n";
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        assert!(plan.noise_event_count >= 128);
        assert!(plan.independent_event_count >= 129);
        assert!(plan.random_runs.is_some());
        plan
    }

    fn without_runs(plan: &CompiledNearCliffordExecutor) -> CompiledNearCliffordExecutor {
        let mut baseline = plan.clone();
        baseline.random_runs = None; // Same quantum plan, explicit original live scalar policy.
        baseline
    }

    fn flatten(shots: &[NearCliffordShot]) -> Vec<u8> {
        shots
            .iter()
            .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
            .collect()
    }

    #[test]
    fn single_row_prepared_values_and_replay_match_frozen_old_typed_policy() {
        let plan = mixed_plan();
        for seed in [0, 1, 63, 65, 2415] {
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            for _row in 0..3 {
                let expected = {
                    let mut old = FrozenScalarRowRandom::live(&mut a);
                    plan.random_kinds
                        .iter()
                        .map(|&kind| old.draw(kind))
                        .collect::<Vec<_>>()
                };
                let mut actual = vec![u64::MAX; plan.random_kinds.len()];
                plan.random_runs.as_ref().unwrap().fill_row(
                    &mut RowRandom::live(&mut b),
                    &plan.random_kinds,
                    &mut actual,
                );
                assert_eq!(actual, expected);
                assert_eq!(a.clone().next_u64(), b.clone().next_u64());
                let mut old = FrozenScalarRowRandom::recorded(&expected, &mut a);
                let mut replay = RowRandom::recorded(&actual, &mut b);
                for &kind in &plan.random_kinds {
                    assert_eq!(old.draw(kind), replay.draw(kind));
                }
                assert_eq!(old.cursor, replay.cursor);
            }
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn structured_prepared_rows_match_same_plan_old_scalar_across_cache_and_call_sizes() {
        let plan = mixed_plan();
        let baseline = without_runs(&plan);
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        let sweep = [false, true];
        for cache in [0, 1, initial + 288, DEFAULT_CACHE_BYTE_BUDGET] {
            for shots in [0, 1, 31, 32, 64, 65, 129] {
                let mut a = StdRng::seed_from_u64(3715);
                let mut b = a.clone();
                let mut old = baseline.prepare_sampler_with_cache_budget(cache).unwrap();
                let mut prepared = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                let expected = old.sample_with_sweep(shots, &sweep, &mut a).unwrap();
                let actual = prepared.sample_with_sweep(shots, &sweep, &mut b).unwrap();
                assert_eq!(actual, expected); // Includes annotation fields, not only bits.
                assert!(!old.last_scalar_prepared);
                assert_eq!(prepared.last_scalar_prepared, shots != 0);
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }

    #[test]
    fn flat_and_split_calls_match_old_scalar_not_a_new_packet_oracle() {
        let plan = mixed_plan();
        let baseline = without_runs(&plan);
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        for cache in [0, initial + 288, DEFAULT_CACHE_BYTE_BUDGET] {
            for sweep in [vec![], vec![false, true]] {
                for shots in [0, 1, 31, 32, 64, 65, 129] {
                    let mut a = StdRng::seed_from_u64(3716);
                    let mut b = a.clone();
                    let expected = baseline
                        .prepare_sampler_with_cache_budget(cache)
                        .unwrap()
                        .sample_with_sweep(shots, &sweep, &mut a)
                        .unwrap();
                    let actual = plan
                        .prepare_sampler_with_cache_budget(cache)
                        .unwrap()
                        .sample_measurements_u8_with_sweep(shots, &sweep, &mut b)
                        .unwrap();
                    assert_eq!(actual.measurements, flatten(&expected));
                    assert_eq!(a.next_u64(), b.next_u64());
                }
                for parts in [
                    vec![1, 128],
                    vec![31, 98],
                    vec![32, 97],
                    vec![64, 65],
                    vec![65, 64],
                    vec![129, 0],
                    vec![1, 31, 32, 64, 1],
                ] {
                    let mut a = StdRng::seed_from_u64(3717);
                    let mut b = a.clone();
                    let expected = baseline
                        .prepare_sampler_with_cache_budget(cache)
                        .unwrap()
                        .sample_with_sweep(129, &sweep, &mut a)
                        .unwrap();
                    let mut prepared = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    let mut structured = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    let mut c = StdRng::seed_from_u64(3717);
                    let mut actual = Vec::new();
                    let mut structured_actual = Vec::new();
                    for shots in parts {
                        actual.extend(
                            prepared
                                .sample_measurements_u8_with_sweep(shots, &sweep, &mut b)
                                .unwrap()
                                .measurements,
                        );
                        structured_actual
                            .extend(structured.sample_with_sweep(shots, &sweep, &mut c).unwrap());
                    }
                    assert_eq!(actual, flatten(&expected));
                    assert_eq!(structured_actual, expected);
                    let continuation = a.next_u64();
                    assert_eq!(continuation, b.next_u64());
                    assert_eq!(continuation, c.next_u64());
                }
            }
        }
    }

    #[test]
    fn scalar_reuses_full_packet_tape_without_shrinking_and_preserves_later_packets() {
        let plan = mixed_plan();
        let baseline = without_runs(&plan);
        let mut a = StdRng::seed_from_u64(3718);
        let mut b = a.clone();
        let mut old = baseline.prepare_sampler().unwrap();
        let mut mixed = plan.prepare_sampler().unwrap();
        let sweep = [false, true];
        assert_eq!(
            mixed
                .sample_measurements_u8_with_sweep(64, &sweep, &mut b)
                .unwrap()
                .measurements,
            flatten(&old.sample_with_sweep(64, &sweep, &mut a).unwrap())
        );
        let capacity = mixed.packet_tape.capacity();
        let len = mixed.packet_tape.len();
        assert!(len >= 64 * plan.random_kinds.len());
        assert_eq!(
            mixed.sample_with_sweep(1, &sweep, &mut b).unwrap(),
            old.sample_with_sweep(1, &sweep, &mut a).unwrap()
        );
        assert!(mixed.last_scalar_prepared);
        assert_eq!(mixed.packet_tape.capacity(), capacity);
        assert_eq!(mixed.packet_tape.len(), len);
        assert_eq!(
            mixed
                .sample_measurements_u8_with_sweep(65, &sweep, &mut b)
                .unwrap()
                .measurements,
            flatten(&old.sample_with_sweep(65, &sweep, &mut a).unwrap())
        );
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn actual_shared_capacity_budget_rejection_falls_back_to_original_live_scalar() {
        let plan = mixed_plan();
        let baseline = without_runs(&plan);
        let mut prepared = plan.prepare_sampler_with_cache_budget(0).unwrap();
        resize_packet(&mut prepared.packet_x, plan.num_qubits).unwrap();
        resize_packet(&mut prepared.packet_z, plan.num_qubits).unwrap();
        resize_packet(&mut prepared.packet_records, plan.measurement_count).unwrap();
        resize_packet(&mut prepared.packet_noise_masks, plan.noise_event_count).unwrap();
        prepared.packet_independent =
            IndependentPacket::new(plan.independent_event_count, PACKET_BYTE_BUDGET);
        assert!(prepared.packet_independent.is_some());
        assert!(prepared.try_prepare_scalar_tape(PACKET_BYTE_BUDGET));
        let occupied = prepared.scalar_tape_other_bytes().unwrap()
            + prepared.packet_tape.capacity() * size_of::<u64>();
        assert!(occupied <= PACKET_BYTE_BUDGET);
        let capacity = prepared.packet_tape.capacity();
        for (budget, expected_prepared) in [(0, false), (occupied - 1, false), (occupied, true)] {
            let mut a = StdRng::seed_from_u64(3719);
            let mut b = a.clone();
            let expected = baseline
                .prepare_sampler_with_cache_budget(0)
                .unwrap()
                .row(&[], &mut a)
                .unwrap();
            let actual = prepared
                .row_with_scalar_tape_budget(&[], &mut b, budget)
                .unwrap();
            assert_eq!(expected, actual);
            assert_eq!(prepared.last_scalar_prepared, expected_prepared);
            assert_eq!(prepared.packet_tape.capacity(), capacity);
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut fresh = plan.prepare_sampler_with_cache_budget(0).unwrap();
        assert!(!fresh.try_prepare_scalar_tape(0));
        assert_eq!(fresh.packet_tape.capacity(), 0); // Rejects before allocation/growth.
    }

    #[test]
    fn below_uniform_noise_threshold_retains_live_scalar_without_tape() {
        let count = MIN_SCALAR_PREPARED_NOISE_EVENTS - 1;
        let text = format!("H 0\nT 0\nREPEAT {count} {{\nX_ERROR(0.001) 0\n}}\nMX 0\n");
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        let baseline = without_runs(&plan);
        assert_eq!(plan.noise_event_count, count);
        let mut prepared = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut a = StdRng::seed_from_u64(3723);
        let mut b = a.clone();
        assert_eq!(
            prepared.sample(1, &mut a).unwrap(),
            baseline
                .prepare_sampler_with_cache_budget(0)
                .unwrap()
                .sample(1, &mut b)
                .unwrap()
        );
        assert!(!prepared.last_scalar_prepared);
        assert_eq!(prepared.packet_tape.capacity(), 0);
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn zero_calls_and_output_errors_do_not_draw_or_allocate_scalar_tape() {
        let plan = mixed_plan();
        let mut prepared = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut a = StdRng::seed_from_u64(3720);
        let mut b = a.clone();
        assert!(prepared.sample(0, &mut a).unwrap().is_empty());
        assert!(
            prepared
                .sample_measurements_u8(0, &mut a)
                .unwrap()
                .measurements
                .is_empty()
        );
        assert_eq!(prepared.packet_tape.capacity(), 0);
        assert!(!prepared.last_scalar_prepared);
        assert!(prepared.sample_measurements_u8(usize::MAX, &mut a).is_err());
        assert!(prepared.sample(usize::MAX, &mut a).is_err());
        assert_eq!(prepared.packet_tape.capacity(), 0);
        assert_eq!(a.next_u64(), b.next_u64());
        let empty = CompiledNearCliffordExecutor::compile_text("I 0\n").unwrap();
        let mut sampler = empty.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(3721);
        let mut b = a.clone();
        assert!(
            sampler
                .sample(129, &mut a)
                .unwrap()
                .iter()
                .all(|shot| shot.measurements.is_empty())
        );
        assert_eq!(sampler.packet_tape.capacity(), 0);
        assert!(!sampler.last_scalar_prepared);
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn execution_error_restores_tape_and_does_not_retry_or_redraw() {
        let mut text = String::from("H 0\nT 0\nMX 0\n");
        text.push_str("REPEAT 129 {\nX_ERROR(0.001) 1\n}\nM 1\n");
        let mut plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        assert!(plan.noise_event_count >= 128);
        // Deliberately invalid quantum state exercises the existing logical Err.
        // Failed-call prefix equality is not a cross-strategy API contract.
        plan.initial_coefficients =
            Arc::new(vec![ComplexAmp::default(); plan.initial_coefficients.len()]);
        let mut prepared = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut a = StdRng::seed_from_u64(3722);
        let mut expected_rng = a.clone();
        {
            let mut frozen = FrozenScalarRowRandom::live(&mut expected_rng);
            for &kind in &plan.random_kinds {
                frozen.draw(kind);
            }
        }
        let baseline = without_runs(&plan);
        let mut old_rng = StdRng::seed_from_u64(3722);
        let old_error = baseline
            .prepare_sampler_with_cache_budget(0)
            .unwrap()
            .row(&[], &mut old_rng)
            .unwrap_err();
        let prepared_error = prepared.row(&[], &mut a).unwrap_err();
        assert_eq!(prepared_error, old_error);
        assert!(prepared.last_scalar_prepared);
        assert!(prepared.packet_tape.len() >= plan.random_kinds.len());
        assert_eq!(a.next_u64(), expected_rng.next_u64()); // Exactly one full-row draw.
    }

    fn mixed_plan_with_arithmetic(
        policy: CompiledRotationArithmetic,
    ) -> CompiledNearCliffordExecutor {
        let text = "H 0 1 2 3\nT 0 1 2 3\nREPEAT 129 {\nX_ERROR(0.001) 0 4 0 4\nDEPOLARIZE1(0.001) 4\nDEPOLARIZE2(0.001) 0 1\nR 4\nH 4\nMR(0.001) !4\nCX rec[-1] 0\n}\nDEPOLARIZE1(0) 1\nX_ERROR(1) 4\nDEPOLARIZE2(1) 0 1\nY_ERROR(0.37) 2\nMPP(0.007) !Y0*X1 X0*!Y1\nMRX(0.37) !0\nCX rec[-1] 3\nCX sweep[1] 2\nT_DAG 1\nMY 2\nMX 3\nM(1) 0 1 4\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(2) rec[-3]\n";
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
        assert!(plan.noise_event_count >= 128);
        assert!(plan.independent_event_count >= 129);
        assert!(plan.random_runs.is_some());
        plan
    }

    #[test]
    fn noncompact_full_noise_packet_matches_frozen_typed_rows_with_both_independent_modes() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = mixed_plan_with_arithmetic(policy);
            assert!(plan.noise_event_count > 0);
            let baseline = without_runs(&plan);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for cache_budget in [initial, DEFAULT_CACHE_BYTE_BUDGET] {
                for compact_independent in [false, true] {
                    let mut packed = plan
                        .prepare_sampler_with_cache_budget(cache_budget)
                        .unwrap();
                    assert!(packed.cache.is_some());
                    resize_packet(&mut packed.packet_x, plan.num_qubits).unwrap();
                    resize_packet(&mut packed.packet_z, plan.num_qubits).unwrap();
                    resize_packet(&mut packed.packet_records, plan.measurement_count).unwrap();
                    resize_packet(&mut packed.packet_noise_masks, plan.noise_event_count).unwrap();
                    resize_packet(&mut packed.packet_tape, 64 * plan.random_kinds.len()).unwrap();
                    packed.packet_tape.fill(u64::MAX);
                    if compact_independent {
                        let occupied = packed.scalar_tape_other_bytes().unwrap()
                            + packed.packet_tape.capacity() * size_of::<u64>();
                        packed.packet_independent = IndependentPacket::new(
                            plan.independent_event_count,
                            PACKET_BYTE_BUDGET.checked_sub(occupied).unwrap(),
                        );
                        assert!(packed.packet_independent.is_some());
                    }
                    let sweep = [false, true];
                    let mut a = StdRng::seed_from_u64(1613);
                    let mut b = a.clone();
                    let mut scalar = baseline.prepare_sampler_with_cache_budget(0).unwrap();
                    let mut expected = Vec::new();
                    for _ in 0..64 {
                        let values = {
                            let mut old = FrozenScalarRowRandom::live(&mut a);
                            plan.random_kinds
                                .iter()
                                .map(|&kind| old.draw(kind))
                                .collect::<Vec<_>>()
                        };
                        let mut replay = RowRandom::recorded(&values, &mut a);
                        expected.extend(
                            scalar
                                .row_with_random(&sweep, &mut replay)
                                .unwrap()
                                .measurements
                                .into_iter()
                                .map(u8::from),
                        );
                        assert_eq!(replay.cursor, plan.random_kinds.len());
                    }
                    let mut actual = Vec::new();
                    // None is also the public mode after optional Noise rejection.
                    packed
                        .packet(
                            64,
                            &sweep,
                            &mut b,
                            &mut BatchOutput::Measurements(&mut actual),
                            false,
                            None,
                        )
                        .unwrap();
                    assert_eq!(
                        actual, expected,
                        "{policy:?} cache={cache_budget} independent={compact_independent}"
                    );
                    if cache_budget == initial {
                        assert_eq!(packed.last_packet_live, 0);
                    }
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }

    #[test]
    fn both_arithmetic_policies_preserve_full_raw_rows_rng_and_cache_across_cold_warm_tails() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = mixed_plan_with_arithmetic(policy);
            let baseline = without_runs(&plan);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for cache in [0, initial + 288, DEFAULT_CACHE_BYTE_BUDGET] {
                for seed in [0, 63, 3716] {
                    let mut a = StdRng::seed_from_u64(seed);
                    let mut b = a.clone();
                    let mut c = a.clone();
                    let mut old = baseline.prepare_sampler_with_cache_budget(0).unwrap();
                    old.pack_enabled = false;
                    let mut flat = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    let mut structured = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    structured.pack_enabled = false;
                    // First call is cold; repeated 64/65 calls retain warmed
                    // transition state. Measurements include physical resets,
                    // readout inversions, MPP, feedback and 129 independent events.
                    for shots in [0, 1, 7, 31, 32, 63, 64, 65, 129, 64, 1, 65] {
                        let expected = old
                            .sample_with_sweep(shots, &[false, true], &mut a)
                            .unwrap();
                        let raw = flat
                            .sample_measurements_u8_with_sweep(shots, &[false, true], &mut b)
                            .unwrap();
                        let actual = structured
                            .sample_with_sweep(shots, &[false, true], &mut c)
                            .unwrap();
                        assert_eq!(
                            raw.measurements,
                            flatten(&expected),
                            "policy={policy:?}; cache={cache}; seed={seed}; shots={shots}"
                        );
                        assert_eq!(
                            actual, expected,
                            "policy={policy:?}; all structured annotation fields"
                        );
                        // Cloned continuation leaves the live streams undisturbed.
                        assert_eq!(a.clone().next_u64(), b.clone().next_u64());
                        assert_eq!(a.clone().next_u64(), c.clone().next_u64());
                        assert!(!old.last_scalar_prepared);
                    }
                }
            }
        }
    }

    #[test]
    fn both_arithmetic_policies_preserve_split_raw_streams_rng_and_annotation_fields() {
        for policy in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = mixed_plan_with_arithmetic(policy);
            let baseline = without_runs(&plan);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for cache in [0, initial + 288, DEFAULT_CACHE_BYTE_BUDGET] {
                for parts in [
                    vec![1, 128],
                    vec![31, 98],
                    vec![32, 97],
                    vec![64, 65],
                    vec![65, 64],
                    vec![129, 0],
                    vec![0, 1, 7, 24, 32, 64, 1],
                ] {
                    let mut a = StdRng::seed_from_u64(3717);
                    let mut b = a.clone();
                    let mut c = a.clone();
                    let mut old = baseline.prepare_sampler_with_cache_budget(0).unwrap();
                    old.pack_enabled = false;
                    let expected = old.sample_with_sweep(129, &[false, true], &mut a).unwrap();
                    let mut flat = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    let mut structured = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    structured.pack_enabled = false;
                    let mut raw = Vec::new();
                    let mut actual = Vec::new();
                    for shots in parts {
                        raw.extend(
                            flat.sample_measurements_u8_with_sweep(shots, &[false, true], &mut b)
                                .unwrap()
                                .measurements,
                        );
                        actual.extend(
                            structured
                                .sample_with_sweep(shots, &[false, true], &mut c)
                                .unwrap(),
                        );
                    }
                    assert_eq!(raw, flatten(&expected), "policy={policy:?}; cache={cache}");
                    assert_eq!(actual, expected);
                    assert_eq!(a.clone().next_u64(), b.clone().next_u64());
                    assert_eq!(a.clone().next_u64(), c.clone().next_u64());
                }
            }
        }
    }
}

#[cfg(test)]
mod postselected_packet_tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn all_rejected_packets_skip_suffix_physics_without_disabling_admitted_cache() {
        let text = "X 0\nM 0\nDETECTOR rec[-1]\nH 1 2 3 4\nT 1 2 3 4\nREPEAT 129 {\nDEPOLARIZE1(0.001) 1 2 3 4\nH 5\nM(0.003) 5\nR 5\n}\nMPP X1*Y2*X3*Y4\nM 1 2 3 4\nOBSERVABLE_INCLUDE(7) rec[-1]\n";
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)
                .unwrap();
            assert_eq!(plan.peak_active_rank, 4);
            assert!(plan.independent_event_count >= 129);
            for budget in [0, DEFAULT_CACHE_BYTE_BUDGET] {
                let mut candidate = plan.clone();
                if budget == 0 {
                    // This coherent-only invalid suffix would fail if executed.
                    // Compilation and the retained random inventory stay valid.
                    let pauli = candidate
                        .operations
                        .iter_mut()
                        .find_map(|op| {
                            if let PlanOp::Rotate { pauli, .. } = op {
                                Some(pauli)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    pauli.physical.phase = 4;
                }
                let mut native = candidate.prepare_sampler_with_cache_budget(budget).unwrap();
                let initial_states = native.cache.as_ref().map(|cache| cache.states.len());
                let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut a = StdRng::seed_from_u64(719);
                let mut b = a.clone();
                for shots in [64, 65, 129, 64] {
                    let rows = reference.sample(shots, &mut a).unwrap();
                    assert!(rows.iter().all(|row| row.detectors.iter().any(|bit| *bit)));
                    assert_eq!(
                        native.sample_postselected_counts(shots, 7, &mut b).unwrap(),
                        NearCliffordPostselectedCounts {
                            attempted: shots,
                            accepted: 0,
                            logical_errors: 0
                        }
                    );
                    assert_eq!(native.last_packet_live, 0);
                    assert!(
                        native.pack_enabled,
                        "postselection must not look like failed cache admission"
                    );
                    assert_eq!(
                        native.cache.as_ref().map(|cache| cache.states.len()),
                        initial_states
                    );
                    if let Some(cache) = &native.cache {
                        assert!(cache.nodes.iter().all(|op| matches!(op, CachedOp::None)));
                    }
                    assert!(native.packet_independent.is_some());
                    for _ in 0..16 {
                        assert_eq!(a.next_u64(), b.next_u64());
                    }
                }
            }
        }
    }

    #[test]
    fn zero_live_postselection_still_replays_accepted_admission_failures() {
        let text = "H 0 1\nT 0 1\nCX 0 1\nMY !0\nDETECTOR rec[-1]\nMPP X0*X1\nOBSERVABLE_INCLUDE(7) rec[-1]\n";
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
            let mut native = plan
                .prepare_sampler_with_cache_budget(initial + 288)
                .unwrap();
            let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(583);
            let mut b = a.clone();
            for shots in [64, 64, 65, 129] {
                let rows = reference.sample(shots, &mut a).unwrap();
                let accepted: Vec<_> = rows
                    .iter()
                    .filter(|row| row.detectors.iter().all(|bit| !*bit))
                    .collect();
                let errors = accepted
                    .iter()
                    .filter(|row| {
                        row.observables
                            .iter()
                            .filter(|(index, _)| *index == 7)
                            .fold(false, |parity, (_, bit)| parity ^ bit)
                    })
                    .count();
                let counts = native.sample_postselected_counts(shots, 7, &mut b).unwrap();
                assert_eq!(
                    counts,
                    NearCliffordPostselectedCounts {
                        attempted: shots,
                        accepted: accepted.len(),
                        logical_errors: errors
                    }
                );
                assert!(counts.accepted > 0 && counts.accepted < shots);
                assert_eq!(
                    native.last_packet_live, 0,
                    "accepted rows must come from admission-failure replay"
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            let expected = reference.sample_measurements_u8(65, &mut a).unwrap();
            let actual = native.sample_measurements_u8(65, &mut b).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}

#[cfg(test)]
mod recorded_rejection_tail_tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    #[test]
    fn retiring_rejected_rows_matches_literal_live_and_recorded_draws() {
        let mut kinds = vec![RandomKind::Independent; 131];
        for p in [0., 0.001, 0.01, 0.37, 1.] {
            for choices in [1, 3, 15] {
                kinds.extend(
                    [RandomKind::Noise {
                        probability: p,
                        choices,
                    }; 129],
                );
                kinds.push(RandomKind::Active);
                kinds.extend([RandomKind::Independent; 65]);
            }
        }
        for seed in [1, 739, 1739, 1002739] {
            for first in [0, 1, 63, 64, 65, 131, 260, kinds.len() - 1, kinds.len()] {
                let mut a = StdRng::seed_from_u64(seed);
                let mut b = a.clone();
                let mut literal = RowRandom::live(&mut a);
                let mut selected = RowRandom::live(&mut b);
                let mut tape = Vec::new();
                for (i, &kind) in kinds.iter().enumerate() {
                    let value = literal.draw(kind);
                    tape.push(value);
                    if i < first {
                        assert_eq!(selected.draw(kind), value);
                    }
                }
                selected.discard_remaining(&kinds[first..]);
                assert_eq!(
                    (
                        literal.bit_word,
                        literal.bits_left,
                        literal.noise_probability,
                        literal.noise_log_failure,
                        literal.noise_skip
                    ),
                    (
                        selected.bit_word,
                        selected.bits_left,
                        selected.noise_probability,
                        selected.noise_log_failure,
                        selected.noise_skip
                    )
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
                let mut a = StdRng::seed_from_u64(seed + 2);
                let mut b = a.clone();
                let mut literal = RowRandom::recorded(&tape, &mut a);
                let mut selected = RowRandom::recorded(&tape, &mut b);
                for &kind in &kinds[..first] {
                    assert_eq!(literal.draw(kind), selected.draw(kind));
                }
                for &kind in &kinds[first..] {
                    literal.draw(kind);
                }
                selected.discard_remaining(&kinds[first..]);
                assert_eq!(literal.cursor, selected.cursor);
                assert_eq!(selected.cursor, tape.len());
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }
}

#[cfg(test)]
mod highest_rotation_gather_tests {
    use super::*;

    fn check<const FUSED: bool, const IMAGINARY: bool>() {
        let values = [
            0.,
            -0.,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            0.75,
            -0.25,
            0.3826834323650898,
            -0.9238795325112867,
        ];
        for len in [4, 8, 16, 64, 256, 1024] {
            let before: Vec<_> = (0..len)
                .map(|i| {
                    ComplexAmp::new(
                        values[(i * 3) % values.len()],
                        values[(i * 7 + 1) % values.len()],
                    )
                })
                .collect();
            let masks: Vec<_> = if len == 1024 {
                vec![2, 3, 255, 256, 257, 511, 512, 513, 1022, 1023]
            } else {
                (2..len).collect()
            };
            for x in masks {
                for factor in [0.3826834323650898, -0.3826834323650898] {
                    let c = 0.9238795325112867;
                    // Snapshot gather visits every coefficient independently,
                    // without production's block, pair or XOR-group traversal.
                    let expected: Vec<_> = (0..len)
                        .map(|i| {
                            let own = before[i];
                            let partner = before[i ^ x];
                            let (pr, pi) = if IMAGINARY {
                                (-partner.im, partner.re)
                            } else {
                                (partner.re, partner.im)
                            };
                            if FUSED {
                                ComplexAmp::new(
                                    pr.mul_add(factor, own.re * c),
                                    pi.mul_add(factor, own.im * c),
                                )
                            } else {
                                ComplexAmp::new(own.re * c + pr * factor, own.im * c + pi * factor)
                            }
                        })
                        .collect();
                    let mut actual = before.clone();
                    if x & 1 != 0 {
                        CompiledNearCliffordSampler::rotate_highest_z0::<FUSED, IMAGINARY, true>(
                            &mut actual,
                            x,
                            c,
                            factor,
                        );
                    } else {
                        CompiledNearCliffordSampler::rotate_highest_z0::<FUSED, IMAGINARY, false>(
                            &mut actual,
                            x,
                            c,
                            factor,
                        );
                    }
                    for (i, (a, b)) in actual.iter().zip(expected).enumerate() {
                        assert_eq!(
                            (a.re.to_bits(), a.im.to_bits()),
                            (b.re.to_bits(), b.im.to_bits()),
                            "fused={FUSED}; imaginary={IMAGINARY}; len={len}; x={x}; i={i}; factor={factor}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn both_policies_highest_pairs_match_snapshot_gather_at_all_mask_boundaries() {
        check::<false, false>();
        check::<false, true>();
        check::<true, false>();
        check::<true, true>();
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod avx2_rotation_bits_tests {
    use super::*;

    #[test]
    fn fused_avx2_pairs_match_independent_full_vector_gather_bits() {
        if !std::arch::is_x86_feature_detected!("avx2")
            || !std::arch::is_x86_feature_detected!("fma")
        {
            return;
        }
        let values = [
            0.,
            -0.,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            0.75,
            -0.25,
            0.3826834323650898,
            -0.9238795325112867,
        ];
        for len in [4, 8, 64, 1024] {
            for kind in 0..4 {
                let before: Vec<_> = (0..len)
                    .map(|i| {
                        let scale = if kind == 3 {
                            (i + 1) as f64 / len as f64
                        } else {
                            1.
                        };
                        ComplexAmp::new(
                            values[(i * 3 + kind) % values.len()] * scale,
                            values[(i * 7 + 2 * kind) % values.len()] * scale,
                        )
                    })
                    .collect();
                for x in 2..len {
                    for factor in [0.3826834323650898, -0.3826834323650898] {
                        let c = 0.9238795325112867;
                        // Independent full-vector gather: no split, grouping,
                        // permutation helper or SIMD intrinsic from production.
                        let expected: Vec<_> = (0..len)
                            .map(|i| {
                                let own = before[i];
                                let partner = before[i ^ x];
                                ComplexAmp::new(
                                    (-partner.im).mul_add(factor, own.re * c),
                                    partner.re.mul_add(factor, own.im * c),
                                )
                            })
                            .collect();
                        let mut actual = before.clone();
                        // SAFETY: The feature checks above establish the
                        // helper's CPU precondition; x is inside this vector.
                        unsafe {
                            if x & 1 != 0 {
                                CompiledNearCliffordSampler::rotate_highest_z0_avx2::<true>(
                                    &mut actual,
                                    x,
                                    c,
                                    factor,
                                );
                            } else {
                                CompiledNearCliffordSampler::rotate_highest_z0_avx2::<false>(
                                    &mut actual,
                                    x,
                                    c,
                                    factor,
                                );
                            }
                        }
                        for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                            assert_eq!(
                                actual.re.to_bits(),
                                expected.re.to_bits(),
                                "re len={len};kind={kind};x={x};i={i};factor={factor}"
                            );
                            assert_eq!(
                                actual.im.to_bits(),
                                expected.im.to_bits(),
                                "im len={len};kind={kind};x={x};i={i};factor={factor}"
                            );
                        }
                    }
                }
            }
        }
    }
}
