use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};
use rstim::sim::bit_table::BitTable;

fn circuit_with_measurements_detectors_and_observables() -> Vec<rstim::ir::StimInstr> {
    parse_lines(
        "X 0\n\
         M 0\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap()
}

fn table_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|major| {
            (0..table.num_minor())
                .map(|minor| table.get(major, minor))
                .collect()
        })
        .collect()
}

fn sample_with_mode(mode: SampleOutputMode) -> rstim::sampler::BatchOutput {
    let instrs = circuit_with_measurements_detectors_and_observables();
    let mut rng = StdRng::seed_from_u64(123);
    sample_batch_with_options(
        &instrs,
        8,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            output_mode: mode,
            ..SampleOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn measurement_only_mode_preserves_measurement_bits() {
    let full = sample_with_mode(SampleOutputMode::Full);
    let measurements_only = sample_with_mode(SampleOutputMode::MeasurementsOnly);

    assert_eq!(
        table_rows(&measurements_only.measurements),
        table_rows(&full.measurements)
    );
    assert_eq!(measurements_only.measurements.num_major(), 1);
    assert_eq!(measurements_only.measurements.num_minor(), 8);
}

#[test]
fn measurement_only_mode_skips_detector_and_observable_materialization() {
    let out = sample_with_mode(SampleOutputMode::MeasurementsOnly);

    assert_eq!(out.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(out.detections.num_major(), 0);
    assert_eq!(out.detections.num_minor(), 8);
    assert_eq!(out.observable_flips.num_major(), 0);
    assert_eq!(out.observable_flips.num_minor(), 8);
    assert_eq!(out.detector_materializations, 0);
    assert_eq!(out.observable_materializations, 0);
}

#[test]
fn full_mode_still_materializes_detector_and_observable_bits() {
    let out = sample_with_mode(SampleOutputMode::Full);

    assert_eq!(out.output_mode, SampleOutputMode::Full);
    assert_eq!(out.detections.num_major(), 1);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.detector_materializations, 1);
    assert_eq!(out.observable_materializations, 1);
    for shot in 0..8 {
        assert!(out.measurements.get(0, shot), "shot {shot}");
        assert!(out.detections.get(0, shot), "shot {shot}");
        assert!(out.observable_flips.get(0, shot), "shot {shot}");
    }
}

#[test]
fn default_sample_options_remain_full_output() {
    assert_eq!(SampleOptions::default().output_mode, SampleOutputMode::Full);

    let instrs = circuit_with_measurements_detectors_and_observables();
    let mut rng = StdRng::seed_from_u64(123);
    let out = sample_batch_with_options(&instrs, 8, &mut rng, SampleOptions::default()).unwrap();

    assert_eq!(out.output_mode, SampleOutputMode::Full);
    assert_eq!(out.detections.num_major(), 1);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.detector_materializations, 1);
    assert_eq!(out.observable_materializations, 1);
    for shot in 0..8 {
        assert!(out.measurements.get(0, shot), "shot {shot}");
        assert!(out.detections.get(0, shot), "shot {shot}");
        assert!(out.observable_flips.get(0, shot), "shot {shot}");
    }
}
