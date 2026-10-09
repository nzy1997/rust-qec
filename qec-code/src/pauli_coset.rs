//! Bounded exact minimum-support queries in a signed stabilizer coset.
//!
//! This exhaustive cache is intended for small-rank declarations, not a
//! scalable distance algorithm. It preserves phases and original-generator
//! subset order. Redundant declarations still count against the generator cap.

use crate::phased_pauli::{PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup};
use thiserror::Error;

const MAX_GENERATORS: usize = 12;
const MAX_WIDTH: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CosetSearchError {
    #[error("bounded coset search permits at most {limit} declared generators, got {actual}")]
    GeneratorLimit { actual: usize, limit: usize },
    #[error("bounded coset search permits at most {limit} physical qubits, got {actual}")]
    WidthLimit { actual: usize, limit: usize },
    #[error("coset cache needs {required} products, exceeding requested budget {limit}")]
    ProductLimit { required: usize, limit: usize },
    #[error(transparent)]
    Algebra(#[from] PhaseAlgebraError),
}

/// Reusable exact exhaustive cache of `target * signed_stabilizer` candidates.
///
/// Construction is capped at 12 *declared* generators, 4096 physical qubits,
/// and a caller-specified product budget. Consistent redundant generators are
/// accepted within these caps; their products may repeat. A query considers
/// all generator subsets, so its minimum is exact. Ties choose the first
/// subset in ascending binary-mask order on the original declaration.
#[derive(Debug, Clone)]
pub struct PauliCosetSearch {
    n: usize,
    products: Vec<PhasedPauli>,
    packed_supports: Vec<(Vec<u64>, Vec<u64>)>,
}

fn pack(bits: &[u8]) -> Vec<u64> {
    bits.chunks(64)
        .map(|chunk| {
            chunk
                .iter()
                .enumerate()
                .fold(0, |word, (i, &bit)| word | (u64::from(bit) << i))
        })
        .collect()
}

impl PauliCosetSearch {
    pub fn new(
        group: &SignedStabilizerGroup,
        max_products: usize,
    ) -> Result<Self, CosetSearchError> {
        let count = group.generators().len();
        if count > MAX_GENERATORS {
            return Err(CosetSearchError::GeneratorLimit {
                actual: count,
                limit: MAX_GENERATORS,
            });
        }
        if group.n() > MAX_WIDTH {
            return Err(CosetSearchError::WidthLimit {
                actual: group.n(),
                limit: MAX_WIDTH,
            });
        }
        let required = 1usize << count;
        if required > max_products {
            return Err(CosetSearchError::ProductLimit {
                required,
                limit: max_products,
            });
        }
        let mut products = Vec::with_capacity(required);
        products.push(PhasedPauli::identity(group.n()));
        for generator in group.generators() {
            let previous = products.len();
            for i in 0..previous {
                products.push(products[i].multiply(generator)?);
            }
        }
        let packed_supports = products
            .iter()
            .map(|p| (pack(p.support().x_bits()), pack(p.support().z_bits())))
            .collect();
        Ok(Self {
            n: group.n(),
            products,
            packed_supports,
        })
    }

    pub fn product_count(&self) -> usize {
        self.products.len()
    }

    /// Return the minimum physical support representative, including its exact
    /// scalar. The ordered product is `target * stabilizer`, so its action on
    /// the group's signed +1 codespace equals the original target's action.
    /// The target need not centralize the group; Hermiticity is not assumed.
    pub fn minimum_weight(&self, target: &PhasedPauli) -> Result<PhasedPauli, CosetSearchError> {
        if target.n() != self.n {
            return Err(PhaseAlgebraError::WidthMismatch {
                expected: self.n,
                actual: target.n(),
            }
            .into());
        }
        let x = pack(target.support().x_bits());
        let z = pack(target.support().z_bits());
        let index = self
            .packed_supports
            .iter()
            .enumerate()
            .min_by_key(|(_, (sx, sz))| {
                x.iter()
                    .zip(&z)
                    .zip(sx.iter().zip(sz))
                    .map(|((&a, &b), (&c, &d))| ((a ^ c) | (b ^ d)).count_ones() as usize)
                    .sum::<usize>()
            })
            .map(|(i, _)| i)
            .expect("cache always contains identity");
        Ok(target.multiply(&self.products[index])?)
    }
}
