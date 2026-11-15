//! Owner-bound proposals over real records, separate from physical marginal sampling.
use super::*;
use crate::{
    gf2::{AffineSupport, LinearFactorization},
    noise::RandRangeSource,
    probability::Probability,
};
use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};

#[derive(Clone, Copy, Debug)]
pub struct OriginalProposalOptions {
    /// Sum of admitted numerator/denominator bits, also bounding products.
    pub max_probability_bits: usize,
    pub max_geometry_words: usize,
    pub max_elimination_work: usize,
    pub max_certificate_bytes: usize,
    pub max_range_calls: usize,
}
impl Default for OriginalProposalOptions {
    fn default() -> Self {
        Self {
            max_probability_bits: 1 << 20,
            max_geometry_words: 1 << 20,
            max_elimination_work: 64 << 20,
            max_certificate_bytes: 65536,
            max_range_calls: 1 << 20,
        }
    }
}
#[derive(Debug)]
pub enum OriginalProposalDrawError<E> {
    Source(E),
    Original(OriginalError),
    OutOfRange,
}
impl<E: std::fmt::Display> std::fmt::Display for OriginalProposalDrawError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => write!(f, "record proposal range source: {e}"),
            Self::Original(e) => e.fmt(f),
            Self::OutOfRange => f.write_str(
                "record proposal range source returned a value outside its requested bound",
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for OriginalProposalDrawError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(e) => Some(e),
            Self::Original(e) => Some(e),
            Self::OutOfRange => None,
        }
    }
}
pub struct OriginalProposalDraw {
    pub assignment: Arc<OriginalCompactAssignment>,
    pub proposal_probability: BigRational,
    /// Protocol requests, not entropy words consumed by the caller's source.
    pub range_requests: usize,
}
#[derive(Clone)]
pub struct OriginalIndependentRecordProposal {
    plan: OriginalAssignmentPlan,
    probabilities: Vec<Probability>,
    options: OriginalProposalOptions,
}
impl OriginalIndependentRecordProposal {
    /// Every real record is proposed with strict full support. Unspecified
    /// records use1/2; unknown names are rejected rather than silently ignored.
    pub fn new(
        plan: &OriginalAssignmentPlan,
        overrides: &BTreeMap<String, Probability>,
        options: OriginalProposalOptions,
    ) -> Result<Self, OriginalError> {
        let bytes = overrides
            .iter()
            .map(|(n, p)| {
                32 + n.len() as u128
                    + (p.value().numer().bits() + p.value().denom().bits()) as u128 / 8
            })
            .sum();
        admit(
            "max_metadata_bytes",
            plan.0.instrument.limits.max_metadata_bytes,
            bytes,
        )?;
        for (name, p) in overrides {
            if !plan.records().contains(name) {
                return Err(invalid("unknown proposal record", name));
            }
            if p.value().is_zero() || p.value().is_one() {
                return Err(invalid(
                    "full-support record probability must be strictly between zero and one",
                    name,
                ));
            }
        }
        let bits = plan
            .records()
            .iter()
            .map(|name| {
                overrides.get(name).map_or(3, |p| {
                    p.value().numer().bits() as u128 + p.value().denom().bits() as u128
                })
            })
            .sum();
        admit(
            "original_proposal.max_probability_bits",
            options.max_probability_bits,
            bits,
        )?;
        admit(
            "original_proposal.max_geometry_words",
            options.max_geometry_words,
            4 * plan.records().len() as u128,
        )?;
        let half: Probability = "1/2".parse().expect("half probability");
        let probabilities = plan
            .records()
            .iter()
            .map(|n| overrides.get(n).unwrap_or(&half).clone())
            .collect();
        Ok(Self {
            plan: plan.clone(),
            probabilities,
            options,
        })
    }
    pub fn draw<R: RandRangeSource + ?Sized>(
        &self,
        random: &mut R,
        group: OriginalSharedFaults,
    ) -> Result<OriginalProposalDraw, OriginalProposalDrawError<R::Error>> {
        self.plan
            .admit_group(group.faults(), group.report_flips())
            .map_err(OriginalProposalDrawError::Original)?;
        admit(
            "original_proposal.max_range_calls",
            self.options.max_range_calls,
            self.probabilities.len() as u128,
        )
        .map_err(OriginalProposalDrawError::Original)?;
        let mut word = BitVector::zeros(self.probabilities.len())
            .map_err(OriginalError::from)
            .map_err(OriginalProposalDrawError::Original)?;
        let mut q = BigRational::one();
        for (index, p) in self.probabilities.iter().enumerate() {
            let bound = p
                .value()
                .denom()
                .to_biguint()
                .expect("positive denominator");
            let value = random
                .randrange(&bound)
                .map_err(OriginalProposalDrawError::Source)?;
            if value >= bound {
                return Err(OriginalProposalDrawError::OutOfRange);
            }
            let bit = value
                < p.value()
                    .numer()
                    .to_biguint()
                    .expect("positive probability");
            word.set(index, bit)
                .map_err(OriginalError::from)
                .map_err(OriginalProposalDrawError::Original)?;
            q *= if bit {
                p.value().clone()
            } else {
                BigRational::one() - p.value()
            };
        }
        Ok(OriginalProposalDraw {
            assignment: self
                .plan
                .compact(word, group)
                .map_err(OriginalProposalDrawError::Original)?,
            proposal_probability: q,
            range_requests: self.probabilities.len(),
        })
    }
}
#[derive(Clone)]
pub struct OriginalAffineRecordProposal {
    plan: OriginalAssignmentPlan,
    family: AffineSupport,
    coverage_certificate: String,
    options: OriginalProposalOptions,
}
impl OriginalAffineRecordProposal {
    /// The caller supplies physical coverage evidence for this source/input/
    /// noise family. This constructor checks affine algebra,not that physical
    /// claim. Missing evidence,empty support and foreign dimensions reject.
    pub fn new(
        plan: &OriginalAssignmentPlan,
        rows: &[BitVector],
        outcomes: &BitVector,
        coverage_certificate: &str,
        options: OriginalProposalOptions,
    ) -> Result<Self, OriginalError> {
        let n = plan.records().len();
        let width = n.div_ceil(64) as u128;
        admit(
            "original_proposal.max_certificate_bytes",
            options.max_certificate_bytes,
            coverage_certificate.len() as u128,
        )?;
        if coverage_certificate.is_empty() {
            return Err(invalid(
                "affine record proposal requires a physical coverage certificate",
                "",
            ));
        }
        admit(
            "original_proposal.max_geometry_words",
            options.max_geometry_words,
            (n as u128 + rows.len() as u128) * width + n as u128,
        )?;
        admit(
            "original_proposal.max_elimination_work",
            options.max_elimination_work,
            (rows.len() as u128 + n as u128) * n as u128 * width,
        )?;
        let factor = LinearFactorization::new(n, rows)?;
        let offset = factor
            .solve(outcomes)?
            .ok_or_else(|| invalid("affine record proposal has empty support", ""))?;
        let family = AffineSupport::new(offset, factor.nullspace()?)?;
        admit(
            "original_proposal.max_probability_bits",
            options.max_probability_bits,
            family.dimension() as u128 + 1,
        )?;
        Ok(Self {
            plan: plan.clone(),
            family,
            coverage_certificate: coverage_certificate.into(),
            options,
        })
    }
    /// Dynamic integer boundary: accept only exact0/1 before constructing bits.
    pub fn from_integer_equations(
        plan: &OriginalAssignmentPlan,
        rows: &[BitVector],
        rhs: &[i64],
        certificate: &str,
        options: OriginalProposalOptions,
    ) -> Result<Self, OriginalError> {
        if rows.len() != rhs.len() || rhs.iter().any(|&b| b != 0 && b != 1) {
            return Err(invalid(
                "affine proposal RHS must contain one integer zero or one per row",
                "",
            ));
        }
        admit(
            "original_proposal.max_geometry_words",
            options.max_geometry_words,
            rhs.len().div_ceil(64) as u128,
        )?;
        let outcomes = BitVector::from_indices(
            rhs.len(),
            rhs.iter()
                .enumerate()
                .filter_map(|(i, &b)| (b == 1).then_some(i)),
        )?;
        Self::new(plan, rows, &outcomes, certificate, options)
    }
    pub fn family(&self) -> &AffineSupport {
        &self.family
    }
    pub fn coverage_certificate(&self) -> &str {
        &self.coverage_certificate
    }
    pub fn draw<R: RandRangeSource + ?Sized>(
        &self,
        random: &mut R,
        group: OriginalSharedFaults,
    ) -> Result<OriginalProposalDraw, OriginalProposalDrawError<R::Error>> {
        self.plan
            .admit_group(group.faults(), group.report_flips())
            .map_err(OriginalProposalDrawError::Original)?;
        let dimensions = self.family.dimension();
        let calls = usize::from(dimensions > 0);
        admit(
            "original_proposal.max_range_calls",
            self.options.max_range_calls,
            calls as u128,
        )
        .map_err(OriginalProposalDrawError::Original)?;
        let bound = BigUint::one() << dimensions;
        let selected = if dimensions == 0 {
            BigUint::zero()
        } else {
            random
                .randrange(&bound)
                .map_err(OriginalProposalDrawError::Source)?
        };
        if selected >= bound {
            return Err(OriginalProposalDrawError::OutOfRange);
        }
        let mut word = self.family.offset().clone();
        // BigUint bit coordinates are explicit; no narrowing to a machine word.
        for (j, column) in self.family.columns().iter().enumerate() {
            if selected.bit(j.to_u64().expect("usize coordinate fits u64")) {
                word.xor_assign(column)
                    .map_err(OriginalError::from)
                    .map_err(OriginalProposalDrawError::Original)?;
            }
        }
        Ok(OriginalProposalDraw {
            assignment: self
                .plan
                .compact(word, group)
                .map_err(OriginalProposalDrawError::Original)?,
            proposal_probability: BigRational::new(1.into(), bound.into()),
            range_requests: calls,
        })
    }
}
