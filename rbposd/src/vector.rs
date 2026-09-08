/// A measured parity syndrome, stored as one Boolean per check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syndrome(Vec<bool>);

/// A proposed bit correction, stored as one Boolean per matrix bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction(Vec<bool>);

impl Syndrome {
    /// Number of checks in the syndrome.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Borrow the syndrome bits.
    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

    /// Number of set syndrome bits.
    pub fn weight(&self) -> usize {
        self.0.iter().filter(|&&bit| bit).count()
    }
}

impl Correction {
    /// Number of bits in the correction.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Borrow the correction bits.
    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

    /// Construct an all-zero correction of the requested length.
    pub fn zero(len: usize) -> Self {
        Self(vec![false; len])
    }
}

impl From<Vec<bool>> for Syndrome {
    fn from(bits: Vec<bool>) -> Self {
        Self(bits)
    }
}

impl From<Vec<bool>> for Correction {
    fn from(bits: Vec<bool>) -> Self {
        Self(bits)
    }
}
