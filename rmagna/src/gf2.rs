//! Packed binary vectors and reusable row reduction, with explicit dimensions.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gf2Error {
    WidthMismatch { expected: usize, actual: usize },
    IndexOutOfBounds { index: usize, width: usize },
    InvalidWordCount,
    NonzeroPadding,
    CertificateWordLimit { required: usize, limit: usize },
    AllocationFailed,
}

impl fmt::Display for Gf2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WidthMismatch { expected, actual } => {
                write!(f, "binary vector width is {actual}; expected {expected}")
            }
            Self::IndexOutOfBounds { index, width } => {
                write!(f, "binary vector index {index} is outside width {width}")
            }
            Self::InvalidWordCount => {
                write!(f, "packed word count does not match binary vector width")
            }
            Self::NonzeroPadding => write!(
                f,
                "packed binary vector has nonzero bits beyond its declared width"
            ),
            Self::CertificateWordLimit { required, limit } => write!(
                f,
                "full row certificates require {required} packed words; limit is {limit}"
            ),
            Self::AllocationFailed => write!(f, "unable to allocate packed binary vector storage"),
        }
    }
}

impl std::error::Error for Gf2Error {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BitVector {
    width: usize,
    words: Vec<u64>,
}

impl BitVector {
    pub fn zeros(width: usize) -> Result<Self, Gf2Error> {
        let count = width.div_ceil(64);
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| Gf2Error::AllocationFailed)?;
        words.resize(count, 0);
        Ok(Self { width, words })
    }

    pub fn from_words(width: usize, words: Vec<u64>) -> Result<Self, Gf2Error> {
        if words.len() != width.div_ceil(64) {
            return Err(Gf2Error::InvalidWordCount);
        }
        if !width.is_multiple_of(64) && words.last().is_some_and(|word| word >> (width % 64) != 0) {
            return Err(Gf2Error::NonzeroPadding);
        }
        Ok(Self { width, words })
    }

    pub fn from_indices(
        width: usize,
        indices: impl IntoIterator<Item = usize>,
    ) -> Result<Self, Gf2Error> {
        let mut value = Self::zeros(width)?;
        for index in indices {
            value.set(index, true)?;
        }
        Ok(value)
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn words(&self) -> &[u64] {
        &self.words
    }
    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|&word| word == 0)
    }
    pub fn count_ones(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    fn check_index(&self, index: usize) -> Result<(), Gf2Error> {
        if index >= self.width {
            return Err(Gf2Error::IndexOutOfBounds {
                index,
                width: self.width,
            });
        }
        Ok(())
    }

    fn check_width(&self, expected: usize) -> Result<(), Gf2Error> {
        if self.width != expected {
            return Err(Gf2Error::WidthMismatch {
                expected,
                actual: self.width,
            });
        }
        Ok(())
    }

    pub fn get(&self, index: usize) -> Result<bool, Gf2Error> {
        self.check_index(index)?;
        Ok((self.words[index / 64] >> (index % 64)) & 1 != 0)
    }

    pub fn set(&mut self, index: usize, bit: bool) -> Result<(), Gf2Error> {
        self.check_index(index)?;
        let mask = 1_u64 << (index % 64);
        if bit {
            self.words[index / 64] |= mask;
        } else {
            self.words[index / 64] &= !mask;
        }
        Ok(())
    }

    pub fn xor_assign(&mut self, other: &Self) -> Result<(), Gf2Error> {
        other.check_width(self.width)?;
        for (left, right) in self.words.iter_mut().zip(&other.words) {
            *left ^= right;
        }
        Ok(())
    }

    /// XOR indices at or above `start`, preserving lower indices and padding.
    pub(crate) fn xor_tail_assign(&mut self, other: &Self, start: usize) -> Result<(), Gf2Error> {
        other.check_width(self.width)?;
        if start == self.width {
            return Ok(());
        }
        self.check_index(start)?;
        let word = start / 64;
        self.words[word] ^= other.words[word] & (u64::MAX << (start % 64));
        for (left, right) in self.words.iter_mut().zip(&other.words).skip(word + 1) {
            *left ^= right;
        }
        Ok(())
    }

    pub(crate) fn xor_and_assign(&mut self, left: &Self, right: &Self) -> Result<(), Gf2Error> {
        self.check_width(left.width)?;
        self.check_width(right.width)?;
        for ((output, left), right) in self.words.iter_mut().zip(&left.words).zip(&right.words) {
            *output ^= left & right;
        }
        Ok(())
    }

    pub fn dot(&self, other: &Self) -> Result<bool, Gf2Error> {
        other.check_width(self.width)?;
        let parity = self
            .words
            .iter()
            .zip(&other.words)
            .fold(0, |acc, (a, b)| acc ^ (a & b).count_ones());
        Ok(parity & 1 != 0)
    }

    /// Keep indices below `limit`, clipping the declared width to the existing
    /// finite input. Even `usize::MAX` never expands storage or adds padding.
    pub fn low(&self, limit: usize) -> Result<Self, Gf2Error> {
        let mut result = Self::zeros(self.width.min(limit))?;
        let count = result.words.len();
        result.words.copy_from_slice(&self.words[..count]);
        if !result.width.is_multiple_of(64)
            && let Some(last) = result.words.last_mut()
        {
            *last &= (1_u64 << (result.width % 64)) - 1;
        }
        Ok(result)
    }

    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, &word)| {
                let mut remaining = word;
                std::iter::from_fn(move || {
                    if remaining == 0 {
                        return None;
                    }
                    let bit = remaining.trailing_zeros() as usize;
                    remaining &= remaining - 1;
                    Some(word_index * 64 + bit)
                })
            })
    }
}

/// Parities of each row against `value`, treating missing coordinates as zero.
/// Rows may have different explicit widths. The result width is `rows.len()`;
/// an all-zero result retains that width instead of using native Bits size ABI.
/// This does not change the strict equal-width [`BitVector::dot`] interface.
pub fn parities(rows: &[BitVector], value: &BitVector) -> Result<BitVector, Gf2Error> {
    let mut result = BitVector::zeros(rows.len())?;
    for (i, row) in rows.iter().enumerate() {
        let parity = row
            .words
            .iter()
            .zip(&value.words)
            .fold(0, |acc, (a, b)| acc ^ (a & b).count_ones());
        if parity & 1 != 0 {
            result.set(i, true)?;
        }
    }
    Ok(result)
}

/// XOR the selected rows of a rectangular matrix with explicit column `width`.
/// Every row must have this width. Nonzero selector bits outside the finite row
/// list reject; extra zero selector coordinates are harmless. Validation
/// precedes output allocation. Empty selections produce a width-preserving zero.
pub fn xors(
    width: usize,
    rows: &[BitVector],
    selection: &BitVector,
) -> Result<BitVector, Gf2Error> {
    for row in rows {
        row.check_width(width)?;
    }
    for index in selection.ones() {
        if index >= rows.len() {
            return Err(Gf2Error::IndexOutOfBounds {
                index,
                width: rows.len(),
            });
        }
    }
    let mut result = BitVector::zeros(width)?;
    for index in selection.ones() {
        result.xor_assign(&rows[index])?;
    }
    Ok(result)
}

/// XOR selected rows below `limit` into an existing destination. The row bound
/// clips to the finite row list; selector bits beyond it are intentionally
/// ignored. Selected rows may be shorter than the destination, preserving its
/// higher coordinates, but cannot be wider. All selected widths are checked
/// before any mutation. This operation allocates no storage and never grows
/// either the destination or inputs, including for `usize::MAX` bounds.
pub fn xor_rows_prefix(
    destination: &mut BitVector,
    rows: &[BitVector],
    selection: &BitVector,
    limit: usize,
) -> Result<(), Gf2Error> {
    let bound = limit.min(rows.len());
    for index in selection.ones().take_while(|&i| i < bound) {
        if rows[index].width > destination.width {
            return Err(Gf2Error::WidthMismatch {
                expected: destination.width,
                actual: rows[index].width,
            });
        }
    }
    for index in selection.ones().take_while(|&i| i < bound) {
        for (output, input) in destination.words.iter_mut().zip(&rows[index].words) {
            *output ^= input;
        }
    }
    Ok(())
}

/// One immutable factorization reused for multiple right-hand sides.
///
/// Dense row provenance is retained for ordinary systems. Tall systems instead
/// solve a small independent subset and verify every original equation, avoiding
/// a square provenance matrix in the number of boundary legs.
/// Free variables are zero in `solve`; `nullspace` supplies the independent
/// homogeneous directions for callers that need the whole affine solution set.
#[derive(Clone, Debug)]
pub struct LinearFactorization {
    width: usize,
    rows: Vec<BitVector>,
    rhs: RightHandSide,
    pivots: Vec<usize>,
}
#[derive(Clone, Debug)]
enum RightHandSide {
    Dense(Vec<BitVector>),
    Independent {
        original: Vec<BitVector>,
        selected: Vec<usize>,
        basis: Box<LinearFactorization>,
    },
}

#[cfg(test)]
thread_local! {
    pub(crate) static FACTORIZATION_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
impl LinearFactorization {
    pub fn new(width: usize, rows: &[BitVector]) -> Result<Self, Gf2Error> {
        let compressed = rows.len() > 64 && rows.len() > width.saturating_mul(2);
        Self::factor(width, rows, compressed)
    }

    /// Retain every row's reduction certificate, including the complete left
    /// kernel, even for tall systems. The quota counts retained packed `u64`
    /// certificate words: `rows.len() * ceil(rows.len() / 64)`. It excludes the
    /// input/reduced matrix, vector metadata and temporary reduction storage.
    /// Width and quota errors precede reduction or allocation. [`Self::new`]
    /// keeps its existing compressed behavior and does not require this quota.
    pub fn new_with_certificates(
        width: usize,
        rows: &[BitVector],
        max_certificate_words: usize,
    ) -> Result<Self, Gf2Error> {
        for row in rows {
            row.check_width(width)?;
        }
        let required = rows
            .len()
            .checked_mul(rows.len().div_ceil(64))
            .ok_or(Gf2Error::AllocationFailed)?;
        if required > max_certificate_words {
            return Err(Gf2Error::CertificateWordLimit {
                required,
                limit: max_certificate_words,
            });
        }
        Self::factor(width, rows, false)
    }

    fn factor(width: usize, rows: &[BitVector], compressed: bool) -> Result<Self, Gf2Error> {
        #[cfg(test)]
        FACTORIZATION_CALLS.with(|n| n.set(n.get() + 1));
        for row in rows {
            row.check_width(width)?;
        }
        let mut reduced = rows.to_vec();
        let mut provenance = if compressed {
            None
        } else {
            Some(
                (0..rows.len())
                    .map(|i| BitVector::from_indices(rows.len(), [i]))
                    .collect::<Result<Vec<_>, _>>()?,
            )
        };
        let mut origins = (0..rows.len()).collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut pivots = Vec::new();
        for column in 0..width {
            let rank = pivots.len();
            if rank == rows.len() {
                break;
            }
            let Some(found) =
                (rank..rows.len()).find(|&i| reduced[i].get(column).expect("validated column"))
            else {
                continue;
            };
            reduced.swap(rank, found);
            origins.swap(rank, found);
            selected.push(origins[rank]);
            if let Some(provenance) = &mut provenance {
                provenance.swap(rank, found);
            }
            let pivot_row = reduced[rank].clone();
            let pivot_provenance = provenance.as_ref().map(|p| p[rank].clone());
            for i in 0..rows.len() {
                if i != rank && reduced[i].get(column)? {
                    reduced[i].xor_assign(&pivot_row)?;
                    if let (Some(provenance), Some(pivot)) = (&mut provenance, &pivot_provenance) {
                        provenance[i].xor_assign(pivot)?;
                    }
                }
            }
            pivots.push(column);
        }
        let rhs = if let Some(provenance) = provenance {
            RightHandSide::Dense(provenance)
        } else {
            // Each selected original row is independent of all earlier selected
            // originals: eliminating earlier pivots left a new nonzero pivot.
            let independent = selected
                .iter()
                .map(|&i| rows[i].clone())
                .collect::<Vec<_>>();
            RightHandSide::Independent {
                original: rows.to_vec(),
                selected,
                basis: Box::new(Self::new(width, &independent)?),
            }
        };
        Ok(Self {
            width,
            rows: reduced,
            rhs,
            pivots,
        })
    }

    pub fn num_variables(&self) -> usize {
        self.width
    }
    pub fn num_equations(&self) -> usize {
        self.rows.len()
    }
    pub fn pivot_columns(&self) -> &[usize] {
        &self.pivots
    }

    pub fn rank(&self) -> usize {
        self.pivots.len()
    }
    pub fn row_space_basis(&self) -> &[BitVector] {
        &self.rows[..self.rank()]
    }

    /// Certificates for all reduced rows in order: XOR the selected original
    /// equations to recover that row. The first `rank()` rows correspond to
    /// [`Self::row_space_basis`]; the rest certify zero rows. Returns `None`
    /// when the default constructor selected compressed provenance.
    pub fn row_certificates(&self) -> Option<&[BitVector]> {
        match &self.rhs {
            RightHandSide::Dense(rows) => Some(rows),
            RightHandSide::Independent { .. } => None,
        }
    }

    /// A complete independent left kernel when full certificates are retained.
    /// Use [`Self::new_with_certificates`] to require this for a tall system.
    pub fn left_kernel(&self) -> Option<&[BitVector]> {
        self.row_certificates().map(|rows| &rows[self.rank()..])
    }

    /// Solve with every non-pivot coordinate fixed to its value in `seed`.
    /// Pivots keep the default lowest-column convention. Both input widths are
    /// validated before solving; inconsistency returns `None`. The factor,
    /// right-hand side and seed are never mutated.
    pub fn solve_seeded(
        &self,
        rhs: &BitVector,
        seed: &BitVector,
    ) -> Result<Option<BitVector>, Gf2Error> {
        rhs.check_width(self.num_equations())?;
        seed.check_width(self.width)?;
        let Some(particular) = self.solve(rhs)? else {
            return Ok(None);
        };
        let mut solution = seed.clone();
        for (row, &pivot) in self.rows.iter().zip(&self.pivots) {
            solution.set(
                pivot,
                particular.get(pivot)? ^ row.dot(seed)? ^ seed.get(pivot)?,
            )?;
        }
        Ok(Some(solution))
    }

    pub fn solve(&self, rhs: &BitVector) -> Result<Option<BitVector>, Gf2Error> {
        rhs.check_width(self.num_equations())?;
        let provenance = match &self.rhs {
            RightHandSide::Dense(provenance) => provenance,
            RightHandSide::Independent {
                original,
                selected,
                basis,
            } => {
                let subset = BitVector::from_indices(
                    selected.len(),
                    selected
                        .iter()
                        .enumerate()
                        .filter_map(|(j, &i)| rhs.get(i).expect("validated RHS").then_some(j)),
                )?;
                let Some(solution) = basis.solve(&subset)? else {
                    return Ok(None);
                };
                for (i, row) in original.iter().enumerate() {
                    if row.dot(&solution)? != rhs.get(i)? {
                        return Ok(None);
                    }
                }
                return Ok(Some(solution));
            }
        };
        let transformed = provenance
            .iter()
            .map(|row| row.dot(rhs))
            .collect::<Result<Vec<_>, _>>()?;
        if transformed[self.rank()..].iter().any(|&bit| bit) {
            return Ok(None);
        }
        let mut solution = BitVector::zeros(self.width)?;
        for (&pivot, &bit) in self.pivots.iter().zip(&transformed) {
            solution.set(pivot, bit)?;
        }
        Ok(Some(solution))
    }

    pub fn nullspace(&self) -> Result<Vec<BitVector>, Gf2Error> {
        let mut basis = Vec::new();
        for free in 0..self.width {
            if self.pivots.binary_search(&free).is_ok() {
                continue;
            }
            let mut vector = BitVector::from_indices(self.width, [free])?;
            for (row, &pivot) in self.rows.iter().zip(&self.pivots) {
                vector.set(pivot, row.get(free)?)?;
            }
            basis.push(vector);
        }
        Ok(basis)
    }
}

/// Transpose equal-width columns into `width` rows of width `columns.len()`.
/// All input dimensions are validated before allocation, and both the outer
/// row list and individual packed rows report [`Gf2Error::AllocationFailed`].
/// This is the same kernel used internally for affine-support preparation.
pub fn transpose_columns(width: usize, columns: &[BitVector]) -> Result<Vec<BitVector>, Gf2Error> {
    for column in columns {
        column.check_width(width)?;
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(width)
        .map_err(|_| Gf2Error::AllocationFailed)?;
    for _ in 0..width {
        rows.push(BitVector::zeros(columns.len())?);
    }
    for (j, column) in columns.iter().enumerate() {
        for i in column.ones() {
            rows[i].set(j, true)?;
        }
    }
    Ok(rows)
}

/// A nonempty affine binary family with independent support columns.
/// This type represents a set, with no quantum phase or probability attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineSupport {
    offset: BitVector,
    columns: Vec<BitVector>,
}
impl AffineSupport {
    pub fn new(offset: BitVector, columns: Vec<BitVector>) -> Result<Self, Gf2Error> {
        for column in &columns {
            column.check_width(offset.width())?;
        }
        let factor =
            LinearFactorization::new(columns.len(), &transpose_columns(offset.width(), &columns)?)?;
        let independent = factor
            .pivot_columns()
            .iter()
            .map(|&j| columns[j].clone())
            .collect();
        Ok(Self {
            offset,
            columns: independent,
        })
    }
    pub fn width(&self) -> usize {
        self.offset.width()
    }
    pub fn dimension(&self) -> usize {
        self.columns.len()
    }
    pub fn offset(&self) -> &BitVector {
        &self.offset
    }
    pub fn columns(&self) -> &[BitVector] {
        &self.columns
    }
    pub fn translated(&self, shift: &BitVector) -> Result<Self, Gf2Error> {
        let mut result = self.clone();
        result.offset.xor_assign(shift)?;
        Ok(result)
    }
    pub fn linear_image(&self, rows: &[BitVector]) -> Result<Self, Gf2Error> {
        for row in rows {
            row.check_width(self.width())?;
        }
        let map = |value: &BitVector| -> Result<BitVector, Gf2Error> {
            let mut result = BitVector::zeros(rows.len())?;
            for (j, row) in rows.iter().enumerate() {
                result.set(j, row.dot(value)?)?;
            }
            Ok(result)
        };
        Self::new(
            map(&self.offset)?,
            self.columns
                .iter()
                .map(map)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
    /// Restrict the set by parity outcomes; `None` denotes an empty family.
    pub fn condition(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
    ) -> Result<Option<Self>, Gf2Error> {
        outcomes.check_width(rows.len())?;
        let mut matrix = Vec::with_capacity(rows.len());
        let mut rhs = outcomes.clone();
        for (j, row) in rows.iter().enumerate() {
            row.check_width(self.width())?;
            let mut equation = BitVector::zeros(self.dimension())?;
            for (k, column) in self.columns.iter().enumerate() {
                equation.set(k, row.dot(column)?)?;
            }
            matrix.push(equation);
            rhs.set(j, rhs.get(j)? ^ row.dot(&self.offset)?)?;
        }
        let factor = LinearFactorization::new(self.dimension(), &matrix)?;
        let Some(shift) = factor.solve(&rhs)? else {
            return Ok(None);
        };
        let map = |value: &BitVector| -> Result<BitVector, Gf2Error> {
            let mut result = BitVector::zeros(self.width())?;
            for j in value.ones() {
                result.xor_assign(&self.columns[j])?;
            }
            Ok(result)
        };
        let mut offset = self.offset.clone();
        offset.xor_assign(&map(&shift)?)?;
        // An independent source map remains injective on the nullspace basis.
        Ok(Some(Self {
            offset,
            columns: factor
                .nullspace()?
                .iter()
                .map(map)
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}
