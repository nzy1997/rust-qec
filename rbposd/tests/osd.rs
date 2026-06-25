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
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.1, 0.9]),
        DecoderConfig {
            max_bp_iterations: 1,
            osd_order: 0,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true, true])).unwrap();

    assert_eq!(result.stats.decode_call_count, 1);
    assert_eq!(result.stats.osd_use_count, usize::from(result.used_osd));
    assert_eq!(result.stats.osd_candidate_count, 0);
    if result.used_osd {
        assert_eq!(result.stats.gf2_solve_count, 1);
        assert_eq!(result.stats.gf2_full_elimination_count, 1);
    }
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
    assert!(result.stats.osd_candidate_count > 0);
    assert_eq!(
        result.stats.gf2_solve_count,
        result.stats.gf2_full_elimination_count
    );
    assert!(result.stats.gf2_solve_count >= result.stats.osd_candidate_count + 1);
}
