use rbposd::{ChannelModel, CssDecoders, DecoderConfig, ParityCheckMatrix, Syndrome};

#[test]
fn css_decoders_route_x_and_z_syndromes_to_different_matrices() {
    let hx = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let hz = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![1]]).unwrap();

    let css = CssDecoders::new(
        hx.clone(),
        hz.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let x = css.decode_x(&Syndrome::from(vec![true])).unwrap();
    let z = css.decode_z(&Syndrome::from(vec![true])).unwrap();

    assert_eq!(hx.multiply(&x.correction), Syndrome::from(vec![true]));
    assert_eq!(hz.multiply(&z.correction), Syndrome::from(vec![true]));
}
