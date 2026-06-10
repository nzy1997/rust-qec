use rsinter::decode::{Decoder, RmatchingDemDecoder};
use rstim::dem::DetectorErrorModel;

#[test]
fn rmatching_dem_decoder_compiles_for_dem() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 D1 L0\nerror(0.05) D0\nerror(0.05) D1\n")
        .unwrap();
    let decoder = RmatchingDemDecoder;

    let compiled = decoder.compile_for_dem(&dem);
    let predictions = compiled.decode_shots_bit_packed(&[0b0000_0011], 1, 2, 1);

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn rmatching_dem_decoder_handles_non_byte_aligned_widths() {
    let dem = DetectorErrorModel::parse(
        "error(0.1) D0 D8 L8\nerror(0.05) D0\nerror(0.05) D8\n",
    )
    .unwrap();
    let decoder = RmatchingDemDecoder;
    let compiled = decoder.compile_for_dem(&dem);

    let predictions = compiled.decode_shots_bit_packed(
        &[
            0b0000_0001,
            0b0000_0001,
            0b0000_0000,
            0b1111_1110,
        ],
        2,
        9,
        9,
    );

    assert_eq!(predictions, vec![0b0000_0000, 0b0000_0001, 0b0000_0000, 0b0000_0000]);
}
