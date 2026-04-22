use rbposd::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};

#[test]
fn decoder_config_default_contract() {
    let cfg = DecoderConfig::default();

    assert_eq!(cfg.max_bp_iterations, 30);
    assert!(cfg.early_stop);
    assert_eq!(cfg.bp_variant, BpVariant::MinimumSum);
    assert_eq!(cfg.schedule, Schedule::Parallel);
    assert_eq!(cfg.osd_variant, OsdVariant::Osd0);
}

#[test]
fn channel_model_contract() {
    let bsc = ChannelModel::Bsc { error_rate: 0.05 };
    assert_eq!(bsc, ChannelModel::Bsc { error_rate: 0.05 });

    let bit_flips = ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]);
    assert_eq!(bit_flips, ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]));
}
