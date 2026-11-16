//! Exact unreduced grouped likelihood payloads, separate from physical laws.
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Zero};
use std::collections::BTreeMap;
/// A nonnegative numerator and positive denominator, stored without reduction.
/// Products preserve cross-factor cancellation (for example, 2/3 times 3/2 is
/// retained as 6/6). An input factor with equal numerator/denominator is skipped.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawNoiseFactor {
    numerator: BigUint,
    denominator: BigUint,
}
impl RawNoiseFactor {
    pub fn new(numerator: BigUint, denominator: BigUint) -> Result<Self, NoiseWeightError> {
        if denominator.is_zero() {
            return Err(NoiseWeightError::DenominatorZero);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
    /// Capture a signed raw pair without reduction. Negative numerators and
    /// nonpositive denominators fail before conversion to unsigned magnitudes.
    pub fn from_ratio(numerator: BigInt, denominator: BigInt) -> Result<Self, NoiseWeightError> {
        if denominator.is_zero() {
            return Err(NoiseWeightError::DenominatorZero);
        }
        if numerator.sign() == Sign::Minus || denominator.sign() == Sign::Minus {
            return Err(NoiseWeightError::NegativeFactor);
        }
        Self::new(numerator.into_parts().1, denominator.into_parts().1)
    }
    pub fn numerator(&self) -> &BigUint {
        &self.numerator
    }
    pub fn denominator(&self) -> &BigUint {
        &self.denominator
    }
    fn effective(&self) -> Self {
        if self.numerator == self.denominator {
            Self::one()
        } else {
            self.clone()
        }
    }
    pub fn one() -> Self {
        Self {
            numerator: BigUint::one(),
            denominator: BigUint::one(),
        }
    }
    fn multiply(&self, right: &Self) -> Self {
        Self {
            numerator: &self.numerator * &right.numerator,
            denominator: &self.denominator * &right.denominator,
        }
    }
    fn power(&self, n: u32) -> Self {
        Self {
            numerator: self.numerator.pow(n),
            denominator: self.denominator.pow(n),
        }
    }
    fn bound(&self) -> (u64, u64) {
        if self.numerator == self.denominator {
            (0, 0)
        } else {
            (self.numerator.bits(), self.denominator.bits())
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct NoiseWeightLimits {
    pub max_sites: usize,
    pub max_factor_bits: u64,
    pub max_product_bits: u64,
    /// Sum of retained minimal unsigned magnitude bytes; zero uses no bytes.
    /// Excludes container metadata, allocator capacity and construction scratch.
    pub max_table_integer_payload_bytes: usize,
    /// Count-table threshold, not a refusal limit. Larger groups use exact
    /// integer powers; zero disables count tables.
    pub count_table_max_sites: usize,
}
impl Default for NoiseWeightLimits {
    fn default() -> Self {
        Self {
            max_sites: 65536,
            max_factor_bits: 4096,
            max_product_bits: 1 << 20,
            max_table_integer_payload_bytes: 64 << 20,
            count_table_max_sites: 512,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoiseWeightError {
    DenominatorZero,
    Sites {
        required: usize,
        limit: usize,
    },
    FactorBits {
        site: usize,
        factor: &'static str,
        required: u64,
        limit: u64,
    },
    ProductBits {
        required: u128,
        limit: u64,
    },
    TableBytes {
        required: u128,
        limit: usize,
    },
    Shape {
        expected: usize,
        actual: usize,
    },
    InvalidOptions {
        option: &'static str,
    },
    NegativeFactor,
}
struct Group {
    sites: Vec<usize>,
    identity: RawNoiseFactor,
    error: RawNoiseFactor,
    table: Option<Vec<RawNoiseFactor>>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoiseWeightStatistics {
    pub groups: usize,
    pub count_tables: usize,
    pub fallback_groups: usize,
    pub table_rows: usize,
    pub table_integer_payload_bytes: usize,
}
/// An owned grouped factor plan, separate from any physical or proposal law.
/// The two factors are (identity, selected nonidentity), regardless of alternative.
/// Default count tables cover groups through 512 sites; 513+ uses integer powers.
pub struct PreparedNoiseWeights {
    sites: usize,
    groups: Vec<Group>,
    statistics: NoiseWeightStatistics,
}
impl PreparedNoiseWeights {
    pub fn new(
        factors: &[(RawNoiseFactor, RawNoiseFactor)],
        limits: NoiseWeightLimits,
    ) -> Result<Self, NoiseWeightError> {
        for (option, invalid) in [
            ("max_sites", limits.max_sites > u32::MAX as usize),
            ("max_factor_bits", limits.max_factor_bits == 0),
            ("max_product_bits", limits.max_product_bits == 0),
        ] {
            if invalid {
                return Err(NoiseWeightError::InvalidOptions { option });
            }
        }
        if factors.len() > limits.max_sites {
            return Err(NoiseWeightError::Sites {
                required: factors.len(),
                limit: limits.max_sites,
            });
        }
        let (mut nbits, mut dbits) = (0u128, 0u128);
        // Admit the whole input before cloning factors or building any table.
        for (site, (i, e)) in factors.iter().enumerate() {
            for (factor, f) in [("identity", i), ("error", e)] {
                if f.numerator.bits().max(f.denominator.bits()) > limits.max_factor_bits {
                    return Err(NoiseWeightError::FactorBits {
                        site,
                        factor,
                        required: f.numerator.bits().max(f.denominator.bits()),
                        limit: limits.max_factor_bits,
                    });
                }
            }
            let (i, e) = (i.bound(), e.bound());
            nbits += u128::from(i.0.max(e.0));
            dbits += u128::from(i.1.max(e.1));
        }
        if nbits.max(dbits).max(1) > u128::from(limits.max_product_bits) {
            return Err(NoiseWeightError::ProductBits {
                required: nbits.max(dbits).max(1),
                limit: limits.max_product_bits,
            });
        }
        let mut grouped = BTreeMap::<(RawNoiseFactor, RawNoiseFactor), Vec<usize>>::new();
        for (site, (i, e)) in factors.iter().enumerate() {
            grouped
                .entry((i.effective(), e.effective()))
                .or_default()
                .push(site);
        }
        let mut bytes = 0u128;
        for ((i, e), sites) in &grouped {
            if sites.len() <= limits.count_table_max_sites {
                let per_n = (u128::from(i.numerator.bits().max(e.numerator.bits()))
                    * sites.len() as u128)
                    .max(1)
                    .div_ceil(8);
                let per_d = (u128::from(i.denominator.bits().max(e.denominator.bits()))
                    * sites.len() as u128)
                    .max(1)
                    .div_ceil(8);
                bytes += (sites.len() as u128 + 1) * (per_n + per_d);
            }
        }
        if bytes > limits.max_table_integer_payload_bytes as u128 {
            return Err(NoiseWeightError::TableBytes {
                required: bytes,
                limit: limits.max_table_integer_payload_bytes,
            });
        }
        let mut statistics = NoiseWeightStatistics {
            groups: grouped.len(),
            ..Default::default()
        };
        let mut groups = vec![];
        for ((identity, error), sites) in grouped {
            let table = if sites.len() <= limits.count_table_max_sites {
                let n = sites.len();
                let mut ip = vec![RawNoiseFactor::one()];
                let mut ep = vec![RawNoiseFactor::one()];
                for _ in 0..n {
                    ip.push(ip.last().unwrap().multiply(&identity));
                    ep.push(ep.last().unwrap().multiply(&error));
                }
                let table = (0..=n)
                    .map(|k| ip[n - k].multiply(&ep[k]))
                    .collect::<Vec<_>>();
                for value in &table {
                    statistics.table_integer_payload_bytes += value.numerator.bits().div_ceil(8)
                        as usize
                        + value.denominator.bits().div_ceil(8) as usize;
                }
                statistics.count_tables += 1;
                statistics.table_rows += table.len();
                Some(table)
            } else {
                statistics.fallback_groups += 1;
                None
            };
            groups.push(Group {
                sites,
                identity,
                error,
                table,
            });
        }
        debug_assert!(
            statistics.table_integer_payload_bytes <= limits.max_table_integer_payload_bytes
        );
        Ok(Self {
            sites: factors.len(),
            groups,
            statistics,
        })
    }
    pub fn statistics(&self) -> NoiseWeightStatistics {
        self.statistics
    }
    pub fn evaluate(&self, selected: &[bool]) -> Result<RawNoiseFactor, NoiseWeightError> {
        if selected.len() != self.sites {
            return Err(NoiseWeightError::Shape {
                expected: self.sites,
                actual: selected.len(),
            });
        }
        let mut result = RawNoiseFactor::one();
        for group in &self.groups {
            let k = group.sites.iter().filter(|&&site| selected[site]).count();
            let value = match &group.table {
                Some(table) => table[k].clone(),
                None => group
                    .identity
                    .power((group.sites.len() - k) as u32)
                    .multiply(&group.error.power(k as u32)),
            };
            result = result.multiply(&value);
        }
        Ok(result)
    }
}

impl std::fmt::Display for NoiseWeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DenominatorZero => write!(f, "noise weight denominator must be positive"),
            Self::NegativeFactor => write!(
                f,
                "noise weight numerator must be nonnegative and denominator positive"
            ),
            Self::Sites { required, limit } => write!(
                f,
                "noise_weights.max_sites={limit} is insufficient; need {required}"
            ),
            Self::FactorBits {
                site,
                factor,
                required,
                limit,
            } => write!(
                f,
                "noise_weights.max_factor_bits={limit} is insufficient for site {site} {factor} factor; need {required}"
            ),
            Self::ProductBits { required, limit } => write!(
                f,
                "noise_weights.max_product_bits={limit} is insufficient; conservative requirement {required}"
            ),
            Self::TableBytes { required, limit } => write!(
                f,
                "noise_weights.max_table_integer_payload_bytes={limit} is insufficient; conservative requirement {required}"
            ),
            Self::Shape { expected, actual } => write!(
                f,
                "noise weight selection has {actual} sites; expected {expected}"
            ),
            Self::InvalidOptions { option } => write!(
                f,
                "invalid noise weight option {option}: bit limits must be positive and max_sites must fit u32"
            ),
        }
    }
}
impl std::error::Error for NoiseWeightError {}

impl std::fmt::Debug for PreparedNoiseWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedNoiseWeights")
            .field("sites", &self.sites)
            .field("statistics", &self.statistics)
            .finish_non_exhaustive()
    }
}
