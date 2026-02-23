use rand::Rng;

#[derive(Debug, Clone)]
pub struct BitTable {
    num_major: usize,
    num_minor: usize,
    words_per_row: usize,
    data: Vec<u64>,
}

impl BitTable {
    pub fn new(num_major: usize, num_minor: usize) -> Self {
        let words_per_row = (num_minor + 63) / 64;
        Self {
            num_major,
            num_minor,
            words_per_row,
            data: vec![0u64; num_major * words_per_row],
        }
    }

    pub fn num_major(&self) -> usize { self.num_major }
    pub fn num_minor(&self) -> usize { self.num_minor }
    pub fn words_per_row(&self) -> usize { self.words_per_row }

    pub fn get(&self, major: usize, minor: usize) -> bool {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        (self.data[word_idx] >> bit_idx) & 1 == 1
    }

    pub fn set(&mut self, major: usize, minor: usize, val: bool) {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        if val {
            self.data[word_idx] |= 1u64 << bit_idx;
        } else {
            self.data[word_idx] &= !(1u64 << bit_idx);
        }
    }

    pub fn toggle(&mut self, major: usize, minor: usize) {
        let word_idx = major * self.words_per_row + minor / 64;
        let bit_idx = minor % 64;
        self.data[word_idx] ^= 1u64 << bit_idx;
    }

    pub fn xor_row(&mut self, dst: usize, src: usize) {
        let dst_start = dst * self.words_per_row;
        let src_start = src * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[dst_start + w] ^= self.data[src_start + w];
        }
    }

    pub fn swap_rows(&mut self, a: usize, b: usize) {
        let a_start = a * self.words_per_row;
        let b_start = b * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data.swap(a_start + w, b_start + w);
        }
    }

    pub fn clear_row(&mut self, row: usize) {
        let start = row * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[start + w] = 0;
        }
    }

    pub fn randomize_row(&mut self, row: usize, rng: &mut impl Rng) {
        let start = row * self.words_per_row;
        for w in 0..self.words_per_row {
            self.data[start + w] = rng.r#gen();
        }
    }

    pub fn row_words(&self, row: usize) -> &[u64] {
        let start = row * self.words_per_row;
        &self.data[start..start + self.words_per_row]
    }

    pub fn row_words_mut(&mut self, row: usize) -> &mut [u64] {
        let start = row * self.words_per_row;
        &mut self.data[start..start + self.words_per_row]
    }
}
