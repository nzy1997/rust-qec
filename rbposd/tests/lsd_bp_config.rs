use rbposd::{
    BpLsdDecoder, BpVariant, ChannelModel, DecoderConfig, LsdConfig, ParityCheckMatrix,
    Schedule, Syndrome,
};

#[test]
fn bplsddecoder_with_bp_config_respects_max_bp_iterations() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let mut bp_config = DecoderConfig::default();
    bp_config.max_bp_iterations = 0;
    let decoder = BpLsdDecoder::with_bp_config(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
        bp_config,
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 0);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_with_bp_config_uses_product_sum_serial_execution() {
    let pcm =
        ParityCheckMatrix::from_sparse_rows(3, 4, vec![vec![0, 1], vec![1, 2], vec![2, 3]])
            .unwrap();
    let decoder = BpLsdDecoder::with_bp_config(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.2, 0.35, 0.2, 0.2]),
        LsdConfig::default(),
        DecoderConfig {
            max_bp_iterations: 3,
            early_stop: false,
            bp_variant: BpVariant::ProductSum,
            schedule: Schedule::Serial,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false, true]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert_eq!(result.bp_iterations, 3);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}
