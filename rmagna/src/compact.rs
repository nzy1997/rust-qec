//! Adaptive owned bit masks and Pauli frames for retained suffix transport.
use crate::{
    gf2::{BitVector, Gf2Error},
    pauli::{PauliError, PauliWord},
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Storage {
    Empty,
    Single { at: usize, value: u64 },
    Span { first: usize, words: Vec<u64> },
    Sparse(Vec<(usize, u64)>),
}
/// Owned masks retain their declared width independently of their populated
/// endpoint. A single offset word stays inline; separated words use index/value
/// pairs only when those logical payload bytes beat a contiguous offset span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactBitVector {
    width: usize,
    storage: Storage,
}
/// Heap word capacity and inline metadata, excluding allocator overhead/RSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompactStorageStatistics {
    pub nonzero_words: usize,
    pub word_limit: usize,
    pub sparse: bool,
    pub heap_payload_bytes: usize,
    pub heap_buffers: usize,
    pub metadata_bytes: usize,
}
impl CompactBitVector {
    fn from_sorted(width: usize, rows: Vec<(usize, u64)>) -> Self {
        let storage = match rows.as_slice() {
            [] => Storage::Empty,
            &[(at, value)] => Storage::Single { at, value },
            rows => {
                let first = rows[0].0;
                let span = rows.last().unwrap().0 - first + 1;
                if span <= 2 * rows.len() {
                    let mut words = vec![0; span];
                    for &(at, value) in rows {
                        words[at - first] = value;
                    }
                    Storage::Span { first, words }
                } else {
                    Storage::Sparse(rows.to_vec())
                }
            }
        };
        Self { width, storage }
    }
    pub fn from_dense(bits: &BitVector) -> Self {
        Self::from_word_slice(bits.width(), bits.words())
    }
    fn from_word_slice(width: usize, words: &[u64]) -> Self {
        let Some(first) = words.iter().position(|&word| word != 0) else {
            return Self::zeros(width);
        };
        let last = words
            .iter()
            .rposition(|&word| word != 0)
            .expect("nonempty mask");
        let span = &words[first..=last];
        let nonzero = span.iter().filter(|&&word| word != 0).count();
        let storage = if first == last {
            Storage::Single {
                at: first,
                value: words[first],
            }
        } else if span.len() <= 2 * nonzero {
            Storage::Span {
                first,
                words: span.to_vec(),
            }
        } else {
            let mut rows = Vec::with_capacity(nonzero);
            rows.extend(
                span.iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(j, word)| (word != 0).then_some((first + j, word))),
            );
            Storage::Sparse(rows)
        };
        Self { width, storage }
    }
    pub fn from_indices(
        width: usize,
        indices: impl IntoIterator<Item = usize>,
    ) -> Result<Self, Gf2Error> {
        let mut rows = std::collections::BTreeMap::<usize, u64>::new();
        for index in indices {
            if index >= width {
                return Err(Gf2Error::IndexOutOfBounds { index, width });
            }
            *rows.entry(index / 64).or_default() |= 1u64 << (index % 64);
        }
        Ok(Self::from_sorted(width, rows.into_iter().collect()))
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn zeros(width: usize) -> Self {
        Self {
            width,
            storage: Storage::Empty,
        }
    }
    pub fn statistics(&self) -> CompactStorageStatistics {
        CompactStorageStatistics {
            nonzero_words: self.words().count(),
            word_limit: self.word_limit(),
            sparse: self.is_sparse(),
            heap_payload_bytes: self.heap_payload_bytes(),
            heap_buffers: usize::from(matches!(
                self.storage,
                Storage::Span { .. } | Storage::Sparse(_)
            )),
            metadata_bytes: std::mem::size_of::<Self>(),
        }
    }
    pub fn basis(width: usize, index: usize) -> Result<Self, Gf2Error> {
        if index >= width {
            return Err(Gf2Error::IndexOutOfBounds { index, width });
        }
        Ok(Self {
            width,
            storage: Storage::Single {
                at: index / 64,
                value: 1u64 << (index % 64),
            },
        })
    }
    /// Iterate set coordinates without expanding their leading zero words.
    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.words().flat_map(|(word, mut value)| {
            std::iter::from_fn(move || {
                if value == 0 {
                    return None;
                }
                let at = value.trailing_zeros() as usize;
                value &= value - 1;
                Some(word * 64 + at)
            })
        })
    }
    pub fn get(&self, index: usize) -> Result<bool, Gf2Error> {
        if index >= self.width {
            return Err(Gf2Error::IndexOutOfBounds {
                index,
                width: self.width,
            });
        }
        let at = index / 64;
        let value = match &self.storage {
            Storage::Empty => 0,
            Storage::Single { at: stored, value } => {
                if *stored == at {
                    *value
                } else {
                    0
                }
            }
            Storage::Span { first, words } => at
                .checked_sub(*first)
                .and_then(|j| words.get(j))
                .copied()
                .unwrap_or(0),
            Storage::Sparse(rows) => rows
                .binary_search_by_key(&at, |&(j, _)| j)
                .ok()
                .map_or(0, |j| rows[j].1),
        };
        Ok(value & (1u64 << (index % 64)) != 0)
    }
    pub fn set(&mut self, index: usize, value: bool) -> Result<(), Gf2Error> {
        if self.get(index)? != value {
            *self = self.xor(&Self::basis(self.width, index)?)?;
        }
        Ok(())
    }
    pub fn xor_assign(&mut self, right: &Self) -> Result<(), Gf2Error> {
        *self = self.xor(right)?;
        Ok(())
    }
    fn words(&self) -> Words<'_> {
        Words {
            storage: &self.storage,
            cursor: 0,
        }
    }
    pub fn word_limit(&self) -> usize {
        match &self.storage {
            Storage::Empty => 0,
            Storage::Single { at, .. } => at + 1,
            Storage::Span { first, words } => first + words.len(),
            Storage::Sparse(rows) => rows.last().unwrap().0 + 1,
        }
    }
    pub fn is_sparse(&self) -> bool {
        matches!(self.storage, Storage::Sparse(_))
    }
    /// Change the declared width, rejecting any live coordinate it would clip.
    pub fn resized(&self, width: usize) -> Result<Self, Gf2Error> {
        if width < self.width
            && let Some(index) = self.ones().find(|&at| at >= width)
        {
            return Err(Gf2Error::IndexOutOfBounds { index, width });
        }
        let mut result = self.clone();
        result.width = width;
        Ok(result)
    }
    pub fn heap_payload_bytes(&self) -> usize {
        match &self.storage {
            Storage::Empty | Storage::Single { .. } => 0,
            Storage::Span { words, .. } => words.capacity() * std::mem::size_of::<u64>(),
            Storage::Sparse(rows) => rows.capacity() * std::mem::size_of::<(usize, u64)>(),
        }
    }
    pub fn materialize(&self) -> Result<BitVector, Gf2Error> {
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.width.div_ceil(64))
            .map_err(|_| Gf2Error::AllocationFailed)?;
        words.resize(self.width.div_ceil(64), 0);
        for (index, value) in self.words() {
            words[index] = value;
        }
        BitVector::from_words(self.width, words)
    }
    pub fn xor(&self, right: &Self) -> Result<Self, Gf2Error> {
        if self.width != right.width {
            return Err(Gf2Error::WidthMismatch {
                expected: self.width,
                actual: right.width,
            });
        }
        if matches!(self.storage, Storage::Empty) {
            return Ok(right.clone());
        }
        if matches!(right.storage, Storage::Empty) {
            return Ok(self.clone());
        }
        let limit = self.word_limit().max(right.word_limit());
        let mut words = [0; 8];
        if limit <= words.len() {
            for (index, value) in self.words() {
                words[index] = value;
            }
            for (index, value) in right.words() {
                words[index] ^= value;
            }
            return Ok(Self::from_word_slice(self.width, &words[..limit]));
        }
        let (mut a, mut b) = (self.words().peekable(), right.words().peekable());
        let mut rows = Vec::new();
        loop {
            let row = match (a.peek(), b.peek()) {
                (Some(&(i, _)), Some(&(j, _))) if i == j => {
                    let (i, x) = a.next().unwrap();
                    (i, x ^ b.next().unwrap().1)
                }
                (Some(&(i, _)), Some(&(j, _))) if i < j => a.next().unwrap(),
                (Some(_), Some(_)) => b.next().unwrap(),
                (Some(_), None) => a.next().unwrap(),
                (None, Some(_)) => b.next().unwrap(),
                (None, None) => break,
            };
            if row.1 != 0 {
                rows.push(row);
            }
        }
        Ok(Self::from_sorted(self.width, rows))
    }
    pub fn dot(&self, right: &Self) -> Result<bool, Gf2Error> {
        if self.width != right.width {
            return Err(Gf2Error::WidthMismatch {
                expected: self.width,
                actual: right.width,
            });
        }
        let (mut a, mut b) = (self.words().peekable(), right.words().peekable());
        let mut parity = 0;
        while let (Some(&(i, x)), Some(&(j, y))) = (a.peek(), b.peek()) {
            match i.cmp(&j) {
                std::cmp::Ordering::Less => {
                    a.next();
                }
                std::cmp::Ordering::Greater => {
                    b.next();
                }
                std::cmp::Ordering::Equal => {
                    parity ^= (x & y).count_ones();
                    a.next();
                    b.next();
                }
            }
        }
        Ok(parity & 1 != 0)
    }
}
struct Words<'a> {
    storage: &'a Storage,
    cursor: usize,
}
impl Iterator for Words<'_> {
    type Item = (usize, u64);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let at = self.cursor;
            self.cursor += 1;
            let row = match self.storage {
                Storage::Empty => return None,
                Storage::Single { at: index, value } if at == 0 => (*index, *value),
                Storage::Single { .. } => return None,
                Storage::Span { first, words } => (first + at, *words.get(at)?),
                Storage::Sparse(rows) => *rows.get(at)?,
            };
            if row.1 != 0 {
                return Some(row);
            }
        }
    }
}
/// `omega^phase X^x Z^z`, with adaptive owned coordinate masks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactPauliWord {
    x: CompactBitVector,
    z: CompactBitVector,
    phase: u8,
}
impl CompactPauliWord {
    pub fn new(x: CompactBitVector, z: CompactBitVector, phase: u8) -> Result<Self, PauliError> {
        if x.width() != z.width() {
            return Err(PauliError::DimensionMismatch);
        }
        if phase > 7 {
            return Err(PauliError::InvalidPhase);
        }
        Ok(Self { x, z, phase })
    }
    pub fn identity(width: usize) -> Self {
        Self {
            x: CompactBitVector::zeros(width),
            z: CompactBitVector::zeros(width),
            phase: 0,
        }
    }
    pub fn width(&self) -> usize {
        self.x.width()
    }
    pub fn x(&self) -> &CompactBitVector {
        &self.x
    }
    pub fn z(&self) -> &CompactBitVector {
        &self.z
    }
    pub fn phase(&self) -> u8 {
        self.phase
    }
    pub fn with_phase(&self, phase: u8) -> Result<Self, PauliError> {
        Self::new(self.x.clone(), self.z.clone(), phase)
    }
    /// Retained heap capacity only. The two mask headers are inline in Self.
    pub fn heap_payload_bytes(&self) -> usize {
        self.x.heap_payload_bytes() + self.z.heap_payload_bytes()
    }
    pub fn from_dense(word: &PauliWord) -> Self {
        Self {
            x: CompactBitVector::from_dense(word.x()),
            z: CompactBitVector::from_dense(word.z()),
            phase: word.phase(),
        }
    }
    pub fn multiply(&self, right: &Self) -> Result<Self, PauliError> {
        if self.width() != right.width() {
            return Err(PauliError::DimensionMismatch);
        }
        Ok(Self {
            x: self.x.xor(&right.x)?,
            z: self.z.xor(&right.z)?,
            phase: (self.phase + right.phase + 4 * u8::from(self.z.dot(&right.x)?)) & 7,
        })
    }
    /// XOR coordinate masks while retaining the target's scalar phase.
    pub fn xor_coordinates(&self, target: &Self) -> Result<Self, PauliError> {
        if self.width() != target.width() {
            return Err(PauliError::DimensionMismatch);
        }
        Ok(Self {
            x: self.x.xor(&target.x)?,
            z: self.z.xor(&target.z)?,
            phase: target.phase,
        })
    }
    pub fn materialize(&self) -> Result<PauliWord, PauliError> {
        PauliWord::new(self.x.materialize()?, self.z.materialize()?, self.phase)
    }
}
