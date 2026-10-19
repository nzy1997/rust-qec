//! Small exhaustive QASM instrument oracle. No production conversion code is used.
//!
//! Each branch carries an unnormalized Kraus operator, including every input
//! basis column. Summing outer products of vectorized Kraus operators gives a
//! Choi matrix per visible output. This retains coherence with an untouched
//! reference system, unlike checking only probabilities from the zero state.
use rstim::ir::{StimInstr, StimTarget};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI};

#[derive(Clone, Copy, Debug, Default)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}
impl Complex {
    pub fn real(re: f64) -> Self {
        Self { re, im: 0. }
    }
    fn cis(t: f64) -> Self {
        Self {
            re: t.cos(),
            im: t.sin(),
        }
    }
    fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }
    fn norm(self) -> f64 {
        self.re.hypot(self.im)
    }
}
impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            re: self.re + b.re,
            im: self.im + b.im,
        }
    }
}
impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self {
            re: self.re - b.re,
            im: self.im - b.im,
        }
    }
}
impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}
impl std::ops::Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, b: f64) -> Self {
        Self {
            re: self.re * b,
            im: self.im * b,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Pauli {
    X,
    Y,
    Z,
}
#[derive(Clone)]
pub enum Primitive {
    U(usize, f64, f64, f64),
    CX(usize, usize),
    ControlledU(usize, usize, f64, f64, f64),
    ControlledUPhase(usize, usize, f64, f64, f64, f64),
    ControlledRz(usize, usize, f64),
    Toffoli(usize, usize, usize),
    Measure(usize, usize),
    Reset(usize),
    If(usize, bool, Pauli, usize),
}
#[derive(Clone)]
struct Branch {
    kraus: Vec<Complex>,
    record: Vec<bool>,
    classical: Vec<bool>,
}
fn identity(n: usize, bits: usize) -> Branch {
    let d = 1 << n;
    let mut kraus = vec![Complex::default(); d * d];
    for j in 0..d {
        kraus[j * d + j] = Complex::real(1.);
    }
    Branch {
        kraus,
        record: Vec::new(),
        classical: vec![false; bits],
    }
}
fn one(branch: &mut Branch, q: usize, m: [Complex; 4]) {
    let d = branch.kraus.len().isqrt();
    for row in 0..d {
        if row & (1 << q) != 0 {
            continue;
        }
        let other = row | (1 << q);
        for col in 0..d {
            let a = branch.kraus[row * d + col];
            let b = branch.kraus[other * d + col];
            branch.kraus[row * d + col] = m[0] * a + m[1] * b;
            branch.kraus[other * d + col] = m[2] * a + m[3] * b;
        }
    }
}
fn cx(branch: &mut Branch, a: usize, b: usize) {
    let d = branch.kraus.len().isqrt();
    for row in 0..d {
        if row & (1 << a) != 0 && row & (1 << b) == 0 {
            for col in 0..d {
                branch.kraus.swap(row * d + col, (row | (1 << b)) * d + col);
            }
        }
    }
}
fn controlled(branch: &mut Branch, a: usize, b: usize, matrix: [Complex; 4]) {
    let d = branch.kraus.len().isqrt();
    for row in 0..d {
        if row & (1 << a) == 0 || row & (1 << b) != 0 {
            continue;
        }
        let other = row | (1 << b);
        for col in 0..d {
            let x = branch.kraus[row * d + col];
            let y = branch.kraus[other * d + col];
            branch.kraus[row * d + col] = matrix[0] * x + matrix[1] * y;
            branch.kraus[other * d + col] = matrix[2] * x + matrix[3] * y;
        }
    }
}
fn u(theta: f64, phi: f64, lambda: f64) -> [Complex; 4] {
    let (s, c) = (theta / 2.).sin_cos();
    [
        Complex::real(c),
        Complex::cis(lambda) * (-s),
        Complex::cis(phi) * s,
        Complex::cis(phi + lambda) * c,
    ]
}
fn p(branch: &mut Branch, q: usize, pauli: Pauli) {
    let z = Complex::default();
    let o = Complex::real(1.);
    let i = Complex { re: 0., im: 1. };
    one(
        branch,
        q,
        match pauli {
            Pauli::X => [z, o, o, z],
            Pauli::Y => [z, i * (-1.), i, z],
            Pauli::Z => [o, z, z, o * (-1.)],
        },
    );
}
fn split(branch: Branch, q: usize, dest: Option<usize>, reset: bool) -> Vec<Branch> {
    [false, true]
        .into_iter()
        .filter_map(|value| {
            let mut next = branch.clone();
            let d = next.kraus.len().isqrt();
            for row in 0..d {
                if (row & (1 << q) != 0) != value {
                    next.kraus[row * d..(row + 1) * d].fill(Complex::default());
                }
            }
            if reset && value {
                p(&mut next, q, Pauli::X);
            }
            if let Some(bit) = dest {
                next.record.push(value);
                next.classical[bit] = value;
            }
            if next.kraus.iter().all(|c| c.norm() < 1e-15) {
                None
            } else {
                Some(next)
            }
        })
        .collect()
}
fn source(n: usize, bits: usize, ops: &[Primitive]) -> Vec<Branch> {
    let mut branches = vec![identity(n, bits)];
    for op in ops {
        match *op {
            Primitive::Measure(q, b) => {
                branches = branches
                    .into_iter()
                    .flat_map(|v| split(v, q, Some(b), false))
                    .collect()
            }
            Primitive::Reset(q) => {
                branches = branches
                    .into_iter()
                    .flat_map(|v| split(v, q, None, true))
                    .collect()
            }
            _ => {
                for branch in &mut branches {
                    match *op {
                        Primitive::U(q, t, f, l) => one(branch, q, u(t, f, l)),
                        Primitive::CX(a, b) => cx(branch, a, b),
                        Primitive::ControlledU(a, b, t, f, l) => {
                            controlled(branch, a, b, u(t, f, l))
                        }
                        Primitive::ControlledUPhase(a, b, t, f, l, phase) => {
                            controlled(branch, a, b, u(t, f, l).map(|v| v * Complex::cis(phase)));
                        }
                        Primitive::ControlledRz(a, b, t) => controlled(
                            branch,
                            a,
                            b,
                            [
                                Complex::cis(-t / 2.),
                                Complex::default(),
                                Complex::default(),
                                Complex::cis(t / 2.),
                            ],
                        ),
                        Primitive::Toffoli(a, b, c) => {
                            let d = branch.kraus.len().isqrt();
                            for row in 0..d {
                                if row & (1 << a) != 0 && row & (1 << b) != 0 && row & (1 << c) == 0
                                {
                                    for col in 0..d {
                                        branch
                                            .kraus
                                            .swap(row * d + col, (row | (1 << c)) * d + col);
                                    }
                                }
                            }
                        }
                        Primitive::If(bit, value, pauli, q) => {
                            if branch.classical[bit] == value {
                                p(branch, q, pauli);
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
    branches
}

// These matrices directly specify native gate semantics. In particular the
// rotations are exp(-i*theta*Pauli/2), rather than QASM U-phase aliases.
fn native_matrix(name: &str, args: &[f64]) -> [Complex; 4] {
    let z = Complex::default();
    let o = Complex::real(1.);
    let i = Complex { re: 0., im: 1. };
    match name {
        "I" => [o, z, z, o],
        "H" => [
            o * FRAC_1_SQRT_2,
            o * FRAC_1_SQRT_2,
            o * FRAC_1_SQRT_2,
            o * (-FRAC_1_SQRT_2),
        ],
        "X" => [z, o, o, z],
        "Y" => [z, i * (-1.), i, z],
        "Z" => [o, z, z, o * (-1.)],
        "S" => [o, z, z, i],
        "S_DAG" => [o, z, z, i * (-1.)],
        "T" => [o, z, z, Complex::cis(FRAC_PI_4)],
        "T_DAG" => [o, z, z, Complex::cis(-FRAC_PI_4)],
        "SQRT_X" | "SQRT_X_DAG" => {
            let sign = if name == "SQRT_X" { 1. } else { -1. };
            [
                o * FRAC_1_SQRT_2,
                i * (-sign * FRAC_1_SQRT_2),
                i * (-sign * FRAC_1_SQRT_2),
                o * FRAC_1_SQRT_2,
            ]
        }
        "SQRT_Y" | "SQRT_Y_DAG" => {
            let sign = if name == "SQRT_Y" { 1. } else { -1. };
            [
                o * FRAC_1_SQRT_2,
                o * (-sign * FRAC_1_SQRT_2),
                o * (sign * FRAC_1_SQRT_2),
                o * FRAC_1_SQRT_2,
            ]
        }
        "R_X" | "R_Y" | "R_Z" => {
            let (s, c) = (args[0] * PI / 2.).sin_cos();
            match name {
                "R_X" => [o * c, i * (-s), i * (-s), o * c],
                "R_Y" => [o * c, o * (-s), o * s, o * c],
                _ => [
                    Complex::cis(-args[0] * PI / 2.),
                    z,
                    z,
                    Complex::cis(args[0] * PI / 2.),
                ],
            }
        }
        _ => panic!("unrecognized oracle gate {name}"),
    }
}
fn native(n: usize, instructions: &[StimInstr]) -> Vec<Branch> {
    let mut branches = vec![identity(n, 0)];
    for instruction in instructions {
        let StimInstr::Op {
            name,
            args,
            targets,
            ..
        } = instruction
        else {
            panic!("unexpected repeat in QASM lowering");
        };
        if matches!(name.as_str(), "M" | "R" | "RZ") {
            for target in targets {
                let q = target.qubit_index().expect("qubit") as usize;
                assert!(matches!(target, StimTarget::Qubit(_)));
                if name == "M" {
                    branches = branches
                        .into_iter()
                        .flat_map(|v| {
                            let mut out = split(v, q, None, false);
                            // split(None) deliberately hides reset outcomes; expose M here.
                            for next in &mut out {
                                let d = next.kraus.len().isqrt();
                                let bit = (0..d).any(|r| {
                                    r & (1 << q) != 0
                                        && next.kraus[r * d..(r + 1) * d]
                                            .iter()
                                            .any(|a| a.norm() > 1e-15)
                                });
                                next.record.push(bit);
                            }
                            out
                        })
                        .collect();
                } else {
                    branches = branches
                        .into_iter()
                        .flat_map(|v| split(v, q, None, true))
                        .collect();
                }
            }
        } else if matches!(name.as_str(), "CX" | "CY" | "CZ") {
            for pair in targets.chunks_exact(2) {
                let b = pair[1].qubit_index().expect("target") as usize;
                for branch in &mut branches {
                    match pair[0] {
                        StimTarget::Rec(offset) => {
                            let index = (branch.record.len() as i64 + i64::from(offset)) as usize;
                            if branch.record[index] {
                                p(
                                    branch,
                                    b,
                                    match name.as_str() {
                                        "CX" => Pauli::X,
                                        "CY" => Pauli::Y,
                                        _ => Pauli::Z,
                                    },
                                );
                            }
                        }
                        StimTarget::Qubit(a) => {
                            let a = a as usize;
                            if name == "CY" {
                                one(branch, b, native_matrix("S_DAG", &[]));
                            }
                            if name == "CZ" {
                                one(branch, b, native_matrix("H", &[]));
                            }
                            cx(branch, a, b);
                            if name == "CY" {
                                one(branch, b, native_matrix("S", &[]));
                            }
                            if name == "CZ" {
                                one(branch, b, native_matrix("H", &[]));
                            }
                        }
                        _ => panic!("unexpected control"),
                    }
                }
            }
        } else if name == "TICK" {
            continue;
        } else {
            let matrix = native_matrix(name, args);
            for target in targets {
                let q = target.qubit_index().expect("qubit") as usize;
                for branch in &mut branches {
                    one(branch, q, matrix);
                }
            }
        }
    }
    branches
}

fn choi(
    branches: Vec<Branch>,
    outputs: impl Fn(&Branch) -> Vec<bool>,
) -> BTreeMap<Vec<bool>, Vec<Complex>> {
    let mut result = BTreeMap::new();
    for branch in branches {
        let key = outputs(&branch);
        let size = branch.kraus.len();
        let matrix = result
            .entry(key)
            .or_insert_with(|| vec![Complex::default(); size * size]);
        for a in 0..size {
            for b in 0..size {
                matrix[a * size + b] =
                    matrix[a * size + b] + branch.kraus[a] * branch.kraus[b].conj();
            }
        }
    }
    result
}

pub fn assert_instrument(
    source_ops: &[Primitive],
    source_bits: usize,
    plan: &rstim::qasm::LoweredQasmCircuit,
) {
    let expected = choi(source(plan.num_qubits, source_bits, source_ops), |b| {
        let mut k = b.record.clone();
        k.extend(&b.classical);
        k
    });
    let actual = choi(native(plan.num_qubits, &plan.instructions), |b| {
        let mut k: Vec<_> = plan
            .measurement_events
            .iter()
            .map(|&i| b.record[i])
            .collect();
        k.extend(plan.project_classical(&b.record).expect("valid raw record"));
        k
    });
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>(),
        "classical outcome labels"
    );
    for (key, want) in expected {
        for (index, (a, b)) in want.iter().zip(&actual[&key]).enumerate() {
            assert!(
                (*a - *b).norm() <= 1e-12 + 1e-11 * a.norm(),
                "Choi mismatch for {key:?} entry {index}: expected {a:?}, got {b:?}"
            );
        }
    }
}

pub fn assert_unitary(source_ops: &[Primitive], plan: &rstim::qasm::LoweredQasmCircuit) {
    let expected = source(plan.num_qubits, 0, source_ops);
    let actual = native(plan.num_qubits, &plan.instructions);
    assert_eq!(expected.len(), 1);
    assert_eq!(actual.len(), 1);
    let a = &expected[0].kraus;
    let b = &actual[0].kraus;
    let pivot = a
        .iter()
        .position(|v| v.norm() > 1e-10)
        .expect("unitary nonzero");
    let ratio = b[pivot] * a[pivot].conj() * (1. / (a[pivot].norm() * a[pivot].norm()));
    assert!((ratio.norm() - 1.).abs() < 1e-12);
    for (index, (want, got)) in a.iter().zip(b).enumerate() {
        assert!(
            (*want * ratio - *got).norm() <= 1e-12 + 1e-11 * want.norm(),
            "full operator mismatch at {index}: {want:?} vs {got:?}"
        );
    }
}

pub fn h(q: usize) -> Primitive {
    Primitive::U(q, FRAC_PI_2, 0., PI)
}
pub fn x(q: usize) -> Primitive {
    Primitive::U(q, PI, 0., PI)
}
