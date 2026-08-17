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
        if dem.num_observables() > 64 {
            return Err(format!(
                "rmatching supports at most 64 observables, got {}",
                dem.num_observables()
            ));
        }
        validate_no_observable_only_errors(dem.instructions())?;
        let mut matching =
            Matching::from_dem(&dem.to_string()).map_err(|error| error.to_string())?;
        matching.prepare();
        Ok(Box::new(CompiledRmatchingDemDecoder {
            matching: Mutex::new(matching),
        }))
    }
}

fn validate_no_observable_only_errors(
    instructions: &[rstim::dem::DemInstruction],
) -> Result<(), String> {
    use std::collections::BTreeSet;

    use rstim::dem::{DemInstruction, DemTarget};

    for instruction in instructions {
        match instruction {
            DemInstruction::Error { targets, .. } => {
                let mut detectors = BTreeSet::new();
                let mut observables = BTreeSet::new();
                for target in targets.iter().chain(std::iter::once(&DemTarget::Separator)) {
                    match target {
                        DemTarget::Detector(index) => toggle(&mut detectors, *index),
                        DemTarget::Observable(index) => toggle(&mut observables, *index),
                        DemTarget::Separator => {
                            if detectors.is_empty() && !observables.is_empty() {
                                return Err(
                                    "rmatching does not support observable-only DEM error components"
                                        .into(),
                                );
                            }
                            detectors.clear();
                            observables.clear();
                        }
                    }
                }
            }
            DemInstruction::Repeat { body, .. } => {
                validate_no_observable_only_errors(body.instructions())?;
            }
            DemInstruction::ShiftDetectors { .. }
            | DemInstruction::Detector { .. }
            | DemInstruction::LogicalObservable { .. } => {}
        }
    }
    Ok(())
}

fn toggle(values: &mut std::collections::BTreeSet<usize>, value: usize) {
    if !values.insert(value) {
        values.remove(&value);
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
