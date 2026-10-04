use rand::Rng;

#[derive(Debug, Clone)]
pub struct StabilizerState {
    n: usize,
    x: Vec<Vec<bool>>, // 2n rows
    z: Vec<Vec<bool>>, // 2n rows
    phase: Vec<u8>,    // mod 4, represents i^phase
}

impl StabilizerState {
    pub fn new(n: usize) -> Self {
        let mut x = vec![vec![false; n]; 2 * n];
        let mut z = vec![vec![false; n]; 2 * n];
        let phase = vec![0u8; 2 * n];
        // Initialize to |0..0>, destabilizers are X_i, stabilizers are Z_i
        for i in 0..n {
            x[i][i] = true; // destabilizer X
            z[i + n][i] = true; // stabilizer Z
        }
        Self { n, x, z, phase }
    }

    // BEGIN issue-456 read-only snapshot accessor
    #[doc(hidden)]
    pub fn canonical_snapshot(&self) -> crate::sim::packed_inverse_tableau::CanonicalTableauSnapshot {
        crate::sim::packed_inverse_tableau::CanonicalTableauSnapshot {
            num_qubits: self.n,
            x: self.x.clone(),
            z: self.z.clone(),
            phase: self.phase.clone(),
        }
    }
    pub(crate) fn canonical_row(&self, row: usize) -> (&[bool], &[bool], u8) {
        (&self.x[row], &self.z[row], self.phase[row])
    }
    // END issue-456 read-only snapshot accessor
    // BEGIN near-clifford tableau extensions
    pub(crate) fn num_qubits(&self) -> usize {
        self.n
    }

    /// Multiply distinct rows without the per-column Pauli-product match.
    /// For P(x,z) = i^(xz) X^x Z^z, the product phase is
    /// xh*zh + xi*zi + 2*zh*xi - (xh^xi)*(zh^zi), modulo four.
    pub(crate) fn row_mult_near_clifford(&mut self, h: usize, i: usize) {
        fn rows(rows: &mut [Vec<bool>], h: usize, i: usize) -> (&mut [bool], &[bool]) {
            assert_ne!(h, i);
            if h < i {
                let (left, right) = rows.split_at_mut(i);
                (&mut left[h], &right[0])
            } else {
                let (left, right) = rows.split_at_mut(h);
                (&mut right[0], &left[i])
            }
        }
        let (xh, xi) = rows(&mut self.x, h, i);
        let (zh, zi) = rows(&mut self.z, h, i);
        let mut phase = 0u32;
        for (((xh, zh), &xi), &zi) in xh.iter_mut().zip(zh).zip(xi).zip(zi) {
            let x = *xh ^ xi;
            let z = *zh ^ zi;
            phase += u32::from(*xh & *zh)
                + u32::from(xi & zi)
                + 2 * u32::from(*zh & xi)
                + 3 * u32::from(x & z);
            *xh = x;
            *zh = z;
        }
        self.phase[h] = ((u32::from(self.phase[h]) + u32::from(self.phase[i]) + phase) & 3) as u8;
    }

    #[cfg(test)]
    pub(crate) fn row_mult_reference(&mut self, h: usize, i: usize) {
        self.row_mult(h, i);
    }

    #[cfg(test)]
    pub(crate) fn check_near_clifford_local_products() {
        for left in 0..4 {
            for right in 0..4 {
                for left_phase in 0..4 {
                    for right_phase in 0..4 {
                        for (h, i) in [(0, 1), (1, 0)] {
                            let mut state = Self::new(1);
                            state.x[h][0] = left & 1 != 0;
                            state.z[h][0] = left & 2 != 0;
                            state.x[i][0] = right & 1 != 0;
                            state.z[i][0] = right & 2 != 0;
                            state.phase[h] = left_phase;
                            state.phase[i] = right_phase;
                            let mut reference = state.clone();
                            reference.row_mult(h, i);
                            state.row_mult_near_clifford(h, i);
                            assert_eq!(state.canonical_snapshot(), reference.canonical_snapshot());
                        }
                    }
                }
            }
        }
    }

    // Right composition changes the virtual basis, not the physical state.
    pub(crate) fn right_h(&mut self, q: usize) {
        self.x.swap(q, self.n + q);
        self.z.swap(q, self.n + q);
        self.phase.swap(q, self.n + q);
    }

    pub(crate) fn right_s(&mut self, q: usize) {
        self.row_mult_near_clifford(q, self.n + q);
        self.phase[q] = (self.phase[q] + 1) % 4;
    }

    pub(crate) fn right_cx(&mut self, control: usize, target: usize) {
        self.row_mult_near_clifford(control, target);
        self.row_mult_near_clifford(self.n + target, self.n + control);
    }

    pub(crate) fn right_cz(&mut self, a: usize, b: usize) {
        self.right_h(b);
        self.right_cx(a, b);
        self.right_h(b);
    }

    /// Single-pass physical gates for the near-Clifford frame. Keep the
    /// composed public gates below as independent differential references.
    pub(crate) fn s_dag_near_clifford(&mut self, q: usize) {
        for ((x, z), phase) in self.x.iter().zip(&mut self.z).zip(&mut self.phase) {
            *phase = (*phase + 2 * u8::from(x[q] && !z[q])) & 3;
            z[q] ^= x[q];
        }
    }

    pub(crate) fn y_near_clifford(&mut self, q: usize) {
        for ((x, z), phase) in self.x.iter().zip(&self.z).zip(&mut self.phase) {
            *phase = (*phase + 2 * u8::from(x[q] ^ z[q])) & 3;
        }
    }

    pub(crate) fn cz_near_clifford(&mut self, a: usize, b: usize) {
        for ((x, z), phase) in self.x.iter().zip(&mut self.z).zip(&mut self.phase) {
            *phase = (*phase + 2 * u8::from(x[a] && x[b] && (z[a] ^ z[b]))) & 3;
            z[a] ^= x[b];
            z[b] ^= x[a];
        }
    }

    /// Apply a sampled branch without drawing from the caller's RNG. The
    /// original measure_z below stays byte-for-byte pinned as a legacy oracle.
    pub(crate) fn measure_z_with_forced_random_outcome(
        &mut self,
        q: usize,
        sampled: u8,
    ) -> (u8, bool) {
        let pivot = (self.n..2 * self.n).find(|&row| self.x[row][q]);
        if let Some(p) = pivot {
            for row in 0..2 * self.n {
                if row != p && self.x[row][q] {
                    self.row_mult_near_clifford(row, p);
                }
            }
            self.copy_row(p, p - self.n);
            self.x[p].fill(false);
            self.z[p].fill(false);
            self.z[p][q] = true;
            self.phase[p] = if sampled == 0 { 0 } else { 2 };
            return (sampled, true);
        }

        let mut temp_x = vec![false; self.n];
        let mut temp_z = vec![false; self.n];
        temp_z[q] = true;
        let mut temp_phase = 0;
        for row in 0..self.n {
            if self.x[row][q] {
                self.row_mult_temp(&mut temp_x, &mut temp_z, &mut temp_phase, row + self.n);
            }
        }
        (u8::from(temp_phase % 4 == 2), false)
    }

    /// Compile one measurement with affine row-sign dependencies. The returned
    /// tuple is (random-bit mask, fixed sign, draws a fresh random bit).
    pub(crate) fn measure_z_symbolic<M: crate::sim::symbolic_mask::SymbolicMask>(
        &mut self,
        signs: &mut [M],
        random_count: &mut usize,
        words: usize,
        q: usize,
    ) -> Option<(M, bool, bool)> {
        let n = self.n;
        if let Some(pivot) = (n..2 * n).find(|&row| self.x[row][q]) {
            let random_mask = M::fresh_bit(*random_count, words)?;
            *random_count += 1;
            let pivot_sign = signs[pivot].clone();
            for row in 0..2 * n {
                if row != pivot && self.x[row][q] {
                    signs[row].xor_assign(&pivot_sign);
                }
            }
            signs[pivot - n] = pivot_sign;
            signs[pivot] = random_mask;
            self.measure_z_with_forced_random_outcome(q, 0);
            Some((signs[pivot].clone(), false, true))
        } else {
            let mut mask = M::zero(words);
            for row in 0..n {
                if self.x[row][q] {
                    mask.xor_assign(&signs[n + row]);
                }
            }
            let (outcome, _) = self.measure_z_with_forced_random_outcome(q, 0);
            Some((mask, outcome != 0, false))
        }
    }
    // END near-clifford tableau extensions
    pub fn h(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.x[i][q] && self.z[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            let tmp = self.x[i][q];
            self.x[i][q] = self.z[i][q];
            self.z[i][q] = tmp;
        }
    }

    pub fn s(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.x[i][q] && self.z[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            self.z[i][q] ^= self.x[i][q];
        }
    }

    pub fn s_dag(&mut self, q: usize) {
        // S^\u2020 is S applied three times
        self.s(q);
        self.s(q);
        self.s(q);
    }

    pub fn x_gate(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.z[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
        }
    }

    pub fn z_gate(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.x[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
        }
    }

    pub fn y_gate(&mut self, q: usize) {
        self.x_gate(q);
        self.z_gate(q);
    }

    pub fn cx(&mut self, c: usize, t: usize) {
        for i in 0..2 * self.n {
            if self.x[i][c] && self.z[i][t] && (self.x[i][t] ^ self.z[i][c] ^ true) {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            self.x[i][t] ^= self.x[i][c];
            self.z[i][c] ^= self.z[i][t];
        }
    }

    pub fn cz(&mut self, a: usize, b: usize) {
        self.h(b);
        self.cx(a, b);
        self.h(b);
    }

    pub fn sqrt_x(&mut self, q: usize) {
        self.h(q);
        self.s(q);
        self.h(q);
    }

    pub fn sqrt_x_dag(&mut self, q: usize) {
        self.h(q);
        self.s_dag(q);
        self.h(q);
    }

    pub fn sqrt_y(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.x[i][q] && !self.z[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            let tmp = self.x[i][q];
            self.x[i][q] = self.z[i][q];
            self.z[i][q] = tmp;
        }
    }

    pub fn sqrt_y_dag(&mut self, q: usize) {
        for i in 0..2 * self.n {
            if self.z[i][q] && !self.x[i][q] {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            let tmp = self.x[i][q];
            self.x[i][q] = self.z[i][q];
            self.z[i][q] = tmp;
        }
    }

    /// H_XY: X → Y, Y → X, Z → −Z
    pub fn h_xy(&mut self, q: usize) {
        for i in 0..2 * self.n {
            let xi = self.x[i][q];
            let zi = self.z[i][q];
            if zi && !xi {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            self.z[i][q] = xi ^ zi;
        }
    }

    /// H_YZ: X → −X, Y → Z, Z → Y
    pub fn h_yz(&mut self, q: usize) {
        for i in 0..2 * self.n {
            let xi = self.x[i][q];
            let zi = self.z[i][q];
            if xi && !zi {
                self.phase[i] = (self.phase[i] + 2) % 4;
            }
            self.x[i][q] = xi ^ zi;
        }
    }

    pub fn c_xyz(&mut self, q: usize) { self.s_dag(q); self.h(q); }
    pub fn c_zyx(&mut self, q: usize) { self.h(q); self.s(q); }
    pub fn c_nxyz(&mut self, q: usize) { self.c_xyz(q); self.z_gate(q); }
    pub fn c_nzyx(&mut self, q: usize) { self.c_zyx(q); self.z_gate(q); }
    pub fn c_xnyz(&mut self, q: usize) { self.c_xyz(q); self.x_gate(q); }
    pub fn c_xynz(&mut self, q: usize) { self.c_xyz(q); self.y_gate(q); }
    pub fn c_znyx(&mut self, q: usize) { self.c_zyx(q); self.x_gate(q); }
    pub fn c_zynx(&mut self, q: usize) { self.c_zyx(q); self.y_gate(q); }
    pub fn h_nxy(&mut self, q: usize) { self.h_xy(q); self.z_gate(q); }
    pub fn h_nxz(&mut self, q: usize) { self.h(q); self.y_gate(q); }
    pub fn h_nyz(&mut self, q: usize) { self.h_yz(q); self.x_gate(q); }

    pub fn cy(&mut self, c: usize, t: usize) {
        self.s_dag(t);
        self.cx(c, t);
        self.s(t);
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        for i in 0..2 * self.n {
            self.x[i].swap(a, b);
            self.z[i].swap(a, b);
        }
    }

    pub fn iswap(&mut self, a: usize, b: usize) {
        self.s(a);
        self.s(b);
        self.cz(a, b);
        self.swap(a, b);
    }

    pub fn iswap_dag(&mut self, a: usize, b: usize) {
        self.s_dag(a);
        self.s_dag(b);
        self.cz(a, b);
        self.swap(a, b);
    }

    pub fn xcx(&mut self, a: usize, b: usize) {
        self.h(a);
        self.cx(a, b);
        self.h(a);
    }

    pub fn xcz(&mut self, a: usize, b: usize) {
        self.cx(b, a);
    }

    pub fn xcy(&mut self, a: usize, b: usize) {
        self.h_yz(b);
        self.cx(b, a);
        self.h_yz(b);
    }

    pub fn ycx(&mut self, a: usize, b: usize) {
        self.h_yz(a);
        self.cx(a, b);
        self.h_yz(a);
    }

    pub fn ycz(&mut self, a: usize, b: usize) {
        self.cy(b, a);
    }

    pub fn ycy(&mut self, a: usize, b: usize) {
        self.h_yz(a);
        self.h_yz(b);
        self.cz(a, b);
        self.h_yz(b);
        self.h_yz(a);
    }

    pub fn cxswap(&mut self, a: usize, b: usize) {
        self.cx(b, a);
        self.cx(a, b);
    }

    pub fn swapcx(&mut self, a: usize, b: usize) {
        self.cx(a, b);
        self.cx(b, a);
    }

    pub fn czswap(&mut self, a: usize, b: usize) {
        self.cz(a, b);
        self.swap(a, b);
    }

    /// Like measure_z but always returns 0 for random outcomes (bias toward +Z).
    /// Used by reference_sample for noiseless baseline.
    pub fn measure_z_biased(&mut self, q: usize) -> u8 {
        let mut p = None;
        for i in self.n..2 * self.n {
            if self.x[i][q] {
                p = Some(i);
                break;
            }
        }

        if let Some(p) = p {
            let r: u8 = 0;
            for i in 0..2 * self.n {
                if i != p && self.x[i][q] {
                    self.row_mult(i, p);
                }
            }
            let d = p - self.n;
            self.copy_row(p, d);
            self.x[p].fill(false);
            self.z[p].fill(false);
            self.z[p][q] = true;
            self.phase[p] = 0;
            return r;
        }

        let mut temp_x = vec![false; self.n];
        let mut temp_z = vec![false; self.n];
        temp_z[q] = true;
        let mut temp_phase: u8 = 0;
        for i in 0..self.n {
            if self.x[i][q] {
                self.row_mult_temp(&mut temp_x, &mut temp_z, &mut temp_phase, i + self.n);
            }
        }
        if temp_phase % 4 == 2 { 1 } else { 0 }
    }

    pub fn reset_z_biased(&mut self, q: usize) {
        let outcome = self.measure_z_biased(q);
        if outcome == 1 {
            self.x_gate(q);
        }
    }

    pub fn reset_x_biased(&mut self, q: usize) {
        self.h(q);
        self.reset_z_biased(q);
        self.h(q);
    }

    pub fn reset_y_biased(&mut self, q: usize) {
        self.s_dag(q);
        self.h(q);
        self.reset_z_biased(q);
        self.h(q);
        self.s(q);
    }

    pub fn reset_z(&mut self, q: usize, rng: &mut impl Rng) {
        let (outcome, _) = self.measure_z(q, rng);
        if outcome == 1 {
            self.x_gate(q);
        }
    }

    pub fn reset_x(&mut self, q: usize, rng: &mut impl Rng) {
        self.h(q);
        self.reset_z(q, rng);
        self.h(q);
    }

    pub fn reset_y(&mut self, q: usize, rng: &mut impl Rng) {
        self.s_dag(q);
        self.h(q);
        self.reset_z(q, rng);
        self.h(q);
        self.s(q);
    }

    pub fn measure_z(&mut self, q: usize, rng: &mut impl Rng) -> (u8, bool) {
        // Find a stabilizer row with X on q
        let mut p = None;
        for i in self.n..2 * self.n {
            if self.x[i][q] {
                p = Some(i);
                break;
            }
        }

        if let Some(p) = p {
            // Random outcome
            let r: u8 = if rng.r#gen::<bool>() { 1 } else { 0 };

            // Clear X in column q for all rows except p
            for i in 0..2 * self.n {
                if i != p && self.x[i][q] {
                    self.row_mult(i, p);
                }
            }

            // Copy row p into corresponding destabilizer
            let d = p - self.n;
            self.copy_row(p, d);

            // Set row p to Z_q with phase based on r
            self.x[p].fill(false);
            self.z[p].fill(false);
            self.z[p][q] = true;
            self.phase[p] = if r == 0 { 0 } else { 2 };

            return (r, true);
        }

        // Deterministic outcome
        // Build temporary row for Z_q
        let mut temp_x = vec![false; self.n];
        let mut temp_z = vec![false; self.n];
        temp_z[q] = true;
        let mut temp_phase: u8 = 0;

        for i in 0..self.n {
            if self.x[i][q] {
                self.row_mult_temp(&mut temp_x, &mut temp_z, &mut temp_phase, i + self.n);
            }
        }

        let outcome = if temp_phase % 4 == 2 { 1 } else { 0 };
        (outcome, false)
    }

    fn copy_row(&mut self, src: usize, dst: usize) {
        let x_src = self.x[src].clone();
        let z_src = self.z[src].clone();
        self.x[dst].clone_from_slice(&x_src);
        self.z[dst].clone_from_slice(&z_src);
        self.phase[dst] = self.phase[src];
    }

    fn row_mult(&mut self, h: usize, i: usize) {
        let mut ph = 0u8;
        for q in 0..self.n {
            let (x3, z3, k) = mul_pauli(self.x[h][q], self.z[h][q], self.x[i][q], self.z[i][q]);
            self.x[h][q] = x3;
            self.z[h][q] = z3;
            ph = (ph + k) % 4;
        }
        self.phase[h] = (self.phase[h] + self.phase[i] + ph) % 4;
    }

    fn row_mult_temp(
        &self,
        x: &mut [bool],
        z: &mut [bool],
        phase: &mut u8,
        i: usize,
    ) {
        let mut ph = 0u8;
        for q in 0..self.n {
            let (x3, z3, k) = mul_pauli(x[q], z[q], self.x[i][q], self.z[i][q]);
            x[q] = x3;
            z[q] = z3;
            ph = (ph + k) % 4;
        }
        *phase = (*phase + self.phase[i] + ph) % 4;
    }
}

fn mul_pauli(x1: bool, z1: bool, x2: bool, z2: bool) -> (bool, bool, u8) {
    let (p1, p2) = ((x1, z1), (x2, z2));
    // (x,z) encoding: I(0,0), X(1,0), Z(0,1), Y(1,1)
    match (p1, p2) {
        ((false, false), _) => (x2, z2, 0),
        (_, (false, false)) => (x1, z1, 0),
        ((true, false), (true, false)) => (false, false, 0), // X*X=I
        ((false, true), (false, true)) => (false, false, 0), // Z*Z=I
        ((true, true), (true, true)) => (false, false, 0),   // Y*Y=I
        ((true, false), (false, true)) => (true, true, 3),   // X*Z=-iY
        ((false, true), (true, false)) => (true, true, 1),   // Z*X=iY
        ((true, false), (true, true)) => (false, true, 1),   // X*Y=iZ
        ((true, true), (true, false)) => (false, true, 3),   // Y*X=-iZ
        ((false, true), (true, true)) => (true, false, 3),   // Z*Y=-iX
        ((true, true), (false, true)) => (true, false, 1),   // Y*Z=iX
    }
}

#[cfg(test)]
mod tests {
    use super::mul_pauli;

    #[test]
    fn pauli_product_tracks_xz_and_zx_phases() {
        // (x, z) selects the canonical Pauli I/X/Z/Y, while the returned
        // phase is the exponent of i multiplying that Pauli.
        assert_eq!(mul_pauli(true, false, false, true), (true, true, 3));
        assert_eq!(mul_pauli(false, true, true, false), (true, true, 1));
    }
}
// BEGIN near-clifford tableau tests

#[cfg(test)]
mod near_clifford_physical_gate_tests {
    use super::StabilizerState;

    #[test]
    fn near_clifford_physical_gates_preserve_every_signed_local_pauli() {
        for left in 0..4 {
            for right in 0..4 {
                for phase in 0..4 {
                    let mut base = StabilizerState::new(2);
                    for row in 0..4 {
                        base.x[row] = vec![left & 1 != 0, right & 1 != 0];
                        base.z[row] = vec![left & 2 != 0, right & 2 != 0];
                        base.phase[row] = phase;
                    }
                    for reverse in [false, true] {
                        let (a, b) = if reverse { (1, 0) } else { (0, 1) };
                        for gate in 0..3 {
                            let mut actual = base.clone();
                            let mut expected = base.clone();
                            match gate {
                                0 => {
                                    actual.s_dag_near_clifford(a);
                                    expected.s_dag(a);
                                }
                                1 => {
                                    actual.y_near_clifford(a);
                                    expected.y_gate(a);
                                }
                                _ => {
                                    actual.cz_near_clifford(a, b);
                                    expected.cz(a, b);
                                }
                            }
                            assert_eq!(
                                actual.canonical_snapshot(),
                                expected.canonical_snapshot(),
                                "left={left} right={right} phase={phase} gate={gate} reverse={reverse}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn near_clifford_physical_gates_preserve_wide_frame_chains() {
        for width in [3, 63, 64, 65, 127, 128, 129, 193, 4096] {
            let mut actual = StabilizerState::new(width);
            for q in 0..width {
                actual.h(q);
                if q % 2 == 0 {
                    actual.s(q);
                }
                if q > 0 {
                    actual.cx(q - 1, q);
                }
            }
            let mut expected = actual.clone();
            for step in 0..12 {
                let a = step % width;
                let b = (a + width - 1) % width;
                actual.cz_near_clifford(a, b);
                expected.cz(a, b);
                actual.s_dag_near_clifford(b);
                expected.s_dag(b);
                actual.y_near_clifford(a);
                expected.y_gate(a);
                assert_eq!(actual.canonical_snapshot(), expected.canonical_snapshot());
            }
        }
    }
}
// END near-clifford tableau tests
