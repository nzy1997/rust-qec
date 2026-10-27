//! Optional native deterministic pairing, with constraint contraction first.
use super::*;

/// Logical candidate-only bounds, independent of final transport admission.
/// Work counts conservative integer/phase operations; geometry counts logical
/// machine words, including integer and packed-vector payloads. Allocator and
/// object overhead are excluded; these are not native memory measurements.
#[derive(Clone, Copy, Debug)]
pub struct PhaseInjectionOptions {
    pub max_work: u128,
    pub max_geometry_words: usize,
    /// Bound surviving supports, never the Cartesian product of pair branches.
    pub max_terms: usize,
}
impl Default for PhaseInjectionOptions {
    fn default() -> Self {
        Self {
            max_work: 64 << 20,
            max_geometry_words: 8 << 20,
            max_terms: 1 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseInjectionReceipt {
    pub original_selection: PhaseSelectionRoute,
    pub original_terms: usize,
    pub odd_rotations: usize,
    /// Native integer ordering, expressed as resource-coordinate bit vectors.
    pub groups: Vec<Vec<BitVector>>,
    pub constraint_rank: usize,
    pub candidate_terms: Option<usize>,
    pub constructed: bool,
    pub selected: bool,
    /// The exact budget field that stopped this optional attempt.
    pub budget_skip: Option<String>,
    pub work_bound: u128,
    pub geometry_words: u128,
}
fn hard(cause: impl fmt::Display) -> CatalogueError {
    error("phase_injection.internal", cause)
}
fn bits(rank: usize, mask: usize) -> Result<BitVector, CatalogueError> {
    BitVector::from_indices(rank, (0..rank).filter(|j| mask >> j & 1 != 0)).map_err(hard)
}
fn mask(vector: &BitVector) -> usize {
    vector.words().first().copied().unwrap_or(0) as usize
}
fn highest(value: usize) -> usize {
    usize::BITS as usize - 1 - value.leading_zeros() as usize
}
fn remainder(mut value: usize, span: &BTreeMap<usize, usize>) -> usize {
    for (&pivot, &row) in span.iter().rev() {
        if value >> pivot & 1 != 0 {
            value ^= row;
        }
    }
    value
}
fn budget(
    receipt: &mut PhaseInjectionReceipt,
    field: &str,
    required: u128,
    configured: u128,
) -> bool {
    if required > configured {
        receipt.budget_skip = Some(field.into());
        false
    } else {
        true
    }
}
fn work(
    receipt: &mut PhaseInjectionReceipt,
    additional: u128,
    options: &PhaseInjectionOptions,
) -> bool {
    receipt.work_bound = receipt.work_bound.saturating_add(additional);
    budget(
        receipt,
        "phase_injection.max_work",
        receipt.work_bound,
        options.max_work,
    )
}
fn geometry(
    receipt: &mut PhaseInjectionReceipt,
    required: u128,
    options: &PhaseInjectionOptions,
) -> bool {
    receipt.geometry_words = receipt.geometry_words.max(required);
    budget(
        receipt,
        "phase_injection.max_geometry_words",
        receipt.geometry_words,
        options.max_geometry_words as u128,
    )
}
fn eligible(entry: &CertifiedPhaseDecomposition, parity: usize) -> bool {
    entry.terms().len() == 2
        && entry.terms().iter().enumerate().all(|(j, term)| {
            !term.coefficient.is_zero()
                && term.constraints.len() == 1
                && mask(&term.constraints[0].0) == parity
                && term.constraints[0].1 == (j == 1)
        })
}
fn canonical_parities(table: &[u8], rank: usize) -> Result<Vec<(usize, u8)>, CatalogueError> {
    let mut polynomial = table.to_vec();
    for bit in 0..rank {
        for word in 0..table.len() {
            if word >> bit & 1 != 0 {
                polynomial[word] = (polynomial[word] + 8 - polynomial[word ^ (1 << bit)]) & 7;
            }
        }
    }
    let mut counter = BTreeMap::<usize, u8>::new();
    for (monomial, coefficient) in polynomial.into_iter().enumerate() {
        if coefficient == 0 {
            continue;
        }
        let degree = monomial.count_ones();
        if !(1..=3).contains(&degree) || coefficient != 1 << (degree - 1) {
            return Err(hard("noncanonical weighted monomial in core"));
        }
        let mut subset = monomial;
        while subset != 0 {
            let value = counter.entry(subset).or_default();
            *value = (*value + if subset.count_ones() & 1 != 0 { 1 } else { 7 }) & 7;
            subset = (subset - 1) & monomial;
        }
    }
    Ok(counter.into_iter().filter(|(_, c)| *c != 0).collect())
}
fn pair(mut remaining: Vec<usize>) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut span = BTreeMap::new();
    while remaining.len() > 1 {
        let mut cosets = BTreeMap::<usize, Vec<usize>>::new();
        for mask in remaining {
            cosets.entry(remainder(mask, &span)).or_default().push(mask);
        }
        remaining = Vec::new();
        for masks in cosets.into_values() {
            let mut pairs = masks.chunks_exact(2);
            for pair in &mut pairs {
                groups.push(pair.to_vec());
            }
            if let Some(&mask) = pairs.remainder().first() {
                remaining.push(mask);
            }
        }
        if remaining.len() < 2 {
            break;
        }
        let mut counts = BTreeMap::<usize, usize>::new();
        for (i, &a) in remaining.iter().enumerate() {
            for &b in &remaining[i + 1..] {
                *counts.entry(remainder(a ^ b, &span)).or_default() += 1;
            }
        }
        let mut best = None;
        for (&difference, &count) in &counts {
            if best.is_none_or(|(_, old)| count > old) {
                best = Some((difference, count));
            }
        }
        let (difference, _) = best.expect("two distinct remaining cosets");
        span.insert(highest(difference), difference);
    }
    if !remaining.is_empty() {
        groups.push(remaining);
    }
    groups
}
/// Reduce every group row before enumerating any branch. Highest-pivot RREF
/// determines the same independent constraints and free-coordinate extension
/// as native Factor, independently of how many groups share a constraint.
fn support(groups: &[Vec<usize>]) -> Vec<usize> {
    let mut pivots = BTreeMap::new();
    for group in groups {
        let row = remainder(group.iter().fold(0, |a, b| a ^ b), &pivots);
        if row != 0 {
            pivots.insert(highest(row), row);
        }
    }
    let mut rows = pivots.into_values().collect::<Vec<_>>();
    for j in 0..rows.len() {
        for i in j + 1..rows.len() {
            if rows[i] >> highest(rows[j]) & 1 != 0 {
                rows[i] ^= rows[j];
            }
        }
    }
    rows
}
fn pulled_phase(
    rank: usize,
    group: &[usize],
    phase: &ParityPhase,
) -> Result<ParityPhase, CatalogueError> {
    let terms = phase
        .terms()
        .iter()
        .map(|(row, c)| {
            let pulled = row.ones().fold(0, |value, j| value ^ group[j]);
            Ok((bits(rank, pulled)?, *c))
        })
        .collect::<Result<Vec<_>, CatalogueError>>()?;
    ParityPhase::new(rank, phase.constant(), terms).map_err(hard)
}
fn extend_clifford(
    rank: usize,
    free: &[usize],
    q: &crate::quadratic::QuadraticForm,
) -> Result<ParityPhase, CatalogueError> {
    let mut counter = BTreeMap::<usize, u8>::new();
    for (j, &wire) in free.iter().enumerate() {
        *counter.entry(1 << wire).or_default() += q.linear()[j];
        for k in q.upper()[j].ones() {
            *counter.entry(1 << wire).or_default() += 1;
            *counter.entry(1 << free[k]).or_default() += 1;
            *counter.entry((1 << wire) | (1 << free[k])).or_default() += 3;
        }
        // At most 2*rank additions can accumulate on one free singleton.
        for value in counter.values_mut() {
            *value &= 3;
        }
    }
    ParityPhase::new(
        rank,
        0,
        counter
            .into_iter()
            .filter(|(_, c)| c & 3 != 0)
            .map(|(row, c)| Ok((bits(rank, row)?, 2 * (c & 3))))
            .collect::<Result<Vec<_>, CatalogueError>>()?,
    )
    .map_err(hard)
}
pub(super) fn attempt(
    original: &CertifiedPhaseDecomposition,
    catalogue: Option<&PhaseCatalogue>,
    route: PhaseSelectionRoute,
    options: &PhaseInjectionOptions,
    construction: &CatalogueLimits,
    arithmetic: &ScalarArithmetic,
) -> Result<(Option<PhaseCatalogue>, PhaseInjectionReceipt), CatalogueError> {
    let mut receipt = PhaseInjectionReceipt {
        original_selection: route,
        original_terms: original.terms().len(),
        odd_rotations: 0,
        groups: Vec::new(),
        constraint_rank: 0,
        candidate_terms: None,
        constructed: false,
        selected: false,
        budget_skip: None,
        work_bound: 0,
        geometry_words: 0,
    };
    let rank = original.dimension();
    if rank <= 1 || original.terms().len() <= 1 {
        return Ok((None, receipt));
    }
    let Some(catalogue) = catalogue else {
        return Ok((None, receipt));
    };
    let (Some(one), Some(two)) = (catalogue.get(1, &[0, 1]), catalogue.get(2, &[0, 1, 1, 2]))
    else {
        return Ok((None, receipt));
    };
    if !eligible(one, 1) || !eligible(two, 3) {
        return Ok((None, receipt));
    }
    let values = original.phase_table().len() as u128;
    let r = rank as u128;
    let workspace = 8 * values + 8 * r * r + 32;
    if !geometry(&mut receipt, workspace, options) || !work(&mut receipt, (r + 8) * values, options)
    {
        return Ok((None, receipt));
    }
    let core = canonical_parities(original.phase_table(), rank)?;
    let odd = core
        .iter()
        .filter(|(_, c)| c & 1 != 0)
        .map(|(row, _)| *row)
        .collect::<Vec<_>>();
    receipt.odd_rotations = odd.len();
    let t = odd.len() as u128;
    if odd.is_empty() {
        return Ok((None, receipt));
    }
    let pairing_work = (t + 1)
        .checked_pow(3)
        .and_then(|v| v.checked_mul(r + 1))
        .unwrap_or(u128::MAX);
    let workspace = workspace + 32 * t;
    if !geometry(&mut receipt, workspace, options) || !work(&mut receipt, pairing_work, options) {
        return Ok((None, receipt));
    }
    let groups = pair(odd);
    let independent = support(&groups);
    receipt.groups = groups
        .iter()
        .map(|group| group.iter().map(|&row| bits(rank, row)).collect())
        .collect::<Result<_, _>>()?;
    receipt.constraint_rank = independent.len();
    let Some(count) = 1_usize.checked_shl(independent.len() as u32) else {
        receipt.budget_skip = Some("phase_injection.index_capacity".into());
        return Ok((None, receipt));
    };
    receipt.candidate_terms = Some(count);
    if count >= original.terms().len() {
        return Ok((None, receipt));
    }
    if !budget(
        &mut receipt,
        "phase_injection.max_terms",
        count as u128,
        options.max_terms as u128,
    ) {
        return Ok((None, receipt));
    }
    // This admission is proportional to surviving supports. No Cartesian
    // branch count is computed or allocated, even when groups outnumber rank.
    let coefficient_bits = one
        .terms()
        .iter()
        .chain(two.terms())
        .flat_map(|term| term.coefficient.coefficients())
        .map(|c| u128::from(c.bits()))
        .max()
        .unwrap_or(0);
    let coefficient_words = (2 * (groups.len() as u128) * (coefficient_bits + 2)).div_ceil(64) * 4;
    let candidate_geometry =
        workspace + count as u128 * (8 * r * r + 8 * r + coefficient_words + 16);
    let candidate_work =
        (count as u128).saturating_mul((t + 1) * (r + 1).pow(3) + values * (r * r + r + 2));
    if !geometry(&mut receipt, candidate_geometry, options)
        || !work(&mut receipt, candidate_work, options)
    {
        return Ok((None, receipt));
    }
    let pivot_positions = independent
        .iter()
        .map(|&row| highest(row))
        .collect::<Vec<_>>();
    let free = (0..rank)
        .filter(|j| !pivot_positions.contains(j))
        .collect::<Vec<_>>();
    let directions = free
        .iter()
        .map(|&j| {
            bits(
                rank,
                independent
                    .iter()
                    .enumerate()
                    .fold(1 << j, |value, (k, row)| {
                        value | (((row >> j) & 1) << pivot_positions[k])
                    }),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let common = core
        .iter()
        .filter(|(_, c)| c & 6 != 0)
        .map(|(row, c)| Ok((bits(rank, *row)?, c - (c & 1))))
        .collect::<Result<Vec<_>, CatalogueError>>()?;
    let mut terms = Vec::with_capacity(count);
    for word in 0..count {
        let shift_mask = pivot_positions
            .iter()
            .enumerate()
            .fold(0, |value, (j, &pivot)| value | (((word >> j) & 1) << pivot));
        let shift = bits(rank, shift_mask)?;
        let mut coefficient = ExactScalar::one();
        let mut parities = common.clone();
        let mut global = 0_u8;
        for group in &groups {
            let row = group.iter().fold(0, |a, b| a ^ b);
            let outcome = ((row & shift_mask).count_ones() & 1) as usize;
            let component = &(if group.len() == 1 { one } else { two }).terms()[outcome];
            coefficient = arithmetic
                .multiply(&coefficient, &component.coefficient)
                .map_err(|e| error("phase_injection.arithmetic", e))?;
            let pulled = pulled_phase(rank, group, &component.phase)?;
            global = (global + pulled.constant()) & 7;
            parities.extend_from_slice(pulled.terms());
        }
        let phase = ParityPhase::new(rank, global, parities).map_err(hard)?;
        let restricted = phase.restrict(&shift, &directions).map_err(hard)?;
        let (global, q) = restricted.clifford().map_err(hard)?;
        coefficient = arithmetic
            .multiply(
                &coefficient,
                &arithmetic
                    .omega(i64::from((global + 2 * q.constant()) & 7), 0)
                    .map_err(|e| error("phase_injection.arithmetic", e))?,
            )
            .map_err(|e| error("phase_injection.arithmetic", e))?;
        let constraints = independent
            .iter()
            .enumerate()
            .map(|(j, &row)| Ok((bits(rank, row)?, word >> j & 1 != 0)))
            .collect::<Result<Vec<_>, CatalogueError>>()?;
        terms.push(CliffordIndicator {
            coefficient,
            constraints,
            phase: extend_clifford(rank, &free, &q)?,
        });
    }
    let mut candidate = PhaseCatalogue::new();
    let input = PhaseDecompositionInput {
        dimension: rank,
        phase_table: original.phase_table().to_vec(),
        terms,
    };
    match candidate.insert(input, construction, arithmetic) {
        Ok(()) => {}
        Err(cause)
            if matches!(
                cause.field.as_str(),
                "catalogue.max_entries"
                    | "catalogue.max_dimension"
                    | "catalogue.max_table_values"
                    | "catalogue.max_terms"
                    | "catalogue.max_constraint_rows"
                    | "catalogue.max_parity_rows"
                    | "catalogue.max_verification_work"
            ) =>
        {
            receipt.budget_skip = Some(cause.field);
            return Ok((None, receipt));
        }
        Err(cause) => return Err(cause),
    }
    receipt.constructed = true;
    receipt.selected = true;
    Ok((Some(candidate), receipt))
}
