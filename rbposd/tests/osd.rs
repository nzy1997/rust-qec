use rbposd::{
    BpOsdDecoder, ChannelModel, Correction, DecodeError, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Syndrome,
};

fn channel_prior_scoring_fixture() -> (ParityCheckMatrix, ChannelModel, Syndrome) {
    (
        ParityCheckMatrix::from_sparse_rows(2, 4, vec![vec![0, 1, 3], vec![1, 2, 3]]).unwrap(),
        ChannelModel::BitFlipProbabilities(vec![0.05, 0.15, 0.12, 0.08]),
        Syndrome::from(vec![true, true]),
    )
}

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

#[test]
fn diagnose_osd_path_reports_candidate_search_planning() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 4, vec![vec![0, 2], vec![1, 3]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;
    config.osd_order = 2;

    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3, 0.4]),
        config,
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let diagnostic = decoder.diagnose_osd_path(&syndrome).unwrap();

    assert_eq!(diagnostic.syndrome_weight, syndrome.weight());
    assert!(!diagnostic.bp_converged);
    assert_eq!(diagnostic.bp_iterations, 0);
    assert!(diagnostic.used_osd);
    assert_eq!(diagnostic.residual_syndrome_weight, syndrome.weight());
    assert_eq!(diagnostic.osd_order, 2);
    assert_eq!(diagnostic.free_column_count, 2);
    assert_eq!(diagnostic.candidate_search_frontier_size, 2);
    assert_eq!(diagnostic.max_candidate_order, 2);
    assert_eq!(diagnostic.planned_candidate_count, 3);
}

#[test]
fn ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        2,
        10,
        vec![
            vec![0, 2, 3, 4, 5, 6, 7, 8, 9],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        ],
    )
    .unwrap();
    let channel = ChannelModel::BitFlipProbabilities(vec![
        0.2, 0.2, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08,
    ]);
    let syndrome = Syndrome::from(vec![true, true]);

    let ldpc = BpOsdDecoder::new(
        pcm.clone(),
        channel.clone(),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let ldpc_diagnostic = ldpc.diagnose_osd_path(&syndrome).unwrap();

    assert_eq!(ldpc_diagnostic.osd_planner, "ldpc_osd_cs");
    assert!(ldpc_diagnostic.free_column_count >= 7);
    assert_eq!(ldpc_diagnostic.candidate_search_frontier_size, 7);
    assert_eq!(ldpc_diagnostic.max_candidate_order, 2);
    assert_eq!(
        ldpc_diagnostic.planned_candidate_count,
        ldpc_diagnostic.free_column_count as u128 + 21
    );
    let ldpc_decode = ldpc.decode(&syndrome).unwrap();
    assert_eq!(
        ldpc_decode.stats.osd_candidate_count as u128,
        ldpc_diagnostic.planned_candidate_count
    );
    assert_eq!(ldpc_decode.stats.gf2_solve_count, 1);
    assert_eq!(pcm.multiply(&ldpc_decode.correction), syndrome);
    let ldpc_profile = ldpc
        .profile_decode_with_osd_candidate_limit(&syndrome, usize::MAX)
        .unwrap();
    assert_eq!(
        ldpc_profile.osd_candidate_count as u128,
        ldpc_diagnostic.planned_candidate_count
    );

    let legacy = BpOsdDecoder::new(
        pcm,
        channel,
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let legacy_diagnostic = legacy.diagnose_osd_path(&syndrome).unwrap();

    assert_eq!(legacy_diagnostic.osd_planner, "legacy_combination_sweep");
    assert_ne!(legacy_diagnostic.osd_planner, ldpc_diagnostic.osd_planner);
    assert_ne!(
        legacy_diagnostic.planned_candidate_count,
        ldpc_diagnostic.planned_candidate_count
    );
}

#[test]
fn ldpc_osd_cs_uses_channel_prior_candidate_weight() {
    let (pcm, channel, syndrome) = channel_prior_scoring_fixture();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        channel,
        DecoderConfig {
            max_bp_iterations: 1,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, false, false, true])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(result.stats.osd_candidate_count, 3);
}

#[test]
fn legacy_osd_candidate_scoring_keeps_existing_reliability_behavior() {
    let (pcm, channel, syndrome) = channel_prior_scoring_fixture();
    let legacy = BpOsdDecoder::new(
        pcm.clone(),
        channel,
        DecoderConfig {
            max_bp_iterations: 1,
            osd_variant: OsdVariant::LegacyCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = legacy.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, true, false, false])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);

    let invalid = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.05, f64::NAN, 0.12, 0.08]),
        DecoderConfig {
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap_err();
    assert_eq!(invalid, DecodeError::InvalidProbability);
}

#[test]
fn explicit_osd0_planner_reports_zero_candidates_on_osd_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::Osd0,
            osd_order: 0,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let syndrome = Syndrome::from(vec![true, false]);

    let diagnostic = decoder.diagnose_osd_path(&syndrome).unwrap();
    assert!(diagnostic.used_osd);
    assert_eq!(diagnostic.osd_planner, "osd0");
    assert_eq!(diagnostic.candidate_search_frontier_size, 0);
    assert_eq!(diagnostic.max_candidate_order, 0);
    assert_eq!(diagnostic.planned_candidate_count, 0);

    let profile = decoder
        .profile_decode_with_osd_candidate_limit(&syndrome, 4)
        .unwrap();
    assert_eq!(profile.osd_use_count, 1);
    assert_eq!(profile.osd_candidate_count, 0);
    assert_eq!(profile.gf2_solve_count, 1);
}

#[test]
fn legacy_combination_sweep_order_zero_decodes_base_solution_without_candidates() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LegacyCombinationSweep,
            osd_order: 0,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(result.stats.osd_candidate_count, 0);
    assert_eq!(result.stats.gf2_solve_count, 1);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn ldpc_osd_cs_reports_zero_and_single_free_column_plans() {
    let full_rank = BpOsdDecoder::new(
        ParityCheckMatrix::from_sparse_rows(1, 1, vec![vec![0]]).unwrap(),
        ChannelModel::BitFlipProbabilities(vec![0.1]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let full_rank_plan = full_rank
        .diagnose_osd_path(&Syndrome::from(vec![true]))
        .unwrap();
    assert_eq!(full_rank_plan.osd_planner, "ldpc_osd_cs");
    assert_eq!(full_rank_plan.free_column_count, 0);
    assert_eq!(full_rank_plan.max_candidate_order, 0);
    assert_eq!(full_rank_plan.planned_candidate_count, 0);
    let full_rank_decode = full_rank.decode(&Syndrome::from(vec![true])).unwrap();
    assert_eq!(full_rank_decode.stats.osd_candidate_count, 0);

    let single_free = BpOsdDecoder::new(
        ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let single_free_plan = single_free
        .diagnose_osd_path(&Syndrome::from(vec![true]))
        .unwrap();
    assert_eq!(single_free_plan.free_column_count, 1);
    assert_eq!(single_free_plan.candidate_search_frontier_size, 1);
    assert_eq!(single_free_plan.max_candidate_order, 1);
    assert_eq!(single_free_plan.planned_candidate_count, 1);
    let single_free_profile = single_free
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true]), usize::MAX)
        .unwrap();
    assert_eq!(single_free_profile.osd_candidate_count, 1);
}

#[test]
fn ldpc_osd_cs_pair_candidate_can_improve_over_singles() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        4,
        6,
        vec![vec![0, 4], vec![1, 4], vec![2, 5], vec![3, 5]],
    )
    .unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![
            0.119_202_922_022_117_55,
            0.119_202_922_022_117_55,
            0.119_202_922_022_117_55,
            0.119_202_922_022_117_55,
            0.047_425_873_177_566_78,
            0.047_425_873_177_566_78,
        ]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let syndrome = Syndrome::from(vec![true, true, true, true]);

    let result = decoder.decode(&syndrome).unwrap();

    assert_eq!(
        result.correction,
        Correction::from(vec![false, false, false, false, true, true])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(result.stats.osd_candidate_count, 3);
}

#[test]
fn diagnose_osd_path_rejects_syndrome_dimension_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        DecoderConfig::default(),
    )
    .unwrap();

    let error = decoder
        .diagnose_osd_path(&Syndrome::from(vec![true]))
        .unwrap_err();

    assert!(error.to_string().contains("syndrome"));
    assert!(error.to_string().contains("expected 2"));
    assert!(error.to_string().contains("got 1"));
}

#[test]
fn diagnose_osd_path_reports_zero_syndrome_prior_fast_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.osd_order = 3;
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        config,
    )
    .unwrap();

    let diagnostic = decoder
        .diagnose_osd_path(&Syndrome::from(vec![false, false]))
        .unwrap();

    assert_eq!(diagnostic.syndrome_weight, 0);
    assert!(diagnostic.bp_converged);
    assert_eq!(diagnostic.bp_iterations, 0);
    assert!(!diagnostic.used_osd);
    assert_eq!(diagnostic.residual_syndrome_weight, 0);
    assert_eq!(diagnostic.osd_order, 3);
    assert_eq!(diagnostic.free_column_count, 0);
    assert_eq!(diagnostic.candidate_search_frontier_size, 0);
    assert_eq!(diagnostic.max_candidate_order, 0);
    assert_eq!(diagnostic.planned_candidate_count, 0);
}

#[test]
fn diagnose_osd_path_reports_bp_convergence_without_osd() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 1, vec![vec![0]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 4;
    config.osd_order = 5;
    let decoder =
        BpOsdDecoder::new(pcm, ChannelModel::BitFlipProbabilities(vec![0.9]), config).unwrap();

    let diagnostic = decoder
        .diagnose_osd_path(&Syndrome::from(vec![true]))
        .unwrap();

    assert_eq!(diagnostic.syndrome_weight, 1);
    assert!(diagnostic.bp_converged);
    assert_eq!(diagnostic.bp_iterations, 1);
    assert!(!diagnostic.used_osd);
    assert_eq!(diagnostic.residual_syndrome_weight, 0);
    assert_eq!(diagnostic.osd_order, 5);
    assert_eq!(diagnostic.free_column_count, 0);
    assert_eq!(diagnostic.candidate_search_frontier_size, 0);
    assert_eq!(diagnostic.max_candidate_order, 0);
    assert_eq!(diagnostic.planned_candidate_count, 0);
}

#[test]
fn osd0_decode_reports_zero_candidate_and_one_gf2_solve() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.1, 0.1]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 0,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true, true])).unwrap();

    assert_eq!(result.stats.decode_call_count, 1);
    assert!(result.used_osd);
    assert_eq!(result.stats.osd_use_count, 1);
    assert_eq!(result.stats.osd_candidate_count, 0);
    assert_eq!(result.stats.gf2_solve_count, 1);
    assert_eq!(result.stats.gf2_full_elimination_count, 1);
}

#[test]
fn osd_order_two_decode_reports_candidate_and_gf2_counters() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.49, 0.48, 0.47]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true])).unwrap();

    assert_eq!(result.stats.decode_call_count, 1);
    assert!(result.used_osd);
    assert_eq!(result.stats.osd_use_count, 1);
    assert!(result.stats.bp_seconds.is_finite());
    assert!(result.stats.bp_seconds >= 0.0);
    assert!(result.stats.osd_seconds.is_finite());
    assert!(result.stats.osd_seconds >= 0.0);
    assert!(result.stats.osd_candidate_count > 0);
    assert_eq!(result.stats.gf2_solve_count, 1);
    assert_eq!(result.stats.gf2_full_elimination_count, 1);
}

#[test]
fn osd_order7_reuses_factorization_without_changing_correction() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        2,
        10,
        vec![
            vec![0, 2, 3, 4, 5, 6, 7, 8, 9],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        ],
    )
    .unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![
            0.2, 0.2, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08,
        ]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true, true])).unwrap();

    assert_eq!(
        result.correction,
        Correction::from(vec![
            false, false, false, false, false, false, false, false, false, true
        ])
    );
    assert_eq!(
        pcm.multiply(&result.correction),
        Syndrome::from(vec![true, true])
    );
    assert!(result.used_osd);
    assert!(result.stats.osd_candidate_count > 1);
    assert_eq!(result.stats.gf2_solve_count, 1);
    assert_eq!(result.stats.gf2_full_elimination_count, 1);
}

#[test]
fn profile_decode_with_osd_candidate_limit_counts_bounded_actual_candidates() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 4, vec![vec![0, 2], vec![1, 3]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3, 0.4]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LegacyCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true, false]), 10)
        .unwrap();

    assert_eq!(stats.decode_call_count, 1);
    assert_eq!(stats.osd_use_count, 1);
    assert_eq!(stats.osd_candidate_count, 3);
    assert_eq!(stats.gf2_solve_count, 1);
    assert_eq!(stats.gf2_full_elimination_count, 1);
    assert!(stats.osd_seconds.is_finite());
    assert!(stats.osd_seconds >= 0.0);

    let limited_stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true, false]), 1)
        .unwrap();
    assert_eq!(limited_stats.osd_candidate_count, 1);
}

#[test]
fn ldpc_osd_cs_profile_limit_can_stop_during_single_column_sweep() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        2,
        10,
        vec![
            vec![0, 2, 3, 4, 5, 6, 7, 8, 9],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        ],
    )
    .unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![
            0.2, 0.2, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08,
        ]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true, true]), 1)
        .unwrap();

    assert_eq!(stats.osd_candidate_count, 1);
    assert_eq!(stats.gf2_solve_count, 1);
}

#[test]
fn profile_decode_with_candidate_limit_rejects_syndrome_dimension_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let error = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true]), 1)
        .unwrap_err();

    assert_eq!(
        error,
        DecodeError::DimensionMismatch {
            what: "syndrome",
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn profile_decode_with_candidate_limit_reports_zero_syndrome_prior_fast_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.1]),
        DecoderConfig::default(),
    )
    .unwrap();

    let stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![false]), 4)
        .unwrap();

    assert_eq!(stats.decode_call_count, 1);
    assert_eq!(stats.bp_iteration_count, 0);
    assert_eq!(stats.osd_use_count, 0);
    assert_eq!(stats.osd_candidate_count, 0);
    assert_eq!(stats.gf2_solve_count, 0);
    assert_eq!(stats.gf2_full_elimination_count, 0);
}

#[test]
fn profile_decode_with_candidate_limit_reports_bp_convergence_without_osd() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 1, vec![vec![0]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.9]),
        DecoderConfig {
            max_bp_iterations: 4,
            osd_order: 3,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true]), 4)
        .unwrap();

    assert_eq!(stats.decode_call_count, 1);
    assert_eq!(stats.bp_iteration_count, 1);
    assert_eq!(stats.osd_use_count, 0);
    assert_eq!(stats.osd_candidate_count, 0);
    assert_eq!(stats.gf2_solve_count, 0);
    assert_eq!(stats.gf2_full_elimination_count, 0);
    assert!(stats.bp_seconds.is_finite());
    assert!(stats.bp_seconds >= 0.0);
    assert_eq!(stats.osd_seconds, 0.0);
}

#[test]
fn profile_decode_with_zero_candidate_limit_counts_only_base_gf2_solve() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.49, 0.48, 0.47]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let stats = decoder
        .profile_decode_with_osd_candidate_limit(&Syndrome::from(vec![true]), 0)
        .unwrap();

    assert_eq!(stats.decode_call_count, 1);
    assert_eq!(stats.bp_iteration_count, 0);
    assert_eq!(stats.osd_use_count, 1);
    assert_eq!(stats.osd_candidate_count, 0);
    assert_eq!(stats.gf2_solve_count, 1);
    assert_eq!(stats.gf2_full_elimination_count, 1);
    assert!(stats.osd_seconds.is_finite());
    assert!(stats.osd_seconds >= 0.0);
}
