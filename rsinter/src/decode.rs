use rstim::dem::DetectorErrorModel;

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
    ) -> Vec<u8>;
}

pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder>;
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
    ) -> Vec<u8> {
        let obs_bytes = (num_obs + 7) / 8;
        vec![0u8; num_shots * obs_bytes]
    }
}

impl Decoder for VacuousDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        Box::new(VacuousCompiled)
    }
}
