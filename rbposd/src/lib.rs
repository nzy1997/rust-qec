pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod decoder;
mod gf2;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use decoder::{BpOsdDecoder, DecodeResult};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
