// Rows are U† X_q U and U† Z_q U in canonical i^phase X...X Z...Z order.
// This stores the inverse Clifford directly; it never reconstructs a Boolean
// tableau or solves an inverse-coordinate system at query time.
use super::*;

#[derive(Debug)]
pub(super) struct CompileFrame {
    pub(super) num_qubits: usize,
    inverse: Vec<PackedPauli>,
}

impl CompileFrame {
    fn zeros<T: Default + Clone>(len: usize) -> Result<Vec<T>, String> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|e| format!("packed compile frame allocation failed: {e}"))?;
        values.resize(len, T::default());
        Ok(values)
    }

    pub(super) fn identity(n: usize) -> Result<Self, String> {
        if n > 4096 {
            return Err("packed compile frame exceeds 4096 physical qubits".into());
        }
        let words = n.div_ceil(64);
        let rows = n.checked_mul(2).ok_or("packed frame row overflow")?;
        let row_bytes = words
            .checked_mul(2 * std::mem::size_of::<u64>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PackedPauli>()))
            .ok_or("packed frame byte overflow")?;
        let bytes = rows
            .checked_mul(row_bytes)
            .ok_or("packed frame byte overflow")?;
        // Header + exact plane-capacity reservation; this is not an RSS limit.
        if bytes > 9 * 1024 * 1024 {
            return Err("packed compile frame reservation exceeds 9 MiB".into());
        }
        let mut inverse = Vec::new();
        inverse
            .try_reserve_exact(rows)
            .map_err(|e| format!("packed compile frame allocation failed: {e}"))?;
        for index in 0..rows {
            let mut row = PackedPauli {
                x: Self::zeros(words)?,
                z: Self::zeros(words)?,
                phase: 0,
            };
            if index < n {
                row.x[index / 64] |= 1 << (index % 64);
            } else {
                let q = index - n;
                row.z[q / 64] |= 1 << (q % 64);
            }
            inverse.push(row);
        }
        Ok(Self {
            num_qubits: n,
            inverse,
        })
    }

    // Reuse the frame between capture and structural planning; never keep two
    // maximum-width inverse planes resident just to replace the basis.
    pub(super) fn reset_identity(&mut self) {
        let n = self.num_qubits;
        for (index, row) in self.inverse.iter_mut().enumerate() {
            row.x.fill(0);
            row.z.fill(0);
            row.phase = 0;
            if index < n {
                row.x[index / 64] |= 1 << (index % 64);
            } else {
                let q = index - n;
                row.z[q / 64] |= 1 << (q % 64);
            }
        }
    }

    // Exact canonical Pauli multiplication: Z_left crosses X_right.
    fn multiply(left: &mut PackedPauli, right: &PackedPauli) {
        debug_assert_eq!(left.x.len(), right.x.len());
        let crossing = left
            .z
            .iter()
            .zip(&right.x)
            .fold(0, |parity, (z, x)| parity ^ ((z & x).count_ones() & 1));
        left.phase = (left.phase + right.phase + 2 * crossing as u8) & 3;
        for (x, rhs) in left.x.iter_mut().zip(&right.x) {
            *x ^= rhs;
        }
        for (z, rhs) in left.z.iter_mut().zip(&right.z) {
            *z ^= rhs;
        }
    }

    // Borrow two disjoint rows without cloning or allocating a source row.
    fn multiply_row(&mut self, destination: usize, source: usize) {
        debug_assert_ne!(destination, source);
        if destination < source {
            let (lower, upper) = self.inverse.split_at_mut(source);
            Self::multiply(&mut lower[destination], &upper[0]);
        } else {
            let (lower, upper) = self.inverse.split_at_mut(destination);
            Self::multiply(&mut upper[0], &lower[source]);
        }
    }

    fn check_qubit(&self, q: usize) -> Result<(), String> {
        if q >= self.num_qubits {
            Err(format!("qubit {q} outside packed compile frame"))
        } else {
            Ok(())
        }
    }

    // Physical left composition U'=G U substitutes G† X_q G / G† Z_q G
    // in the inverse generator rows. Only the affected rows change.
    pub(super) fn apply_clifford(&mut self, gate: CliffordGate) -> Result<(), String> {
        let n = self.num_qubits;
        match gate {
            CliffordGate::H(q)
            | CliffordGate::S(q)
            | CliffordGate::SDag(q)
            | CliffordGate::X(q)
            | CliffordGate::Y(q)
            | CliffordGate::Z(q) => self.check_qubit(q)?,
            CliffordGate::CX(a, b) | CliffordGate::CZ(a, b) | CliffordGate::Swap(a, b) => {
                self.check_qubit(a)?;
                self.check_qubit(b)?;
                if a == b {
                    return Err("packed compile Clifford requires distinct qubits".into());
                }
            }
        }
        match gate {
            CliffordGate::H(q) => self.inverse.swap(q, n + q),
            CliffordGate::S(q) | CliffordGate::SDag(q) => {
                self.multiply_row(q, n + q);
                self.inverse[q].phase = (self.inverse[q].phase
                    + if matches!(gate, CliffordGate::S(_)) {
                        3
                    } else {
                        1
                    })
                    & 3;
            }
            CliffordGate::X(q) => self.inverse[n + q].phase ^= 2,
            CliffordGate::Z(q) => self.inverse[q].phase ^= 2,
            CliffordGate::Y(q) => {
                self.inverse[q].phase ^= 2;
                self.inverse[n + q].phase ^= 2;
            }
            CliffordGate::CX(a, b) => {
                self.multiply_row(a, b);
                self.multiply_row(n + b, n + a);
            }
            CliffordGate::CZ(a, b) => {
                self.multiply_row(a, n + b);
                self.multiply_row(b, n + a);
            }
            CliffordGate::Swap(a, b) => {
                self.inverse.swap(a, b);
                self.inverse.swap(n + a, n + b);
            }
        }
        Ok(())
    }

    // Right composition U'=U C conjugates every inverse row by C†.
    // BasisGate indices have already passed the structural planner's checks.
    pub(super) fn right(&mut self, gate: BasisGate) {
        for row in &mut self.inverse {
            match gate {
                BasisGate::H(q) => {
                    debug_assert!(q < self.num_qubits);
                    let x = get(&row.x, q);
                    let z = get(&row.z, q);
                    row.phase ^= 2 * u8::from(x && z);
                    if x != z {
                        flip(&mut row.x, q);
                        flip(&mut row.z, q);
                    }
                }
                BasisGate::S(q) => {
                    debug_assert!(q < self.num_qubits);
                    if get(&row.x, q) {
                        row.phase = (row.phase + 3) & 3;
                        flip(&mut row.z, q);
                    }
                }
                BasisGate::CX(a, b) => {
                    debug_assert!(a < self.num_qubits && b < self.num_qubits && a != b);
                    if get(&row.x, a) {
                        flip(&mut row.x, b);
                    }
                    if get(&row.z, b) {
                        flip(&mut row.z, a);
                    }
                }
                BasisGate::CZ(a, b) => {
                    debug_assert!(a < self.num_qubits && b < self.num_qubits && a != b);
                    let xa = get(&row.x, a);
                    let xb = get(&row.x, b);
                    row.phase ^= 2 * u8::from(xa && xb);
                    if xb {
                        flip(&mut row.z, a);
                    }
                    if xa {
                        flip(&mut row.z, b);
                    }
                }
            }
        }
    }

    fn unpack(&self, packed: &PackedPauli) -> Result<Pauli, String> {
        let mut x = Self::zeros(self.num_qubits)?;
        let mut z = Self::zeros(self.num_qubits)?;
        for_support(&packed.x, |q| x[q] = true);
        for_support(&packed.z, |q| z[q] = true);
        Ok(Pauli {
            x,
            z,
            phase: packed.phase,
        })
    }

    pub(super) fn reexpress(&self, original: &PackedPauli) -> Result<Pauli, String> {
        let words = self.num_qubits.div_ceil(64);
        if original.x.len() != words || original.z.len() != words {
            return Err("packed input Pauli width differs from compile frame".into());
        }
        let tail = self.num_qubits % 64;
        if tail != 0 && (original.x[words - 1] | original.z[words - 1]) >> tail != 0 {
            return Err("packed input Pauli has nonzero padding bits".into());
        }
        let mut output = PackedPauli {
            x: Self::zeros(words)?,
            z: Self::zeros(words)?,
            phase: original.phase & 3,
        };
        // i^phase [product X_q] [product Z_q]; do not insert canonical Y phases.
        for_support(&original.x, |q| {
            Self::multiply(&mut output, &self.inverse[q])
        });
        for_support(&original.z, |q| {
            Self::multiply(&mut output, &self.inverse[self.num_qubits + q])
        });
        self.unpack(&output)
    }

    pub(super) fn pauli(&self, q: usize, basis: MeasurementBasis) -> Result<Pauli, String> {
        self.check_qubit(q)?;
        match basis {
            MeasurementBasis::X => self.unpack(&self.inverse[q]),
            MeasurementBasis::Z => self.unpack(&self.inverse[self.num_qubits + q]),
            MeasurementBasis::Y => {
                let x = &self.inverse[q];
                let mut product = PackedPauli {
                    x: Self::zeros(x.x.len())?,
                    z: Self::zeros(x.z.len())?,
                    phase: (x.phase + 1) & 3, // physical Y = i X Z
                };
                product.x.copy_from_slice(&x.x);
                product.z.copy_from_slice(&x.z);
                Self::multiply(&mut product, &self.inverse[self.num_qubits + q]);
                self.unpack(&product)
            }
        }
    }
}

// Differential tests calibrate
// canonical iXZ signs against the incumbent inverse-coordinate reconstruction;
// the existing independent dense tests remain the physical correctness oracle.
#[cfg(test)]
mod packed_compile_frame_tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn compare(frame: &CompileFrame, reference: &ActiveState, input: &PackedPauli) {
        let n = frame.num_qubits;
        let mut x = vec![false; n];
        let mut z = vec![false; n];
        for_support(&input.x, |q| x[q] = true);
        for_support(&input.z, |q| z[q] = true);
        let canonical = (x.iter().zip(&z).filter(|(x, z)| **x && **z).count() % 4) as u8;
        let mut expected = reference.physical_pauli(&x, &z).unwrap();
        expected.phase = (expected.phase + input.phase + 4 - canonical) & 3;
        let actual = frame.reexpress(input).unwrap();
        assert_eq!(actual.x, expected.x);
        assert_eq!(actual.z, expected.z);
        assert_eq!(actual.phase, expected.phase);
    }

    #[test]
    fn packed_compile_inverse_matches_signed_incumbent_after_left_and_right_gates() {
        let mut rng = StdRng::seed_from_u64(2026100701);
        for n in [1, 3, 5, 63, 64, 65, 129] {
            let mut frame = CompileFrame::identity(n).unwrap();
            let mut reference = ActiveState::new(n, 16);
            for _ in 0..24 {
                let a = rng.gen_range(0..n);
                let b = if n == 1 {
                    0
                } else {
                    (a + rng.gen_range(1..n)) % n
                };
                let gate = match rng.gen_range(0..if n == 1 { 6 } else { 9 }) {
                    0 => CliffordGate::H(a),
                    1 => CliffordGate::S(a),
                    2 => CliffordGate::SDag(a),
                    3 => CliffordGate::X(a),
                    4 => CliffordGate::Y(a),
                    5 => CliffordGate::Z(a),
                    6 => CliffordGate::CX(a, b),
                    7 => CliffordGate::CZ(a, b),
                    _ => CliffordGate::Swap(a, b),
                };
                frame.apply_clifford(gate).unwrap();
                reference.apply_clifford(gate).unwrap();
                let right = match rng.gen_range(0..if n == 1 { 2 } else { 4 }) {
                    0 => BasisGate::H(a),
                    1 => BasisGate::S(a),
                    2 => BasisGate::CX(a, b),
                    _ => BasisGate::CZ(a, b),
                };
                frame.right(right);
                let legacy = Arc::make_mut(&mut reference.frame);
                match right {
                    BasisGate::H(q) => legacy.right_h(q),
                    BasisGate::S(q) => legacy.right_s(q),
                    BasisGate::CX(c, t) => legacy.right_cx(c, t),
                    BasisGate::CZ(c, t) => legacy.right_cz(c, t),
                }
                let mut input = Pauli::identity(n);
                for q in 0..n {
                    input.x[q] = rng.r#gen();
                    input.z[q] = rng.r#gen();
                }
                input.phase = ((input
                    .x
                    .iter()
                    .zip(&input.z)
                    .filter(|(x, z)| **x && **z)
                    .count()
                    % 4) as u8
                    + 2 * u8::from(rng.r#gen::<bool>()))
                    & 3;
                compare(&frame, &reference, &PackedPauli::new(&input));
                for basis in [
                    MeasurementBasis::X,
                    MeasurementBasis::Y,
                    MeasurementBasis::Z,
                ] {
                    let actual = frame.pauli(a, basis).unwrap();
                    let expected = reference.single_qubit_pauli(a, basis).unwrap();
                    assert_eq!(actual.x, expected.x);
                    assert_eq!(actual.z, expected.z);
                    assert_eq!(actual.phase, expected.phase);
                }
            }
        }
    }

    #[test]
    fn packed_compile_s_inverse_and_padding_have_explicit_sign_contracts() {
        let mut frame = CompileFrame::identity(1).unwrap();
        frame.apply_clifford(CliffordGate::S(0)).unwrap();
        let minus_y = frame.pauli(0, MeasurementBasis::X).unwrap();
        assert_eq!(minus_y.x, [true]);
        assert_eq!(minus_y.z, [true]);
        assert_eq!(minus_y.phase, 3);
        frame.right(BasisGate::S(0));
        let minus_x = frame.pauli(0, MeasurementBasis::X).unwrap();
        assert_eq!(minus_x.x, [true]);
        assert_eq!(minus_x.z, [false]);
        assert_eq!(minus_x.phase, 2);
        assert!(frame.apply_clifford(CliffordGate::CX(0, 0)).is_err());
        assert!(frame.pauli(1, MeasurementBasis::Z).is_err());
        assert!(CompileFrame::identity(4097).is_err());
        let wide = CompileFrame::identity(65).unwrap();
        assert!(
            wide.reexpress(&PackedPauli {
                x: vec![0, 2],
                z: vec![0, 0],
                phase: 0
            })
            .is_err()
        );
        frame.reset_identity();
        assert_eq!(frame.pauli(0, MeasurementBasis::X).unwrap().phase, 0);
        assert_eq!(frame.pauli(0, MeasurementBasis::Y).unwrap().phase, 1);
        let empty = CompileFrame::identity(0).unwrap();
        let scalar = empty
            .reexpress(&PackedPauli {
                x: vec![],
                z: vec![],
                phase: 2,
            })
            .unwrap();
        assert!(scalar.x.is_empty() && scalar.z.is_empty());
        assert_eq!(scalar.phase, 2);
    }
}
