use rbposd::{
    BpLsdDecoder, ChannelModel, Correction, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};

#[test]
fn bplsddecoder_public_api_matches_reference_contract() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_zero_syndrome_uses_prior_fast_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.9, 0.9]),
        LsdConfig::default(),
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
fn bplsddecoder_order_zero_fallback_repairs_bp_residual_without_osd() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 30);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, true, false])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_rejects_channel_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();

    let err = BpLsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        LsdConfig::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "channel probabilities",
            expected: 3,
            actual: 2,
        }
    );
}

#[test]
fn bplsddecoder_rejects_nonzero_lsd_order_until_algorithm_milestone() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let config = LsdConfig {
        lsd_order: 1,
        ..LsdConfig::default()
    };

    let err = BpLsdDecoder::new(pcm, ChannelModel::Bsc { error_rate: 0.05 }, config).unwrap_err();

    assert_eq!(err, DecodeError::UnsupportedLsdOrder { order: 1 });
}
