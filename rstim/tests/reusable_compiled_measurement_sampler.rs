use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::data_path::ReferenceSampleMode;
use rstim::parser::parse_lines;
use rstim::sampler::SampleOutputMode;
use rstim::{CompiledMeasurementSampler, CompiledMeasurementSamplerDiagnostics};

fn assert_diagnostics(
    actual: CompiledMeasurementSamplerDiagnostics,
    compiled_ir_builds: usize,
    reference_builds: usize,
    sample_calls: usize,
) {
    assert_eq!(actual.compiled_ir_builds, compiled_ir_builds);
    assert_eq!(actual.reference_builds, reference_builds);
    assert_eq!(actual.sample_calls, sample_calls);
}

#[test]
fn compile_once_samples_many_batches() {
    let instrs =
        parse_lines("X_ERROR(0.2) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .unwrap();
    let mut sampler =
        CompiledMeasurementSampler::compile(&instrs, ReferenceSampleMode::AssumeAllZero).unwrap();
    assert_diagnostics(sampler.diagnostics(), 1, 1, 0);

    let mut rng = StdRng::seed_from_u64(452);
    for (sample_calls, shots) in [0, 1, 2, 3, 7, 16, 31, 64, 1024].into_iter().enumerate() {
        let output_mode = if sample_calls % 2 == 0 {
            SampleOutputMode::Full
        } else {
            SampleOutputMode::MeasurementsOnly
        };
        sampler.sample(shots, &mut rng, output_mode).unwrap();
        assert_diagnostics(sampler.diagnostics(), 1, 1, sample_calls + 1);
    }

    assert_diagnostics(sampler.diagnostics(), 1, 1, 9);
    println!("PASS reusable compiled measurement sampler");
}

#[test]
fn compiled_sampler_caches_nonzero_reference_bits() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let mut sampler =
        CompiledMeasurementSampler::compile(&instrs, ReferenceSampleMode::SimulateNoiseless)
            .unwrap();
    let mut rng = StdRng::seed_from_u64(452);

    let output = sampler
        .sample(1, &mut rng, SampleOutputMode::MeasurementsOnly)
        .unwrap();

    assert!(output.measurements.get(0, 0));
    assert_diagnostics(sampler.diagnostics(), 1, 1, 1);
}

#[test]
fn compiled_sampler_preserves_both_output_modes() {
    let instrs =
        parse_lines("X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .unwrap();
    let mut sampler =
        CompiledMeasurementSampler::compile(&instrs, ReferenceSampleMode::AssumeAllZero).unwrap();
    let mut rng = StdRng::seed_from_u64(452);

    let measurements_only = sampler
        .sample(8, &mut rng, SampleOutputMode::MeasurementsOnly)
        .unwrap();
    assert_eq!(
        measurements_only.output_mode,
        SampleOutputMode::MeasurementsOnly
    );
    assert_eq!(measurements_only.measurements.num_major(), 1);
    assert_eq!(measurements_only.detections.num_major(), 0);
    assert_eq!(measurements_only.observable_flips.num_major(), 0);
    assert_eq!(measurements_only.detector_materializations, 0);
    assert_eq!(measurements_only.observable_materializations, 0);

    let full = sampler.sample(8, &mut rng, SampleOutputMode::Full).unwrap();
    assert_eq!(full.output_mode, SampleOutputMode::Full);
    assert_eq!(full.measurements.num_major(), 1);
    assert_eq!(full.detections.num_major(), 1);
    assert_eq!(full.observable_flips.num_major(), 1);
    assert_eq!(full.detector_materializations, 1);
    assert_eq!(full.observable_materializations, 1);
    assert_diagnostics(sampler.diagnostics(), 1, 1, 2);
}

#[test]
fn unsupported_circuit_is_rejected_at_compile_time() {
    let instrs = parse_lines("S 0\nM 0\n").unwrap();

    let err = CompiledMeasurementSampler::compile(&instrs, ReferenceSampleMode::AssumeAllZero)
        .unwrap_err();

    assert_eq!(
        err,
        "unsupported sampler instructions require the interpreted path"
    );
}
