use rand::Rng;

use crate::compiled::{
    choose_sampler_path, compile_circuit, sample_compiled_batch_with_reference, CompiledCircuit,
    SamplerPathDecision, SamplingFallbackReason,
};
use crate::data_path::{
    build_reference_sample, build_reference_sample_with_decision,
    build_reference_sample_with_sweep_bits_and_decision, ReferenceSampleDecision,
    ReferenceSampleMode,
};
use crate::executor::max_qubit;
use crate::executor::Executor;
use crate::ir::StimInstr;
use crate::loss_sampler::LossSamplerPlan;
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

    pub(crate) fn measurements_only(
        measurements: BitTable,
        n_shots: usize,
    ) -> Result<Self, String> {
        Self::measurements_only_with_materializations(measurements, n_shots, 0, 0)
    }

    pub(crate) fn measurements_only_with_materializations(
        measurements: BitTable,
        n_shots: usize,
        detector_materializations: usize,
        observable_materializations: usize,
    ) -> Result<Self, String> {
        Ok(Self {
            measurements,
            detections: alloc_bit_table(0, n_shots)?,
            observable_flips: alloc_bit_table(0, n_shots)?,
            output_mode: SampleOutputMode::MeasurementsOnly,
            detector_materializations,
            observable_materializations,
        })
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleBatchDecision {
    PackedInverse,
    Interpreted,
    InterpretedLegacy(SamplingFallbackReason),
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
        Self::compile_with_decision(instrs, reference_mode).map_err(|reason| reason.to_string())
    }

    #[doc(hidden)]
    pub fn compile_with_decision(
        instrs: &[StimInstr],
        reference_mode: ReferenceSampleMode,
    ) -> Result<Self, SamplingFallbackReason> {
        let compiled =
            compile_circuit(instrs).map_err(SamplingFallbackReason::UnsupportedOperation)?;
        match choose_sampler_path(&compiled) {
            SamplerPathDecision::FastPath => {}
            SamplerPathDecision::Fallback(reason) => return Err(reason),
        }

        let reference_sample = match reference_mode {
            ReferenceSampleMode::SimulateNoiseless => {
                let reference = build_reference_sample_with_decision(instrs)
                    .map_err(SamplingFallbackReason::UnsupportedOperation)?;
                match reference.decision {
                    ReferenceSampleDecision::PackedInverse => reference.bits,
                    ReferenceSampleDecision::LegacyFallback(reason) => return Err(reason),
                }
            }
            ReferenceSampleMode::AssumeAllZero => vec![false; compiled.num_measurements],
        };
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
    Ok(sample_batch_with_options_and_decision(instrs, n_shots, rng, options)?.0)
}

#[doc(hidden)]
pub fn sample_batch_with_options_and_decision(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<(BatchOutput, SampleBatchDecision), String> {
    sample_batch_with_options_sweep_bits_and_decision(instrs, n_shots, rng, options, None)
}

#[doc(hidden)]
pub fn sample_batch_with_options_sweep_bits_and_decision(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
    sweep_bits: Option<&[bool]>,
) -> Result<(BatchOutput, SampleBatchDecision), String> {
    match options.backend {
        SamplingBackend::Interpreted => Ok((
            sample_batch_interpreted_with_sweep_bits(instrs, n_shots, rng, options, sweep_bits)?,
            SampleBatchDecision::Interpreted,
        )),
        SamplingBackend::Compiled => {
            let mut sampler = CompiledMeasurementSampler::compile_with_decision(
                instrs,
                options.reference_sample_mode,
            )
            .map_err(|reason| reason.to_string())?;
            let out = sampler.sample(n_shots, rng, options.output_mode)?;
            Ok((out, SampleBatchDecision::PackedInverse))
        }
        SamplingBackend::Auto => {
            match CompiledMeasurementSampler::compile_with_decision(
                instrs,
                options.reference_sample_mode,
            ) {
                Ok(mut sampler) => {
                    let out = sampler.sample(n_shots, rng, options.output_mode)?;
                    Ok((out, SampleBatchDecision::PackedInverse))
                }
                Err(reason) => {
                    let out = sample_batch_interpreted_with_sweep_bits(
                        instrs, n_shots, rng, options, sweep_bits,
                    )?;
                    Ok((out, SampleBatchDecision::InterpretedLegacy(reason)))
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

fn sample_batch_interpreted_with_sweep_bits(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
    sweep_bits: Option<&[bool]>,
) -> Result<BatchOutput, String> {
    if sweep_bits.is_some() || uses_executor_sampling_fallback(instrs) {
        return sample_batch_with_executor(instrs, n_shots, rng, options, sweep_bits);
    }

    let ref_sample = build_reference_sample(instrs, options.reference_sample_mode)?;
    let num_qubits = max_qubit(instrs)?;
    let mut frame = FrameSimulator::try_new(num_qubits, n_shots)?;
    frame.randomize_initial_z_frames(rng);
    frame
        .set_materialize_detector_observable_outputs(options.output_mode == SampleOutputMode::Full);
    frame.run(instrs, &ref_sample, rng)?;

    let measurements = frame.try_measurements(&ref_sample)?;
    match options.output_mode {
        SampleOutputMode::Full => Ok(BatchOutput::full(
            measurements,
            frame.try_detections()?,
            frame.try_observable_flips()?,
            frame.detector_materializations(),
            frame.observable_materializations(),
        )),
        SampleOutputMode::MeasurementsOnly => BatchOutput::measurements_only_with_materializations(
            measurements,
            n_shots,
            frame.detector_materializations(),
            frame.observable_materializations(),
        ),
    }
}

fn sample_batch_with_executor(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
    sweep_bits: Option<&[bool]>,
) -> Result<BatchOutput, String> {
    let ref_sample = match options.reference_sample_mode {
        ReferenceSampleMode::SimulateNoiseless => {
            build_reference_sample_with_sweep_bits_and_decision(instrs, sweep_bits)?.bits
        }
        ReferenceSampleMode::AssumeAllZero => vec![false; crate::stats::num_measurements(instrs)],
    };
    let n_meas = ref_sample.len();
    let loss_plan = if sweep_bits.is_none() {
        LossSamplerPlan::try_compile(instrs)
    } else {
        None
    };
    let mut executor = if loss_plan.is_none() {
        Some(Executor::from_instrs(instrs.to_vec())?)
    } else {
        None
    };

    let measurements = if let Some(plan) = &loss_plan {
        plan.run_batch(n_shots, &ref_sample, rng)?
    } else {
        let mut measurements = alloc_bit_table(n_meas, n_shots)?;
        for shot in 0..n_shots {
            let shot_measurements = executor
                .as_mut()
                .expect("legacy executor is available when the loss plan is absent")
                .run_with_sweep_bits(rng, sweep_bits)?
                .measurements;
            if shot_measurements.len() != n_meas {
                return Err(format!(
                    "executor produced {} measurements but reference sample expects {}",
                    shot_measurements.len(),
                    n_meas
                ));
            }
            for (m, &bit) in shot_measurements.iter().enumerate() {
                measurements.set(m, shot, bit);
            }
        }
        measurements
    };

    if options.output_mode == SampleOutputMode::MeasurementsOnly {
        return BatchOutput::measurements_only(measurements, n_shots);
    }

    let sweep_table = sweep_bits
        .map(|bits| repeated_sweep_table(instrs, bits, n_shots))
        .transpose()?;
    let m2d = measurements_to_detections_with_options(
        instrs,
        &measurements,
        sweep_table.as_ref(),
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

fn repeated_sweep_table(
    instrs: &[StimInstr],
    sweep_bits: &[bool],
    n_shots: usize,
) -> Result<BitTable, String> {
    let n_sweep = crate::stats::num_sweep_bits(instrs).max(sweep_bits.len());
    let mut table = alloc_bit_table(n_sweep, n_shots)?;
    for (sweep_index, bit) in sweep_bits.iter().copied().enumerate() {
        if bit {
            for shot in 0..n_shots {
                table.set(sweep_index, shot, true);
            }
        }
    }
    Ok(table)
}

fn alloc_bit_table(num_major: usize, num_minor: usize) -> Result<BitTable, String> {
    BitTable::try_new(num_major, num_minor)
        .map_err(|err| format!("BitTable allocation failed: {err:?}"))
}

fn uses_executor_sampling_fallback(instrs: &[StimInstr]) -> bool {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
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
                if is_feedback_operation(name, targets)
                    || is_sweep_dependent_operation(name, targets)
                {
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

fn is_feedback_operation(name: &str, targets: &[crate::ir::StimTarget]) -> bool {
    matches!(name, "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ")
        && targets.chunks_exact(2).any(|pair| {
            matches!(
                pair,
                [
                    crate::ir::StimTarget::Rec(_),
                    crate::ir::StimTarget::Qubit(_)
                ]
            )
        })
}

fn is_sweep_dependent_operation(name: &str, targets: &[crate::ir::StimTarget]) -> bool {
    targets
        .iter()
        .any(|target| matches!(target, crate::ir::StimTarget::Sweep(_)))
        && !matches!(
            name,
            "I" | "I_ERROR"
                | "II_ERROR"
                | "X_ERROR"
                | "Y_ERROR"
                | "Z_ERROR"
                | "DEPOLARIZE1"
                | "DEPOLARIZE2"
                | "TICK"
                | "QUBIT_COORDS"
                | "SHIFT_COORDS"
                | "DETECTOR"
                | "OBSERVABLE_INCLUDE"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use crate::data_path::ReferenceSampleMode;
    use crate::ir::StimTarget;

    #[test]
    fn sample_batch_with_executor_uses_shared_measurement_count_for_sweep_targets() {
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
            None,
        );

        let output = result.expect("shared measurement count should match executor output");
        assert_eq!(output.measurements.num_major(), 0);
        assert_eq!(output.measurements.num_minor(), 1);
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
            BatchOutput::measurements_only_with_materializations(BitTable::new(1, 4), 4, 2, 3)
                .unwrap();

        assert_eq!(out.output_mode, SampleOutputMode::MeasurementsOnly);
        assert_eq!(out.detections.num_major(), 0);
        assert_eq!(out.detections.num_minor(), 4);
        assert_eq!(out.observable_flips.num_major(), 0);
        assert_eq!(out.observable_flips.num_minor(), 4);
        assert_eq!(out.detector_materializations, 2);
        assert_eq!(out.observable_materializations, 3);
    }
}
