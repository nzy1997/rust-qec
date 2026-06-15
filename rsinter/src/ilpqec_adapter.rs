use rilpqec::{BackendKind, IlpDecoderConfig};
use rstim::dem::DetectorErrorModel;

use crate::decode::{CompiledDecoder, Decoder};

pub struct IlpDemDecoder {
    config: IlpDecoderConfig,
}

impl IlpDemDecoder {
    pub fn new(config: IlpDecoderConfig) -> Self {
        Self { config }
    }
}

impl Default for IlpDemDecoder {
    fn default() -> Self {
        let mut config = IlpDecoderConfig::default();
        config.backend.kind = BackendKind::Auto;
        Self::new(config)
    }
}

struct CompiledIlpDemDecoder {
    decoder: rilpqec::IlpDemDecoder,
}

impl Decoder for IlpDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        let decoder = rilpqec::IlpDemDecoder::from_dem(dem, self.config.clone())
            .map_err(|error| error.to_string())?;
        Ok(Box::new(CompiledIlpDemDecoder { decoder }))
    }
}

impl CompiledDecoder for CompiledIlpDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        self.decoder
            .decode_batch_bit_packed(dets, num_shots, num_dets, num_obs)
            .map_err(|error| error.to_string())
    }
}
