use rilpqec::{BackendConfig, BackendKind, IlpDecodeError, IlpDecoderConfig, IlpDemDecoder};
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
