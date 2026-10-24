//! Positive physical branches of conditioned reported parity groups.
use super::*;
use crate::gf2::{AffineSupport, LinearFactorization};

#[derive(Clone, Copy, Debug)]
pub struct RecordEnumerationOptions {
    pub max_branches: usize,
    pub max_record_bits: usize,
    pub max_marginal_queries: usize,
    pub max_terms: usize,
}
impl Default for RecordEnumerationOptions {
    fn default() -> Self {
        Self {
            max_branches: 4096,
            max_record_bits: 64,
            max_marginal_queries: 1_000_000,
            max_terms: 256,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedBranch {
    pub values: Vec<bool>,
    /// Joint physical mass of the given conditions and this complete group.
    pub mass: ExactScalar,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedEnumeration {
    pub branches: Vec<ReportedBranch>,
    pub condition_mass: ExactScalar,
    pub structural_rank: usize,
    pub marginal_queries: usize,
}
fn limit(option: &'static str, bound: usize, required: u128) -> Result<(), SamplingError> {
    if required > bound as u128 {
        return Err(SamplingError::EnumerationLimit {
            option,
            limit: bound,
            required,
        });
    }
    Ok(())
}
struct Node {
    values: Vec<bool>,
    mass: ExactScalar,
}
struct Coordinates {
    family: AffineSupport,
    selected: Vec<usize>,
    factor: LinearFactorization,
}
impl Coordinates {
    fn new(family: AffineSupport) -> Result<Self, SamplingError> {
        let rank = family.dimension();
        let selected = LinearFactorization::new(family.width(), family.columns())
            .map_err(InstrumentError::from)?
            .pivot_columns()
            .to_vec();
        let mut rows = Vec::with_capacity(rank);
        for &label in &selected {
            let mut row = BitVector::zeros(rank).map_err(InstrumentError::from)?;
            for (j, column) in family.columns().iter().enumerate() {
                row.set(j, column.get(label).map_err(InstrumentError::from)?)
                    .map_err(InstrumentError::from)?;
            }
            rows.push(row);
        }
        let coordinates = LinearFactorization::new(rank, &rows).map_err(InstrumentError::from)?;
        Ok(Self {
            family,
            selected,
            factor: coordinates,
        })
    }
    fn decode(&self, values: &[bool]) -> Result<Vec<bool>, SamplingError> {
        let rank = self.selected.len();
        let mut rhs = BitVector::zeros(rank).map_err(InstrumentError::from)?;
        for (j, (&label, &value)) in self.selected.iter().zip(values).enumerate() {
            rhs.set(
                j,
                value
                    ^ self
                        .family
                        .offset()
                        .get(label)
                        .map_err(InstrumentError::from)?,
            )
            .map_err(InstrumentError::from)?;
        }
        let coordinates = self
            .factor
            .solve(&rhs)
            .map_err(InstrumentError::from)?
            .ok_or(SamplingError::ImpossibleSupport)?;
        let mut word = self.family.offset().clone();
        for j in coordinates.ones() {
            word.xor_assign(&self.family.columns()[j])
                .map_err(InstrumentError::from)?;
        }
        let decoded = (0..self.family.width())
            .map(|j| word.get(j))
            .collect::<Result<Vec<_>, _>>()
            .map_err(InstrumentError::from)?;
        Ok(decoded)
    }
}
/// Enumerate positive physical reported outcomes with exact prefix marginals.
/// The independent-bit limit applies after conditioning structural support, so
/// repeated or complemented output labels do not consume additional bits.
/// Returned masses include the given conditions and exclude postselection.
pub fn enumerate_reported_conditioned(
    instrument: &CompiledInstrument,
    query: &RecordQuery<'_>,
    faults: &[FaultSelection],
    options: &RecordEnumerationOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<ReportedEnumeration, SamplingError> {
    enumerate_reported_conditioned_observed(instrument, query, faults, options, arithmetic, None)
}

pub(crate) fn enumerate_reported_conditioned_observed(
    instrument: &CompiledInstrument,
    query: &RecordQuery<'_>,
    faults: &[FaultSelection],
    options: &RecordEnumerationOptions,
    arithmetic: &ScalarArithmetic,
    meter: Option<&ExecutionMeter>,
) -> Result<ReportedEnumeration, SamplingError> {
    limit("max_branches", options.max_branches, 1)?;
    let family = observe(meter, OperationKind::Support, || {
        // Enumeration verifies each physical marginal, so a conservative
        // branch envelope does not require the full probability-parent resource.
        conditioned_support(instrument, query, faults, None, arithmetic)
    })?;
    let rank = family.dimension();
    limit("max_record_bits", options.max_record_bits, rank as u128)?;
    let coordinates = Coordinates::new(family)?;
    let parities = query
        .given
        .iter()
        .map(|(p, _)| p.clone())
        .chain(
            coordinates
                .selected
                .iter()
                .map(|&j| query.parities[j].clone()),
        )
        .collect::<Vec<_>>();
    let given = query.given.iter().map(|(_, v)| *v).collect::<Vec<_>>();
    let mut marginal_queries = 0_usize;
    let mut mass = |values: &[bool]| -> Result<ExactScalar, SamplingError> {
        limit(
            "max_marginal_queries",
            options.max_marginal_queries,
            marginal_queries as u128 + 1,
        )?;
        marginal_queries += 1;
        let bits = BitVector::from_indices(
            values.len(),
            values
                .iter()
                .enumerate()
                .filter_map(|(j, &b)| b.then_some(j)),
        )
        .map_err(InstrumentError::from)?;
        observe(meter, OperationKind::Query, || {
            Ok(instrument
                .reported_probability(
                    &parities[..values.len()],
                    &bits,
                    faults,
                    options.max_terms,
                    arithmetic,
                )?
                .value)
        })
    };
    let condition_mass = mass(&given)?;
    if condition_mass.real_sign()? != Ordering::Greater {
        return Err(SamplingError::ImpossibleCondition);
    }
    let mut stack = vec![Node {
        values: vec![],
        mass: condition_mass.clone(),
    }];
    let mut branches = Vec::new();
    while let Some(node) = stack.pop() {
        if let Some(meter) = meter {
            meter.check_time()?;
        }
        if node.values.len() == rank {
            let values = coordinates.decode(&node.values)?;
            branches.push(ReportedBranch {
                values,
                mass: node.mass,
            });
            continue;
        }
        let mut prefix = given
            .iter()
            .copied()
            .chain(node.values.iter().copied())
            .collect::<Vec<_>>();
        prefix.push(false);
        let zero = mass(&prefix)?;
        let one = arithmetic.subtract(&node.mass, &zero)?;
        let zero_sign = zero.real_sign()?;
        let one_sign = one.real_sign()?;
        if zero_sign == Ordering::Less || one_sign == Ordering::Less {
            return Err(SamplingError::InvalidMass);
        }
        let positive = usize::from(zero_sign == Ordering::Greater)
            + usize::from(one_sign == Ordering::Greater);
        limit(
            "max_branches",
            options.max_branches,
            branches.len() as u128 + stack.len() as u128 + positive as u128,
        )?;
        // Reverse stack insertion visits false before true without RNG.
        if one_sign == Ordering::Greater {
            let mut values = node.values.clone();
            values.push(true);
            stack.push(Node { values, mass: one });
        }
        if zero_sign == Ordering::Greater {
            let mut values = node.values;
            values.push(false);
            stack.push(Node { values, mass: zero });
        }
    }
    Ok(ReportedEnumeration {
        branches,
        condition_mass,
        structural_rank: rank,
        marginal_queries,
    })
}
