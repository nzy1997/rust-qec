#[derive(Debug, Clone)]
pub struct MeasureRecordBatch {
    batch_size: usize,
    words_per_row: usize,
    records: Vec<Vec<u64>>,
}

impl MeasureRecordBatch {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            words_per_row: (batch_size + 63) / 64,
            records: Vec::new(),
        }
    }

    /// Push a row of measurement bits (one word-slice per measurement)
    pub fn push_row(&mut self, words: &[u64]) {
        let mut row = vec![0u64; self.words_per_row];
        let copy_len = row.len().min(words.len());
        row[..copy_len].copy_from_slice(&words[..copy_len]);
        self.records.push(row);
    }

    pub fn push_zeros(&mut self) {
        self.records.push(vec![0u64; self.words_per_row]);
    }

    /// lookback(k, shot): get bit for rec[-k] for the given shot (k >= 1)
    pub fn lookback(&self, k: usize, shot: usize) -> bool {
        let idx = self.records.len() - k;
        let word = shot / 64;
        let bit = shot % 64;
        (self.records[idx][word] >> bit) & 1 == 1
    }

    pub fn lookback_words(&self, k: usize) -> &[u64] {
        let idx = self.records.len() - k;
        &self.records[idx]
    }

    pub fn xor_lookback_into(&self, k: usize, dest: &mut [u64]) {
        let idx = self.records.len() - k;
        for (d, s) in dest.iter_mut().zip(self.records[idx].iter()) {
            *d ^= *s;
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn words_per_row(&self) -> usize {
        self.words_per_row
    }
}
