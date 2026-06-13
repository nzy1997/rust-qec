use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};

#[test]
fn osd0_recovers_a_valid_solution_when_bp_is_disabled() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;

    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        config,
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn osd0_uses_the_prior_hard_decision_as_its_base_solution() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;

    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.1, 0.9]),
        config,
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(result.correction, Correction::from(vec![false, true, true]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn osd0_decoder_reuse_returns_valid_solutions_for_multiple_syndromes() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;

    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        config,
    )
    .unwrap();

    for syndrome in [
        Syndrome::from(vec![true, false]),
        Syndrome::from(vec![false, true]),
        Syndrome::from(vec![true, true]),
    ] {
        let result = decoder.decode(&syndrome).unwrap();
        assert!(result.used_osd);
        assert_eq!(result.residual_syndrome_weight, 0);
        assert_eq!(pcm.multiply(&result.correction), syndrome);
    }
}

#[test]
fn osd_order_one_can_improve_over_osd0() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
    let syndrome = Syndrome::from(vec![true, true]);
    let channel = ChannelModel::BitFlipProbabilities(vec![
        0.268_941_421_369_995_1,
        0.268_941_421_369_995_1,
        0.182_425_523_806_356_35,
    ]);

    let mut osd0_config = DecoderConfig::default();
    osd0_config.max_bp_iterations = 0;
    osd0_config.osd_order = 0;
    let osd0 = BpOsdDecoder::new(pcm.clone(), channel.clone(), osd0_config)
        .unwrap()
        .decode(&syndrome)
        .unwrap();

    let mut osd1_config = DecoderConfig::default();
    osd1_config.max_bp_iterations = 0;
    osd1_config.osd_order = 1;
    let osd1 = BpOsdDecoder::new(pcm.clone(), channel, osd1_config)
        .unwrap()
        .decode(&syndrome)
        .unwrap();

    assert_eq!(osd0.correction, Correction::from(vec![true, true, false]));
    assert_eq!(osd1.correction, Correction::from(vec![false, false, true]));
    assert_eq!(pcm.multiply(&osd1.correction), syndrome);
}
