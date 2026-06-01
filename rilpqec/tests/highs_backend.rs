use rilpqec::backend::build_batch_backend;
use rilpqec::{
    lower_dem_to_problem, BackendConfig, BackendKind, IlpDecodeError, IlpDecoderConfig,
    IlpDemDecoder,
};
use rstim::dem::DetectorErrorModel;

#[test]
fn highs_decodes_a_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let predictions = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();
    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn highs_reuses_one_batch_backend_for_multiple_shots() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.1) D1\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let predictions = decoder
        .decode_batch_bit_packed(&[0b0000_0001, 0b0000_0000], 2, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001, 0b0000_0000]);
}

#[test]
fn decode_batch_rejects_short_packed_detection_buffer() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.1) D1\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let err = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 2, 2, 1)
        .unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::PackedDetectionsLengthMismatch {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn decode_batch_rejects_detector_width_mismatch_before_building_a_backend() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let err = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::DetectorWidthMismatch {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn decode_batch_rejects_observable_width_mismatch_before_building_a_backend() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let err = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 1, 2)
        .unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::ObservableWidthMismatch {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn highs_supports_parity_solutions_requiring_two_columns_on_one_detector() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 D1\nerror(0.1) D0 L0\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let predictions = decoder
        .decode_batch_bit_packed(&[0b0000_0010], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn decode_batch_handles_baseline_only_problem_without_building_a_solver_model() {
    let dem = DetectorErrorModel::parse("error(0.75) L0\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let predictions = decoder.decode_batch_bit_packed(&[], 1, 0, 1).unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn direct_highs_backend_supports_optional_solver_settings() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();
    let config = IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Highs,
            time_limit_seconds: Some(1.0),
            mip_gap: Some(0.05),
            threads: Some(1),
            verbose: false,
        },
    };

    let mut backend = build_batch_backend(&problem, &config).unwrap();
    let correction = backend.solve(&[true, false]).unwrap();

    assert_eq!(correction, vec![true, false]);
}

#[test]
fn direct_highs_backend_rejects_detector_width_mismatch() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();
    let config = IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Highs,
            time_limit_seconds: None,
            mip_gap: None,
            threads: Some(1),
            verbose: false,
        },
    };

    let mut backend = build_batch_backend(&problem, &config).unwrap();
    let err = backend.solve(&[true, false]).unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::DetectorWidthMismatch {
            expected: 1,
            actual: 2,
        }
    );
}
