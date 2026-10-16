#[derive(Debug, Clone)]
pub struct MeasureRecordBatch {
    batch_size: usize,
    words_per_row: usize,
    row_count: usize,
    records: Vec<u64>,
}

impl MeasureRecordBatch {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            words_per_row: (batch_size + 63) / 64,
            row_count: 0,
            records: Vec::new(),
        }
    }

    fn row_range(&self, row: usize) -> std::ops::Range<usize> {
        let start = row * self.words_per_row;
        start..start + self.words_per_row
    }

    fn row_words(&self, row: usize) -> &[u64] {
        let range = self.row_range(row);
        &self.records[range]
    }

    fn lookback_row(&self, k: usize) -> usize {
        self.row_count - k
    }

    /// Push a row of measurement bits (one word-slice per measurement)
    pub fn push_row(&mut self, words: &[u64]) {
        let start = self.records.len();
        self.records.resize(start + self.words_per_row, 0);
        let copy_len = self.words_per_row.min(words.len());
        self.records[start..start + copy_len].copy_from_slice(&words[..copy_len]);
        self.row_count += 1;
    }

    pub fn push_zeros(&mut self) {
        self.records
            .resize(self.records.len() + self.words_per_row, 0);
        self.row_count += 1;
    }

    /// lookback(k, shot): get bit for rec[-k] for the given shot (k >= 1)
    pub fn lookback(&self, k: usize, shot: usize) -> bool {
        let row = self.lookback_row(k);
        let word = shot / 64;
        let bit = shot % 64;
        (self.row_words(row)[word] >> bit) & 1 == 1
    }

    pub fn lookback_words(&self, k: usize) -> &[u64] {
        let row = self.lookback_row(k);
        self.row_words(row)
    }

    pub fn xor_lookback_into(&self, k: usize, dest: &mut [u64]) {
        let row = self.lookback_row(k);
        for (d, s) in dest.iter_mut().zip(self.row_words(row).iter()) {
            *d ^= *s;
        }
    }

    pub fn len(&self) -> usize {
        self.row_count
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn words_per_row(&self) -> usize {
        self.words_per_row
    }

    pub fn contiguous_words(&self) -> &[u64] {
        &self.records
    }
}
