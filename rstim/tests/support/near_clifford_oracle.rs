//! Independent, small-qubit state-vector oracle for near-Clifford tests.
//! This deliberately applies matrix entries to computational amplitudes rather
//! than sharing any production stabilizer/frame update code.

#[derive(Clone, Copy, Debug, Default)]
pub struct Amp {
    pub re: f64,
    pub im: f64,
}

impl Amp {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }
}

impl std::ops::Add for Amp {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl std::ops::Sub for Amp {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl std::ops::Mul for Amp {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl std::ops::Mul<f64> for Amp {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

#[derive(Clone, Debug)]
pub struct DenseOracle {
    num_qubits: usize,
    amplitudes: Vec<Amp>,
}

impl DenseOracle {
    pub fn new(num_qubits: usize) -> Self {
        assert!(num_qubits <= 10, "dense oracle is limited to 10 qubits");
        let mut amplitudes = vec![Amp::default(); 1 << num_qubits];
        amplitudes[0] = Amp::new(1.0, 0.0);
        Self {
            num_qubits,
            amplitudes,
        }
    }

    pub fn amplitudes(&self) -> &[Amp] {
        &self.amplitudes
    }

    fn mask(&self, qubit: usize) -> usize {
        assert!(qubit < self.num_qubits);
        1 << (self.num_qubits - 1 - qubit)
    }

    pub fn h(&mut self, qubit: usize) {
        let mask = self.mask(qubit);
        let scale = std::f64::consts::FRAC_1_SQRT_2;
        for low in 0..self.amplitudes.len() {
            if low & mask == 0 {
                let high = low | mask;
                let a = self.amplitudes[low];
                let b = self.amplitudes[high];
                self.amplitudes[low] = (a + b) * scale;
                self.amplitudes[high] = (a - b) * scale;
            }
        }
    }

    pub fn cx(&mut self, control: usize, target: usize) {
        assert_ne!(control, target);
        let c = self.mask(control);
        let t = self.mask(target);
        for zero_target in 0..self.amplitudes.len() {
            if zero_target & c != 0 && zero_target & t == 0 {
                self.amplitudes.swap(zero_target, zero_target | t);
            }
        }
    }

    pub fn t(&mut self, qubit: usize) {
        self.phase(qubit, std::f64::consts::FRAC_PI_4);
    }

    pub fn t_dag(&mut self, qubit: usize) {
        self.phase(qubit, -std::f64::consts::FRAC_PI_4);
    }

    fn phase(&mut self, qubit: usize, angle: f64) {
        let mask = self.mask(qubit);
        let phase = Amp::new(angle.cos(), angle.sin());
        for (index, amplitude) in self.amplitudes.iter_mut().enumerate() {
            if index & mask != 0 {
                *amplitude = *amplitude * phase;
            }
        }
    }

    pub fn pauli_xx_expectation(&self, a: usize, b: usize) -> f64 {
        assert_ne!(a, b);
        let flip = self.mask(a) | self.mask(b);
        self.amplitudes
            .iter()
            .enumerate()
            .map(|(index, amp)| (amp.conj() * self.amplitudes[index ^ flip]).re)
            .sum()
    }

    pub fn even_x_parity_probability(&self, a: usize, b: usize) -> f64 {
        (1.0 + self.pauli_xx_expectation(a, b)) / 2.0
    }
}
