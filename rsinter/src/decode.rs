use std::sync::Mutex;

use rmatching::Matching;
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

/// MWPM decoder backed by the local `rmatching` crate.
pub struct MwpmDecoder;

struct CompiledMwpmDecoder {
    matching: Mutex<Matching>,
}

impl CompiledDecoder for CompiledMwpmDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        let det_bytes = num_dets.div_ceil(8);
        let obs_bytes = num_obs.div_ceil(8);
        let mut out = Vec::with_capacity(num_shots * obs_bytes);
        let mut matching = self.matching.lock().unwrap();
        let mut syndrome = vec![0u8; num_dets];
        let mut predictions = Vec::new();

        for shot in 0..num_shots {
            let shot_dets = &dets[shot * det_bytes..(shot + 1) * det_bytes];

            syndrome.fill(0);
            for d in 0..num_dets {
                if shot_dets[d / 8] & (1 << (d % 8)) != 0 {
                    syndrome[d] = 1;
                }
            }

            matching.decode_into(&syndrome, &mut predictions);

            let out_start = out.len();
            out.resize(out_start + obs_bytes, 0);
            for (o, &val) in predictions.iter().enumerate() {
                if val != 0 {
                    out[out_start + o / 8] |= 1 << (o % 8);
                }
            }
        }

        out
    }
}

impl Decoder for MwpmDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let matching = Matching::from_dem(&dem.to_string()).unwrap();
        Box::new(CompiledMwpmDecoder {
            matching: Mutex::new(matching),
        })
    }
}
