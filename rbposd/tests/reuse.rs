use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};

fn repetition_pcm() -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(4, 5, vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]])
        .unwrap()
}

#[test]
fn decoder_reuse_handles_zero_and_nonzero_syndromes_in_sequence() {
    let pcm = repetition_pcm();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let zero = decoder
        .decode(&Syndrome::from(vec![false, false, false, false]))
        .unwrap();
    assert_eq!(
        pcm.multiply(&zero.correction),
        Syndrome::from(vec![false, false, false, false])
    );
    assert!(zero.converged);
    assert!(!zero.used_osd);
    assert_eq!(zero.bp_iterations, 0);

    let nonzero = decoder
        .decode(&Syndrome::from(vec![false, true, false, false]))
        .unwrap();
    assert_eq!(
        nonzero.correction,
        Correction::from(vec![true, true, false, false, false])
    );
    assert_eq!(
        pcm.multiply(&nonzero.correction),
        Syndrome::from(vec![false, true, false, false])
    );
}

#[test]
fn osd_decoder_reuse_handles_back_to_back_calls_without_stale_state() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;

    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        config,
    )
    .unwrap();

    let first = decoder.decode(&Syndrome::from(vec![true, false])).unwrap();
    let second = decoder.decode(&Syndrome::from(vec![false, true])).unwrap();

    assert!(first.used_osd);
    assert!(second.used_osd);
    assert_eq!(
        pcm.multiply(&first.correction),
        Syndrome::from(vec![true, false])
    );
    assert_eq!(
        pcm.multiply(&second.correction),
        Syndrome::from(vec![false, true])
    );
}
