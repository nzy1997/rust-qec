//! Independent commuting-observation compilation at original gate boundaries.
use super::*;
use crate::gf2::{LinearFactorization, transpose_columns};

struct Slot {
    gate: usize,
    qubits: Vec<usize>,
    coefficient: u8,
    axis: PauliWord,
}
struct Selected {
    gate: usize,
    qubits: Vec<usize>,
    coordinates: BitVector,
    coefficient: u8,
    negative: bool,
}
pub(super) struct PhaseUpdates {
    width: usize,
    cut_gate: usize,
    cliffords: Vec<Option<CliffordGate>>,
    selected: Vec<Selected>,
    outside: Vec<(BitVector, u8)>,
    coordinates: usize,
}
impl PhaseUpdates {
    pub(super) fn phase(
        &self,
        insertions: &BTreeMap<usize, Vec<&PauliWord>>,
    ) -> Result<ParityPhase, UnitaryError> {
        let mut frame = PauliWord::identity(self.width)?;
        let mut flips = BitVector::zeros(self.selected.len())?;
        let mut selected = self.selected.iter().enumerate().peekable();
        for position in self.cut_gate..self.cliffords.len() {
            // At the cut gate all physical insertions precede the snapshot.
            // Their signs already reside in the copied observation frame.
            if position > self.cut_gate
                && let Some(words) = insertions.get(&position)
            {
                for word in words {
                    frame = word.multiply(&frame)?;
                }
            }
            while selected.peek().is_some_and(|(_, s)| s.gate == position) {
                let (index, observation) = selected.next().expect("peeked selected slot");
                let mut flip = false;
                for &q in &observation.qubits {
                    flip ^= frame.x().get(q)?;
                }
                flips.set(index, flip)?;
            }
            if let Some(gate) = self.cliffords[position] {
                frame = frame.conjugated(gate)?;
            }
        }
        self.phase_from_effects(&flips)
    }
    pub(super) fn effect_dimensions(&self) -> (usize, usize) {
        (self.width, self.selected.len())
    }
    pub(super) fn phase_from_effects(
        &self,
        flips: &BitVector,
    ) -> Result<ParityPhase, UnitaryError> {
        if flips.width() != self.selected.len() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let mut constant = 0;
        let mut terms = self.outside.clone();
        for (index, observation) in self.selected.iter().enumerate() {
            let negative = observation.negative ^ flips.get(index)?;
            if negative {
                constant = (constant + observation.coefficient) & 7;
            }
            terms.push((
                observation.coordinates.clone(),
                if negative {
                    (8 - observation.coefficient) & 7
                } else {
                    observation.coefficient
                },
            ));
        }
        Ok(ParityPhase::new(self.coordinates, constant, terms)?)
    }
}

/// Reverse linear action on later observation signs. Eighth-root global Pauli
/// phases stay in the separate exact Clifford frame, not these binary masks.
pub(super) struct InsertionEffects<'a> {
    updates: &'a PhaseUpdates,
    x: Vec<BitVector>,
    z: Vec<BitVector>,
    cursor: usize,
    next_selected: usize,
}
impl<'a> InsertionEffects<'a> {
    pub(super) fn new(updates: &'a PhaseUpdates) -> Result<Self, UnitaryError> {
        let (width, observations) = updates.effect_dimensions();
        let rows = if observations == 0 { 0 } else { width };
        Ok(Self {
            updates,
            x: vec![BitVector::zeros(observations)?; rows],
            z: vec![BitVector::zeros(observations)?; rows],
            cursor: updates.cliffords.len(),
            next_selected: observations,
        })
    }
    fn prepend(&mut self, gate: CliffordGate) -> Result<(), UnitaryError> {
        // Only commutation signs matter, so Pauli X/Z gate phases disappear.
        match gate {
            CliffordGate::H(q) => std::mem::swap(&mut self.x[q], &mut self.z[q]),
            CliffordGate::S { qubit, turns } => {
                if turns & 1 != 0 {
                    let z = self.z[qubit].clone();
                    self.x[qubit].xor_assign(&z)?;
                }
            }
            CliffordGate::Cx { control, target } => {
                let x = self.x[target].clone();
                self.x[control].xor_assign(&x)?;
                let z = self.z[control].clone();
                self.z[target].xor_assign(&z)?;
            }
            CliffordGate::Cz { a, b } => {
                let zb = self.z[b].clone();
                self.x[a].xor_assign(&zb)?;
                let za = self.z[a].clone();
                self.x[b].xor_assign(&za)?;
            }
            CliffordGate::Swap { a, b } => {
                self.x.swap(a, b);
                self.z.swap(a, b);
            }
            CliffordGate::X(_) | CliffordGate::Z(_) => {}
        }
        Ok(())
    }
    pub(super) fn image_at(
        &mut self,
        position: usize,
        word: &PauliWord,
    ) -> Result<BitVector, UnitaryError> {
        let observations = self.updates.selected.len();
        let mut effects = BitVector::zeros(observations)?;
        if observations == 0 || position <= self.updates.cut_gate {
            return Ok(effects);
        }
        if position > self.cursor || word.width() != self.updates.width {
            return Err(UnitaryError::DimensionMismatch);
        }
        while self.cursor > position {
            self.cursor -= 1;
            if let Some(gate) = self.updates.cliffords[self.cursor] {
                self.prepend(gate)?;
            }
            // Direct evaluation observes before applying this gate. The reverse
            // pass first transports later signs, then installs this observation.
            while self.next_selected > 0
                && self.updates.selected[self.next_selected - 1].gate == self.cursor
            {
                self.next_selected -= 1;
                for &q in &self.updates.selected[self.next_selected].qubits {
                    let value = !self.x[q].get(self.next_selected)?;
                    self.x[q].set(self.next_selected, value)?;
                }
            }
        }
        for q in word.x().ones() {
            effects.xor_assign(&self.x[q])?;
        }
        for q in word.z().ones() {
            effects.xor_assign(&self.z[q])?;
        }
        Ok(effects)
    }
}

fn physical_clifford(gate: UnitaryGate) -> Option<CliffordGate> {
    match gate {
        UnitaryGate::Clifford(c) => Some(c),
        UnitaryGate::T { qubit, turns } if turns & 1 == 0 => Some(CliffordGate::S {
            qubit,
            turns: turns / 2,
        }),
        UnitaryGate::Cs { a, b, turns: 2 } => Some(CliffordGate::Cz { a, b }),
        _ => None,
    }
}
fn signed(word: &PauliWord, phase: u8) -> Result<PauliWord, PauliError> {
    PauliWord::new(
        word.x().clone(),
        word.z().clone(),
        (word.phase() + phase) & 7,
    )
}
fn update_inverse(
    x: &mut [PauliWord],
    z: &mut [PauliWord],
    gate: CliffordGate,
) -> Result<(), UnitaryError> {
    match gate {
        CliffordGate::H(q) => std::mem::swap(&mut x[q], &mut z[q]),
        CliffordGate::S { qubit: q, turns } => {
            let word = if turns & 1 != 0 {
                x[q].multiply(&z[q])?
            } else {
                x[q].clone()
            };
            x[q] = signed(&word, (8 - 2 * turns) & 7)?;
        }
        CliffordGate::X(q) => z[q] = signed(&z[q], 4)?,
        CliffordGate::Z(q) => x[q] = signed(&x[q], 4)?,
        CliffordGate::Cx {
            control: c,
            target: t,
        } => {
            x[c] = x[c].multiply(&x[t])?;
            z[t] = z[c].multiply(&z[t])?;
        }
        CliffordGate::Cz { a, b } => {
            x[a] = x[a].multiply(&z[b])?;
            x[b] = z[a].multiply(&x[b])?;
        }
        CliffordGate::Swap { a, b } => {
            x.swap(a, b);
            z.swap(a, b);
        }
    }
    Ok(())
}
fn packed(word: &PauliWord) -> Result<BitVector, UnitaryError> {
    let width = word
        .width()
        .checked_mul(2)
        .ok_or(UnitaryError::DimensionOverflow)?;
    Ok(BitVector::from_indices(
        width,
        word.x()
            .ones()
            .chain(word.z().ones().map(|j| word.width() + j)),
    )?)
}
fn commute(a: &PauliWord, b: &PauliWord) -> Result<bool, Gf2Error> {
    Ok(a.x().dot(b.z())? == a.z().dot(b.x())?)
}
fn copy_observation(gates: &mut Vec<CliffordGate>, word: &PauliWord, ancilla: usize) {
    gates.push(CliffordGate::H(ancilla));
    if word.phase() != 0 {
        gates.push(CliffordGate::S {
            qubit: ancilla,
            turns: word.phase() / 2,
        });
    }
    for q in word.z().ones() {
        gates.push(CliffordGate::Cz { a: ancilla, b: q });
    }
    for q in word.x().ones() {
        gates.push(CliffordGate::Cx {
            control: ancilla,
            target: q,
        });
    }
    gates.push(CliffordGate::H(ancilla));
}

pub(super) fn compile(
    source: &UnitaryCircuit,
    input: &BitVector,
    arithmetic: &ScalarArithmetic,
    max_path_bits: usize,
) -> Result<CompiledUnitary, UnitaryError> {
    if input.width() != source.width {
        return Err(UnitaryError::DimensionMismatch);
    }
    let mut x = (0..source.width)
        .map(|q| {
            PauliWord::new(
                BitVector::from_indices(source.width, [q])?,
                BitVector::zeros(source.width)?,
                0,
            )
        })
        .collect::<Result<Vec<_>, PauliError>>()?;
    let mut z = (0..source.width)
        .map(|q| {
            PauliWord::new(
                BitVector::zeros(source.width)?,
                BitVector::from_indices(source.width, [q])?,
                0,
            )
        })
        .collect::<Result<Vec<_>, PauliError>>()?;
    let cliffords = source
        .gates
        .iter()
        .copied()
        .map(physical_clifford)
        .collect::<Vec<_>>();
    let mut slots = Vec::new();
    for (position, &gate) in source.gates.iter().enumerate() {
        if let Some(c) = cliffords[position] {
            update_inverse(&mut x, &mut z, c)?;
        } else {
            for (qubits, coefficient) in diagonal_terms(gate) {
                if coefficient == 0 {
                    continue;
                }
                let mut axis = PauliWord::identity(source.width)?;
                for &q in &qubits {
                    axis = axis.multiply(&z[q])?;
                }
                slots.push(Slot {
                    gate: position,
                    qubits,
                    coefficient,
                    axis,
                });
            }
        }
    }
    if slots.is_empty() {
        let mut result = source.compile_parity(input, arithmetic, max_path_bits)?;
        Arc::make_mut(&mut result.compilation).strategy = CompilerStrategy::SharedCommutingLayer;
        return Ok(result);
    }
    let mut groups = vec![(0, 0)];
    for (index, slot) in slots.iter().enumerate() {
        let (start, _) = *groups.last().expect("initial group");
        let mut compatible = true;
        for previous in &slots[start..index] {
            if !commute(&slot.axis, &previous.axis)? {
                compatible = false;
                break;
            }
        }
        if !compatible {
            groups.push((index, index));
        }
        groups.last_mut().expect("active group").1 = index + 1;
    }
    let mut reference_layer = 0;
    for index in 1..groups.len() {
        if groups[index].1 - groups[index].0 > groups[reference_layer].1 - groups[reference_layer].0
        {
            reference_layer = index;
        }
    }
    let (start, end) = groups[reference_layer];
    let cut_gate = slots[start].gate;
    let axes = slots[start..end]
        .iter()
        .map(|s| packed(&s.axis))
        .collect::<Result<Vec<_>, _>>()?;
    let factor = LinearFactorization::new(axes.len(), &transpose_columns(axes[0].width(), &axes)?)?;
    let basis = factor
        .pivot_columns()
        .iter()
        .map(|&j| slots[start + j].axis.clone())
        .collect::<Vec<_>>();
    let rank = basis.len();
    let packed_basis = basis.iter().map(packed).collect::<Result<Vec<_>, _>>()?;
    let coordinates =
        LinearFactorization::new(rank, &transpose_columns(axes[0].width(), &packed_basis)?)?;
    let outside_count = slots.len() - (end - start);
    let phase_width = rank
        .checked_add(outside_count)
        .ok_or(UnitaryError::DimensionOverflow)?;
    let coherent_width = source
        .width
        .checked_add(phase_width)
        .ok_or(UnitaryError::DimensionOverflow)?;
    let mut selected = Vec::new();
    for (slot, axis) in slots[start..end].iter().zip(&axes) {
        let coefficients = coordinates
            .solve(axis)?
            .expect("selected axes span their basis");
        let mut product = PauliWord::identity(source.width)?;
        for j in coefficients.ones() {
            product = product.multiply(&basis[j])?;
        }
        debug_assert_eq!(product.x(), slot.axis.x());
        debug_assert_eq!(product.z(), slot.axis.z());
        let difference = (slot.axis.phase() + 8 - product.phase()) & 7;
        debug_assert!(difference == 0 || difference == 4);
        selected.push(Selected {
            gate: slot.gate,
            qubits: slot.qubits.clone(),
            coordinates: BitVector::from_indices(phase_width, coefficients.ones())?,
            coefficient: slot.coefficient,
            negative: difference == 4,
        });
    }
    let mut copied = basis;
    for &gate in cliffords[..cut_gate].iter().flatten() {
        for word in &mut copied {
            *word = word.conjugated(gate)?;
        }
    }
    let mut gates = Vec::new();
    let mut positions = Vec::new();
    let mut outside = Vec::new();
    let mut next_slot = 0;
    for (position, _) in source.gates.iter().enumerate() {
        positions.push(gates.len());
        if let Some(c) = cliffords[position] {
            gates.push(c);
        }
        while slots
            .get(next_slot)
            .is_some_and(|slot| slot.gate == position)
        {
            let slot = &slots[next_slot];
            if next_slot == start {
                for (j, word) in copied.iter().enumerate() {
                    copy_observation(&mut gates, word, source.width + j);
                }
            }
            if !(start..end).contains(&next_slot) {
                let index = rank + outside.len();
                for &control in &slot.qubits {
                    gates.push(CliffordGate::Cx {
                        control,
                        target: source.width + index,
                    });
                }
                outside.push((
                    BitVector::from_indices(phase_width, [index])?,
                    slot.coefficient,
                ));
            }
            next_slot += 1;
        }
    }
    positions.push(gates.len());
    let updates = PhaseUpdates {
        width: source.width,
        cut_gate,
        cliffords,
        selected,
        outside,
        coordinates: phase_width,
    };
    let phase = Arc::new(updates.phase(&BTreeMap::new())?);
    let raw_path_variables = check_raw_paths(&gates, max_path_bits)?;
    let initial =
        CliffordCircuit::computational(BitVector::from_indices(coherent_width, input.ones())?)?;
    let circuit = CliffordCircuit::new(coherent_width, gates)?;
    let boundary = Arc::new(
        circuit
            .evolve(&initial, arithmetic)?
            .expect("Clifford observation circuit is unitary"),
    );
    let observation_reduction =
        observations::ObservationReduction::new(&boundary, source.width, rank)?;
    let reduced_rank = observation_reduction
        .as_ref()
        .map_or(rank, observations::ObservationReduction::rank);
    let (query_reference, query_phase) = if let Some(reduction) = &observation_reduction {
        reduction.apply(&boundary, &phase)?
    } else {
        (Arc::clone(&boundary), Arc::clone(&phase))
    };
    let compilation = Arc::new(CompilationDiagnostics {
        strategy: CompilerStrategy::SharedCommutingLayer,
        layer_slots: groups.iter().map(|(a, b)| b - a).collect(),
        reference_layer: Some(reference_layer),
        reference_layer_slots: end - start,
        phase_ancillas: outside_count,
        shared_phase_coordinates: reduced_rank,
        coherent_width: query_reference.width(),
        reference_coherent_width: coherent_width,
        shared_observations_before_reduction: rank,
        clifford_gates: circuit.gates().len(),
        path_variables: boundary.columns().len(),
        raw_path_variables,
    });
    let program = Arc::new(InsertionProgram {
        reference: Arc::clone(&boundary),
        query_reference: Arc::clone(&query_reference),
        observation_reduction,
        reference_phase: Arc::clone(&phase),
        query_reference_phase: Arc::clone(&query_phase),
        shared: Some(updates),
        circuit,
        positions,
    });
    Ok(CompiledUnitary {
        physical_width: source.width,
        boundary: query_reference,
        phase: query_phase,
        program,
        compilation,
    })
}

#[cfg(test)]
mod insertion_effect_tests {
    use super::*;
    #[test]
    fn reverse_effects_match_forward_physical_frames_across_mask_words_and_cut() {
        let gates = vec![
            Some(CliffordGate::H(0)),
            Some(CliffordGate::S {
                qubit: 64,
                turns: 3,
            }),
            Some(CliffordGate::Cx {
                control: 0,
                target: 128,
            }),
            Some(CliffordGate::Cz { a: 63, b: 64 }),
            Some(CliffordGate::H(128)),
            Some(CliffordGate::S { qubit: 0, turns: 1 }),
            Some(CliffordGate::Swap { a: 64, b: 128 }),
            Some(CliffordGate::X(63)),
            Some(CliffordGate::Z(128)),
            Some(CliffordGate::Cx {
                control: 128,
                target: 0,
            }),
            Some(CliffordGate::S {
                qubit: 63,
                turns: 2,
            }),
            None,
        ];
        let selected = (0..130)
            .map(|j| Selected {
                gate: 3 + j / 15,
                qubits: vec![j % 129, (j + 64) % 129],
                coordinates: BitVector::from_indices(131, [j]).unwrap(),
                coefficient: ([1, 3, 4, 7])[j % 4],
                negative: j % 3 == 0,
            })
            .collect::<Vec<_>>();
        let updates = PhaseUpdates {
            width: 129,
            cut_gate: 3,
            cliffords: gates,
            selected,
            outside: vec![(BitVector::from_indices(131, [130]).unwrap(), 2)],
            coordinates: 131,
        };
        let word = PauliWord::new(
            BitVector::from_indices(129, [0, 63, 64, 128]).unwrap(),
            BitVector::from_indices(129, [1, 64, 127]).unwrap(),
            5,
        )
        .unwrap();
        let mut reverse = InsertionEffects::new(&updates).unwrap();
        for position in (0..=updates.cliffords.len()).rev() {
            let actual = reverse.image_at(position, &word).unwrap();
            let mut frame = PauliWord::identity(129).unwrap();
            let mut expected = BitVector::zeros(130).unwrap();
            for gate in updates.cut_gate..updates.cliffords.len() {
                if gate == position && gate > updates.cut_gate {
                    frame = word.clone();
                }
                for (j, observation) in updates
                    .selected
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.gate == gate)
                {
                    let flip = observation
                        .qubits
                        .iter()
                        .fold(false, |v, &q| v ^ frame.x().get(q).unwrap());
                    expected.set(j, flip).unwrap();
                }
                if let Some(c) = updates.cliffords[gate] {
                    frame = frame.conjugated(c).unwrap();
                }
            }
            assert_eq!(actual, expected, "position {position}");
            if position <= updates.cut_gate {
                assert!(actual.is_zero());
            }
        }
        assert!(matches!(
            updates.phase_from_effects(&BitVector::zeros(129).unwrap()),
            Err(UnitaryError::DimensionMismatch)
        ));
    }
}
