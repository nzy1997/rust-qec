use crate::DecoderConfig;
use crate::config::ChannelModel;
use crate::decoder::{BpOsdDecoder, DecodeResult};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::Syndrome;

/// Independent BP+OSD decoders for the X and Z checks of a CSS code.
#[derive(Debug, Clone)]
pub struct CssDecoders {
    x: BpOsdDecoder,
    z: BpOsdDecoder,
}

impl CssDecoders {
    /// Construct X and Z decoders with a shared decoder configuration.
    pub fn new(
        hx: ParityCheckMatrix,
        hz: ParityCheckMatrix,
        x_channel: ChannelModel,
        z_channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            x: BpOsdDecoder::new(hx, x_channel, config.clone())?,
            z: BpOsdDecoder::new(hz, z_channel, config)?,
        })
    }

    /// Decode an X-check syndrome.
    pub fn decode_x(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.x.decode(syndrome)
    }

    /// Decode a Z-check syndrome.
    pub fn decode_z(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.z.decode(syndrome)
    }
}
