use rbposd::{BpLsdDecoder, ChannelModel, DecoderConfig, LsdConfig, ParityCheckMatrix, Syndrome};

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
