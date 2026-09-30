//! Affine random-bit masks used when compiling a Clifford measurement suffix.

pub(crate) trait SymbolicMask: Clone {
    fn zero(words: usize) -> Self;
    fn fresh_bit(index: usize, words: usize) -> Option<Self>;
    fn xor_assign(&mut self, other: &Self);
    fn set_bit(&mut self, index: usize);
    fn parity_with(&self, other: &Self) -> bool;
}

impl SymbolicMask for u64 {
    fn zero(_: usize) -> Self {
        0
    }
    fn fresh_bit(index: usize, _: usize) -> Option<Self> {
        (index < 64).then(|| 1u64 << index)
    }
    fn xor_assign(&mut self, other: &Self) {
        *self ^= *other;
    }
    fn set_bit(&mut self, index: usize) {
        *self |= 1u64 << index;
    }
    fn parity_with(&self, other: &Self) -> bool {
        (self & other).count_ones() & 1 != 0
    }
}

impl SymbolicMask for u128 {
    fn zero(_: usize) -> Self {
        0
    }
    fn fresh_bit(index: usize, _: usize) -> Option<Self> {
        (index < 128).then(|| 1u128 << index)
    }
    fn xor_assign(&mut self, other: &Self) {
        *self ^= *other;
    }
    fn set_bit(&mut self, index: usize) {
        *self |= 1u128 << index;
    }
    fn parity_with(&self, other: &Self) -> bool {
        (self & other).count_ones() & 1 != 0
    }
}

impl SymbolicMask for Vec<u64> {
    fn zero(words: usize) -> Self {
        vec![0; words]
    }
    fn fresh_bit(index: usize, words: usize) -> Option<Self> {
        if index / 64 >= words {
            return None;
        }
        let mut mask = Self::zero(words);
        mask.set_bit(index);
        Some(mask)
    }
    fn xor_assign(&mut self, other: &Self) {
        for (word, other_word) in self.iter_mut().zip(other) {
            *word ^= other_word;
        }
    }
    fn set_bit(&mut self, index: usize) {
        self[index / 64] |= 1u64 << (index % 64);
    }
    fn parity_with(&self, other: &Self) -> bool {
        self.iter()
            .zip(other)
            .fold(0, |parity, (a, b)| parity ^ ((a & b).count_ones() & 1))
            != 0
    }
}
