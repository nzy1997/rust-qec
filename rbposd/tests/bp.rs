use rbposd::{
    BpOsdDecoder, ChannelModel, Correction, DecodeError, DecoderConfig, ParityCheckMatrix, Syndrome,
};

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

    assert_eq!(second, first);
    assert_eq!(pcm.multiply(&second.correction), syndrome);
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
