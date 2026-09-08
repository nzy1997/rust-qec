//! Reusable execution routines shared by command-line frontends.
//!
//! This module deliberately contains no argument-parser or viewer dependencies,
//! so applications can use these operations with `rstim`'s default features.

use std::io::Write;

use rand::{SeedableRng, rngs::StdRng};

use crate::codegen::NoiseParams;
use crate::error_analyzer::ErrorAnalyzer;
use crate::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_dets, write_shots_hits,
    write_shots_ptb64, write_shots_r8,
};
use crate::parser::parse_lines;
use crate::sampler::{SampleOptions, SampleOutputMode, sample_batch, sample_batch_with_options};
use crate::sim::bit_table::BitTable;

pub fn make_rng(seed: Option<u64>) -> StdRng {
    seed.map_or_else(StdRng::from_entropy, StdRng::seed_from_u64)
}

pub fn write_format(
    fmt: OutputFormat,
    table: &BitTable,
    out: &mut dyn Write,
) -> Result<(), String> {
    match fmt {
        OutputFormat::Format01 => write_shots_01(table, out),
        OutputFormat::B8 => write_shots_b8(table, out),
        OutputFormat::R8 => write_shots_r8(table, out),
        OutputFormat::Hits => write_shots_hits(table, out),
        OutputFormat::Dets => return Err("use write_shots_dets for dets format".to_string()),
        OutputFormat::Ptb64 => write_shots_ptb64(table, out),
    }
    .map_err(|error| format!("write error: {error}"))
}

pub fn try_merge_detections_observables(
    detections: &BitTable,
    observables: &BitTable,
) -> Result<BitTable, String> {
    let shots = detections.num_minor();
    if observables.num_minor() != shots {
        return Err(format!(
            "observable shot count {} does not match detection shot count {shots}",
            observables.num_minor()
        ));
    }
    let rows = detections
        .num_major()
        .checked_add(observables.num_major())
        .ok_or_else(|| "detector and observable row count overflows".to_string())?;
    let mut merged = BitTable::try_new(rows, shots)
        .map_err(|error| format!("BitTable allocation failed: {error:?}"))?;
    for row in 0..detections.num_major() {
        for shot in 0..shots {
            merged.set(row, shot, detections.get(row, shot));
        }
    }
    for row in 0..observables.num_major() {
        for shot in 0..shots {
            merged.set(
                detections.num_major() + row,
                shot,
                observables.get(row, shot),
            );
        }
    }
    Ok(merged)
}

fn write_detection_outputs(
    detections: &BitTable,
    observables: &BitTable,
    format: OutputFormat,
    append_observables: bool,
    out: &mut dyn Write,
    obs_out: Option<(&mut dyn Write, OutputFormat)>,
) -> Result<(), String> {
    if format == OutputFormat::Dets {
        write_shots_dets(detections, observables, out)
            .map_err(|error| format!("write error: {error}"))?;
    } else if append_observables {
        write_format(
            format,
            &try_merge_detections_observables(detections, observables)?,
            out,
        )?;
    } else {
        write_format(format, detections, out)?;
    }
    if let Some((writer, obs_format)) = obs_out {
        write_format(obs_format, observables, writer)?;
    }
    Ok(())
}

pub fn run_gen_with_params(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    params: NoiseParams,
    out: &mut dyn Write,
) -> Result<(), String> {
    let circuit = generate_common_circuit_text_with_params(code, task, distance, rounds, params)?;
    out.write_all(circuit.as_bytes())
        .map_err(|error| format!("write error: {error}"))
}

pub(crate) fn generate_common_circuit_text_with_params(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    params: NoiseParams,
) -> Result<String, String> {
    let instructions = match (code, task) {
        ("repetition_code", "memory") => {
            crate::codegen::repetition_code_memory_with_params(distance, rounds, params)
        }
        ("surface_code", "rotated_memory_x") => {
            crate::codegen::surface_code::rotated_memory_x_with_params(distance, rounds, params)
        }
        ("surface_code", "rotated_memory_z") => {
            crate::codegen::surface_code::rotated_memory_z_with_params(distance, rounds, params)
        }
        ("surface_code", "unrotated_memory_x") => {
            crate::codegen::surface_code::unrotated_memory_x_with_params(distance, rounds, params)
        }
        ("surface_code", "unrotated_memory_z") => {
            crate::codegen::surface_code::unrotated_memory_z_with_params(distance, rounds, params)
        }
        ("color_code", "memory_xyz") => {
            crate::codegen::color_code::memory_xyz_with_params(distance, rounds, params)
        }
        _ => return Err(format!("unknown code/task: {code}/{task}")),
    };
    Ok(crate::ir::circuit_to_string(&instructions))
}

pub fn run_gen(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
    out: &mut dyn Write,
) -> Result<(), String> {
    run_gen_with_params(
        code,
        task,
        distance,
        rounds,
        NoiseParams::uniform(noise),
        out,
    )
}

pub(crate) fn generate_common_circuit_text(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
) -> Result<String, String> {
    generate_common_circuit_text_with_params(
        code,
        task,
        distance,
        rounds,
        NoiseParams::uniform(noise),
    )
}

pub fn run_sample(
    circuit: &str,
    shots: usize,
    format: &str,
    seed: Option<u64>,
    skip_reference_sample: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let format = OutputFormat::from_str(format)?;
    let instructions = parse_lines(circuit)?;
    let mut rng = make_rng(seed);
    let options = sample_cli_options(skip_reference_sample);
    let result = sample_batch_with_options(&instructions, shots, &mut rng, options)?;
    if format == OutputFormat::Dets {
        return Err("dets format not applicable to sample command; use detect".to_string());
    }
    write_format(format, &result.measurements, out)
}

pub fn sample_cli_options(skip_reference_sample: bool) -> SampleOptions {
    SampleOptions {
        reference_sample_mode: if skip_reference_sample {
            crate::data_path::ReferenceSampleMode::AssumeAllZero
        } else {
            crate::data_path::ReferenceSampleMode::SimulateNoiseless
        },
        output_mode: SampleOutputMode::MeasurementsOnly,
        ..SampleOptions::default()
    }
}

pub fn run_detect(
    circuit: &str,
    shots: usize,
    format: &str,
    seed: Option<u64>,
    append_observables: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let format = OutputFormat::from_str(format)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&parse_lines(circuit)?, shots, &mut rng)?;
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        format,
        append_observables,
        out,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_detect_with_obs(
    circuit: &str,
    shots: usize,
    format: &str,
    seed: Option<u64>,
    append_observables: bool,
    out: &mut dyn Write,
    obs_out: &mut dyn Write,
    obs_format: &str,
) -> Result<(), String> {
    let format = OutputFormat::from_str(format)?;
    let obs_format = OutputFormat::from_str(obs_format)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&parse_lines(circuit)?, shots, &mut rng)?;
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        format,
        append_observables,
        out,
        Some((obs_out, obs_format)),
    )
}

pub fn run_analyze_errors_with_flags(
    circuit: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    decompose_errors: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let options = crate::error_analyzer::AnalyzeOptions {
        backend: crate::error_analyzer::AnalyzeBackend::Auto,
        approximate_disjoint_errors,
        allow_gauge_detectors,
    };
    let instructions = parse_lines(circuit)?;
    let dem = if decompose_errors {
        ErrorAnalyzer::circuit_to_dem_with_options_decomposed(&instructions, options)?
    } else {
        ErrorAnalyzer::circuit_to_dem_with_options(&instructions, options)?
    };
    out.write_all(dem.to_string().as_bytes())
        .map_err(|error| format!("write error: {error}"))
}

#[allow(clippy::too_many_arguments)]
pub fn run_export_decoder_dataset_with_logical_flip(
    circuit: &str,
    shots: u64,
    mode: &str,
    logical_flip: Option<crate::decoder_dataset::LogicalFlip>,
    public_out: &str,
    private_out: &str,
    seed: Option<u64>,
    error_trace: bool,
) -> Result<(), String> {
    run_export_decoder_dataset_with_logical_flip_in_batches(
        circuit,
        shots,
        crate::decoder_dataset::DEFAULT_DECODER_DATASET_BATCH_SHOTS,
        mode,
        logical_flip,
        public_out,
        private_out,
        seed,
        error_trace,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_export_decoder_dataset_with_logical_flip_in_batches(
    circuit: &str,
    shots: u64,
    batch_shots: usize,
    mode: &str,
    logical_flip: Option<crate::decoder_dataset::LogicalFlip>,
    public_out: &str,
    private_out: &str,
    seed: Option<u64>,
    error_trace: bool,
) -> Result<(), String> {
    let circuit_text = std::fs::read_to_string(circuit)
        .map_err(|error| format!("failed to read circuit {circuit}: {error}"))?;
    let mode = crate::decoder_dataset::DecoderDatasetMode::parse(mode)?;
    let shots =
        usize::try_from(shots).map_err(|_| "--shots is too large for this platform".to_string())?;
    crate::decoder_dataset::export_decoder_dataset_with_logical_flip_in_batches(
        crate::decoder_dataset::ExportDecoderDatasetLogicalFlipConfig {
            circuit_text,
            shots,
            mode,
            logical_flip,
            public_out: public_out.into(),
            private_out: private_out.into(),
            seed,
            error_trace,
        },
        batch_shots,
    )
    .map(drop)
}

#[allow(clippy::too_many_arguments)]
pub fn run_export_decoder_dataset(
    circuit: &str,
    shots: u64,
    mode: &str,
    logical_x_qubits: Option<&str>,
    public_out: &str,
    private_out: &str,
    seed: Option<u64>,
) -> Result<(), String> {
    let logical_flip = logical_x_qubits
        .map(|value| {
            crate::decoder_dataset::LogicalFlip::parse(
                crate::decoder_dataset::LogicalPauli::X,
                value,
            )
        })
        .transpose()?;
    run_export_decoder_dataset_with_logical_flip(
        circuit,
        shots,
        mode,
        logical_flip,
        public_out,
        private_out,
        seed,
        false,
    )
}
