// SPDX-License-Identifier: MPL-2.0
// Shared input-stream and byte representations for the attributed original GF2 cases.
use rmagna::gf2::BitVector;

// Standard mt19937_64 parameters. Compare every input bit and each case's
// unconsumed next word with the actual C++ engine, including across twists.
#[derive(Clone)]
pub struct Mt19937 {
    state: [u64; 312],
    index: usize,
    pub draws: usize,
    pub fingerprint: u64,
}
impl Mt19937 {
    pub fn new(seed: u64) -> Self {
        let mut state = [0; 312];
        state[0] = seed;
        for i in 1..312 {
            state[i] = 6364136223846793005_u64
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 62))
                .wrapping_add(i as u64);
        }
        Self {
            state,
            index: 312,
            draws: 0,
            fingerprint: 14695981039346656037,
        }
    }
    pub fn next(&mut self) -> u64 {
        if self.index == 312 {
            for i in 0..312 {
                let x =
                    (self.state[i] & 0xffffffff80000000) | (self.state[(i + 1) % 312] & 0x7fffffff);
                self.state[i] = self.state[(i + 156) % 312]
                    ^ (x >> 1)
                    ^ if x & 1 != 0 { 0xb5026f5aa96619e9 } else { 0 };
            }
            self.index = 0;
        }
        let mut x = self.state[self.index];
        self.index += 1;
        x ^= (x >> 29) & 0x5555555555555555;
        x ^= (x << 17) & 0x71d67fffeda60000;
        x ^= (x << 37) & 0xfff7eee000000000;
        let value = x ^ (x >> 43);
        self.draws += 1;
        self.fingerprint = (self.fingerprint ^ value).wrapping_mul(1099511628211);
        value
    }
    pub fn row(&mut self, n: usize, density: u64) -> Vec<u8> {
        (0..n)
            .map(|_| u8::from(self.next().is_multiple_of(density)))
            .collect()
    }
}

pub fn bytes(hex: &str, width: usize) -> Vec<u8> {
    assert_eq!(hex.len(), 2 * width.div_ceil(8));
    let packed = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    if !width.is_multiple_of(8) {
        assert_eq!(packed.last().unwrap() >> (width % 8), 0);
    }
    (0..width).map(|j| (packed[j / 8] >> (j % 8)) & 1).collect()
}
pub fn pack(row: &[u8]) -> BitVector {
    BitVector::from_indices(
        row.len(),
        row.iter()
            .enumerate()
            .filter_map(|(j, &b)| (b == 1).then_some(j)),
    )
    .unwrap()
}
pub fn equals_bytes(actual: &BitVector, expected: &[u8]) {
    assert_eq!(actual.width(), expected.len());
    for (i, &bit) in expected.iter().enumerate() {
        assert_eq!(actual.get(i).unwrap(), bit == 1, "coordinate {i}");
    }
    assert_eq!(
        actual.count_ones(),
        expected.iter().map(|&b| usize::from(b)).sum::<usize>()
    );
    assert_eq!(
        BitVector::from_words(actual.width(), actual.words().to_vec()).unwrap(),
        *actual
    );
}
