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
fn decoder_config_defaults_do_not_silently_change() {
    let cfg = DecoderConfig::default();

    assert_eq!(cfg.bp_variant, BpVariant::MinimumSum);
    assert_eq!(cfg.schedule, Schedule::Parallel);
}

#[test]
fn decoder_config_exposes_bp_method_and_schedule_variants() {
    let methods = [BpVariant::MinimumSum, BpVariant::ProductSum];
    let schedules = [Schedule::Parallel, Schedule::Serial];

    assert_eq!(methods[0], BpVariant::MinimumSum);
    assert_eq!(methods[1], BpVariant::ProductSum);
    assert_eq!(schedules[0], Schedule::Parallel);
    assert_eq!(schedules[1], Schedule::Serial);

    let cfg = DecoderConfig {
        bp_variant: BpVariant::ProductSum,
        schedule: Schedule::Serial,
        ..DecoderConfig::default()
    };

    assert_eq!(cfg.bp_variant, BpVariant::ProductSum);
    assert_eq!(cfg.schedule, Schedule::Serial);
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
fn unsupported_osd_method_is_rejected_without_fallback() {
    let error = OsdVariant::from_method_name("osd_cs_typo").unwrap_err();

    assert_eq!(
        error,
        DecodeError::UnsupportedOsdMethod {
            method: "osd_cs_typo".to_string()
        }
    );
    assert!(error.to_string().contains("osd_cs_typo"));
}

#[test]
fn osd_method_aliases_map_to_explicit_planners() {
    assert_eq!(
        OsdVariant::from_method_name("combination_sweep").unwrap(),
        OsdVariant::LegacyCombinationSweep
    );
    assert_eq!(
        OsdVariant::from_method_name("legacy_combination_sweep").unwrap(),
        OsdVariant::LegacyCombinationSweep
    );
    assert_eq!(
        OsdVariant::from_method_name("ldpc_osd_cs").unwrap(),
        OsdVariant::LdpcCombinationSweep
    );
    assert_eq!(
        OsdVariant::from_method_name("osd_cs").unwrap(),
        OsdVariant::LdpcCombinationSweep
    );

    assert_eq!(OsdVariant::Osd0.planner_name(), "osd0");
    assert_eq!(
        OsdVariant::LegacyCombinationSweep.planner_name(),
        "legacy_combination_sweep"
    );
    assert_eq!(
        OsdVariant::LdpcCombinationSweep.planner_name(),
        "ldpc_osd_cs"
    );
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
        DecodeError::NoLsdSolution.to_string(),
        "no LSD solution found"
    );
    assert_eq!(
        DecodeError::UnsupportedOsdMethod {
            method: "bad".to_string()
        }
        .to_string(),
        "unsupported OSD method \"bad\"; supported methods are combination_sweep, legacy_combination_sweep, ldpc_osd_cs, osd_cs"
    );
    assert_eq!(
        DecodeError::UnsupportedLsdOrder { order: 2 }.to_string(),
        "unsupported LSD order 2; only orders 0 and 1 are supported"
    );
}
