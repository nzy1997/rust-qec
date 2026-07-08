use rand::Rng;

use crate::compiled::CompiledCircuit;
use crate::data_path::build_reference_sample;
use crate::sampler::{BatchOutput, SampleOptions, SampleOutputMode};
use crate::sim::frame::FrameSimulator;

pub fn sample_compiled_batch(
    compiled: &CompiledCircuit,
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    let ref_sample = build_reference_sample(&compiled.source, options.reference_sample_mode)?;
    let mut frame = FrameSimulator::new(compiled.num_qubits, n_shots);
    frame
        .set_materialize_detector_observable_outputs(options.output_mode == SampleOutputMode::Full);
    frame.run_compiled_blocks(&compiled.blocks, &ref_sample, rng)?;

    let measurements = frame.measurements(&ref_sample);

    match options.output_mode {
        SampleOutputMode::Full => Ok(BatchOutput::full(
            measurements,
            frame.detections(),
            frame.observable_flips(),
            frame.detector_materializations(),
            frame.observable_materializations(),
        )),
        SampleOutputMode::MeasurementsOnly => {
            Ok(BatchOutput::measurements_only_with_materializations(
                measurements,
                n_shots,
                frame.detector_materializations(),
                frame.observable_materializations(),
            ))
        }
    }
}
