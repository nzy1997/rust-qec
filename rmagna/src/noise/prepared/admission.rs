//! Admission of captured laws and conservative retained/output payloads.
use super::*;
pub(super) struct Admitted {
    pub(super) totals: Vec<Probability>,
    pub(super) rows: Vec<Vec<Probability>>,
    pub(super) model_payload_bound: u128,
    pub(super) draw_payload_bound: u128,
}
pub(super) fn admit(
    channels: &[Vec<Probability>],
    actions: &[Vec<SparseNoiseAction>],
    factors: &[(RawNoiseFactor, RawNoiseFactor)],
    options: PreparedNoiseOptions,
) -> Result<Admitted, PreparedNoiseError> {
    for (field, actual) in [("actions", actions.len()), ("factors", factors.len())] {
        if actual != channels.len() {
            return Err(PreparedNoiseError::Shape {
                field,
                site: None,
                expected: channels.len(),
                actual,
            });
        }
    }
    if factors.len() > options.weights.max_sites {
        return Err(NoiseWeightError::Sites {
            required: factors.len(),
            limit: options.weights.max_sites,
        }
        .into());
    }
    if channels.len() > options.sparse.max_sites {
        return Err(SparseNoiseError::Limit {
            option: "sparse_noise.max_sites",
            required: channels.len() as u128,
            limit: options.sparse.max_sites,
        }
        .into());
    }
    let mut totals = vec![];
    let mut rows = vec![];
    let mut count = 0u128;
    let mut model_payload_bound = 256u128;
    for (site, law) in channels.iter().enumerate() {
        count += law.len() as u128;
        if count > options.sparse.max_actions as u128 {
            return Err(SparseNoiseError::Limit {
                option: "sparse_noise.max_actions",
                required: count,
                limit: options.sparse.max_actions,
            }
            .into());
        }
        if law.len() != actions[site].len() {
            return Err(PreparedNoiseError::Shape {
                field: "actions",
                site: Some(site),
                expected: law.len(),
                actual: actions[site].len(),
            });
        }
        let (total, common_bits) = law_total(law, site, options.sparse.noise)?;
        // One category's probability/cumulative/replacement buffers plus
        // global product and owner metadata. Eight copies conservatively
        // cover physical/proposal owners and per-site fallback/conditional
        // samplers before equivalent laws are interned.
        let category_payload = 256
            + (law.len() as u128 + 1)
                * (2 * std::mem::size_of::<BigRational>() as u128
                    + std::mem::size_of::<num_bigint::BigUint>() as u128
                    + 5 * u128::from(common_bits).div_ceil(8));
        model_payload_bound += 8 * category_payload;
        if model_payload_bound > options.max_model_payload_bytes as u128 {
            return Err(PreparedNoiseError::RequestLimit {
                option: "max_model_payload_bytes",
                required: model_payload_bound,
                limit: options.max_model_payload_bytes,
            });
        }
        let mut row = vec![total.complement()];
        row.extend_from_slice(law);
        rows.push(row);
        totals.push(total);
    }
    if model_payload_bound > options.max_model_payload_bytes as u128 {
        return Err(PreparedNoiseError::RequestLimit {
            option: "max_model_payload_bytes",
            required: model_payload_bound,
            limit: options.max_model_payload_bytes,
        });
    }
    let draw_payload_bound = output_bound(&rows, channels.len(), actions, factors, options)?;
    Ok(Admitted {
        totals,
        rows,
        model_payload_bound,
        draw_payload_bound,
    })
}
fn law_total(
    law: &[Probability],
    site: usize,
    options: NoiseOptions,
) -> Result<(Probability, u64), PreparedNoiseError> {
    // Bound the common denominator before probability arithmetic or
    // model cloning. The existing exact scalar compiler uses this gcd.
    let mut common = num_bigint::BigUint::one();
    for p in law {
        let denominator = p.value().denom().magnitude();
        common = (&common / super::super::gcd(common.clone(), denominator.clone())) * denominator;
        if common.bits() > options.max_denominator_bits {
            return Err(NoiseError::DenominatorLimit {
                limit: options.max_denominator_bits,
                required: common.bits(),
            }
            .into());
        }
    }
    let mut numerator = num_bigint::BigUint::zero();
    for p in law {
        numerator += p.value().numer().magnitude() * (&common / p.value().denom().magnitude());
        if numerator > common {
            return Err(PreparedNoiseError::Category(site));
        }
    }
    let common_bits = common.bits();
    let total = Probability::from_ratio(numerator.into(), common.into())
        .expect("admitted nonnegative category");
    Ok((total, common_bits))
}
fn output_bound(
    rows: &[Vec<Probability>],
    sites: usize,
    actions: &[Vec<SparseNoiseAction>],
    factors: &[(RawNoiseFactor, RawNoiseFactor)],
    options: PreparedNoiseOptions,
) -> Result<u128, PreparedNoiseError> {
    // Bound complete output payload before cloning the action owner or
    // constructing count tables. Per-row denominator products bound any
    // exact reduced physical probability numerator and denominator.
    let probability_bits = rows
        .iter()
        .flat_map(|row| row.iter())
        .map(|p| u128::from(p.value().denom().bits()))
        .sum::<u128>();
    let action_bytes = actions
        .iter()
        .map(|row| {
            row.iter()
                .map(|action| {
                    std::mem::size_of::<SparseNoiseAction>() as u128
                        + match action {
                            SparseNoiseAction::Pauli { operation, factors } => {
                                operation.len() as u128
                                    + factors.len() as u128
                                        * std::mem::size_of::<(usize, crate::circuit::Basis)>()
                                            as u128
                            }
                            SparseNoiseAction::ReportFlip { record } => record.len() as u128,
                        }
                })
                .max()
                .unwrap_or(0)
        })
        .sum::<u128>();
    let mut raw_n = 0u128;
    let mut raw_d = 0u128;
    for (i, e) in factors {
        let bits = |f: &RawNoiseFactor| {
            if f.numerator() == f.denominator() {
                (0, 0)
            } else {
                (
                    u128::from(f.numerator().bits()),
                    u128::from(f.denominator().bits()),
                )
            }
        };
        let (i, e) = (bits(i), bits(e));
        raw_n += i.0.max(e.0);
        raw_d += i.1.max(e.1);
    }
    let draw_payload_bound = std::mem::size_of::<PreparedNoiseDraw>() as u128
        + sites as u128 * std::mem::size_of::<Option<usize>>() as u128
        + action_bytes
        + raw_n.max(1).div_ceil(8)
        + raw_d.max(1).div_ceil(8)
        + 2 * probability_bits.max(1).div_ceil(8);
    if draw_payload_bound > options.max_batch_payload_bytes as u128 {
        return Err(PreparedNoiseError::RequestLimit {
            option: "max_batch_payload_bytes",
            required: draw_payload_bound,
            limit: options.max_batch_payload_bytes,
        });
    }
    Ok(draw_payload_bound)
}
