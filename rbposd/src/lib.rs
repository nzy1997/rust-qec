//! Belief-propagation decoders with ordered-statistics (OSD) or
//! localized-statistics (LSD) post-processing for binary linear codes.
//!
//! Construct a sparse [`ParityCheckMatrix`], choose a [`ChannelModel`], and
//! reuse a decoder for multiple syndromes. Decoder calls validate channel and
//! syndrome dimensions and return [`DecodeError`] on invalid inputs or when a
//! post-processor cannot find a solution.
//!
//! Channel probabilities must be finite and strictly between 0 and 1. LSD
//! supports orders 0 and 1. See [`DecoderConfig`] and [`LsdConfig`] for the
//! available algorithms and defaults.
//!
//! # BP+OSD example
//!
//! ```rust
//! use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};
//!
//! let pcm = ParityCheckMatrix::from_sparse_rows(
//!     2,
//!     3,
//!     vec![vec![0, 1], vec![1, 2]],
//! )
//! .unwrap();
//! let decoder = BpOsdDecoder::new(
//!     pcm.clone(),
//!     ChannelModel::Bsc { error_rate: 0.05 },
//!     DecoderConfig::default(),
//! )
//! .unwrap();
//! let syndrome = Syndrome::from(vec![true, false]);
//! let result = decoder.decode(&syndrome).unwrap();
//! assert_eq!(pcm.multiply(&result.correction), syndrome);
//! ```
//!
//! # BP+LSD example
//!
//! ```rust
//! use rbposd::{BpLsdDecoder, ChannelModel, LsdConfig, ParityCheckMatrix, Syndrome};
//!
//! let pcm = ParityCheckMatrix::from_sparse_rows(
//!     2,
//!     3,
//!     vec![vec![0, 1], vec![1, 2]],
//! )
//! .unwrap();
//! let decoder = BpLsdDecoder::new(
//!     pcm.clone(),
//!     ChannelModel::Bsc { error_rate: 0.05 },
//!     LsdConfig::default(),
//! )
//! .unwrap();
//! let syndrome = Syndrome::from(vec![true, false]);
//! let result = decoder.decode(&syndrome).unwrap();
//! assert_eq!(pcm.multiply(&result.correction), syndrome);
//! ```
//!
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod css;
mod decoder;
mod decoder_core;
mod gf2;
mod lsd;
mod lsd_decoder;
mod osd;

pub use config::{
    BpVariant, ChannelModel, DecoderConfig, LsdConfig, LsdMethod, OsdVariant, Schedule,
};
pub use css::CssDecoders;
pub use decoder::{BpOsdDecoder, DecodeResult, DecodeStats, OsdPathDiagnostic};
pub use error::DecodeError;
pub use lsd_decoder::BpLsdDecoder;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
