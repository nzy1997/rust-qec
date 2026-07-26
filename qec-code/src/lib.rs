#[cfg(test)]
extern crate self as qec_code;

pub mod binary;
pub mod binary_chain_complex;
pub mod cli;
pub mod code;
pub mod codes;
pub mod css;
pub mod distance;
pub mod distance_bound;
pub mod distance_exact;
#[cfg(feature = "distance-ilp-highs")]
pub mod distance_ilp;
pub mod error;
mod gf2;
pub mod logical;
pub mod packed_gf2;
pub mod pauli;
pub mod regular_classical;
pub mod sparse_gf2;
mod symplectic;

pub use code::StabilizerCode;
pub use error::QecError;
pub use pauli::Pauli;
