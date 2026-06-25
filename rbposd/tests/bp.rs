#[path = "../dev/parity_runner.rs"]
mod parity_runner;
#[path = "../dev/parity_schema.rs"]
#[allow(dead_code)]
mod parity_schema;

use std::path::PathBuf;

use rbposd::{
    BpOsdDecoder, ChannelModel, Correction, DecodeError, DecodeResult, DecodeStats, DecoderConfig,
    ParityCheckMatrix, Syndrome,
};

fn parity_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}

fn load_parity_case(name: &str) -> parity_schema::ParityCase {
    parity_schema::load_case(&parity_fixture_dir().join(name))
}

fn repetition_pcm() -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(4, 5, vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]])
        .unwrap()
}

#[test]
fn minimum_sum_decodes_a_single_flip_without_osd() {
    let pcm = repetition_pcm();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false, false, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations > 0, true);
    assert_eq!(result.stats.decode_call_count, 1);
    assert_eq!(result.stats.bp_iteration_count, result.bp_iterations);
    assert!(result.stats.bp_seconds.is_finite());
    assert!(result.stats.bp_seconds >= 0.0);
    assert_eq!(result.stats.osd_seconds, 0.0);
    assert_eq!(result.stats.osd_use_count, 0);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(
        result.correction,
        Correction::from(vec![true, false, false, false, false])
    );
}

#[test]
fn minimum_sum_keeps_a_converged_solution_when_early_stop_is_disabled() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![0, 1, 2]]).unwrap();
    let config = DecoderConfig {
        early_stop: false,
        ..DecoderConfig::default()
    };
    let decoder =
        BpOsdDecoder::new(pcm.clone(), ChannelModel::Bsc { error_rate: 0.05 }, config).unwrap();

    let syndrome = Syndrome::from(vec![false, true]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, false, true])
    );
}

#[test]
fn minimum_sum_decoder_reuses_one_instance_for_multiple_syndromes() {
    let pcm = repetition_pcm();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let cases = [
        (
            Syndrome::from(vec![true, false, false, false]),
            Correction::from(vec![true, false, false, false, false]),
        ),
        (
            Syndrome::from(vec![false, true, false, false]),
            Correction::from(vec![true, true, false, false, false]),
        ),
        (
            Syndrome::from(vec![false, false, true, false]),
            Correction::from(vec![false, false, false, true, true]),
        ),
    ];

    for _ in 0..3 {
        for (syndrome, expected) in &cases {
            let result = decoder.decode(syndrome).unwrap();
            assert!(result.converged);
            assert!(!result.used_osd);
            assert_eq!(result.correction, *expected);
            assert_eq!(pcm.multiply(&result.correction), syndrome.clone());
        }
    }
}

#[test]
fn minimum_sum_decoder_clone_preserves_decoding_behavior_with_fresh_workspaces() {
    let pcm = repetition_pcm();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let cloned = decoder.clone();
    let syndrome = Syndrome::from(vec![false, true, false, false]);
    let first = decoder.decode(&syndrome).unwrap();
    let second = cloned.decode(&syndrome).unwrap();

    assert_eq!(second.correction, first.correction);
    assert_eq!(second.converged, first.converged);
    assert_eq!(second.bp_iterations, first.bp_iterations);
    assert_eq!(second.used_osd, first.used_osd);
    assert_eq!(
        second.residual_syndrome_weight,
        first.residual_syndrome_weight
    );
    assert_eq!(second.stats.decode_call_count, 1);
    assert_eq!(first.stats.decode_call_count, 1);
    assert_eq!(second.stats.bp_iteration_count, second.bp_iterations);
    assert_eq!(pcm.multiply(&second.correction), syndrome);
}

#[test]
fn decode_result_equality_ignores_timing_but_compares_counters() {
    let base = DecodeResult {
        correction: Correction::from(vec![true]),
        converged: true,
        bp_iterations: 1,
        used_osd: false,
        residual_syndrome_weight: 0,
        stats: DecodeStats {
            bp_seconds: 1.0,
            decode_call_count: 1,
            bp_iteration_count: 1,
            ..DecodeStats::default()
        },
    };
    let mut changed_timing = base.clone();
    changed_timing.stats.bp_seconds = 2.0;
    changed_timing.stats.osd_seconds = 3.0;
    assert_eq!(base, changed_timing);

    let mut changed_counters = base.clone();
    changed_counters.stats.decode_call_count = 2;
    assert_ne!(base, changed_counters);
}

#[test]
fn zero_syndrome_can_return_a_prior_favored_nullspace_correction() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.9, 0.9]),
        DecoderConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 0);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(result.correction, Correction::from(vec![true, true]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn zero_syndrome_falls_back_to_bp_or_osd_when_hard_decision_is_invalid() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.9, 0.2]),
        DecoderConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(result.correction, Correction::from(vec![true, true]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn decoder_rejects_syndrome_and_channel_dimension_mismatches() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let err = decoder
        .decode(&Syndrome::from(vec![true, false]))
        .unwrap_err();
    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "syndrome",
            expected: 1,
            actual: 2,
        }
    );

    let err = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1]),
        DecoderConfig::default(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "channel probabilities",
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn decoder_rejects_invalid_probability_inputs() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();

    let err = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.0 },
        DecoderConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err, DecodeError::InvalidProbability);

    let err = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 1.0]),
        DecoderConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err, DecodeError::InvalidProbability);
}

#[test]
fn product_sum_serial_changes_bp_snapshot_on_borrowed_case() {
    let default_case = load_parity_case("bp_repetition_single_flip.json");
    let sensitive_case = load_parity_case("bp_product_sum_serial_sensitive.json");

    assert_eq!(
        default_case.config.bp_variant,
        parity_schema::BpVariantSpec::MinimumSum
    );
    assert_eq!(
        default_case.config.schedule,
        parity_schema::ScheduleSpec::Parallel
    );
    assert_eq!(
        sensitive_case.config.bp_variant,
        parity_schema::BpVariantSpec::ProductSum
    );
    assert_eq!(
        sensitive_case.config.schedule,
        parity_schema::ScheduleSpec::Serial
    );

    let default_report = parity_runner::run_case(&default_case);
    let sensitive_report = parity_runner::run_case(&sensitive_case);

    assert_eq!(default_report.matches_expected, Some(true));
    assert_eq!(sensitive_report.matches_expected, Some(true));

    let mut comparison_case = sensitive_case.clone();
    comparison_case.config.bp_variant = parity_schema::BpVariantSpec::MinimumSum;
    comparison_case.config.schedule = parity_schema::ScheduleSpec::Parallel;
    let default_mode_report = parity_runner::run_case(&comparison_case);

    assert_ne!(
        sensitive_report.actual, default_mode_report.actual,
        "product_sum + serial must differ from minimum_sum + parallel on the sensitive fixture"
    );
}

#[test]
fn product_sum_serial_teeth_cases() {
    let sensitive_case = load_parity_case("bp_product_sum_serial_sensitive.json");
    assert_eq!(
        sensitive_case.config.bp_variant,
        parity_schema::BpVariantSpec::ProductSum
    );
    assert_eq!(
        sensitive_case.config.schedule,
        parity_schema::ScheduleSpec::Serial
    );

    let product_sum_serial = parity_runner::run_case(&sensitive_case);
    assert_eq!(product_sum_serial.matches_expected, Some(true));

    let mut minimum_sum_serial_case = sensitive_case.clone();
    minimum_sum_serial_case.config.bp_variant = parity_schema::BpVariantSpec::MinimumSum;
    let minimum_sum_serial = parity_runner::run_case(&minimum_sum_serial_case);

    let mut product_sum_parallel_case = sensitive_case.clone();
    product_sum_parallel_case.config.schedule = parity_schema::ScheduleSpec::Parallel;
    let product_sum_parallel = parity_runner::run_case(&product_sum_parallel_case);

    assert_ne!(
        product_sum_serial.actual, minimum_sum_serial.actual,
        "product_sum must change decoder behavior while schedule stays serial"
    );
    assert_ne!(
        product_sum_serial.actual, product_sum_parallel.actual,
        "serial schedule must change decoder behavior while bp method stays product_sum"
    );
}

#[test]
fn minimum_sum_parallel_regression_suite_still_passes() {
    for fixture_name in [
        "bp_repetition_single_flip.json",
        "osd_equal_reliability_tiebreak.json",
        "osd_small_sparse_code.json",
    ] {
        let case = load_parity_case(fixture_name);
        assert_eq!(
            case.config.bp_variant,
            parity_schema::BpVariantSpec::MinimumSum
        );
        assert_eq!(case.config.schedule, parity_schema::ScheduleSpec::Parallel);
        let report = parity_runner::run_case(&case);
        assert_eq!(
            report.matches_expected,
            Some(true),
            "default regression fixture {fixture_name} changed: expected {:?}, actual {:?}",
            report.expected,
            report.actual
        );
    }
}
