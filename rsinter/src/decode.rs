use rstim::dem::DetectorErrorModel;

pub use crate::ilpqec_adapter::IlpDemDecoder;
pub use crate::rbposd_adapter::RbposdDemDecoder;
pub use crate::rmatching_adapter::RmatchingDemDecoder;

pub trait CompiledDecoder: Send {
    /// Decode bit-packed detection events into bit-packed observable predictions.
    /// `dets`: `num_shots * ceil(num_dets/8)` bytes, b8 format.
    /// Returns: `num_shots * ceil(num_obs/8)` bytes, b8 format.
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String>;
}

pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel)
        -> Result<Box<dyn CompiledDecoder>, String>;
}

/// Always predicts no observable flips. Useful for testing the pipeline.
pub struct VacuousDecoder;

struct VacuousCompiled;

impl CompiledDecoder for VacuousCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        let obs_bytes = (num_obs + 7) / 8;
        Ok(vec![0u8; num_shots * obs_bytes])
    }
}

impl Decoder for VacuousDecoder {
    fn compile_for_dem(
        &self,
        _dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        Ok(Box::new(VacuousCompiled))
    }
}
