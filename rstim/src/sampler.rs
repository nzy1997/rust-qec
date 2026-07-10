use rand::Rng;

use crate::compiled::{
    choose_sampler_path, compile_circuit, sample_compiled_batch,
    sample_compiled_batch_with_reference, CompiledCircuit, CompiledPathDecision,
};
use crate::data_path::{build_reference_sample, ReferenceSampleMode};
use crate::executor::max_qubit;
use crate::executor::Executor;
use crate::ir::StimInstr;
use crate::m2d::{measurements_to_detections_with_options, M2dOptions};
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
        Self::measurements_only_with_materializations(measurements, n_shots, 0, 0)
    }

    pub(crate) fn measurements_only_with_materializations(
        measurements: BitTable,
        n_shots: usize,
        detector_materializations: usize,
        observable_materializations: usize,
    ) -> Self {
        Self {
            measurements,
            detections: BitTable::new(0, n_shots),
            observable_flips: BitTable::new(0, n_shots),
            output_mode: SampleOutputMode::MeasurementsOnly,
            detector_materializations,
            observable_materializations,
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

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompiledMeasurementSamplerDiagnostics {
    pub compiled_ir_builds: usize,
    pub reference_builds: usize,
    pub sample_calls: usize,
}

#[derive(Debug)]
pub struct CompiledMeasurementSampler {
    compiled: CompiledCircuit,
    reference_sample: Vec<bool>,
    diagnostics: CompiledMeasurementSamplerDiagnostics,
}

impl CompiledMeasurementSampler {
    pub fn compile(
        instrs: &[StimInstr],
        reference_mode: ReferenceSampleMode,
    ) -> Result<Self, String> {
        let compiled = compile_circuit(instrs)?;
        match choose_sampler_path(&compiled) {
            CompiledPathDecision::FastPath => {}
            CompiledPathDecision::Fallback(reason) => return Err(reason.to_string()),
        }

        let reference_sample = build_reference_sample(instrs, reference_mode)?;
        Ok(Self {
            compiled,
            reference_sample,
            diagnostics: CompiledMeasurementSamplerDiagnostics {
                compiled_ir_builds: 1,
                reference_builds: 1,
                sample_calls: 0,
            },
        })
    }

    pub fn sample(
        &mut self,
        shots: usize,
        rng: &mut impl Rng,
        output_mode: SampleOutputMode,
    ) -> Result<BatchOutput, String> {
        self.diagnostics.sample_calls += 1;
        sample_compiled_batch_with_reference(
            &self.compiled,
            &self.reference_sample,
            shots,
            rng,
            SampleOptions {
                output_mode,
                ..SampleOptions::default()
            },
        )
    }

    #[doc(hidden)]
    pub fn diagnostics(&self) -> CompiledMeasurementSamplerDiagnostics {
        self.diagnostics
    }
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

fn count_output_materialization_ops(instrs: &[StimInstr]) -> (usize, usize) {
    let mut detector_count = 0usize;
    let mut observable_count = 0usize;

    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } => match name.as_str() {
                "DETECTOR" => {
                    detector_count = detector_count.saturating_add(1);
                }
                "OBSERVABLE_INCLUDE" => {
                    observable_count = observable_count.saturating_add(1);
                }
                _ => {}
            },
            StimInstr::Repeat { count, body } => {
                let (body_detectors, body_observables) = count_output_materialization_ops(body);
                let repeat_count = usize::try_from(*count).unwrap_or(usize::MAX);
                detector_count =
                    detector_count.saturating_add(body_detectors.saturating_mul(repeat_count));
                observable_count =
                    observable_count.saturating_add(body_observables.saturating_mul(repeat_count));
            }
        }
    }

    (detector_count, observable_count)
}

fn sample_batch_interpreted(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<BatchOutput, String> {
    if uses_executor_sampling_fallback(instrs) {
        return sample_batch_with_executor(instrs, n_shots, rng, options);
    }

    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let num_qubits = max_qubit(instrs)?;
    let mut frame = FrameSimulator::new(num_qubits, n_shots);
    frame.randomize_initial_z_frames(rng);
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
            Ok(BatchOutput::measurements_only_with_materializations(
                measurements,
                n_shots,
                frame.detector_materializations(),
                frame.observable_materializations(),
            ))
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
    let (detector_materializations, observable_materializations) =
        count_output_materialization_ops(instrs);

    Ok(BatchOutput::full(
        measurements,
        m2d.detections,
        m2d.observable_flips,
        detector_materializations,
        observable_materializations,
    ))
}

fn uses_executor_sampling_fallback(instrs: &[StimInstr]) -> bool {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } => {
                if matches!(
                    name.as_str(),
                    "LOSS"
                        | "ML"
                        | "MXL"
                        | "MYL"
                        | "MZL"
                        | "MRL"
                        | "MRXL"
                        | "MRYL"
                        | "MRZL"
                        | "MPP"
                ) {
                    return true;
                }
            }
            StimInstr::Repeat { body, .. } => {
                if uses_executor_sampling_fallback(body) {
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
    use rand::rngs::StdRng;
    use rand::SeedableRng;

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

    #[test]
    fn count_output_materialization_ops_expands_repeats_saturating() {
        let instrs = vec![
            StimInstr::new("DETECTOR", vec![], vec![StimTarget::Rec(-1)]),
            StimInstr::Repeat {
                count: 3,
                body: vec![
                    StimInstr::new("DETECTOR", vec![], vec![StimTarget::Rec(-1)]),
                    StimInstr::new("OBSERVABLE_INCLUDE", vec![0.0], vec![StimTarget::Rec(-1)]),
                ],
            },
        ];

        assert_eq!(count_output_materialization_ops(&instrs), (4, 3));
    }

    #[test]
    fn measurements_only_output_can_report_actual_materialization_counters() {
        let out =
            BatchOutput::measurements_only_with_materializations(BitTable::new(1, 4), 4, 2, 3);

        assert_eq!(out.output_mode, SampleOutputMode::MeasurementsOnly);
        assert_eq!(out.detections.num_major(), 0);
        assert_eq!(out.detections.num_minor(), 4);
        assert_eq!(out.observable_flips.num_major(), 0);
        assert_eq!(out.observable_flips.num_minor(), 4);
        assert_eq!(out.detector_materializations, 2);
        assert_eq!(out.observable_materializations, 3);
    }
}
