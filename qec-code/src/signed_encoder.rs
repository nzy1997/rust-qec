//! Bounded signed Clifford encoding with a caller-specified logical frame.
//!
//! Input wires `0..k` hold arbitrary logical inputs; `k..n` must physically
//! be prepared in |0>. The returned unitary maps their logical X/Z operators
//! to the exact supplied representatives, including signs. It contains no
//! preparations, measurements, resets, or free error correction.
//!
//! This deliberately small, deterministic construction uses dense Pauli
//! support arrays, not Hilbert-space matrices. It is neither a large-code
//! synthesis interface nor a gate-count optimizer. Hard limits apply even
//! when callers request a larger gate budget.

use crate::Pauli;
use crate::phased_pauli::{
    GeneratorProductWitness, Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup,
};
use thiserror::Error;

pub const MAX_ENCODER_QUBITS: usize = 16;
pub const MAX_ENCODER_GENERATORS: usize = 256;
pub const MAX_ENCODER_GATES: usize = 8192;

/// Chronological native Clifford instructions. No implicit wire permutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncoderGate {
    H(usize),
    S(usize),
    Sdg(usize),
    Cx { control: usize, target: usize },
}

impl EncoderGate {
    pub fn inverse(self) -> Self {
        match self {
            Self::S(q) => Self::Sdg(q),
            Self::Sdg(q) => Self::S(q),
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncoderLimits {
    /// May tighten, but never increase, [`MAX_ENCODER_GATES`].
    pub max_gates: usize,
}

impl Default for EncoderLimits {
    fn default() -> Self {
        Self {
            max_gates: MAX_ENCODER_GATES,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EncoderError {
    #[error("encoder width {actual} exceeds hard limit {maximum}")]
    QubitLimit { actual: usize, maximum: usize },
    #[error("encoder generator count {actual} exceeds hard limit {maximum}")]
    GeneratorLimit { actual: usize, maximum: usize },
    #[error("encoder gate budget {maximum} exceeded")]
    GateLimit { maximum: usize },
    #[error(transparent)]
    InvalidFrame(#[from] PhaseAlgebraError),
    #[error("signed encoder invariant failed: {0}")]
    Invariant(&'static str),
}

/// A literal unitary, fixed standard input layout, and recomputable evidence.
///
/// Images use `U P U†`, with the same `i^phase X^x Z^z` convention as
/// [`PhasedPauli`]. Witnesses correspond to Z images on input wires `k..n`
/// and index the original stabilizer generators. They need not reproduce
/// the caller's original generator order, since redundant rows are allowed.
#[derive(Clone, Debug)]
pub struct SignedCliffordEncoder {
    n: usize,
    k: usize,
    gates: Vec<EncoderGate>,
    input_x_images: Vec<PhasedPauli>,
    input_z_images: Vec<PhasedPauli>,
    stabilizer_witnesses: Vec<GeneratorProductWitness>,
}

impl SignedCliffordEncoder {
    pub fn n(&self) -> usize {
        self.n
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn gates(&self) -> &[EncoderGate] {
        &self.gates
    }

    /// Literal inverse; it does not prepare, erase, or repair any wire.
    pub fn inverse_gates(&self) -> Vec<EncoderGate> {
        self.gates.iter().rev().map(|g| g.inverse()).collect()
    }

    pub fn input_x_images(&self) -> &[PhasedPauli] {
        &self.input_x_images
    }

    pub fn input_z_images(&self) -> &[PhasedPauli] {
        &self.input_z_images
    }

    pub fn stabilizer_witnesses(&self) -> &[GeneratorProductWitness] {
        &self.stabilizer_witnesses
    }
}

/// Construct an encoder for all logical inputs, including reference-entangled
/// inputs. The supplied frame is revalidated against this exact signed group.
///
/// The algorithm first decodes each logical Z/X pair to its standard input
/// wire, then decodes an independent signed stabilizer basis on the remaining
/// wires. Reversing those gates gives the encoder. Changing the stabilizer
/// generating basis does not change the signed code space. No distance,
/// fault-tolerance, native noisy-channel, or optimality claim is made.
pub fn synthesize_signed_encoder(
    group: &SignedStabilizerGroup,
    logical_x: &[PhasedPauli],
    logical_z: &[PhasedPauli],
    limits: EncoderLimits,
) -> Result<SignedCliffordEncoder, EncoderError> {
    let n = group.n();
    if n > MAX_ENCODER_QUBITS {
        return Err(EncoderError::QubitLimit {
            actual: n,
            maximum: MAX_ENCODER_QUBITS,
        });
    }
    if group.generators().len() > MAX_ENCODER_GENERATORS {
        return Err(EncoderError::GeneratorLimit {
            actual: group.generators().len(),
            maximum: MAX_ENCODER_GENERATORS,
        });
    }
    // Reject oversized borrowed frames before cloning their contents.
    let expected = group.num_logical_qubits();
    if logical_x.len() != expected || logical_z.len() != expected {
        return Err(PhaseAlgebraError::LogicalCount {
            expected,
            x_count: logical_x.len(),
            z_count: logical_z.len(),
        }
        .into());
    }
    // Borrowed rows may have widths unrelated to the bounded group. Reject
    // those before to_vec can clone arbitrarily large support arrays.
    for (kind, operators) in [("X", logical_x), ("Z", logical_z)] {
        for (index, operator) in operators.iter().enumerate() {
            if operator.n() != n {
                return Err(PhaseAlgebraError::LogicalWidth {
                    kind,
                    index,
                    expected: n,
                    actual: operator.n(),
                }
                .into());
            }
        }
    }
    let frame = group.validate_logical_basis(logical_x.to_vec(), logical_z.to_vec())?;
    let k = frame.k();
    let mut operators = logical_x.to_vec();
    operators.extend_from_slice(logical_z);
    operators.extend_from_slice(group.generators());
    let mut reduction = Reduction {
        n,
        operators,
        gates: Vec::new(),
        max_gates: limits.max_gates.min(MAX_ENCODER_GATES),
    };
    for q in 0..k {
        reduction.reduce_to_z(k + q, q)?;
        reduction.reduce_to_x(q, q)?;
    }
    for offset in 0..group.rank() {
        let q = k + offset;
        let row = 2 * k + offset;
        let pivot = (row..reduction.operators.len())
            .find(|&i| {
                (q..n).any(|wire| {
                    let (x, z) = reduction.bits(i, wire);
                    x != 0 || z != 0
                })
            })
            .ok_or(EncoderError::Invariant("missing independent stabilizer"))?;
        reduction.operators.swap(row, pivot);
        reduction.reduce_to_z(row, q)?;
        let stabilizer = reduction.operators[row].clone();
        for other in row + 1..reduction.operators.len() {
            if reduction.bits(other, q).1 != 0 {
                reduction.operators[other] = reduction.operators[other].multiply(&stabilizer)?;
            }
        }
    }
    let gates: Vec<_> = reduction.gates.iter().rev().map(|g| g.inverse()).collect();
    let input_x_images: Vec<_> = (0..n)
        .map(|q| replay(standard(n, q, true), &gates))
        .collect();
    let input_z_images: Vec<_> = (0..n)
        .map(|q| replay(standard(n, q, false), &gates))
        .collect();
    if input_x_images[..k] != *logical_x || input_z_images[..k] != *logical_z {
        return Err(EncoderError::Invariant(
            "logical images differ from requested frame",
        ));
    }
    let stabilizer_witnesses = input_z_images[k..]
        .iter()
        .map(|image| {
            group
                .contains_with_witness(image)?
                .ok_or(EncoderError::Invariant(
                    "stabilizer image outside signed group",
                ))
        })
        .collect::<Result<Vec<_>, EncoderError>>()?;
    Ok(SignedCliffordEncoder {
        n,
        k,
        gates,
        input_x_images,
        input_z_images,
        stabilizer_witnesses,
    })
}

fn standard(n: usize, q: usize, x_axis: bool) -> PhasedPauli {
    let mut x = vec![0; n];
    let mut z = vec![0; n];
    if x_axis {
        x[q] = 1;
    } else {
        z[q] = 1;
    }
    PhasedPauli::new(Pauli::from_xz_bits(x, z).unwrap(), Phase::PlusOne)
}

struct Reduction {
    n: usize,
    operators: Vec<PhasedPauli>,
    gates: Vec<EncoderGate>,
    max_gates: usize,
}

impl Reduction {
    fn bits(&self, row: usize, q: usize) -> (u8, u8) {
        let support = self.operators[row].support();
        (support.x_bits()[q], support.z_bits()[q])
    }

    fn emit(&mut self, gate: EncoderGate) -> Result<(), EncoderError> {
        if self.gates.len() == self.max_gates {
            return Err(EncoderError::GateLimit {
                maximum: self.max_gates,
            });
        }
        for operator in &mut self.operators {
            *operator = conjugate(operator, gate);
        }
        self.gates.push(gate);
        Ok(())
    }

    // All earlier wires have already been removed from this operator.
    fn reduce_to_z(&mut self, row: usize, q: usize) -> Result<(), EncoderError> {
        let pivot = (q..self.n)
            .find(|&wire| self.bits(row, wire) != (0, 0))
            .ok_or(EncoderError::Invariant("identity pivot"))?;
        if pivot != q {
            for (control, target) in [(q, pivot), (pivot, q), (q, pivot)] {
                self.emit(EncoderGate::Cx { control, target })?;
            }
        }
        for wire in q..self.n {
            let (x, z) = self.bits(row, wire);
            if x != 0 {
                if z != 0 {
                    self.emit(EncoderGate::Sdg(wire))?;
                }
                self.emit(EncoderGate::H(wire))?;
            }
        }
        for wire in q + 1..self.n {
            if self.bits(row, wire).1 != 0 {
                self.emit(EncoderGate::Cx {
                    control: wire,
                    target: q,
                })?;
            }
        }
        if self.operators[row].phase() == Phase::MinusOne {
            // X = H S S H, correcting -Z without adding a new gate kind.
            for gate in [
                EncoderGate::H(q),
                EncoderGate::S(q),
                EncoderGate::S(q),
                EncoderGate::H(q),
            ] {
                self.emit(gate)?;
            }
        }
        if self.operators[row] != standard(self.n, q, false) {
            return Err(EncoderError::Invariant("Z reduction"));
        }
        Ok(())
    }

    // The companion has already become +Z_q and must remain fixed.
    fn reduce_to_x(&mut self, row: usize, q: usize) -> Result<(), EncoderError> {
        for wire in q + 1..self.n {
            match self.bits(row, wire) {
                (0, 1) => self.emit(EncoderGate::H(wire))?,
                (1, 1) => self.emit(EncoderGate::Sdg(wire))?,
                _ => (),
            }
        }
        for wire in q + 1..self.n {
            if self.bits(row, wire).0 != 0 {
                self.emit(EncoderGate::Cx {
                    control: q,
                    target: wire,
                })?;
            }
        }
        if self.bits(row, q).1 != 0 {
            self.emit(EncoderGate::Sdg(q))?;
        }
        if self.operators[row].phase() == Phase::MinusOne {
            self.emit(EncoderGate::S(q))?;
            self.emit(EncoderGate::S(q))?;
        }
        if self.operators[row] != standard(self.n, q, true) {
            return Err(EncoderError::Invariant("X reduction"));
        }
        Ok(())
    }
}

fn replay(mut operator: PhasedPauli, gates: &[EncoderGate]) -> PhasedPauli {
    for &gate in gates {
        operator = conjugate(&operator, gate);
    }
    operator
}

fn conjugate(operator: &PhasedPauli, gate: EncoderGate) -> PhasedPauli {
    let mut x = operator.support().x_bits().to_vec();
    let mut z = operator.support().z_bits().to_vec();
    let mut phase = operator.phase().exponent();
    match gate {
        EncoderGate::H(q) => {
            phase += 2 * x[q] * z[q];
            std::mem::swap(&mut x[q], &mut z[q]);
        }
        EncoderGate::S(q) => {
            phase += x[q];
            z[q] ^= x[q];
        }
        EncoderGate::Sdg(q) => {
            phase += 3 * x[q];
            z[q] ^= x[q];
        }
        EncoderGate::Cx { control, target } => {
            x[target] ^= x[control];
            z[control] ^= z[target];
        }
    }
    PhasedPauli::new(
        Pauli::from_xz_bits(x, z).unwrap(),
        Phase::from_exponent(phase),
    )
}
