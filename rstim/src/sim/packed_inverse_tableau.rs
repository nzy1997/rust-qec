#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedInverseTableau {
    num_qubits: usize,
    words_per_row: usize,
    x_plane: Vec<u64>,
    z_plane: Vec<u64>,
    signs: Vec<u64>,
}

impl PackedInverseTableau {
    pub fn identity(num_qubits: usize) -> Self {
        let words_per_row = words_for_bits(num_qubits);
        let num_rows = num_qubits
            .checked_mul(2)
            .expect("packed inverse tableau row count overflow");
        let plane_len = num_rows
            .checked_mul(words_per_row)
            .expect("packed inverse tableau plane length overflow");

        let mut tableau = Self {
            num_qubits,
            words_per_row,
            x_plane: vec![0; plane_len],
            z_plane: vec![0; plane_len],
            signs: vec![0; words_for_bits(num_rows)],
        };

        for qubit in 0..num_qubits {
            tableau.set_x_storage_bit(qubit, qubit);
            tableau.set_z_storage_bit(num_qubits + qubit, qubit);
        }

        tableau
    }

    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    pub fn num_rows(&self) -> usize {
        2 * self.num_qubits
    }

    pub fn words_per_row(&self) -> usize {
        self.words_per_row
    }

    pub fn x_plane_words(&self) -> &[u64] {
        &self.x_plane
    }

    pub fn z_plane_words(&self) -> &[u64] {
        &self.z_plane
    }

    pub fn sign_words(&self) -> &[u64] {
        &self.signs
    }

    pub fn x(&self, row: usize, qubit: usize) -> bool {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        bit_is_set(self.x_plane[word], qubit % 64)
    }

    pub fn z(&self, row: usize, qubit: usize) -> bool {
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        bit_is_set(self.z_plane[word], qubit % 64)
    }

    pub fn sign_bit(&self, row: usize) -> bool {
        self.check_row(row);
        bit_is_set(self.signs[row / 64], row % 64)
    }

    pub fn canonical_phase(&self, row: usize) -> u8 {
        if self.sign_bit(row) {
            2
        } else {
            0
        }
    }

    pub fn set_sign_bit(&mut self, row: usize, negative: bool) {
        self.check_row(row);
        let word = row / 64;
        let mask = 1u64 << (row % 64);
        if negative {
            self.signs[word] |= mask;
        } else {
            self.signs[word] &= !mask;
        }
    }

    pub fn set_canonical_phase(&mut self, row: usize, phase: u8) {
        match phase {
            0 => self.set_sign_bit(row, false),
            2 => self.set_sign_bit(row, true),
            _ => panic!("canonical phase must be 0 or 2, got {phase}"),
        }
    }

    pub fn copy_row(&mut self, src: usize, dst: usize) {
        self.check_row(src);
        self.check_row(dst);

        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        for offset in 0..self.words_per_row {
            self.x_plane[dst_start + offset] = self.x_plane[src_start + offset];
            self.z_plane[dst_start + offset] = self.z_plane[src_start + offset];
        }

        self.set_sign_bit(dst, self.sign_bit(src));
        self.mask_row_padding(dst);
    }

    /// XORs only the packed X/Z storage planes from `src` into `dst`.
    ///
    /// This is a storage primitive, not phase-aware Pauli multiplication:
    /// the destination sign is intentionally left unchanged.
    pub fn xor_pauli_planes(&mut self, src: usize, dst: usize) {
        self.check_row(src);
        self.check_row(dst);

        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        for offset in 0..self.words_per_row {
            self.x_plane[dst_start + offset] ^= self.x_plane[src_start + offset];
            self.z_plane[dst_start + offset] ^= self.z_plane[src_start + offset];
        }

        self.mask_row_padding(dst);
    }

    fn check_row(&self, row: usize) {
        assert!(
            row < self.num_rows(),
            "row index {row} out of range for {} rows",
            self.num_rows()
        );
    }

    fn check_qubit(&self, qubit: usize) {
        assert!(
            qubit < self.num_qubits,
            "qubit index {qubit} out of range for {} qubits",
            self.num_qubits
        );
    }

    fn row_start(&self, row: usize) -> usize {
        row * self.words_per_row
    }

    fn plane_word_index(&self, row: usize, qubit: usize) -> usize {
        self.row_start(row) + qubit / 64
    }

    fn set_x_storage_bit(&mut self, row: usize, qubit: usize) {
        let word = self.plane_word_index(row, qubit);
        self.x_plane[word] |= 1u64 << (qubit % 64);
    }

    fn set_z_storage_bit(&mut self, row: usize, qubit: usize) {
        let word = self.plane_word_index(row, qubit);
        self.z_plane[word] |= 1u64 << (qubit % 64);
    }

    fn mask_row_padding(&mut self, row: usize) {
        if self.words_per_row == 0 {
            return;
        }

        let mask = self.final_word_mask();
        let last_word = self.row_start(row) + self.words_per_row - 1;
        self.x_plane[last_word] &= mask;
        self.z_plane[last_word] &= mask;
    }

    fn final_word_mask(&self) -> u64 {
        let tail_bits = self.num_qubits % 64;
        if tail_bits == 0 {
            u64::MAX
        } else {
            (1u64 << tail_bits) - 1
        }
    }
}

fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(64)
}

fn bit_is_set(word: u64, bit: usize) -> bool {
    ((word >> bit) & 1) == 1
}
