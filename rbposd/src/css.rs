use crate::config::ChannelModel;
use crate::decoder::{BpOsdDecoder, DecodeResult};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::Syndrome;
use crate::DecoderConfig;

#[derive(Debug, Clone)]
pub struct CssDecoders {
    x: BpOsdDecoder,
    z: BpOsdDecoder,
}

impl CssDecoders {
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

    pub fn decode_x(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.x.decode(syndrome)
    }

    pub fn decode_z(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.z.decode(syndrome)
    }
}
