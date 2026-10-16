#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syndrome(Vec<bool>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction(Vec<bool>);

impl Syndrome {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

    pub fn weight(&self) -> usize {
        self.0.iter().filter(|&&bit| bit).count()
    }
}

impl Correction {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

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
