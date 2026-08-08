use rand::Rng;

use crate::data_path::ReferenceBuildPhaseCounters;

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

#[derive(Debug, Clone)]
struct PackedCanonicalRows {
    num_qubits: usize,
    words_per_row: usize,
    x_plane: Vec<u64>,
    z_plane: Vec<u64>,
    signs: Vec<u64>,
}

#[derive(Debug, Clone)]
struct PackedTransposedInverseTableau {
    num_qubits: usize,
    row_words: usize,
    x_columns: Vec<u64>,
    z_columns: Vec<u64>,
    signs: Vec<u64>,
}

impl PackedCanonicalRows {
    fn new(num_qubits: usize) -> Self {
        let words_per_row = words_for_bits(num_qubits);
        let num_rows = num_qubits
            .checked_mul(2)
            .expect("packed canonical row count overflow");
        let plane_len = num_rows
            .checked_mul(words_per_row)
            .expect("packed canonical plane length overflow");

        Self {
            num_qubits,
            words_per_row,
            x_plane: vec![0; plane_len],
            z_plane: vec![0; plane_len],
            signs: vec![0; words_for_bits(num_rows)],
        }
    }

    fn num_rows(&self) -> usize {
        2 * self.num_qubits
    }

    fn row_start(&self, row: usize) -> usize {
        assert!(row < self.num_rows(), "canonical row index out of range");
        row * self.words_per_row
    }

    fn x(&self, row: usize, qubit: usize) -> bool {
        assert!(
            qubit < self.num_qubits,
            "canonical qubit index out of range"
        );
        bit_is_set(self.x_plane[self.row_start(row) + qubit / 64], qubit % 64)
    }

    fn sign_bit(&self, row: usize) -> bool {
        assert!(row < self.num_rows(), "canonical row index out of range");
        bit_is_set(self.signs[row / 64], row % 64)
    }

    fn set_sign_bit(&mut self, row: usize, negative: bool) {
        assert!(row < self.num_rows(), "canonical row index out of range");
        let mask = 1u64 << (row % 64);
        if negative {
            self.signs[row / 64] |= mask;
        } else {
            self.signs[row / 64] &= !mask;
        }
    }

    fn set_z_row(&mut self, row: usize, qubit: usize) {
        assert!(
            qubit < self.num_qubits,
            "canonical qubit index out of range"
        );
        let start = self.row_start(row);
        self.x_plane[start..start + self.words_per_row].fill(0);
        self.z_plane[start..start + self.words_per_row].fill(0);
        self.z_plane[start + qubit / 64] |= 1u64 << (qubit % 64);
        self.set_sign_bit(row, false);
    }

    fn copy_row(&mut self, src: usize, dst: usize) {
        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        if src != dst {
            self.x_plane
                .copy_within(src_start..src_start + self.words_per_row, dst_start);
            self.z_plane
                .copy_within(src_start..src_start + self.words_per_row, dst_start);
        }
        self.set_sign_bit(dst, self.sign_bit(src));
    }

    fn row_exponent_mod4(&self, row: usize) -> u8 {
        let start = self.row_start(row);
        (2 * u8::from(self.sign_bit(row))
            + words_y_count_mod4(
                &self.x_plane[start..start + self.words_per_row],
                &self.z_plane[start..start + self.words_per_row],
            ))
            % 4
    }

    fn multiply_row_into(&mut self, src: usize, dst: usize) {
        assert_ne!(src, dst, "cannot multiply a canonical row into itself");
        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        let forward_phase = words_dot_count_mod4(
            &self.x_plane[dst_start..dst_start + self.words_per_row],
            &self.z_plane[src_start..src_start + self.words_per_row],
        );
        let reverse_phase = words_dot_count_mod4(
            &self.z_plane[dst_start..dst_start + self.words_per_row],
            &self.x_plane[src_start..src_start + self.words_per_row],
        );
        let exponent = (2 * u8::from(self.sign_bit(dst))
            + 2 * u8::from(self.sign_bit(src))
            + forward_phase
            + 4
            - reverse_phase)
            % 4;

        for offset in 0..self.words_per_row {
            let src_x = self.x_plane[src_start + offset];
            let src_z = self.z_plane[src_start + offset];
            self.x_plane[dst_start + offset] ^= src_x;
            self.z_plane[dst_start + offset] ^= src_z;
        }

        assert!(
            exponent == 0 || exponent == 2,
            "packed canonical rows must remain Hermitian",
        );
        self.set_sign_bit(dst, exponent == 2);
    }

    fn multiply_row_into_acc(
        &self,
        src: usize,
        acc_x: &mut [u64],
        acc_z: &mut [u64],
        exponent: &mut u8,
    ) {
        assert_eq!(acc_x.len(), self.words_per_row);
        assert_eq!(acc_z.len(), self.words_per_row);
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

    fn evaluate_coeff_words(&self, coeff: &[u64]) -> (Vec<u64>, Vec<u64>, bool) {
        assert_eq!(coeff.len(), words_for_bits(self.num_rows()));
        let mut acc_x = vec![0; self.words_per_row];
        let mut acc_z = vec![0; self.words_per_row];
        let mut exponent = 0u8;
        let mut input_y_count = 0u32;

        for qubit in 0..self.num_qubits {
            if bit_from_words(coeff, qubit) && bit_from_words(coeff, self.num_qubits + qubit) {
                input_y_count += 1;
            }
        }
        for row in 0..self.num_rows() {
            if bit_from_words(coeff, row) {
                self.multiply_row_into_acc(row, &mut acc_x, &mut acc_z, &mut exponent);
            }
        }
        exponent = (exponent + (input_y_count % 4) as u8) % 4;
        let negative = sign_from_words(&acc_x, &acc_z, exponent);
        (acc_x, acc_z, negative)
    }
}

impl PackedTransposedInverseTableau {
    fn from_tableau(tableau: &PackedInverseTableau) -> Self {
        let num_qubits = tableau.num_qubits;
        let row_words = words_for_bits(tableau.num_rows());
        let mut view = Self {
            num_qubits,
            row_words,
            x_columns: vec![0; num_qubits * row_words],
            z_columns: vec![0; num_qubits * row_words],
            signs: tableau.signs.clone(),
        };
        transpose_packed_plane(
            &tableau.x_plane,
            tableau.num_rows(),
            num_qubits,
            tableau.words_per_row,
            &mut view.x_columns,
        );
        transpose_packed_plane(
            &tableau.z_plane,
            tableau.num_rows(),
            num_qubits,
            tableau.words_per_row,
            &mut view.z_columns,
        );
        view
    }

    fn write_back(self, tableau: &mut PackedInverseTableau) {
        assert_eq!(self.num_qubits, tableau.num_qubits);
        tableau.signs.clone_from_slice(&self.signs);
        transpose_packed_plane(
            &self.x_columns,
            self.num_qubits,
            tableau.num_rows(),
            self.row_words,
            &mut tableau.x_plane,
        );
        transpose_packed_plane(
            &self.z_columns,
            self.num_qubits,
            tableau.num_rows(),
            self.row_words,
            &mut tableau.z_plane,
        );
    }

    fn collapse_z(&mut self, target: usize) -> bool {
        self.collapse_z_with_pivot(target).is_some()
    }

    fn collapse_z_with_pivot(&mut self, target: usize) -> Option<usize> {
        let pivot = self.find_zx_pivot(target)?;

        for qubit in pivot + 1..self.num_qubits {
            if self.z_x(target, qubit) {
                self.append_zcx(pivot, qubit);
            }
        }

        if self.z_z(target, pivot) {
            self.append_h_yz(pivot);
        } else {
            self.append_h_xz(pivot);
        }
        if self.z_sign(target) {
            self.append_x(pivot);
        }
        Some(pivot)
    }

    fn find_zx_pivot(&self, target: usize) -> Option<usize> {
        (0..self.num_qubits).find(|&qubit| self.z_x(target, qubit))
    }

    fn z_x(&self, z_row_qubit: usize, x_qubit: usize) -> bool {
        bit_from_words(self.x_column(x_qubit), self.num_qubits + z_row_qubit)
    }

    fn z_z(&self, z_row_qubit: usize, z_qubit: usize) -> bool {
        bit_from_words(self.z_column(z_qubit), self.num_qubits + z_row_qubit)
    }

    fn z_sign(&self, target: usize) -> bool {
        bit_from_words(&self.signs, self.num_qubits + target)
    }

    fn append_zcx(&mut self, control: usize, target: usize) {
        for word in 0..self.row_words {
            let cx = self.x_column(control)[word];
            let cz = self.z_column(control)[word];
            let tx = self.x_column(target)[word];
            let tz = self.z_column(target)[word];
            self.signs[word] ^= (cx & tz) & !(cz ^ tx);
            self.z_column_mut(control)[word] ^= tz;
            self.x_column_mut(target)[word] ^= cx;
        }
    }

    fn append_h_xz(&mut self, q: usize) {
        for word in 0..self.row_words {
            let x = self.x_column(q)[word];
            let z = self.z_column(q)[word];
            self.signs[word] ^= x & z;
            self.x_column_mut(q)[word] = z;
            self.z_column_mut(q)[word] = x;
        }
    }

    fn append_h_yz(&mut self, q: usize) {
        for word in 0..self.row_words {
            let x = self.x_column(q)[word];
            let z = self.z_column(q)[word];
            self.signs[word] ^= x & !z;
            self.x_column_mut(q)[word] = x ^ z;
        }
    }

    fn append_x(&mut self, q: usize) {
        for word in 0..self.row_words {
            self.signs[word] ^= self.z_column(q)[word];
        }
    }

    fn x_column(&self, qubit: usize) -> &[u64] {
        let start = qubit * self.row_words;
        &self.x_columns[start..start + self.row_words]
    }

    fn x_column_mut(&mut self, qubit: usize) -> &mut [u64] {
        let start = qubit * self.row_words;
        &mut self.x_columns[start..start + self.row_words]
    }

    fn z_column(&self, qubit: usize) -> &[u64] {
        let start = qubit * self.row_words;
        &self.z_columns[start..start + self.row_words]
    }

    fn z_column_mut(&mut self, qubit: usize) -> &mut [u64] {
        let start = qubit * self.row_words;
        &mut self.z_columns[start..start + self.row_words]
    }
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

    pub(crate) fn reset_identity(&mut self) {
        self.x_plane.fill(0);
        self.z_plane.fill(0);
        self.signs.fill(0);
        for qubit in 0..self.num_qubits {
            self.set_x_storage_bit(qubit, qubit);
            self.set_z_storage_bit(self.num_qubits + qubit, qubit);
        }
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
        if self.sign_bit(row) { 2 } else { 0 }
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
        assert_ne!(c, t, "CX control and target must differ");

        // These pairs are commuting inverse images of basis Paulis. Multiplying directly into
        // the destination avoids four temporary vectors and a scan of every tableau row.
        self.multiply_commuting_row_into(t, c);
        self.multiply_commuting_row_into(self.num_qubits + c, self.num_qubits + t);
    }

    fn canonical_rows(&self) -> PackedCanonicalRows {
        let mut rows = PackedCanonicalRows::new(self.num_qubits);
        let coeff_words = words_for_bits(self.num_rows());

        for target in 0..self.num_rows() {
            let mut coeff = vec![0; coeff_words];
            for coeff_index in 0..self.num_rows() {
                if self.symplectic_inverse_coeff_bit(target, coeff_index) {
                    set_bit(&mut coeff, coeff_index);
                }
            }

            let row_start = rows.row_start(target);
            for qubit in 0..self.num_qubits {
                if bit_from_words(&coeff, qubit) {
                    rows.x_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
                if bit_from_words(&coeff, self.num_qubits + qubit) {
                    rows.z_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
            }

            let (eval_x, eval_z, negative) = self.evaluate_coeff_words(&coeff, false);
            debug_assert!(self.is_basis_words(&eval_x, &eval_z, target));
            rows.set_sign_bit(target, negative);
        }

        rows
    }

    fn replace_from_canonical_rows(&mut self, rows: &PackedCanonicalRows) {
        assert_eq!(rows.num_qubits, self.num_qubits);
        let coeff_words = words_for_bits(self.num_rows());

        for target in 0..self.num_rows() {
            let mut coeff = vec![0; coeff_words];
            for coeff_index in 0..self.num_rows() {
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
                if canonical_raw_matrix_bit(rows, source_row, source_col) {
                    set_bit(&mut coeff, coeff_index);
                }
            }

            let row_start = self.row_start(target);
            self.x_plane[row_start..row_start + self.words_per_row].fill(0);
            self.z_plane[row_start..row_start + self.words_per_row].fill(0);
            for qubit in 0..self.num_qubits {
                if bit_from_words(&coeff, qubit) {
                    self.x_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
                if bit_from_words(&coeff, self.num_qubits + qubit) {
                    self.z_plane[row_start + qubit / 64] |= 1u64 << (qubit % 64);
                }
            }

            let (eval_x, eval_z, negative) = rows.evaluate_coeff_words(&coeff);
            debug_assert!(self.is_basis_words(&eval_x, &eval_z, target));
            self.set_sign_bit(target, negative);
            self.mask_row_padding(target);
        }
    }

    #[doc(hidden)]
    pub fn collapse_z_many_biased(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        counters.direct_inverse_batches += 1;

        let mut bits = Vec::with_capacity(targets.len());
        let mut random_targets = Vec::new();
        for (index, &(q, inverted)) in targets.iter().enumerate() {
            self.check_qubit(q);
            let z_row = self.num_qubits + q;
            if self.row_has_x_support(z_row) {
                bits.push(inverted);
                random_targets.push((index, q, inverted));
            } else {
                bits.push(self.sign_bit(z_row) ^ inverted);
            }
        }

        if random_targets.is_empty() {
            return bits;
        }

        counters.transposed_collapse_batches += 1;
        let mut transposed = PackedTransposedInverseTableau::from_tableau(self);
        for (index, q, inverted) in random_targets {
            if transposed.collapse_z(q) {
                counters.collapse_pivots += 1;
            }
            bits[index] = transposed.z_sign(q) ^ inverted;
        }
        transposed.write_back(self);
        bits
    }

    fn row_has_x_support(&self, row: usize) -> bool {
        self.check_row(row);
        let start = self.row_start(row);
        self.x_plane[start..start + self.words_per_row]
            .iter()
            .any(|word| *word != 0)
    }

    fn measure_z_raw_biased(&mut self, q: usize) -> bool {
        self.measure_z_raw_biased_with_counters(q, None)
    }

    fn measure_z_raw_biased_with_counters(
        &mut self,
        q: usize,
        mut counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        self.check_qubit(q);
        if let Some(counters) = counters.as_deref_mut() {
            counters.canonical_materializations += 1;
        }
        let mut rows = self.canonical_rows();
        let (raw, changed) =
            self.measure_z_raw_biased_in_rows(&mut rows, q, counters.as_deref_mut());
        if changed {
            if let Some(counters) = counters.as_deref_mut() {
                counters.canonical_writebacks += 1;
            }
            self.replace_from_canonical_rows(&rows);
        }
        raw
    }

    fn measure_z_raw_biased_in_rows(
        &self,
        rows: &mut PackedCanonicalRows,
        q: usize,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> (bool, bool) {
        let mut pivot = None;
        for row in self.num_qubits..self.num_rows() {
            if rows.x(row, q) {
                pivot = Some(row);
                break;
            }
        }

        if let Some(p) = pivot {
            if let Some(counters) = counters {
                counters.collapse_pivots += 1;
            }
            let destabilizer = p - self.num_qubits;
            for row in 0..self.num_rows() {
                if row != p && row != destabilizer && rows.x(row, q) {
                    rows.multiply_row_into(p, row);
                }
            }
            rows.copy_row(p, destabilizer);
            rows.set_z_row(p, q);
            return (false, true);
        }

        let mut temp_x = vec![0; self.words_per_row];
        let mut temp_z = vec![0; self.words_per_row];
        temp_z[q / 64] |= 1u64 << (q % 64);
        let mut exponent = 0u8;
        for row in 0..self.num_qubits {
            if rows.x(row, q) {
                rows.multiply_row_into_acc(
                    row + self.num_qubits,
                    &mut temp_x,
                    &mut temp_z,
                    &mut exponent,
                );
            }
        }
        (sign_from_words(&temp_x, &temp_z, exponent), false)
    }

    pub fn measure_z_biased(&mut self, q: usize, inverted: bool) -> bool {
        self.measure_z_raw_biased(q) ^ inverted
    }

    pub fn measure_z_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_z_many_biased_with_optional_counters(targets, None)
    }

    /// Measures several Z observables in target order using random outcomes when required.
    ///
    /// The tableau is transposed once for the whole commuting measurement batch. This keeps
    /// the result equivalent to repeated single-qubit measurements while avoiding a full
    /// canonical materialization per target.
    pub fn measure_z_many<R: Rng + ?Sized>(
        &mut self,
        targets: &[(usize, bool)],
        rng: &mut R,
    ) -> Vec<bool> {
        if targets.is_empty() {
            return Vec::new();
        }
        for &(q, _) in targets {
            self.check_qubit(q);
        }

        let mut transposed = PackedTransposedInverseTableau::from_tableau(self);
        let mut bits = Vec::with_capacity(targets.len());
        for &(q, inverted) in targets {
            if let Some(pivot) = transposed.collapse_z_with_pivot(q) {
                if rng.r#gen::<bool>() {
                    transposed.append_x(pivot);
                }
            }
            bits.push(transposed.z_sign(q) ^ inverted);
        }
        transposed.write_back(self);
        bits
    }

    pub(crate) fn measure_z_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_z_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_z_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        match counters {
            Some(counters) => self.collapse_z_many_biased(targets, counters),
            None => {
                let mut counters = ReferenceBuildPhaseCounters::default();
                self.collapse_z_many_biased(targets, &mut counters)
            }
        }
    }

    pub fn measure_x_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_x_many_biased_with_optional_counters(targets, None)
    }

    pub(crate) fn measure_x_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_x_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_x_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        let qubits = unique_target_qubits(targets);
        for &q in &qubits {
            self.h(q);
        }
        let bits = self.measure_z_many_biased_with_optional_counters(targets, counters);
        for &q in &qubits {
            self.h(q);
        }
        bits
    }

    pub fn measure_y_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_y_many_biased_with_optional_counters(targets, None)
    }

    pub(crate) fn measure_y_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_y_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_y_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        let qubits = unique_target_qubits(targets);
        for &q in &qubits {
            self.s_dag(q);
            self.h(q);
        }
        let bits = self.measure_z_many_biased_with_optional_counters(targets, counters);
        for &q in &qubits {
            self.h(q);
            self.s(q);
        }
        bits
    }

    pub fn measure_x_biased(&mut self, q: usize, inverted: bool) -> bool {
        self.measure_x_biased_with_optional_counters(q, inverted, None)
    }

    fn measure_x_biased_with_optional_counters(
        &mut self,
        q: usize,
        inverted: bool,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        self.h(q);
        let bit = self.measure_z_raw_biased_with_counters(q, counters) ^ inverted;
        self.h(q);
        bit
    }

    pub fn measure_y_biased(&mut self, q: usize, inverted: bool) -> bool {
        self.measure_y_biased_with_optional_counters(q, inverted, None)
    }

    fn measure_y_biased_with_optional_counters(
        &mut self,
        q: usize,
        inverted: bool,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        self.s_dag(q);
        self.h(q);
        let bit = self.measure_z_raw_biased_with_counters(q, counters) ^ inverted;
        self.h(q);
        self.s(q);
        bit
    }

    pub fn measure_reset_z_biased(&mut self, q: usize, inverted: bool) -> bool {
        let raw = self.measure_z_raw_biased(q);
        if raw {
            self.x_gate(q);
        }
        raw ^ inverted
    }

    pub fn measure_reset_z_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_reset_z_many_biased_with_optional_counters(targets, None)
    }

    /// Measures and resets several Z observables using random outcomes when required.
    pub fn measure_reset_z_many<R: Rng + ?Sized>(
        &mut self,
        targets: &[(usize, bool)],
        rng: &mut R,
    ) -> Vec<bool> {
        let qubits: Vec<usize> = targets.iter().map(|(q, _)| *q).collect();
        if has_duplicate_qubits(&qubits) {
            return targets
                .iter()
                .map(|&(q, inverted)| {
                    let bit = self.measure_z_many(&[(q, inverted)], rng)[0];
                    if bit ^ inverted {
                        self.x_gate(q);
                    }
                    bit
                })
                .collect();
        }

        let reported = self.measure_z_many(targets, rng);
        for (&(q, inverted), &bit) in targets.iter().zip(&reported) {
            if bit ^ inverted {
                self.x_gate(q);
            }
        }
        reported
    }

    /// Resets several qubits to |0> while batching their commuting measurements.
    pub fn reset_z_many<R: Rng + ?Sized>(&mut self, qubits: &[usize], rng: &mut R) {
        let targets: Vec<(usize, bool)> = qubits.iter().map(|&q| (q, false)).collect();
        let _ = self.measure_reset_z_many(&targets, rng);
    }

    pub(crate) fn measure_reset_z_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_reset_z_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_reset_z_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        mut counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        let qubits: Vec<usize> = targets.iter().map(|(q, _)| *q).collect();
        if has_duplicate_qubits(&qubits) {
            return targets
                .iter()
                .map(|&(q, inverted)| {
                    self.measure_reset_z_biased_with_optional_counters(
                        q,
                        inverted,
                        counters.as_deref_mut(),
                    )
                })
                .collect();
        }

        let reported = self.measure_z_many_biased_with_optional_counters(targets, counters);
        for (&(q, inverted), &bit) in targets.iter().zip(&reported) {
            if bit ^ inverted {
                self.x_gate(q);
            }
        }
        reported
    }

    fn measure_reset_z_biased_with_optional_counters(
        &mut self,
        q: usize,
        inverted: bool,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        let bit = self.measure_z_many_biased_with_optional_counters(&[(q, inverted)], counters)[0];
        if bit ^ inverted {
            self.x_gate(q);
        }
        bit
    }

    pub fn measure_reset_x_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_reset_x_many_biased_with_optional_counters(targets, None)
    }

    pub(crate) fn measure_reset_x_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_reset_x_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_reset_x_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        let qubits = unique_target_qubits(targets);
        for &q in &qubits {
            self.h(q);
        }
        let bits = self.measure_reset_z_many_biased_with_optional_counters(targets, counters);
        for &q in &qubits {
            self.h(q);
        }
        bits
    }

    pub fn measure_reset_y_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool> {
        self.measure_reset_y_many_biased_with_optional_counters(targets, None)
    }

    pub(crate) fn measure_reset_y_many_biased_with_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: &mut ReferenceBuildPhaseCounters,
    ) -> Vec<bool> {
        self.measure_reset_y_many_biased_with_optional_counters(targets, Some(counters))
    }

    fn measure_reset_y_many_biased_with_optional_counters(
        &mut self,
        targets: &[(usize, bool)],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> Vec<bool> {
        let qubits = unique_target_qubits(targets);
        for &q in &qubits {
            self.s_dag(q);
            self.h(q);
        }
        let bits = self.measure_reset_z_many_biased_with_optional_counters(targets, counters);
        for &q in &qubits {
            self.h(q);
            self.s(q);
        }
        bits
    }

    pub fn measure_reset_x_biased(&mut self, q: usize, inverted: bool) -> bool {
        self.measure_reset_x_biased_with_optional_counters(q, inverted, None)
    }

    fn measure_reset_x_biased_with_optional_counters(
        &mut self,
        q: usize,
        inverted: bool,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        self.h(q);
        let raw = self.measure_z_raw_biased_with_counters(q, counters);
        if raw {
            self.x_gate(q);
        }
        let bit = raw ^ inverted;
        self.h(q);
        bit
    }

    pub fn measure_reset_y_biased(&mut self, q: usize, inverted: bool) -> bool {
        self.measure_reset_y_biased_with_optional_counters(q, inverted, None)
    }

    fn measure_reset_y_biased_with_optional_counters(
        &mut self,
        q: usize,
        inverted: bool,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) -> bool {
        self.s_dag(q);
        self.h(q);
        let raw = self.measure_z_raw_biased_with_counters(q, counters);
        if raw {
            self.x_gate(q);
        }
        let bit = raw ^ inverted;
        self.h(q);
        self.s(q);
        bit
    }

    pub fn reset_z_biased(&mut self, q: usize) {
        let raw = self.measure_z_raw_biased(q);
        if raw {
            self.x_gate(q);
        }
    }

    pub fn reset_z_many_biased(&mut self, qubits: &[usize]) {
        self.reset_z_many_biased_with_optional_counters(qubits, None);
    }

    pub(crate) fn reset_z_many_biased_with_counters(
        &mut self,
        qubits: &[usize],
        counters: &mut ReferenceBuildPhaseCounters,
    ) {
        self.reset_z_many_biased_with_optional_counters(qubits, Some(counters));
    }

    fn reset_z_many_biased_with_optional_counters(
        &mut self,
        qubits: &[usize],
        mut counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) {
        if has_duplicate_qubits(qubits) {
            for &q in qubits {
                self.reset_z_many_biased_with_optional_counters(&[q], counters.as_deref_mut());
            }
            return;
        }

        let targets: Vec<(usize, bool)> = qubits.iter().map(|&q| (q, false)).collect();
        let bits = self.measure_z_many_biased_with_optional_counters(&targets, counters);
        for (&q, &bit) in qubits.iter().zip(&bits) {
            if bit {
                self.x_gate(q);
            }
        }
    }

    pub fn reset_x_many_biased(&mut self, qubits: &[usize]) {
        self.reset_x_many_biased_with_optional_counters(qubits, None);
    }

    pub(crate) fn reset_x_many_biased_with_counters(
        &mut self,
        qubits: &[usize],
        counters: &mut ReferenceBuildPhaseCounters,
    ) {
        self.reset_x_many_biased_with_optional_counters(qubits, Some(counters));
    }

    fn reset_x_many_biased_with_optional_counters(
        &mut self,
        qubits: &[usize],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) {
        let unique_qubits = unique_qubits(qubits);
        for &q in &unique_qubits {
            self.h(q);
        }
        self.reset_z_many_biased_with_optional_counters(qubits, counters);
        for &q in &unique_qubits {
            self.h(q);
        }
    }

    pub fn reset_y_many_biased(&mut self, qubits: &[usize]) {
        self.reset_y_many_biased_with_optional_counters(qubits, None);
    }

    pub(crate) fn reset_y_many_biased_with_counters(
        &mut self,
        qubits: &[usize],
        counters: &mut ReferenceBuildPhaseCounters,
    ) {
        self.reset_y_many_biased_with_optional_counters(qubits, Some(counters));
    }

    fn reset_y_many_biased_with_optional_counters(
        &mut self,
        qubits: &[usize],
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) {
        let unique_qubits = unique_qubits(qubits);
        for &q in &unique_qubits {
            self.s_dag(q);
            self.h(q);
        }
        self.reset_z_many_biased_with_optional_counters(qubits, counters);
        for &q in &unique_qubits {
            self.h(q);
            self.s(q);
        }
    }

    pub fn reset_x_biased(&mut self, q: usize) {
        self.reset_x_biased_with_optional_counters(q, None);
    }

    fn reset_x_biased_with_optional_counters(
        &mut self,
        q: usize,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) {
        self.h(q);
        let raw = self.measure_z_raw_biased_with_counters(q, counters);
        if raw {
            self.x_gate(q);
        }
        self.h(q);
    }

    pub fn reset_y_biased(&mut self, q: usize) {
        self.reset_y_biased_with_optional_counters(q, None);
    }

    fn reset_y_biased_with_optional_counters(
        &mut self,
        q: usize,
        counters: Option<&mut ReferenceBuildPhaseCounters>,
    ) {
        self.s_dag(q);
        self.h(q);
        let raw = self.measure_z_raw_biased_with_counters(q, counters);
        if raw {
            self.x_gate(q);
        }
        self.h(q);
        self.s(q);
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

    fn multiply_commuting_row_into(&mut self, src: usize, dst: usize) {
        self.check_row(src);
        self.check_row(dst);
        assert_ne!(src, dst, "cannot multiply a row into itself");

        let src_start = self.row_start(src);
        let dst_start = self.row_start(dst);
        let mut exponent = self.row_exponent_mod4(dst);
        if z_dot_x_parity(
            &self.z_plane[dst_start..dst_start + self.words_per_row],
            &self.x_plane[src_start..src_start + self.words_per_row],
        ) {
            exponent = (exponent + 2) % 4;
        }
        exponent = (exponent + self.row_exponent_mod4(src)) % 4;

        for offset in 0..self.words_per_row {
            self.x_plane[dst_start + offset] ^= self.x_plane[src_start + offset];
            self.z_plane[dst_start + offset] ^= self.z_plane[src_start + offset];
        }
        let negative = sign_from_words(
            &self.x_plane[dst_start..dst_start + self.words_per_row],
            &self.z_plane[dst_start..dst_start + self.words_per_row],
            exponent,
        );
        self.set_sign_bit(dst, negative);
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

fn canonical_raw_matrix_bit(rows: &PackedCanonicalRows, row: usize, col: usize) -> bool {
    assert!(row < rows.num_rows(), "canonical row index out of range");
    assert!(col < rows.num_rows(), "canonical column index out of range");
    if col < rows.num_qubits {
        bit_is_set(rows.x_plane[rows.row_start(row) + col / 64], col % 64)
    } else {
        let qubit = col - rows.num_qubits;
        bit_is_set(rows.z_plane[rows.row_start(row) + qubit / 64], qubit % 64)
    }
}

fn has_duplicate_qubits(qubits: &[usize]) -> bool {
    let mut sorted = qubits.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|pair| pair[0] == pair[1])
}

fn unique_target_qubits(targets: &[(usize, bool)]) -> Vec<usize> {
    unique_qubits(&targets.iter().map(|(q, _)| *q).collect::<Vec<_>>())
}

fn unique_qubits(qubits: &[usize]) -> Vec<usize> {
    let mut unique = Vec::with_capacity(qubits.len());
    for &q in qubits {
        if !unique.contains(&q) {
            unique.push(q);
        }
    }
    unique
}

fn transpose_packed_plane(
    source: &[u64],
    source_rows: usize,
    source_columns: usize,
    source_words_per_row: usize,
    destination: &mut [u64],
) {
    let destination_words_per_row = words_for_bits(source_rows);
    assert_eq!(
        source.len(),
        source_rows.saturating_mul(source_words_per_row)
    );
    assert_eq!(
        destination.len(),
        source_columns.saturating_mul(destination_words_per_row)
    );
    destination.fill(0);

    for source_row_word in 0..destination_words_per_row {
        let source_row_start = source_row_word * 64;
        let rows_in_tile = (source_rows - source_row_start).min(64);
        for source_column_word in 0..source_words_per_row {
            let source_column_start = source_column_word * 64;
            let columns_in_tile = (source_columns - source_column_start).min(64);
            let mut tile = [0u64; 64];
            for (row_offset, word) in tile.iter_mut().take(rows_in_tile).enumerate() {
                *word = source
                    [(source_row_start + row_offset) * source_words_per_row + source_column_word];
            }
            transpose_64x64(&mut tile);
            for column_offset in 0..columns_in_tile {
                destination[(source_column_start + column_offset) * destination_words_per_row
                    + source_row_word] = tile[63 - column_offset].reverse_bits();
            }
        }
    }
}

fn transpose_64x64(words: &mut [u64; 64]) {
    let mut shift = 32usize;
    let mut mask = 0x0000_0000_ffff_ffffu64;
    while shift != 0 {
        let mut index = 0usize;
        while index < 64 {
            let swap = (words[index] ^ (words[index + shift] >> shift)) & mask;
            words[index] ^= swap;
            words[index + shift] ^= swap << shift;
            index = (index + shift + 1) & !shift;
        }
        shift >>= 1;
        mask ^= mask << shift;
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

fn words_dot_count_mod4(left: &[u64], right: &[u64]) -> u8 {
    left.iter()
        .zip(right)
        .fold(0u32, |sum, (left_word, right_word)| {
            (sum + (left_word & right_word).count_ones()) % 4
        }) as u8
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
