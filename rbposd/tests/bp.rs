use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};

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
