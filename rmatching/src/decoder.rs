use std::sync::Mutex;

use rsinter::decode::{CompiledDecoder, Decoder};
use rstim::dem::DetectorErrorModel;

use crate::Matching;

/// MWPM decoder implementing rsinter's `Decoder` trait.
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
        let det_bytes = (num_dets + 7) / 8;
        let obs_bytes = (num_obs + 7) / 8;
        let mut out = Vec::with_capacity(num_shots * obs_bytes);
        let mut matching = self.matching.lock().unwrap();
        let mut predictions = Vec::new();

        for shot in 0..num_shots {
            let shot_dets = &dets[shot * det_bytes..(shot + 1) * det_bytes];

            // Unpack bit-packed detectors into one-byte-per-detector syndrome
            let mut syndrome = vec![0u8; num_dets];
            for d in 0..num_dets {
                if shot_dets[d / 8] & (1 << (d % 8)) != 0 {
                    syndrome[d] = 1;
                }
            }

            matching.decode_into(&syndrome, &mut predictions);

            // Pack predictions into bit-packed format
            let mut packed = vec![0u8; obs_bytes];
            for (o, &val) in predictions.iter().enumerate() {
                if val != 0 {
                    packed[o / 8] |= 1 << (o % 8);
                }
            }
            out.extend_from_slice(&packed);
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
