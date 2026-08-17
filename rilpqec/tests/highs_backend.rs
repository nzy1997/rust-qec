use qec_ilp_core::BinaryIlpConfig;
use qec_ilp_core::BinaryIlpModel;
use qec_ilp_core::backend::build_binary_backend;
use rilpqec::{
    BackendConfig, BackendKind, IlpDecodeError, IlpDecoderConfig, IlpDemDecoder,
    lower_dem_to_problem,
};
use rstim::dem::DetectorErrorModel;

#[test]
fn lowered_dem_problem_converts_to_shared_binary_model() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();

    let model: BinaryIlpModel = problem.to_binary_ilp_model().unwrap();

    assert_eq!(model.binary_vars.len(), 2);
    assert_eq!(model.integer_vars.len(), 2);
    assert_eq!(model.constraints.len(), 2);
    assert_eq!(model.solution_binary_prefix_len, 2);
}

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
fn compiled_highs_decoder_reuses_backend_across_batches() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.1) D1\n").unwrap();
    let mut decoder = IlpDemDecoder::from_dem(
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
    .unwrap()
    .into_compiled()
    .unwrap();

    let first = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();
    let second = decoder
        .decode_batch_bit_packed(&[0b0000_0000], 1, 2, 1)
        .unwrap();

    assert_eq!(first, vec![0b0000_0001]);
    assert_eq!(second, vec![0b0000_0000]);
}

#[test]
fn highs_decodes_forced_syndrome_after_probability_normalization() {
    let dem = DetectorErrorModel::parse("error(0.75) D0 L0\n").unwrap();
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
        .decode_batch_bit_packed(&[0b0000_0001, 0b0000_0000], 2, 1, 1)
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
    let model = problem.to_binary_ilp_model().unwrap();
    let config = IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Highs,
            time_limit_seconds: Some(1.0),
            mip_gap: Some(0.05),
            threads: Some(1),
            verbose: false,
        },
    };

    let mut backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: config.backend.clone(),
        },
    )
    .unwrap();
    backend.set_rhs(0, 1.0).unwrap();
    backend.set_rhs(1, 0.0).unwrap();
    let correction = backend.solve().unwrap().binary_values;

    assert_eq!(correction, vec![true, false]);
}

#[test]
fn observables_from_correction_rejects_width_mismatch_after_backend_build() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();
    let model = problem.to_binary_ilp_model().unwrap();
    let backend_config = BackendConfig {
        kind: BackendKind::Highs,
        time_limit_seconds: None,
        mip_gap: None,
        threads: Some(1),
        verbose: false,
    };
    let _backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: backend_config,
        },
    )
    .unwrap();

    let err = problem
        .observables_from_correction(&[true, false])
        .unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::CorrectionWidthMismatch {
            expected: 1,
            actual: 2,
        }
    );
}
