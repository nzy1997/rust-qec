//! Bounded CSS syndrome tables for endpoint diagnostics, not physical recovery.
//!
//! Bits follow the caller's original signed-check order. Logical bits `0..k`
//! anticommute with logical X (logical Z coefficients); bits `k..2*k`
//! anticommute with logical Z. The official validated basis is retained.
//! Each plane chooses minimum weight, then the lexicographically first list
//! of increasing qubit indices. Equal-syndrome errors with different logical
//! labels reject construction; no distance assumption substitutes for this check.
use std::collections::{BTreeMap, btree_map::Entry};

use crate::Pauli;
use crate::phased_pauli::{
    Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup, ValidatedLogicalBasis,
};
use thiserror::Error;

pub const MAX_PHYSICAL_QUBITS: usize = 64;
pub const MAX_DECLARED_CHECKS: usize = 128;
pub const MAX_MASKS_PER_PLANE: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    X,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointLimits {
    pub max_physical_qubits: usize,
    /// Counts all enumerated masks, including same-coset duplicates, per plane.
    pub max_masks_per_plane: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ErrorSignature {
    pub syndrome: u128,
    pub logical: u128,
}
impl ErrorSignature {
    fn xor(self, other: Self) -> Self {
        Self {
            syndrome: self.syndrome ^ other.syndrome,
            logical: self.logical ^ other.logical,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PauliFrame {
    pub x: u64,
    pub z: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneEntry {
    pub mask: u64,
    pub logical: u128,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaneTable {
    radius: usize,
    enumerated_masks: usize,
    entries: BTreeMap<u128, PlaneEntry>,
}
impl PlaneTable {
    pub fn radius(&self) -> usize {
        self.radius
    }
    pub fn enumerated_masks(&self) -> usize {
        self.enumerated_masks
    }
    pub fn entries(&self) -> &BTreeMap<u128, PlaneEntry> {
        &self.entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointScore {
    /// The policy has no entry for at least one plane; no correction is scored.
    Uncovered {
        x_syndrome: u128,
        z_syndrome: u128,
        x_covered: bool,
        z_covered: bool,
    },
    /// Correction is a diagnostic convention. These are not emitted gates.
    Covered {
        correction: PauliFrame,
        residual: ErrorSignature,
        correct: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointError {
    #[error("width cap must be in 1..={MAX_PHYSICAL_QUBITS}, got {0}")]
    InvalidWidthCap(usize),
    #[error("mask cap must be in 1..={MAX_MASKS_PER_PLANE}, got {0}")]
    InvalidMaskCap(usize),
    #[error("physical width {actual} exceeds declared cap {limit}")]
    WidthLimit { actual: usize, limit: usize },
    #[error("declared check count {actual} exceeds {MAX_DECLARED_CHECKS}")]
    CheckLimit { actual: usize },
    #[error("radius {radius} exceeds physical width {n}")]
    RadiusLimit { radius: usize, n: usize },
    #[error("{required} masks per plane exceed declared cap {limit}")]
    MaskLimit { required: u128, limit: usize },
    #[error("declared check {index} is mixed X/Z, not a CSS check")]
    NonCssCheck { index: usize },
    #[error("frame has a bit outside physical width {n}")]
    FrameWidth { n: usize },
    #[error(
        "{plane:?} syndrome {syndrome:#x} has incompatible logical classes {first_logical:#x}/{second_logical:#x}"
    )]
    LogicalCollision {
        plane: Plane,
        syndrome: u128,
        first_mask: u64,
        second_mask: u64,
        first_logical: u128,
        second_logical: u128,
    },
    #[error(transparent)]
    Algebra(#[from] PhaseAlgebraError),
}

#[derive(Debug, Clone)]
pub struct CssEndpointDecoder {
    n: usize,
    checks: Vec<PhasedPauli>,
    logicals: ValidatedLogicalBasis,
    x_columns: Vec<ErrorSignature>,
    z_columns: Vec<ErrorSignature>,
    x_table: PlaneTable,
    z_table: PlaneTable,
}
impl CssEndpointDecoder {
    pub fn new(
        group: &SignedStabilizerGroup,
        logicals: &ValidatedLogicalBasis,
        radius: usize,
        limits: EndpointLimits,
    ) -> Result<Self, EndpointError> {
        // All size/count guards precede columns, tables, or mask enumeration.
        if !(1..=MAX_PHYSICAL_QUBITS).contains(&limits.max_physical_qubits) {
            return Err(EndpointError::InvalidWidthCap(limits.max_physical_qubits));
        }
        if !(1..=MAX_MASKS_PER_PLANE).contains(&limits.max_masks_per_plane) {
            return Err(EndpointError::InvalidMaskCap(limits.max_masks_per_plane));
        }
        let n = group.n();
        if n > limits.max_physical_qubits {
            return Err(EndpointError::WidthLimit {
                actual: n,
                limit: limits.max_physical_qubits,
            });
        }
        if group.generators().len() > MAX_DECLARED_CHECKS {
            return Err(EndpointError::CheckLimit {
                actual: group.generators().len(),
            });
        }
        if radius > n {
            return Err(EndpointError::RadiusLimit { radius, n });
        }
        let mut count = 1u128;
        let mut choose = 1u128;
        for w in 1..=radius {
            choose = choose * (n + 1 - w) as u128 / w as u128;
            count += choose;
        }
        if count > limits.max_masks_per_plane as u128 {
            return Err(EndpointError::MaskLimit {
                required: count,
                limit: limits.max_masks_per_plane,
            });
        }
        for (index, check) in group.generators().iter().enumerate() {
            if check.support().x_bits().contains(&1) && check.support().z_bits().contains(&1) {
                return Err(EndpointError::NonCssCheck { index });
            }
        }
        // A basis previously validated against another group is not sufficient.
        let logicals = group
            .validate_logical_basis(logicals.logical_x().to_vec(), logicals.logical_z().to_vec())?;
        let columns = |plane| -> Result<Vec<ErrorSignature>, EndpointError> {
            (0..n)
                .map(|q| {
                    let mut bits = vec![0; n];
                    bits[q] = 1;
                    let support = if plane == Plane::X {
                        Pauli::from_xz_bits(bits, vec![0; n])
                    } else {
                        Pauli::from_xz_bits(vec![0; n], bits)
                    }
                    .expect("binary single support");
                    let p = PhasedPauli::new(support, Phase::PlusOne);
                    let mut sig = ErrorSignature::default();
                    for (i, g) in group.generators().iter().enumerate() {
                        sig.syndrome |= u128::from(!p.commutes_with(g)?) << i;
                    }
                    for (i, g) in logicals
                        .logical_x()
                        .iter()
                        .chain(logicals.logical_z())
                        .enumerate()
                    {
                        sig.logical |= u128::from(!p.commutes_with(g)?) << i;
                    }
                    Ok(sig)
                })
                .collect()
        };
        let x_columns = columns(Plane::X)?;
        let z_columns = columns(Plane::Z)?;
        let x_table = make_table(&x_columns, radius, count as usize, Plane::X)?;
        let z_table = make_table(&z_columns, radius, count as usize, Plane::Z)?;
        Ok(Self {
            n,
            checks: group.generators().to_vec(),
            logicals,
            x_columns,
            z_columns,
            x_table,
            z_table,
        })
    }
    pub fn n(&self) -> usize {
        self.n
    }
    pub fn checks(&self) -> &[PhasedPauli] {
        &self.checks
    }
    pub fn logicals(&self) -> &ValidatedLogicalBasis {
        &self.logicals
    }
    pub fn x_columns(&self) -> &[ErrorSignature] {
        &self.x_columns
    }
    pub fn z_columns(&self) -> &[ErrorSignature] {
        &self.z_columns
    }
    pub fn x_table(&self) -> &PlaneTable {
        &self.x_table
    }
    pub fn z_table(&self) -> &PlaneTable {
        &self.z_table
    }
    pub fn signature(&self, frame: PauliFrame) -> Result<ErrorSignature, EndpointError> {
        if self.n < 64 && (frame.x | frame.z) >> self.n != 0 {
            return Err(EndpointError::FrameWidth { n: self.n });
        }
        Ok(
            plane_signature(frame.x, &self.x_columns)
                .xor(plane_signature(frame.z, &self.z_columns)),
        )
    }
    /// Score an endpoint frame. No data or circuit is changed by this method.
    pub fn score(&self, frame: PauliFrame) -> Result<EndpointScore, EndpointError> {
        self.signature(frame)?;
        let xs = plane_signature(frame.x, &self.x_columns).syndrome;
        let zs = plane_signature(frame.z, &self.z_columns).syndrome;
        let x = self.x_table.entries.get(&xs);
        let z = self.z_table.entries.get(&zs);
        let (Some(x), Some(z)) = (x, z) else {
            return Ok(EndpointScore::Uncovered {
                x_syndrome: xs,
                z_syndrome: zs,
                x_covered: x.is_some(),
                z_covered: z.is_some(),
            });
        };
        let correction = PauliFrame {
            x: x.mask,
            z: z.mask,
        };
        let residual = self.signature(PauliFrame {
            x: frame.x ^ correction.x,
            z: frame.z ^ correction.z,
        })?;
        Ok(EndpointScore::Covered {
            correction,
            residual,
            correct: residual == ErrorSignature::default(),
        })
    }
}
fn plane_signature(mut mask: u64, columns: &[ErrorSignature]) -> ErrorSignature {
    let mut result = ErrorSignature::default();
    while mask != 0 {
        let q = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        result = result.xor(columns[q]);
    }
    result
}
fn make_table(
    columns: &[ErrorSignature],
    radius: usize,
    count: usize,
    plane: Plane,
) -> Result<PlaneTable, EndpointError> {
    let mut entries = BTreeMap::new();
    for w in 0..=radius {
        visit(
            columns,
            0,
            w,
            0,
            ErrorSignature::default(),
            plane,
            &mut entries,
        )?;
    }
    Ok(PlaneTable {
        radius,
        enumerated_masks: count,
        entries,
    })
}
fn visit(
    columns: &[ErrorSignature],
    start: usize,
    remaining: usize,
    mask: u64,
    sig: ErrorSignature,
    plane: Plane,
    entries: &mut BTreeMap<u128, PlaneEntry>,
) -> Result<(), EndpointError> {
    if remaining == 0 {
        match entries.entry(sig.syndrome) {
            Entry::Vacant(e) => {
                e.insert(PlaneEntry {
                    mask,
                    logical: sig.logical,
                });
            }
            Entry::Occupied(e) => {
                let first = e.get();
                if first.logical != sig.logical {
                    return Err(EndpointError::LogicalCollision {
                        plane,
                        syndrome: sig.syndrome,
                        first_mask: first.mask,
                        second_mask: mask,
                        first_logical: first.logical,
                        second_logical: sig.logical,
                    });
                }
            }
        }
    } else {
        for q in start..=columns.len() - remaining {
            visit(
                columns,
                q + 1,
                remaining - 1,
                mask | (1u64 << q),
                sig.xor(columns[q]),
                plane,
                entries,
            )?;
        }
    }
    Ok(())
}
