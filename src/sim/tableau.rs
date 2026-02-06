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
