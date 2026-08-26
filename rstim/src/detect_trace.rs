use std::io::Write;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::executor::{ExecOutput, NoiseApplicabilityEvent};
use crate::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};

pub(crate) const SAMPLE_TRACE_SCHEMA: &str = "rstim.sample_trace.v1";

#[derive(Serialize)]
struct ManifestRecord<'a> {
    record_type: &'static str,
    schema_version: &'static str,
    rstim_version: &'a str,
    circuit_sha256: String,
    seed: Option<u64>,
    shots: usize,
    num_measurements: usize,
    num_detectors: usize,
    num_observables: usize,
}

#[derive(Serialize)]
struct ShotRecord<'a> {
    record_type: &'static str,
    shot_index: usize,
    measurements: &'a [bool],
    detectors: &'a [bool],
    observables: Vec<bool>,
    noise_events: Vec<NoiseEventRecord<'a>>,
    measurement_events: Vec<MeasurementEventRecord<'a>>,
    detector_events: Vec<DetectorEventRecord<'a>>,
    inapplicable_noise_events: Vec<NoiseApplicabilityRecord<'a>>,
}

#[derive(Serialize)]
struct NoiseEventRecord<'a> {
    op_path: &'a [usize],
    repeat_iterations: &'a [u64],
    instr_name: &'a str,
    target_slots: &'a [usize],
    target_qubits: &'a [u32],
    occurred: bool,
    branch_label: Option<&'a str>,
}

#[derive(Serialize)]
struct MeasurementEventRecord<'a> {
    op_path: &'a [usize],
    repeat_iterations: &'a [u64],
    target_slot: usize,
    target_qubit: u32,
    instr_name: &'a str,
    measurement_index: usize,
    bit: bool,
    loss_cause: bool,
    component: &'static str,
}

#[derive(Serialize)]
struct DetectorEventRecord<'a> {
    op_path: &'a [usize],
    repeat_iterations: &'a [u64],
    detector_index: usize,
    flipped: bool,
}

#[derive(Serialize)]
struct NoiseApplicabilityRecord<'a> {
    op_path: &'a [usize],
    repeat_iterations: &'a [u64],
    target_slots: &'a [usize],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_manifest(
    writer: &mut dyn Write,
    circuit_text: &str,
    seed: Option<u64>,
    shots: usize,
    num_measurements: usize,
    num_detectors: usize,
    num_observables: usize,
) -> Result<(), String> {
    write_json_line(
        writer,
        &ManifestRecord {
            record_type: "manifest",
            schema_version: SAMPLE_TRACE_SCHEMA,
            rstim_version: crate::version(),
            circuit_sha256: sha256_hex(circuit_text.as_bytes()),
            seed,
            shots,
            num_measurements,
            num_detectors,
            num_observables,
        },
    )
}

pub(crate) fn write_shot(
    writer: &mut dyn Write,
    shot_index: usize,
    output: &ExecOutput,
    trace: &SampleTrace,
    num_observables: usize,
) -> Result<(), String> {
    let record = ShotRecord {
        record_type: "shot",
        shot_index,
        measurements: &output.measurements,
        detectors: &output.detectors,
        observables: aggregate_observables(output, num_observables)?,
        noise_events: trace
            .noise_events
            .iter()
            .map(NoiseEventRecord::from)
            .collect(),
        measurement_events: trace
            .measurement_events
            .iter()
            .map(MeasurementEventRecord::from)
            .collect(),
        detector_events: trace
            .detector_events
            .iter()
            .map(DetectorEventRecord::from)
            .collect(),
        inapplicable_noise_events: output
            .inapplicable_noise_events
            .iter()
            .map(NoiseApplicabilityRecord::from)
            .collect(),
    };
    write_json_line(writer, &record)
}

pub(crate) fn aggregate_observables(
    output: &ExecOutput,
    num_observables: usize,
) -> Result<Vec<bool>, String> {
    let mut observables = vec![false; num_observables];
    for &(index, bit) in &output.observables {
        let index = index as usize;
        let value = observables.get_mut(index).ok_or_else(|| {
            format!(
                "shot produced observable {index}, but the circuit declares {num_observables} observables"
            )
        })?;
        *value ^= bit;
    }
    Ok(observables)
}

fn write_json_line(writer: &mut dyn Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| format!("failed to serialize sample trace: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("failed to write sample trace: {error}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    use std::fmt::Write as _;
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

impl<'a> From<&'a NoiseEvent> for NoiseEventRecord<'a> {
    fn from(event: &'a NoiseEvent) -> Self {
        Self {
            op_path: &event.op_path,
            repeat_iterations: &event.repeat_iterations,
            instr_name: &event.instr_name,
            target_slots: &event.target_slots,
            target_qubits: &event.target_qubits,
            occurred: event.occurred,
            branch_label: event.branch_label.as_deref(),
        }
    }
}

impl<'a> From<&'a MeasurementEvent> for MeasurementEventRecord<'a> {
    fn from(event: &'a MeasurementEvent) -> Self {
        Self {
            op_path: &event.op_path,
            repeat_iterations: &event.repeat_iterations,
            target_slot: event.target_slot,
            target_qubit: event.target_qubit,
            instr_name: &event.instr_name,
            measurement_index: event.measurement_index,
            bit: event.bit,
            loss_cause: event.loss_cause,
            component: match event.component {
                MeasurementComponent::Value => "value",
                MeasurementComponent::LossFlag => "loss_flag",
            },
        }
    }
}

impl<'a> From<&'a DetectorEvent> for DetectorEventRecord<'a> {
    fn from(event: &'a DetectorEvent) -> Self {
        Self {
            op_path: &event.op_path,
            repeat_iterations: &event.repeat_iterations,
            detector_index: event.detector_index,
            flipped: event.flipped,
        }
    }
}

impl<'a> From<&'a NoiseApplicabilityEvent> for NoiseApplicabilityRecord<'a> {
    fn from(event: &'a NoiseApplicabilityEvent) -> Self {
        Self {
            op_path: &event.op_path,
            repeat_iterations: &event.repeat_iterations,
            target_slots: &event.target_slots,
        }
    }
}
