//! Exact detector constraints safe across the original T rotations.
//!
//! These signatures select a proposal only. Full T-circuit contributions are
//! evaluated separately, and the ordinary branch of the mixture keeps support.
use rmagna::circuit::{Basis, BitExpr, Circuit, Gate, Operation};
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseCategory};
use rmagna::pauli::PauliWord;
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

enum Row {
    Safe(Vec<Vec<bool>>, bool),
    UnsafeT,
    Random(&'static str),
}

fn word(width: usize, factors: &[(usize, Basis)], inverted: bool) -> Result<PauliWord> {
    let mut x = BitVector::zeros(width)?;
    let mut z = x.clone();
    let mut phase = 4 * u8::from(inverted);
    for &(qubit, basis) in factors {
        if x.get(qubit)? || z.get(qubit)? {
            return Err("detector flow received a repeated Pauli target".into());
        }
        x.set(qubit, basis != Basis::Z)?;
        z.set(qubit, basis != Basis::X)?;
        if basis == Basis::Y {
            phase = (phase + 2) & 7;
        }
    }
    Ok(PauliWord::new(x, z, phase)?)
}

fn anticommutes(pauli: &PauliWord, factors: &[(usize, Basis)]) -> Result<bool> {
    let mut result = false;
    for &(qubit, basis) in factors {
        result ^= match basis {
            Basis::X => pauli.z().get(qubit)?,
            Basis::Z => pauli.x().get(qubit)?,
            Basis::Y => pauli.x().get(qubit)? ^ pauli.z().get(qubit)?,
        };
    }
    Ok(result)
}

fn reset(pauli: PauliWord, qubit: usize, basis: Basis) -> Result<Option<PauliWord>> {
    let x = pauli.x().get(qubit)?;
    let z = pauli.z().get(qubit)?;
    if (x || z) && (x, z) != (basis != Basis::Z, basis != Basis::X) {
        return Ok(None);
    }
    let mut xx = pauli.x().clone();
    let mut zz = pauli.z().clone();
    xx.set(qubit, false)?;
    zz.set(qubit, false)?;
    // Removing a local Y also removes its omega^2 factor from XZ notation.
    let phase = (pauli.phase() + 8 - if x && z { 2 } else { 0 }) & 7;
    Ok(Some(PauliWord::new(xx, zz, phase)?))
}

fn back(mut pauli: PauliWord, gate: Gate, qubits: &[usize]) -> Result<PauliWord> {
    let gates = match gate {
        Gate::I => vec![],
        Gate::X => vec![CliffordGate::X(qubits[0])],
        Gate::Z => vec![CliffordGate::Z(qubits[0])],
        Gate::Y => vec![CliffordGate::Z(qubits[0]), CliffordGate::X(qubits[0])],
        Gate::H => vec![CliffordGate::H(qubits[0])],
        Gate::S | Gate::Sdag => vec![CliffordGate::S {
            qubit: qubits[0],
            turns: if gate == Gate::S { 3 } else { 1 },
        }],
        Gate::Cx => vec![CliffordGate::Cx {
            control: qubits[0],
            target: qubits[1],
        }],
        Gate::Cz => vec![CliffordGate::Cz {
            a: qubits[0],
            b: qubits[1],
        }],
        Gate::Swap => vec![CliffordGate::Swap {
            a: qubits[0],
            b: qubits[1],
        }],
        _ => return Err(format!("unsupported cultivation detector-flow gate {gate:?}").into()),
    };
    for gate in gates {
        pauli = pauli.conjugated(gate)?;
    }
    Ok(pauli)
}

fn row(
    circuit: &Circuit,
    categories: &[OriginalNoiseCategory],
    at: &BTreeMap<usize, Vec<(usize, usize)>>,
    expression: &BitExpr,
) -> Result<Row> {
    let mut pauli = PauliWord::identity(circuit.num_qubits())?;
    let mut records = expression.clone();
    let mut signs = categories
        .iter()
        .map(|category| vec![false; category.channels.len()])
        .collect::<Vec<_>>();
    for operation in circuit.operations().iter().rev() {
        if let Some(channels) = at.get(&operation.id) {
            for &(site, alternative) in channels {
                signs[site][alternative] = match &categories[site].channels[alternative].action {
                    OriginalNoiseAction::ReportFlip { record } => records
                        .terms
                        .contains(&rmagna::circuit::BitAtom::Record(*record)),
                    OriginalNoiseAction::Pauli { factors } => anticommutes(&pauli, factors)?,
                };
            }
        }
        match &operation.operation {
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
                if anticommutes(&pauli, &[(qubits[0], basis)])? {
                    records.xor_assign(control);
                }
            }
            Operation::Unitary {
                gate: Gate::T | Gate::Tdag,
                qubits,
                ..
            } => {
                if pauli.x().get(qubits[0])? {
                    return Ok(Row::UnsafeT);
                }
            }
            Operation::Unitary { gate, qubits, .. } => pauli = back(pauli, *gate, qubits)?,
            Operation::Reset { qubit, basis } => {
                let Some(next) = reset(pauli, *qubit, *basis)? else {
                    return Ok(Row::Random("reset"));
                };
                pauli = next;
            }
            Operation::Measure {
                product,
                reset: do_reset,
                record,
                ..
            } => {
                if *do_reset {
                    if product.factors.len() != 1 {
                        return Err(
                            "cultivation detector flow does not support product reset".into()
                        );
                    }
                    let (qubit, basis) = product.factors[0];
                    let Some(next) = reset(pauli, qubit, basis)? else {
                        return Ok(Row::Random("measurement reset"));
                    };
                    pauli = next;
                }
                if anticommutes(&pauli, &product.factors)? {
                    return Ok(Row::Random("measurement"));
                }
                if records
                    .terms
                    .remove(&rmagna::circuit::BitAtom::Record(*record))
                {
                    pauli = pauli.multiply(&word(
                        circuit.num_qubits(),
                        &product.factors,
                        product.inverted,
                    )?)?;
                }
            }
            Operation::Control { .. } => {
                return Err("cultivation detector flow rejects external controls".into());
            }
            _ => {}
        }
    }
    if !records.terms.is_empty() {
        return Err("cultivation detector flow left unresolved source records".into());
    }
    if !pauli.x().is_zero() {
        return Ok(Row::Random("initial state"));
    }
    if pauli.phase() != 0 && pauli.phase() != 4 {
        return Err("cultivation detector has a non-real constant phase".into());
    }
    Ok(Row::Safe(signs, records.constant ^ (pauli.phase() == 4)))
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
    let mut at = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for (site, category) in categories.iter().enumerate() {
        for (alternative, channel) in category.channels.iter().enumerate() {
            at.entry(channel.source.operation)
                .or_default()
                .push((site, alternative));
        }
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
    let mut result = DetectorClosure {
        signatures: categories
            .iter()
            .map(|category| {
                category
                    .channels
                    .iter()
                    .map(|_| BitVector::zeros(detectors))
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .collect::<std::result::Result<Vec<_>, _>>()?,
        offset: BitVector::zeros(detectors)?,
        retained: vec![],
        unsafe_t: vec![],
        signature_words: words as usize,
    };
    let mut covered = BTreeSet::new();
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
            match row(circuit, categories, &at, &constraint)? {
                Row::UnsafeT => result.unsafe_t.push(*index),
                Row::Random(stage) => {
                    return Err(format!(
                        "cultivation detector {index} is nondeterministic at {stage}"
                    )
                    .into());
                }
                Row::Safe(signs, offset) => {
                    result.retained.push(*index);
                    result.offset.set(*index, offset)?;
                    for (target, source) in result.signatures.iter_mut().zip(signs) {
                        for (bits, sign) in target.iter_mut().zip(source) {
                            bits.set(*index, sign)?;
                        }
                    }
                }
            }
        }
    }
    if covered.len() != selected.len() {
        return Err("cultivation postselection contains a non-detector predicate".into());
    }
    Ok(result)
}
