//! Exact detector constraints safe across the original T rotations.
//!
//! Safe rows propagate to deterministic signed observables through the
//! original circuit. A violated selected row gives zero joint acceptance and
//! error. Assignments satisfying these necessary constraints still need the
//! full T-circuit evaluation. The proposal mixture keeps full support.
use rmagna::circuit::{Basis, BitAtom, BitExpr, Circuit, Gate, Operation, PauliProduct};
use rmagna::gf2::BitVector;
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseCategory};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) struct DetectorClosure {
    pub(crate) signatures: Vec<Vec<BitVector>>,
    pub(crate) offset: BitVector,
    pub(crate) retained: Vec<usize>,
    pub(crate) unsafe_t: Vec<usize>,
    pub(crate) signature_words: usize,
}

const DETECTORS_PER_WORD: usize = u64::BITS as usize;

/// Backward Pauli propagation for a word of detectors. Each bit is a detector
/// lane. The two phase planes encode the exponent of i modulo four.
struct DetectorWord {
    x: Vec<u64>,
    z: Vec<u64>,
    records: Vec<u64>,
    phase_low: u64,
    phase_high: u64,
    offset: u64,
    active: u64,
    unsafe_t: u64,
    random: [Option<&'static str>; DETECTORS_PER_WORD],
}

impl DetectorWord {
    fn new(circuit: &Circuit, constraints: &[BitExpr]) -> Result<Self> {
        let mut word = Self {
            x: vec![0; circuit.num_qubits()],
            z: vec![0; circuit.num_qubits()],
            records: vec![0; circuit.num_records()],
            phase_low: 0,
            phase_high: 0,
            offset: 0,
            active: u64::MAX >> (DETECTORS_PER_WORD - constraints.len()),
            unsafe_t: 0,
            random: [None; DETECTORS_PER_WORD],
        };
        for (lane, expression) in constraints.iter().enumerate() {
            word.xor_expression(expression, 1 << lane)?;
        }
        Ok(word)
    }

    fn xor_expression(&mut self, expression: &BitExpr, lanes: u64) -> Result<()> {
        if lanes == 0 {
            return Ok(());
        }
        if expression.constant {
            self.offset ^= lanes;
        }
        for atom in &expression.terms {
            let BitAtom::Record(record) = *atom else {
                return Err("cultivation detector flow left unresolved source records".into());
            };
            self.records[record] ^= lanes;
        }
        Ok(())
    }

    fn anticommutation(&self, factors: &[(usize, Basis)]) -> u64 {
        factors.iter().fold(0, |parity, &(q, basis)| {
            parity
                ^ match basis {
                    Basis::X => self.z[q],
                    Basis::Z => self.x[q],
                    Basis::Y => self.x[q] ^ self.z[q],
                }
        })
    }

    fn add_phase(&mut self, low: u64, high: u64) {
        self.phase_high ^= high ^ (self.phase_low & low);
        self.phase_low ^= low;
    }

    fn mark_random(&mut self, mut lanes: u64, stage: &'static str) {
        lanes &= self.active;
        self.active &= !lanes;
        while lanes != 0 {
            self.random[lanes.trailing_zeros() as usize] = Some(stage);
            lanes &= lanes - 1;
        }
    }

    fn reset(&mut self, q: usize, basis: Basis, stage: &'static str) {
        self.mark_random(self.anticommutation(&[(q, basis)]), stage);
        let y = self.x[q] & self.z[q];
        self.add_phase(y, y); // Removing a local Y subtracts one power of i.
        self.x[q] = 0;
        self.z[q] = 0;
    }

    fn measure(&mut self, product: &PauliProduct, reset: bool, record: usize) -> Result<()> {
        if reset {
            let &[(q, basis)] = product.factors.as_slice() else {
                return Err("cultivation detector flow does not support product reset".into());
            };
            self.reset(q, basis, "measurement reset");
        }
        self.mark_random(self.anticommutation(&product.factors), "measurement");
        let selected = std::mem::take(&mut self.records[record]) & self.active;
        // Multiply on the right by the measured Pauli on selected lanes.
        if product.inverted {
            self.phase_high ^= selected;
        }
        for &(q, basis) in &product.factors {
            if basis != Basis::Z {
                self.phase_high ^= selected & self.z[q];
                self.x[q] ^= selected;
            }
            if basis != Basis::X {
                self.z[q] ^= selected;
            }
            if basis == Basis::Y {
                self.add_phase(selected, 0);
            }
        }
        Ok(())
    }

    fn conjugate_back(&mut self, gate: Gate, qubits: &[usize]) -> Result<()> {
        match gate {
            Gate::I => {}
            Gate::X => self.phase_high ^= self.z[qubits[0]],
            Gate::Y => self.phase_high ^= self.x[qubits[0]] ^ self.z[qubits[0]],
            Gate::Z => self.phase_high ^= self.x[qubits[0]],
            Gate::H => {
                let q = qubits[0];
                self.phase_high ^= self.x[q] & self.z[q];
                std::mem::swap(&mut self.x[q], &mut self.z[q]);
            }
            Gate::S | Gate::Sdag => {
                let q = qubits[0];
                let x = self.x[q];
                self.add_phase(x, if gate == Gate::S { x } else { 0 });
                self.z[q] ^= x;
            }
            Gate::T | Gate::Tdag => {
                let unsafe_t = self.active & self.x[qubits[0]];
                self.unsafe_t |= unsafe_t;
                self.active &= !unsafe_t;
            }
            Gate::Cx => {
                let (c, t) = (qubits[0], qubits[1]);
                self.x[t] ^= self.x[c];
                self.z[c] ^= self.z[t];
            }
            Gate::Cz => {
                let (a, b) = (qubits[0], qubits[1]);
                self.phase_high ^= self.x[a] & self.x[b];
                self.z[a] ^= self.x[b];
                self.z[b] ^= self.x[a];
            }
            Gate::Swap => {
                self.x.swap(qubits[0], qubits[1]);
                self.z.swap(qubits[0], qubits[1]);
            }
            _ => return Err(format!("unsupported cultivation detector-flow gate {gate:?}").into()),
        }
        Ok(())
    }

    fn cross(&mut self, operation: &Operation) -> Result<()> {
        match operation {
            Operation::Unitary {
                gate,
                qubits,
                control: Some(control),
            } => {
                let basis = match gate {
                    Gate::X => Basis::X,
                    Gate::Y => Basis::Y,
                    Gate::Z => Basis::Z,
                    _ => {
                        return Err(
                            "cultivation detector flow requires Pauli record controls".into()
                        );
                    }
                };
                self.xor_expression(
                    control,
                    self.active & self.anticommutation(&[(qubits[0], basis)]),
                )?;
            }
            Operation::Unitary { gate, qubits, .. } => self.conjugate_back(*gate, qubits)?,
            Operation::Reset { qubit, basis } => self.reset(*qubit, *basis, "reset"),
            Operation::Measure {
                product,
                reset,
                record,
                ..
            } => self.measure(product, *reset, *record)?,
            Operation::Control { .. } => {
                return Err("cultivation detector flow rejects external controls".into());
            }
            _ => {}
        }
        Ok(())
    }
}

pub(crate) fn prepare(
    circuit: &Circuit,
    categories: &[OriginalNoiseCategory],
) -> Result<DetectorClosure> {
    let detectors = circuit.detectors().len();
    let channels = categories
        .iter()
        .map(|category| category.channels.len())
        .sum::<usize>();
    let targets = categories
        .iter()
        .flat_map(|category| &category.channels)
        .map(|channel| match &channel.action {
            OriginalNoiseAction::Pauli { factors } => factors.len(),
            _ => 1,
        })
        .sum::<usize>();
    let words = channels as u128 * detectors.div_ceil(64) as u128;
    let work = detectors as u128 * (circuit.operations().len() as u128 + targets as u128);
    if detectors > 4096
        || categories.len() > 65536
        || channels > 1 << 20
        || words > 1 << 22
        || work > 1 << 28
        || circuit.num_qubits() > 65536
    {
        return Err(
            "cultivation detector-flow model exceeds its declared geometry/work limits".into(),
        );
    }
    if circuit.needs_history_sampling()
        || categories.iter().any(|category| category.guard.is_some())
    {
        return Err(
            "cultivation detector flow requires unguarded original noise and affine controls"
                .into(),
        );
    }
    let mut at = BTreeMap::<usize, Vec<(usize, &OriginalNoiseAction)>>::new();
    for (index, channel) in categories
        .iter()
        .flat_map(|site| &site.channels)
        .enumerate()
    {
        at.entry(channel.source.operation)
            .or_default()
            .push((index, &channel.action));
    }
    let mut selected = BTreeMap::new();
    for operation in circuit.operations() {
        if let Operation::Postselect { expression } = &operation.operation {
            if let Some(old) = selected.insert(expression.terms.clone(), expression.constant) {
                if old != expression.constant {
                    return Err(
                        "cultivation source has contradictory selected detector signs".into(),
                    );
                }
            }
        }
    }
    let mut covered = BTreeSet::new();
    let mut constraints = Vec::with_capacity(detectors);
    for operation in circuit.operations() {
        if let Operation::Detector {
            index, expression, ..
        } = &operation.operation
        {
            let mut constraint = expression.clone();
            constraint.constant = *selected
                .get(&expression.terms)
                .ok_or("cultivation source contains an unselected detector")?;
            covered.insert(expression.terms.clone());
            if *index != constraints.len() {
                return Err("cultivation detector indices must be in declaration order".into());
            }
            constraints.push(constraint);
        }
    }
    let words_per_channel = detectors.div_ceil(DETECTORS_PER_WORD);
    let reverse_ops = circuit
        .operations()
        .iter()
        .rev()
        .map(|op| {
            (
                &op.operation,
                at.get(&op.id).map(Vec::as_slice).unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    let mut signatures = vec![0; words as usize];
    let mut offset = Vec::with_capacity(words_per_channel);
    let mut retained = Vec::new();
    let mut unsafe_t = Vec::new();
    for (block, constraints) in constraints.chunks(DETECTORS_PER_WORD).enumerate() {
        let mut word = DetectorWord::new(circuit, constraints)?;
        for &(operation, channels) in &reverse_ops {
            if word.active == 0 {
                break;
            }
            for &(channel, action) in channels {
                signatures[channel * words_per_channel + block] = match action {
                    OriginalNoiseAction::ReportFlip { record } => word.records[*record],
                    OriginalNoiseAction::Pauli { factors } => word.anticommutation(factors),
                };
            }
            word.cross(operation)?;
        }
        let unresolved = word.records.iter().fold(0, |mask, bits| mask | bits);
        let initial_x = word.x.iter().fold(0, |mask, bits| mask | bits);
        for lane in 0..constraints.len() {
            let index = block * DETECTORS_PER_WORD + lane;
            let mask = 1 << lane;
            if word.unsafe_t & mask != 0 {
                unsafe_t.push(index);
            } else {
                if unresolved & word.active & mask != 0 {
                    return Err("cultivation detector flow left unresolved source records".into());
                }
                if let Some(stage) =
                    word.random[lane].or_else(|| (initial_x & mask != 0).then_some("initial state"))
                {
                    return Err(format!(
                        "cultivation detector {index} is nondeterministic at {stage}"
                    )
                    .into());
                }
                if word.phase_low & mask != 0 {
                    return Err("cultivation detector has a non-real constant phase".into());
                }
                retained.push(index);
            }
        }
        for channel in 0..channels {
            signatures[channel * words_per_channel + block] &= word.active;
        }
        offset.push((word.offset ^ word.phase_high) & word.active);
    }
    if covered.len() != selected.len() {
        return Err("cultivation postselection contains a non-detector predicate".into());
    }
    let mut masks = signatures.into_iter();
    Ok(DetectorClosure {
        signatures: categories
            .iter()
            .map(|site| {
                site.channels
                    .iter()
                    .map(|_| {
                        BitVector::from_words(
                            detectors,
                            masks.by_ref().take(words_per_channel).collect(),
                        )
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .collect::<std::result::Result<Vec<_>, _>>()?,
        offset: BitVector::from_words(detectors, offset)?,
        retained,
        unsafe_t,
        signature_words: words as usize,
    })
}
