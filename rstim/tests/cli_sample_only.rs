use rstim::cli::{run_detect, run_sample, sample_cli_options};
use rstim::data_path::ReferenceSampleMode;
use rstim::perf::{
    benchmark_case_by_label, run_case_measurements, PerfRunOptions, PerfSampleOutputMode,
    PerfVariant,
};
use rstim::sampler::SampleOutputMode;

#[test]
fn sample_cli_uses_measurement_only_mode_and_preserves_output() {
    let default_options = sample_cli_options(false);
    assert_eq!(default_options.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(
        default_options.reference_sample_mode,
        ReferenceSampleMode::SimulateNoiseless
    );

    let skipped_options = sample_cli_options(true);
    assert_eq!(skipped_options.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(
        skipped_options.reference_sample_mode,
        ReferenceSampleMode::AssumeAllZero
    );

    let mut out = Vec::new();
    run_sample(
        "R 0\nX 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        2,
        "01",
        Some(7),
        false,
        &mut out,
    )
    .expect("sample command");

    assert_eq!(String::from_utf8(out).unwrap(), "1\n1\n");
}

#[test]
fn perf_sample_workload_records_measurement_only_mode() {
    let case = benchmark_case_by_label("loss-protection-sample").unwrap();
    let records = run_case_measurements(
        case,
        "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n",
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("sample perf records");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].sample_output_mode,
        Some(PerfSampleOutputMode::MeasurementsOnly)
    );
}

#[test]
fn sample_only_mode_does_not_change_detect_output() {
    let mut out = Vec::new();
    run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        1,
        "dets",
        Some(7),
        false,
        &mut out,
    )
    .expect("detect command");

    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("D0"), "detect output missing detector: {text}");
    assert!(text.contains("L0"), "detect output missing observable: {text}");
}

#[test]
fn detect_perf_workload_records_full_output_mode() {
    let case = benchmark_case_by_label("surface-detect-d13-r13").unwrap();
    let records = run_case_measurements(
        case,
        "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("detect perf records");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].sample_output_mode,
        Some(PerfSampleOutputMode::Full)
    );
}
