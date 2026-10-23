//! Catalog-specific suffix Clifford transport, prepared once per program.
use super::*;
use std::{cmp::Reverse, mem::size_of};

#[derive(Clone, Copy, Debug)]
pub struct InsertionPreparationOptions {
    pub max_catalog_entries: usize,
    /// Conservative packed payload/metadata/scratch estimate; not process memory.
    pub max_table_bytes: usize,
}
impl Default for InsertionPreparationOptions {
    fn default() -> Self {
        Self {
            max_catalog_entries: 1_000_000,
            max_table_bytes: 64 << 20,
        }
    }
}
struct Entry {
    original: PauliInsertion,
    transported: PauliWord,
    shared_effect: Option<BitVector>,
}
pub struct PreparedInsertions {
    program: Arc<InsertionProgram>,
    entries: Vec<Entry>,
    lookup: Vec<usize>,
    estimated_peak_bytes: u128,
}
impl PreparedInsertions {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn estimated_peak_bytes(&self) -> u128 {
        self.estimated_peak_bytes
    }
}
struct Suffix {
    x: Vec<PauliWord>,
    z: Vec<PauliWord>,
}
fn phase(word: &PauliWord, extra: u8) -> Result<PauliWord, PauliError> {
    PauliWord::new(
        word.x().clone(),
        word.z().clone(),
        (word.phase() + extra) & 7,
    )
}
impl Suffix {
    fn identity(width: usize) -> Result<Self, UnitaryError> {
        let mut x = Vec::with_capacity(width);
        let mut z = Vec::with_capacity(width);
        for q in 0..width {
            x.push(PauliWord::new(
                BitVector::from_indices(width, [q])?,
                BitVector::zeros(width)?,
                0,
            )?);
            z.push(PauliWord::new(
                BitVector::zeros(width)?,
                BitVector::from_indices(width, [q])?,
                0,
            )?);
        }
        Ok(Self { x, z })
    }
    // If images represent U, update to U*gate by conjugating input generators
    // first. Only the gate's affected generator images change.
    fn prepend(&mut self, gate: CliffordGate) -> Result<(), UnitaryError> {
        match gate {
            CliffordGate::H(q) => std::mem::swap(&mut self.x[q], &mut self.z[q]),
            CliffordGate::S { qubit, turns } => {
                for _ in 0..turns {
                    self.x[qubit] = phase(&self.x[qubit].multiply(&self.z[qubit])?, 2)?;
                }
            }
            CliffordGate::X(q) => self.z[q] = phase(&self.z[q], 4)?,
            CliffordGate::Z(q) => self.x[q] = phase(&self.x[q], 4)?,
            CliffordGate::Cx { control, target } => {
                self.x[control] = self.x[control].multiply(&self.x[target])?;
                self.z[target] = self.z[control].multiply(&self.z[target])?;
            }
            CliffordGate::Cz { a, b } => {
                self.x[a] = self.x[a].multiply(&self.z[b])?;
                self.x[b] = self.z[a].multiply(&self.x[b])?;
            }
            CliffordGate::Swap { a, b } => {
                self.x.swap(a, b);
                self.z.swap(a, b);
            }
        }
        Ok(())
    }
    fn image(&self, word: &PauliWord, width: usize) -> Result<PauliWord, UnitaryError> {
        let mut image = PauliWord::new(
            BitVector::zeros(width)?,
            BitVector::zeros(width)?,
            word.phase(),
        )?;
        for q in word.x().ones() {
            image = image.multiply(&self.x[q])?;
        }
        for q in word.z().ones() {
            image = image.multiply(&self.z[q])?;
        }
        Ok(image)
    }
}
fn estimated_bytes(width: usize, physical: usize, count: usize) -> u128 {
    let word_bytes = |n: usize| (n.div_ceil(64) as u128).saturating_mul(16);
    let full = word_bytes(width);
    let input = word_bytes(physical);
    // All suffix images + all retained entries + sorting/lookup indices +
    // eight full-width temporary words and vector/plan headers. Existing
    // compiled-program storage, caller catalog and allocator rounding excluded.
    (width as u128)
        .saturating_mul(2)
        .saturating_mul(full + size_of::<PauliWord>() as u128)
        .saturating_add((count as u128).saturating_mul(
            full + input + size_of::<Entry>() as u128 + 2 * size_of::<usize>() as u128,
        ))
        .saturating_add(8 * (full + size_of::<PauliWord>() as u128))
        .saturating_add(
            size_of::<PreparedInsertions>() as u128 + 4 * size_of::<Vec<usize>>() as u128,
        )
}
fn effect_bytes(width: usize, observations: usize, count: usize) -> u128 {
    if observations == 0 {
        return 0;
    }
    let masks = (observations.div_ceil(64) as u128).saturating_mul(8);
    (2 * (width as u128) + 8)
        .saturating_mul(masks + size_of::<BitVector>() as u128)
        .saturating_add((count as u128).saturating_mul(masks))
        .saturating_add(size_of::<shared::InsertionEffects<'_>>() as u128)
}
impl CompiledUnitary {
    /// Prepare a supplied catalog against the original immutable program,
    /// even when this receiver is an already perturbed assignment view.
    pub fn prepare_insertions(
        &self,
        catalog: &[PauliInsertion],
        options: InsertionPreparationOptions,
    ) -> Result<PreparedInsertions, UnitaryError> {
        if catalog.len() > options.max_catalog_entries {
            return Err(UnitaryError::InsertionResourceLimit {
                option: "max_catalog_entries",
                limit: options.max_catalog_entries,
                required: catalog.len() as u128,
            });
        }
        let count = self.program.positions.len() - 1;
        for insertion in catalog {
            if insertion.before_gate > count {
                return Err(UnitaryError::InvalidInsertion {
                    position: insertion.before_gate,
                    gate_count: count,
                });
            }
            if insertion.word.width() != self.width() {
                return Err(UnitaryError::DimensionMismatch);
            }
        }
        if catalog.is_empty() {
            return Ok(PreparedInsertions {
                program: Arc::clone(&self.program),
                entries: vec![],
                lookup: vec![],
                estimated_peak_bytes: 0,
            });
        }
        let width = self.program.reference.width();
        let (effect_width, observations) = self
            .program
            .shared
            .as_ref()
            .map_or((0, 0), |s| s.effect_dimensions());
        let estimated_peak_bytes = estimated_bytes(width, self.width(), catalog.len())
            .saturating_add(effect_bytes(effect_width, observations, catalog.len()));
        if estimated_peak_bytes > options.max_table_bytes as u128 {
            return Err(UnitaryError::InsertionResourceLimit {
                option: "max_table_bytes",
                limit: options.max_table_bytes,
                required: estimated_peak_bytes,
            });
        }
        let mut order = (0..catalog.len()).collect::<Vec<_>>();
        order.sort_unstable_by_key(|&j| Reverse((catalog[j].before_gate, j)));
        let mut suffix = Suffix::identity(width)?;
        let mut effects = self
            .program
            .shared
            .as_ref()
            .map(shared::InsertionEffects::new)
            .transpose()?;
        let gates = self.program.circuit.gates();
        let mut cursor = gates.len();
        let mut entries = Vec::with_capacity(catalog.len());
        let mut lookup = vec![0; catalog.len()];
        for index in order {
            let original = &catalog[index];
            let start = self.program.positions[original.before_gate];
            while cursor > start {
                cursor -= 1;
                suffix.prepend(gates[cursor])?;
            }
            let transported = suffix.image(&original.word, width)?;
            let shared_effect = effects
                .as_mut()
                .map(|e| e.image_at(original.before_gate, &original.word))
                .transpose()?;
            lookup[index] = entries.len();
            entries.push(Entry {
                original: original.clone(),
                transported,
                shared_effect,
            });
        }
        Ok(PreparedInsertions {
            program: Arc::clone(&self.program),
            entries,
            lookup,
            estimated_peak_bytes,
        })
    }
    /// Apply chosen catalog entries in stable circuit-time order. Repeated
    /// indices are permitted; each assignment replaces the previous view.
    pub fn with_prepared_insertions(
        &self,
        prepared: &PreparedInsertions,
        indices: &[usize],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &prepared.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if indices.is_empty() {
            return self.with_insertions(&[], arithmetic);
        }
        let mut ordered = BTreeMap::<usize, Vec<&Entry>>::new();
        for &index in indices {
            let Some(&position) = prepared.lookup.get(index) else {
                return Err(UnitaryError::InvalidPreparedInsertion {
                    index,
                    count: prepared.len(),
                });
            };
            let entry = &prepared.entries[position];
            ordered
                .entry(entry.original.before_gate)
                .or_default()
                .push(entry);
        }
        let mut frame = PauliWord::identity(self.program.reference.width())?;
        let mut effects = self
            .program
            .shared
            .as_ref()
            .map(|s| BitVector::zeros(s.effect_dimensions().1))
            .transpose()?;
        let mut grouped = BTreeMap::new();
        for (&position, entries) in &ordered {
            let mut words = Vec::with_capacity(entries.len());
            for entry in entries {
                frame = entry.transported.multiply(&frame)?;
                if let (Some(total), Some(effect)) = (&mut effects, &entry.shared_effect) {
                    total.xor_assign(effect)?;
                }
                words.push(&entry.original.word);
            }
            grouped.insert(position, words);
        }
        let phase = self
            .program
            .shared
            .as_ref()
            .zip(effects.as_ref())
            .map(|(s, e)| s.phase_from_effects(e).map(Arc::new))
            .transpose()?;
        self.with_insertion_frame(&frame, &grouped, phase, arithmetic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverse_suffix_images_match_forward_conjugation_as_phased_operators() {
        let gates = [
            CliffordGate::H(0),
            CliffordGate::S { qubit: 1, turns: 0 },
            CliffordGate::S { qubit: 0, turns: 1 },
            CliffordGate::Cx {
                control: 0,
                target: 2,
            },
            CliffordGate::S { qubit: 2, turns: 2 },
            CliffordGate::Cz { a: 1, b: 2 },
            CliffordGate::S { qubit: 1, turns: 3 },
            CliffordGate::X(2),
            CliffordGate::Z(0),
            CliffordGate::Swap { a: 0, b: 1 },
            CliffordGate::H(2),
        ];
        let bits = |n| BitVector::from_indices(3, (0..3).filter(|&q| n & (1 << q) != 0)).unwrap();
        let mut suffix = Suffix::identity(3).unwrap();
        for start in (0..=gates.len()).rev() {
            if start < gates.len() {
                suffix.prepend(gates[start]).unwrap();
            }
            for x in 0..8 {
                for z in 0..8 {
                    for phase in 0..8 {
                        let word = PauliWord::new(bits(x), bits(z), phase).unwrap();
                        let mut forward = word.clone();
                        for &gate in &gates[start..] {
                            forward = forward.conjugated(gate).unwrap();
                        }
                        assert_eq!(suffix.image(&word, 3).unwrap(), forward);
                    }
                }
            }
        }
    }
}
