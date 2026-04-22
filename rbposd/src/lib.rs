pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod gf2;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
