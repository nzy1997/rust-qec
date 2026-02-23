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
        ((true, false), (false, true)) => (true, true, 1),   // X*Z=iY
        ((false, true), (true, false)) => (true, true, 3),   // Z*X=-iY
        ((true, false), (true, true)) => (false, true, 1),   // X*Y=iZ
        ((true, true), (true, false)) => (false, true, 3),   // Y*X=-iZ
        ((false, true), (true, true)) => (true, false, 3),   // Z*Y=-iX
        ((true, true), (false, true)) => (true, false, 1),   // Y*Z=iX
    }
}
