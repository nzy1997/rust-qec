//! Small exact identities, generated independently of external catalogue data.
use super::*;
use crate::phase::{CanonicalPhaseCore, PhaseCoreLimits, PhaseError};
use crate::quadratic::QuadraticForm;
use std::sync::OnceLock;

/// Shared, independently generated certificates for fifteen small canonical cores.
/// Construction uses finite affine restrictions and exact reconstruction once;
/// callers can clone and extend the result without changing this shared catalogue.
/// This function performs no file I/O, circuit queries or random draws.
pub fn builtin_phase_catalogue() -> Result<&'static PhaseCatalogue, CatalogueError> {
    static BUILTIN: OnceLock<Result<PhaseCatalogue, CatalogueError>> = OnceLock::new();
    BUILTIN.get_or_init(generate).as_ref().map_err(Clone::clone)
}
fn bits(n: usize, mask: usize) -> Result<BitVector, PhaseError> {
    Ok(BitVector::from_indices(
        n,
        (0..n).filter(|j| mask >> j & 1 != 0),
    )?)
}
// Convert omega^(g + 2*q) into parity form using
// 4*x*y = 2*x + 2*y - 2*(x XOR y), modulo eight.
fn clifford_phase(global: u8, q: &QuadraticForm) -> Result<ParityPhase, PhaseError> {
    let n = q.num_variables();
    let mut terms = Vec::new();
    for (j, &coefficient) in q.linear().iter().enumerate() {
        terms.push((bits(n, 1 << j)?, 2 * coefficient));
        for k in q.upper()[j].ones() {
            terms.push((bits(n, 1 << j)?, 2));
            terms.push((bits(n, 1 << k)?, 2));
            terms.push((bits(n, (1 << j) | (1 << k))?, 6));
        }
    }
    ParityPhase::new(n, (global + 2 * q.constant()) & 7, terms)
}
fn canonical(phase: &ParityPhase) -> Result<(ParityPhase, Vec<u8>), PhaseError> {
    let core = CanonicalPhaseCore::new(phase, &PhaseCoreLimits::default())?;
    let (global, q) = core.clifford_correction();
    Ok((
        phase.subtract(&clifford_phase(global, q)?)?,
        core.phase_table().to_vec(),
    ))
}
// Search the tiny coordinate space for a single parity whose two affine
// hyperplanes restrict the phase to Clifford. Free parameters are the original
// non-pivot coordinates, so their Clifford extension ignores the pivot wire.
fn parity_split(
    phase: &ParityPhase,
    arithmetic: &ScalarArithmetic,
) -> Result<Vec<CliffordIndicator>, PhaseError> {
    let n = phase.num_variables();
    match phase.clifford() {
        Ok((global, q)) => {
            return Ok(vec![CliffordIndicator {
                coefficient: arithmetic.omega(i64::from(global), 0)?,
                constraints: vec![],
                phase: clifford_phase(0, &q)?,
            }]);
        }
        Err(PhaseError::NotClifford) => {}
        Err(error) => return Err(error),
    }
    for mask in 1_usize..1 << n {
        let pivot = mask.trailing_zeros() as usize;
        let free = (0..n).filter(|j| *j != pivot).collect::<Vec<_>>();
        let basis = free
            .iter()
            .map(|&j| bits(n, (1 << j) | (((mask >> j) & 1) << pivot)))
            .collect::<Result<Vec<_>, _>>()?;
        let projection = (0..n)
            .map(|j| {
                bits(
                    free.len(),
                    free.iter().position(|&k| k == j).map_or(0, |i| 1 << i),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut branches = Vec::new();
        for bit in [false, true] {
            let restricted = phase.restrict(&bits(n, usize::from(bit) << pivot)?, &basis)?;
            let (global, q) = match restricted.clifford() {
                Ok(value) => value,
                Err(PhaseError::NotClifford) => break,
                Err(error) => return Err(error),
            };
            branches.push(CliffordIndicator {
                coefficient: arithmetic.omega(i64::from(global), 0)?,
                constraints: vec![(bits(n, mask)?, bit)],
                phase: clifford_phase(0, &q)?.restrict(&bits(free.len(), 0)?, &projection)?,
            });
        }
        if branches.len() == 2 {
            return Ok(branches);
        }
    }
    Err(PhaseError::NotClifford)
}
fn ccz_terms(n: usize, axes: [usize; 3]) -> Result<Vec<(BitVector, u8)>, PhaseError> {
    // 4*a*b*c = a+b+c-(a XOR b)-(a XOR c)-(b XOR c)+(a XOR b XOR c).
    (1_usize..8)
        .map(|mask| {
            let row =
                BitVector::from_indices(n, (0..3).filter(|j| mask >> j & 1 != 0).map(|j| axes[j]))?;
            Ok((row, if mask.count_ones() % 2 == 1 { 1 } else { 7 }))
        })
        .collect()
}
fn split_entry(
    phase: &ParityPhase,
    a: &ScalarArithmetic,
) -> Result<PhaseDecompositionInput, PhaseError> {
    let (phase, table) = canonical(phase)?;
    Ok(PhaseDecompositionInput {
        dimension: phase.num_variables(),
        phase_table: table,
        terms: parity_split(&phase, a)?,
    })
}
fn inputs(a: &ScalarArithmetic) -> Result<Vec<PhaseDecompositionInput>, PhaseError> {
    let mut entries = Vec::new();
    // All signatures on at most two coordinates: 1 + 2 + 8 distinct cores.
    for n in 0..=2 {
        let axes = (1 << n) - 1;
        for signature in 0_usize..1 << axes {
            let terms = (1..=axes)
                .filter(|axis| signature >> (axis - 1) & 1 != 0)
                .map(|axis| Ok((bits(n, axis)?, 1)))
                .collect::<Result<Vec<_>, PhaseError>>()?;
            entries.push(split_entry(&ParityPhase::new(n, 0, terms)?, a)?);
        }
    }
    // omega^(4*a*b*c) = 1 - 2*[a=b=c=1], with overlapping supports.
    let ccz = ParityPhase::new(3, 0, ccz_terms(3, [0, 1, 2])?)?;
    let (_, table) = canonical(&ccz)?;
    entries.push(PhaseDecompositionInput {
        dimension: 3,
        phase_table: table,
        terms: vec![
            CliffordIndicator {
                coefficient: ExactScalar::one(),
                constraints: vec![],
                phase: ParityPhase::new(3, 0, [])?,
            },
            CliffordIndicator {
                coefficient: a.integer(-2)?,
                constraints: (0..3)
                    .map(|j| Ok((bits(3, 1 << j)?, true)))
                    .collect::<Result<Vec<_>, PhaseError>>()?,
                phase: ParityPhase::new(3, 0, [])?,
            },
        ],
    });
    // The 422 phase is a+d+4*a*b*c+4*b*c*d. On a XOR d=0 it is
    // Clifford 2*a; on a XOR d=1 it is 1+4*b*c. Two invertible linear
    // changes of coordinates yield the other two canonical core tables.
    let mut terms = vec![(bits(4, 1)?, 1), (bits(4, 8)?, 1)];
    terms.extend(ccz_terms(4, [0, 1, 2])?);
    terms.extend(ccz_terms(4, [1, 2, 3])?);
    let phase = ParityPhase::new(4, 0, terms)?;
    entries.push(split_entry(&phase, a)?);
    for columns in [[8, 9, 2, 4], [8, 13, 2, 4]] {
        let basis = columns
            .into_iter()
            .map(|m| bits(4, m))
            .collect::<Result<Vec<_>, _>>()?;
        entries.push(split_entry(&phase.restrict(&bits(4, 0)?, &basis)?, a)?);
    }
    Ok(entries)
}
fn generate() -> Result<PhaseCatalogue, CatalogueError> {
    let a = ScalarArithmetic::default();
    let mut catalogue = PhaseCatalogue::new();
    for entry in inputs(&a).map_err(|e| error("catalogue.builtin", e))? {
        catalogue.insert(entry, &CatalogueLimits::default(), &a)?;
    }
    Ok(catalogue)
}
