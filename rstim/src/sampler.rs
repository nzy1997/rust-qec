use rand::Rng;

use crate::data_path::build_reference_sample;
use crate::executor::max_qubit;
use crate::ir::StimInstr;
use crate::sim::bit_table::BitTable;
use crate::sim::frame::FrameSimulator;

pub struct BatchOutput {
    pub measurements: BitTable,
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SampleOptions {
    pub reference_sample_mode: crate::data_path::ReferenceSampleMode,
}

pub fn sample_batch_with_options(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let num_qubits = max_qubit(instrs)?;
    let mut frame = FrameSimulator::new(num_qubits, n_shots);
    frame.run(instrs, &ref_sample, rng)?;

    let measurements = frame.measurements(&ref_sample);
    let detections = frame.detections();
    let observable_flips = frame.observable_flips();

    Ok(BatchOutput {
        measurements,
        detections,
        observable_flips,
    })
}

pub fn sample_batch(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
) -> Result<BatchOutput, String> {
    sample_batch_with_options(instrs, n_shots, rng, SampleOptions::default())
}
