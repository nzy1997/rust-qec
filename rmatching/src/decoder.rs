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
    graph_num_obs: usize,
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
        let graph_obs_bytes = self.graph_num_obs.div_ceil(8);
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
                self.graph_num_obs,
                packed_prediction_buf,
            );
            let shot_out = &mut out[shot * obs_bytes..(shot + 1) * obs_bytes];
            let copied_bytes = graph_obs_bytes.min(obs_bytes);
            shot_out[..copied_bytes].copy_from_slice(&packed_prediction_buf[..copied_bytes]);

            if obs_bytes > 0 && num_obs % 8 != 0 {
                let tail_mask = (1u8 << (num_obs % 8)) - 1;
                shot_out[obs_bytes - 1] &= tail_mask;
            }
        }

        out
    }
}

impl Decoder for MwpmDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let mut matching = Matching::from_dem(&dem.to_string()).unwrap();
        let graph_num_obs = matching.graph_num_observables();
        Box::new(CompiledMwpmDecoder {
            graph_num_obs,
            state: Mutex::new(CompiledMwpmDecoderState {
                matching,
                packed_prediction_buf: Vec::new(),
            }),
        })
    }
}
