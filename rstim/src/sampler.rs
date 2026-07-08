use rand::Rng;

use crate::compiled::{
    CompiledPathDecision, choose_sampler_path, compile_circuit, sample_compiled_batch,
};
use crate::data_path::build_reference_sample;
use crate::executor::Executor;
use crate::executor::max_qubit;
use crate::ir::StimInstr;
use crate::m2d::{M2dOptions, measurements_to_detections_with_options};
use crate::sim::bit_table::BitTable;
use crate::sim::frame::FrameSimulator;

pub struct BatchOutput {
    pub measurements: BitTable,
    pub detections: BitTable,
    pub observable_flips: BitTable,
    pub output_mode: SampleOutputMode,
    pub detector_materializations: usize,
    pub observable_materializations: usize,
}

impl BatchOutput {
    pub(crate) fn full(
        measurements: BitTable,
        detections: BitTable,
        observable_flips: BitTable,
        detector_materializations: usize,
        observable_materializations: usize,
    ) -> Self {
        Self {
            measurements,
            detections,
            observable_flips,
            output_mode: SampleOutputMode::Full,
            detector_materializations,
            observable_materializations,
        }
    }

    pub(crate) fn measurements_only(measurements: BitTable, n_shots: usize) -> Self {
        Self {
            measurements,
            detections: BitTable::new(0, n_shots),
            observable_flips: BitTable::new(0, n_shots),
            output_mode: SampleOutputMode::MeasurementsOnly,
            detector_materializations: 0,
            observable_materializations: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SampleOutputMode {
    #[default]
    Full,
    MeasurementsOnly,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SamplingBackend {
    #[default]
    Auto,
    Interpreted,
    Compiled,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SampleOptions {
    pub reference_sample_mode: crate::data_path::ReferenceSampleMode,
    pub backend: SamplingBackend,
    pub output_mode: SampleOutputMode,
}

pub fn sample_batch_with_options(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    match options.backend {
        SamplingBackend::Interpreted => sample_batch_interpreted(instrs, n_shots, rng, options),
        SamplingBackend::Compiled => {
            let compiled = compile_circuit(instrs)?;
            match choose_sampler_path(&compiled) {
                CompiledPathDecision::FastPath => {
                    sample_compiled_batch(&compiled, n_shots, rng, options)
                }
                CompiledPathDecision::Fallback(reason) => Err(reason.to_string()),
            }
        }
        SamplingBackend::Auto => {
            let compiled = compile_circuit(instrs)?;
            match choose_sampler_path(&compiled) {
                CompiledPathDecision::FastPath => {
                    sample_compiled_batch(&compiled, n_shots, rng, options)
                }
                CompiledPathDecision::Fallback(_) => {
                    sample_batch_interpreted(instrs, n_shots, rng, options)
                }
            }
        }
    }
}

pub fn sample_batch(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
) -> Result<BatchOutput, String> {
    sample_batch_with_options(instrs, n_shots, rng, SampleOptions::default())
}

fn sample_batch_interpreted(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    if uses_loss_sampling_fallback(instrs) {
        return sample_batch_with_executor(instrs, n_shots, rng, options);
    }

    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let num_qubits = max_qubit(instrs)?;
    let mut frame = FrameSimulator::new(num_qubits, n_shots);
    frame
        .set_materialize_detector_observable_outputs(options.output_mode == SampleOutputMode::Full);
    frame.run(instrs, &ref_sample, rng)?;

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
            Ok(BatchOutput::measurements_only(measurements, n_shots))
        }
    }
}

fn sample_batch_with_executor(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let n_meas = ref_sample.len();
    let mut measurements = BitTable::new(n_meas, n_shots);

    for shot in 0..n_shots {
        let mut ex = Executor::from_instrs(instrs.to_vec())?;
        let out = ex.run(rng)?;
        if out.measurements.len() != n_meas {
            return Err(format!(
                "executor produced {} measurements but reference sample expects {}",
                out.measurements.len(),
                n_meas
            ));
        }
        for (m, &bit) in out.measurements.iter().enumerate() {
            measurements.set(m, shot, bit);
        }
    }

    if options.output_mode == SampleOutputMode::MeasurementsOnly {
        return Ok(BatchOutput::measurements_only(measurements, n_shots));
    }

    let m2d = measurements_to_detections_with_options(
        instrs,
        &measurements,
        None,
        M2dOptions {
            reference_sample_mode: options.reference_sample_mode,
            ran_without_feedback: false,
        },
    )?;

    Ok(BatchOutput::full(
        measurements,
        m2d.detections,
        m2d.observable_flips,
        0,
        0,
    ))
}

fn uses_loss_sampling_fallback(instrs: &[StimInstr]) -> bool {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } => {
                if matches!(
                    name.as_str(),
                    "LOSS" | "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL"
                ) {
                    return true;
                }
            }
            StimInstr::Repeat { body, .. } => {
                if uses_loss_sampling_fallback(body) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use crate::data_path::ReferenceSampleMode;
    use crate::ir::StimTarget;

    #[test]
    fn sample_batch_with_executor_errors_on_reference_sample_mismatch() {
        let instrs = vec![StimInstr::new("ML", vec![], vec![StimTarget::Sweep(0)])];
        let mut rng = StdRng::seed_from_u64(0);

        let result = sample_batch_with_executor(
            &instrs,
            1,
            &mut rng,
            SampleOptions {
                reference_sample_mode: ReferenceSampleMode::AssumeAllZero,
                ..SampleOptions::default()
            },
        );

        let err = match result {
            Ok(_) => panic!("expected reference sample mismatch"),
            Err(err) => err,
        };

        assert_eq!(
            err,
            "executor produced 0 measurements but reference sample expects 2"
        );
    }
}
