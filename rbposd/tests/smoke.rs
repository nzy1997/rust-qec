use rbposd::{
    BpVariant, ChannelModel, Correction, DecodeError, DecoderConfig, LsdConfig, LsdMethod,
    OsdVariant, Schedule,
};

#[test]
fn decoder_config_default_contract() {
    let cfg = DecoderConfig::default();

    assert_eq!(cfg.max_bp_iterations, 30);
    assert!(cfg.early_stop);
    assert_eq!(cfg.bp_variant, BpVariant::MinimumSum);
    assert_eq!(cfg.schedule, Schedule::Parallel);
    assert_eq!(cfg.osd_variant, OsdVariant::Osd0);
    assert_eq!(cfg.osd_order, 0);
}

#[test]
fn lsd_config_default_contract() {
    let cfg = LsdConfig::default();

    assert_eq!(cfg.method, LsdMethod::LocalizedStatistics);
    assert_eq!(cfg.lsd_order, 0);

    let method = LsdMethod::LocalizedStatistics;
    assert_eq!(method, cfg.method);
}

#[test]
fn channel_model_contract() {
    let bsc = ChannelModel::Bsc { error_rate: 0.05 };
    assert_eq!(bsc, ChannelModel::Bsc { error_rate: 0.05 });

    let bit_flips = ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]);
    assert_eq!(
        bit_flips,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3])
    );
}

#[test]
fn decode_error_contract() {
    let e = DecodeError::InvalidColumnIndex {
        column: 7,
        num_bits: 5,
    };
    assert_eq!(e.to_string(), "column index 7 is out of bounds for 5 bits");

    fn takes_std_error(_: &dyn std::error::Error) {}
    takes_std_error(&e);
}

#[test]
fn correction_helpers_and_error_display_cover_remaining_contracts() {
    let zero = Correction::zero(3);
    assert_eq!(zero.len(), 3);
    assert_eq!(zero.as_slice(), &[false, false, false]);

    assert_eq!(
        DecodeError::EmptyMatrix.to_string(),
        "parity-check matrix is empty"
    );
    assert_eq!(
        DecodeError::InvalidProbability.to_string(),
        "invalid probability value"
    );
    assert_eq!(
        DecodeError::DimensionMismatch {
            what: "syndrome",
            expected: 2,
            actual: 3,
        }
        .to_string(),
        "dimension mismatch for syndrome: expected 2, got 3"
    );
    assert_eq!(
        DecodeError::SingularSystem.to_string(),
        "singular system cannot satisfy the target syndrome"
    );
    assert_eq!(
        DecodeError::BpDidNotConverge.to_string(),
        "belief propagation did not converge"
    );
    assert_eq!(
        DecodeError::NoOsdSolution.to_string(),
        "no OSD solution found"
    );
    assert_eq!(
        DecodeError::UnsupportedLsdOrder { order: 1 }.to_string(),
        "unsupported LSD order 1; only order 0 is supported"
    );
}
