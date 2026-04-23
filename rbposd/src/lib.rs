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
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod decoder;
mod gf2;
mod osd;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use decoder::{BpOsdDecoder, DecodeResult};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
