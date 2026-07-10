#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalTableauSnapshot {
    pub num_qubits: usize,
    pub x: Vec<Vec<bool>>,
    pub z: Vec<Vec<bool>>,
    pub phase: Vec<u8>,
}

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

    pub fn h(&mut self, q: usize) {
        self.check_qubit(q);
        self.swap_rows(q, self.num_qubits + q);
    }

    pub fn s(&mut self, q: usize) {
        self.check_qubit(q);
        let (x, z, negative) = self.evaluate_selected_rows(&[q, self.num_qubits + q], 1, true);
        self.set_row_words(q, &x, &z, negative);
    }

    pub fn s_dag(&mut self, q: usize) {
        self.check_qubit(q);
        let (x, z, negative) = self.evaluate_selected_rows(&[q, self.num_qubits + q], 1, false);
        self.set_row_words(q, &x, &z, negative);
    }

    pub fn x_gate(&mut self, q: usize) {
        self.check_qubit(q);
        self.toggle_sign_bit(self.num_qubits + q);
    }

    pub fn z_gate(&mut self, q: usize) {
        self.check_qubit(q);
        self.toggle_sign_bit(q);
    }

    pub fn y_gate(&mut self, q: usize) {
        self.check_qubit(q);
        self.toggle_sign_bit(q);
        self.toggle_sign_bit(self.num_qubits + q);
    }

    pub fn cx(&mut self, c: usize, t: usize) {
        self.check_qubit(c);
        self.check_qubit(t);

        let mut x_rows = [c, t];
        x_rows.sort_unstable();
        let mut z_rows = [self.num_qubits + c, self.num_qubits + t];
        z_rows.sort_unstable();

        let (new_x_c, new_z_c, new_sign_c) = self.evaluate_selected_rows(&x_rows, 0, false);
        let (new_x_nt, new_z_nt, new_sign_nt) = self.evaluate_selected_rows(&z_rows, 0, false);

        self.set_row_words(c, &new_x_c, &new_z_c, new_sign_c);
        self.set_row_words(self.num_qubits + t, &new_x_nt, &new_z_nt, new_sign_nt);
    }

    pub fn canonical_snapshot(&self) -> CanonicalTableauSnapshot {
        let num_rows = self.num_rows();
        let coeff_words = words_for_bits(num_rows);
        let mut x = vec![vec![false; self.num_qubits]; num_rows];
        let mut z = vec![vec![false; self.num_qubits]; num_rows];
        let mut phase = vec![0; num_rows];

        for target in 0..num_rows {
            let mut coeff = vec![0; coeff_words];
            for coeff_index in 0..num_rows {
                if self.symplectic_inverse_coeff_bit(target, coeff_index) {
                    set_bit(&mut coeff, coeff_index);
                }
            }

            for qubit in 0..self.num_qubits {
                x[target][qubit] = bit_from_words(&coeff, qubit);
                z[target][qubit] = bit_from_words(&coeff, self.num_qubits + qubit);
            }

            let (eval_x, eval_z, negative) = self.evaluate_coeff_words(&coeff, false);
            debug_assert!(
                self.is_basis_words(&eval_x, &eval_z, target),
                "inverse snapshot coefficients did not evaluate to basis row {target}",
            );
            phase[target] = if negative { 2 } else { 0 };
        }

        CanonicalTableauSnapshot {
            num_qubits: self.num_qubits,
            x,
            z,
            phase,
        }
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
        self.check_row(row);
        self.check_qubit(qubit);
        let word = self.plane_word_index(row, qubit);
        self.x_plane[word] |= 1u64 << (qubit % 64);
    }

    fn set_z_storage_bit(&mut self, row: usize, qubit: usize) {
        self.check_row(row);
        self.check_qubit(qubit);
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

    fn toggle_sign_bit(&mut self, row: usize) {
        self.check_row(row);
        let word = row / 64;
        self.signs[word] ^= 1u64 << (row % 64);
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        self.check_row(a);
        self.check_row(b);
        if a == b {
            return;
        }

        let a_start = self.row_start(a);
        let b_start = self.row_start(b);
        for offset in 0..self.words_per_row {
            self.x_plane.swap(a_start + offset, b_start + offset);
            self.z_plane.swap(a_start + offset, b_start + offset);
        }

        let a_sign = self.sign_bit(a);
        let b_sign = self.sign_bit(b);
        self.set_sign_bit(a, b_sign);
        self.set_sign_bit(b, a_sign);
    }

    fn set_row_words(&mut self, row: usize, x: &[u64], z: &[u64], negative: bool) {
        self.check_row(row);
        assert_eq!(x.len(), self.words_per_row);
        assert_eq!(z.len(), self.words_per_row);

        let start = self.row_start(row);
        self.x_plane[start..start + self.words_per_row].clone_from_slice(x);
        self.z_plane[start..start + self.words_per_row].clone_from_slice(z);
        self.set_sign_bit(row, negative);
        self.mask_row_padding(row);
    }

    fn evaluate_selected_rows(
        &self,
        selected_rows: &[usize],
        input_y_count_mod4: u8,
        input_negative: bool,
    ) -> (Vec<u64>, Vec<u64>, bool) {
        let mut coeff = vec![0; words_for_bits(self.num_rows())];
        for &row in selected_rows {
            self.check_row(row);
            toggle_bit(&mut coeff, row);
        }
        self.evaluate_coeff_words_with_input_y_count(&coeff, input_y_count_mod4, input_negative)
    }

    fn evaluate_coeff_words(
        &self,
        coeff: &[u64],
        input_negative: bool,
    ) -> (Vec<u64>, Vec<u64>, bool) {
        let input_y_count_mod4 = self.coeff_y_count_mod4(coeff);
        self.evaluate_coeff_words_with_input_y_count(coeff, input_y_count_mod4, input_negative)
    }

    fn evaluate_coeff_words_with_input_y_count(
        &self,
        coeff: &[u64],
        input_y_count_mod4: u8,
        input_negative: bool,
    ) -> (Vec<u64>, Vec<u64>, bool) {
        assert_eq!(coeff.len(), words_for_bits(self.num_rows()));

        let mut acc_x = vec![0; self.words_per_row];
        let mut acc_z = vec![0; self.words_per_row];
        let mut exponent = 0u8;

        for row in 0..self.num_rows() {
            if bit_from_words(coeff, row) {
                self.multiply_row_into_acc(row, &mut acc_x, &mut acc_z, &mut exponent);
            }
        }

        let input_exponent = (2 * u8::from(input_negative) + input_y_count_mod4) % 4;
        exponent = (exponent + input_exponent) % 4;
        let negative = sign_from_words(&acc_x, &acc_z, exponent);
        (acc_x, acc_z, negative)
    }

    fn multiply_row_into_acc(
        &self,
        src: usize,
        acc_x: &mut [u64],
        acc_z: &mut [u64],
        exponent: &mut u8,
    ) {
        self.check_row(src);
        let start = self.row_start(src);
        let src_x = &self.x_plane[start..start + self.words_per_row];
        let src_z = &self.z_plane[start..start + self.words_per_row];

        if z_dot_x_parity(acc_z, src_x) {
            *exponent = (*exponent + 2) % 4;
        }
        *exponent = (*exponent + self.row_exponent_mod4(src)) % 4;

        for offset in 0..self.words_per_row {
            acc_x[offset] ^= src_x[offset];
            acc_z[offset] ^= src_z[offset];
        }
    }

    fn row_exponent_mod4(&self, row: usize) -> u8 {
        (2 * u8::from(self.sign_bit(row)) + self.row_y_count_mod4(row)) % 4
    }

    fn row_y_count_mod4(&self, row: usize) -> u8 {
        self.check_row(row);
        let start = self.row_start(row);
        words_y_count_mod4(
            &self.x_plane[start..start + self.words_per_row],
            &self.z_plane[start..start + self.words_per_row],
        )
    }

    fn coeff_y_count_mod4(&self, coeff: &[u64]) -> u8 {
        let mut count = 0u32;
        for qubit in 0..self.num_qubits {
            if bit_from_words(coeff, qubit) && bit_from_words(coeff, self.num_qubits + qubit) {
                count += 1;
            }
        }
        (count % 4) as u8
    }

    fn symplectic_inverse_coeff_bit(&self, target: usize, coeff_index: usize) -> bool {
        let source_row = if coeff_index < self.num_qubits {
            self.num_qubits + coeff_index
        } else {
            coeff_index - self.num_qubits
        };
        let source_col = if target < self.num_qubits {
            self.num_qubits + target
        } else {
            target - self.num_qubits
        };
        self.raw_matrix_bit(source_row, source_col)
    }

    fn raw_matrix_bit(&self, row: usize, col: usize) -> bool {
        if col < self.num_qubits {
            let word = self.plane_word_index(row, col);
            bit_is_set(self.x_plane[word], col % 64)
        } else {
            let qubit = col - self.num_qubits;
            let word = self.plane_word_index(row, qubit);
            bit_is_set(self.z_plane[word], qubit % 64)
        }
    }

    fn is_basis_words(&self, x: &[u64], z: &[u64], row: usize) -> bool {
        for qubit in 0..self.num_qubits {
            let expected_x = row < self.num_qubits && row == qubit;
            let expected_z = row >= self.num_qubits && row - self.num_qubits == qubit;
            if bit_from_words(x, qubit) != expected_x || bit_from_words(z, qubit) != expected_z {
                return false;
            }
        }
        true
    }
}

fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(64)
}

fn bit_is_set(word: u64, bit: usize) -> bool {
    ((word >> bit) & 1) == 1
}

fn bit_from_words(words: &[u64], bit: usize) -> bool {
    bit_is_set(words[bit / 64], bit % 64)
}

fn set_bit(words: &mut [u64], bit: usize) {
    words[bit / 64] |= 1u64 << (bit % 64);
}

fn toggle_bit(words: &mut [u64], bit: usize) {
    words[bit / 64] ^= 1u64 << (bit % 64);
}

fn z_dot_x_parity(z: &[u64], x: &[u64]) -> bool {
    z.iter().zip(x).fold(0u32, |parity, (z_word, x_word)| {
        parity ^ ((z_word & x_word).count_ones() & 1)
    }) == 1
}

fn words_y_count_mod4(x: &[u64], z: &[u64]) -> u8 {
    let count = x.iter().zip(z).fold(0u32, |sum, (x_word, z_word)| {
        (sum + (x_word & z_word).count_ones()) % 4
    });
    count as u8
}

fn sign_from_words(x: &[u64], z: &[u64], exponent: u8) -> bool {
    let canonical_exponent = words_y_count_mod4(x, z);
    let diff = (exponent + 4 - canonical_exponent) % 4;
    assert!(
        diff == 0 || diff == 2,
        "packed Pauli row has non-Hermitian phase exponent {exponent}",
    );
    diff == 2
}
