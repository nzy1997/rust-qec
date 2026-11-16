//! Catalog-specific suffix Clifford transport, prepared once per program.
use super::*;
use crate::compact::{CompactBitVector, CompactPauliWord};
use std::{cmp::Reverse, mem::size_of};

#[derive(Clone, Copy, Debug)]
pub struct InsertionPreparationOptions {
    pub max_catalog_entries: usize,
    /// Conservative nonempty-catalog payload/metadata/scratch estimate, not RSS.
    /// Empty catalogs skip tables and report zero estimate, excluding their
    /// fixed owner header (which storage_statistics still reports).
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
    before_gate: usize,
    transported: CompactPauliWord,
    shared_effect: Option<CompactBitVector>,
}
/// Actual retained capacities and one final preparation snapshot, not peak RSS.
/// Payload counts only adaptive mask vectors; inline/table bytes include vector
/// elements and owner headers. The referenced compiled program is excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparedInsertionStorageStatistics {
    pub catalog_entries: usize,
    pub retained_mask_payload_bytes: u128,
    pub retained_inline_and_table_bytes: u128,
    pub retained_heap_buffers: usize,
    pub final_suffix_mask_payload_bytes: u128,
    pub final_suffix_inline_and_table_bytes: u128,
    pub final_suffix_heap_buffers: usize,
}
pub struct PreparedInsertions {
    program: Arc<InsertionProgram>,
    entries: Vec<Entry>,
    lookup: Vec<usize>,
    estimated_peak_bytes: u128,
    storage: PreparedInsertionStorageStatistics,
}
impl PreparedInsertions {
    /// Exact selected fault frame in stable circuit-time/input order. The
    /// returned compact snapshot owns its masks and retains every scalar phase.
    pub fn transported_frame(&self, indices: &[usize]) -> Result<CompactPauliWord, UnitaryError> {
        Ok(self.compose(indices)?.0)
    }
    fn ordered(&self, indices: &[usize]) -> Result<BTreeMap<usize, Vec<&Entry>>, UnitaryError> {
        let mut ordered = BTreeMap::<usize, Vec<&Entry>>::new();
        for &index in indices {
            let Some(&position) = self.lookup.get(index) else {
                return Err(UnitaryError::InvalidPreparedInsertion {
                    index,
                    count: self.len(),
                });
            };
            let entry = &self.entries[position];
            ordered.entry(entry.before_gate).or_default().push(entry);
        }
        Ok(ordered)
    }
    fn compose(
        &self,
        indices: &[usize],
    ) -> Result<(CompactPauliWord, Option<CompactBitVector>), UnitaryError> {
        let ordered = self.ordered(indices)?;
        let mut frame = CompactPauliWord::identity(self.program.reference.width());
        let mut effects = self
            .program
            .shared
            .as_ref()
            .map(|s| CompactBitVector::zeros(s.effect_dimensions().1));
        for entries in ordered.values() {
            for entry in entries {
                frame = entry.transported.multiply(&frame)?;
                if let (Some(total), Some(effect)) = (&mut effects, &entry.shared_effect) {
                    total.xor_assign(effect)?;
                }
            }
        }
        Ok((frame, effects))
    }

    pub fn storage_statistics(&self) -> PreparedInsertionStorageStatistics {
        self.storage
    }
    fn capture_storage(&mut self, suffix: Option<&Suffix>) {
        let masks = self.entries.iter().flat_map(|entry| {
            [
                Some(entry.transported.x()),
                Some(entry.transported.z()),
                entry.shared_effect.as_ref(),
            ]
            .into_iter()
            .flatten()
        });
        let mut storage = PreparedInsertionStorageStatistics {
            catalog_entries: self.entries.len(),
            retained_inline_and_table_bytes: size_of::<Self>() as u128
                + self.entries.capacity() as u128 * size_of::<Entry>() as u128
                + self.lookup.capacity() as u128 * size_of::<usize>() as u128,
            retained_heap_buffers: usize::from(self.entries.capacity() > 0)
                + usize::from(self.lookup.capacity() > 0),
            ..Default::default()
        };
        for mask in masks {
            let stats = mask.statistics();
            storage.retained_mask_payload_bytes += stats.heap_payload_bytes as u128;
            storage.retained_heap_buffers += stats.heap_buffers;
        }
        if let Some(suffix) = suffix {
            storage.final_suffix_inline_and_table_bytes = size_of::<Suffix>() as u128
                + (suffix.x.capacity() as u128 + suffix.z.capacity() as u128)
                    * size_of::<CompactPauliWord>() as u128;
            storage.final_suffix_heap_buffers =
                usize::from(suffix.x.capacity() > 0) + usize::from(suffix.z.capacity() > 0);
            for frame in suffix.x.iter().chain(&suffix.z) {
                for mask in [frame.x(), frame.z()] {
                    let stats = mask.statistics();
                    storage.final_suffix_mask_payload_bytes += stats.heap_payload_bytes as u128;
                    storage.final_suffix_heap_buffers += stats.heap_buffers;
                }
            }
        }
        self.storage = storage;
    }

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
    x: Vec<CompactPauliWord>,
    z: Vec<CompactPauliWord>,
}
fn phase(word: &CompactPauliWord, extra: u8) -> Result<CompactPauliWord, PauliError> {
    CompactPauliWord::new(
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
            x.push(CompactPauliWord::new(
                CompactBitVector::basis(width, q)?,
                CompactBitVector::zeros(width),
                0,
            )?);
            z.push(CompactPauliWord::new(
                CompactBitVector::zeros(width),
                CompactBitVector::basis(width, q)?,
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
    fn image(&self, word: &PauliWord, width: usize) -> Result<CompactPauliWord, UnitaryError> {
        let mut image = CompactPauliWord::identity(width).with_phase(word.phase())?;
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
        .saturating_mul(full + size_of::<CompactPauliWord>() as u128)
        .saturating_add((count as u128).saturating_mul(
            full + input + size_of::<Entry>() as u128 + 2 * size_of::<usize>() as u128,
        ))
        .saturating_add(8 * (full + size_of::<CompactPauliWord>() as u128))
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
        .saturating_mul(masks + size_of::<CompactBitVector>() as u128)
        .saturating_add((count as u128).saturating_mul(masks))
        .saturating_add(size_of::<shared::InsertionEffects<'_>>() as u128)
}
impl CompiledUnitary {
    /// Preflight a catalog's conservative table requirement without creating
    /// or cloning its input words. Does not validate catalog positions/masks.
    pub fn insertion_preparation_requirement(
        &self,
        count: usize,
        options: InsertionPreparationOptions,
    ) -> Result<u128, UnitaryError> {
        if count > options.max_catalog_entries {
            return Err(UnitaryError::InsertionResourceLimit {
                option: "max_catalog_entries",
                limit: options.max_catalog_entries,
                required: count as u128,
            });
        }
        if count == 0 {
            return Ok(0);
        }
        let (width, observations) = self
            .program
            .shared
            .as_ref()
            .map_or((0, 0), |s| s.effect_dimensions());
        let required = estimated_bytes(self.program.reference.width(), self.width(), count)
            .saturating_add(effect_bytes(width, observations, count));
        if required > options.max_table_bytes as u128 {
            return Err(UnitaryError::InsertionResourceLimit {
                option: "max_table_bytes",
                limit: options.max_table_bytes,
                required,
            });
        }
        Ok(required)
    }
    /// Prepare a supplied catalog against the original immutable program,
    /// even when this receiver is an already perturbed assignment view.
    pub fn prepare_insertions(
        &self,
        catalog: &[PauliInsertion],
        options: InsertionPreparationOptions,
    ) -> Result<PreparedInsertions, UnitaryError> {
        let estimated_peak_bytes =
            self.insertion_preparation_requirement(catalog.len(), options)?;
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
            let mut prepared = PreparedInsertions {
                program: Arc::clone(&self.program),
                entries: vec![],
                lookup: vec![],
                estimated_peak_bytes: 0,
                storage: Default::default(),
            };
            prepared.capture_storage(None);
            return Ok(prepared);
        }
        let width = self.program.reference.width();
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
                before_gate: original.before_gate,
                transported,
                shared_effect: shared_effect.as_ref().map(CompactBitVector::from_dense),
            });
        }
        let mut prepared = PreparedInsertions {
            program: Arc::clone(&self.program),
            entries,
            lookup,
            estimated_peak_bytes,
            storage: Default::default(),
        };
        prepared.capture_storage(Some(&suffix));
        Ok(prepared)
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
        let (frame, effects) = prepared.compose(indices)?;
        let effects = effects
            .as_ref()
            .map(CompactBitVector::materialize)
            .transpose()?;
        let phase = self
            .program
            .shared
            .as_ref()
            .zip(effects.as_ref())
            .map(|(s, e)| s.phase_from_effects(e).map(Arc::new))
            .transpose()?;
        self.with_insertion_frame(&frame.materialize()?, &BTreeMap::new(), phase, arithmetic)
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
                        assert_eq!(
                            suffix.image(&word, 3).unwrap().materialize().unwrap(),
                            forward
                        );
                    }
                }
            }
        }
    }
}
