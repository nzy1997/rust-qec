use rand::Rng;

use crate::compiled::CompiledCircuit;
use crate::data_path::build_reference_sample;
use crate::sampler::{BatchOutput, SampleOptions};
use crate::sim::frame::FrameSimulator;

pub fn sample_compiled_batch(
    compiled: &CompiledCircuit,
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    let ref_sample = build_reference_sample(&compiled.source, options.reference_sample_mode)?;
    let mut frame = FrameSimulator::new(compiled.num_qubits, n_shots);
    frame.run_compiled_blocks(&compiled.blocks, &ref_sample, rng)?;

    let measurements = frame.measurements(&ref_sample);

    Ok(BatchOutput {
        measurements,
        detections: frame.detections(),
        observable_flips: frame.observable_flips(),
    })
}
