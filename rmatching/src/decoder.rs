use std::sync::Mutex;

use rsinter::decode::{CompiledDecoder, Decoder};
use rstim::dem::DetectorErrorModel;

use crate::Matching;

/// MWPM decoder implementing rsinter's `Decoder` trait.
pub struct MwpmDecoder;

struct CompiledMwpmDecoderState {
    matching: Matching,
    packed_prediction_buf: Vec<u8>,
}

struct CompiledMwpmDecoder {
    state: Mutex<CompiledMwpmDecoderState>,
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
        let mut out = vec![0u8; num_shots * obs_bytes];
        let mut state = self.state.lock().unwrap();

        for shot in 0..num_shots {
            let det_start = shot * det_bytes;
            let det_end = det_start + det_bytes;
            let CompiledMwpmDecoderState {
                matching,
                packed_prediction_buf,
            } = &mut *state;
            matching.decode_bit_packed_into(
                &dets[det_start..det_end],
                num_dets,
                num_obs,
                packed_prediction_buf,
            );
            out[shot * obs_bytes..(shot + 1) * obs_bytes]
                .copy_from_slice(packed_prediction_buf);
        }

        out
    }
}

impl Decoder for MwpmDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let matching = Matching::from_dem(&dem.to_string()).unwrap();
        Box::new(CompiledMwpmDecoder {
            state: Mutex::new(CompiledMwpmDecoderState {
                matching,
                packed_prediction_buf: Vec::new(),
            }),
        })
    }
}
