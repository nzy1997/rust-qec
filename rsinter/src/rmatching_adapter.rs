use std::sync::Mutex;

use rmatching::Matching;
use rstim::dem::DetectorErrorModel;

use crate::decode::{CompiledDecoder, Decoder};

pub struct RmatchingDemDecoder;

struct CompiledRmatchingDemDecoder {
    matching: Mutex<Matching>,
}

impl Decoder for RmatchingDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        let matching = Matching::from_dem(&dem.to_string()).map_err(|error| error.to_string())?;
        Ok(Box::new(CompiledRmatchingDemDecoder {
            matching: Mutex::new(matching),
        }))
    }
}

impl CompiledDecoder for CompiledRmatchingDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        Ok(self
            .matching
            .lock()
            .map_err(|error| error.to_string())?
            .decode_shots_bit_packed(dets, num_shots, num_dets, num_obs))
    }
}
