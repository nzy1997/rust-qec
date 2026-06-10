#![cfg(feature = "bench")]

use rmatching::Matching;
use rstim::dem::DetectorErrorModel;

fn compile_matching_from_dem(dem_text: &str) -> Matching {
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    Matching::from_dem(&dem.to_string()).unwrap()
}

#[test]
fn matching_compiles_from_dem_without_rsinter_feature() {
    let dem_text = "\
error(0.1) D0 D1 L0
error(0.1) D1 D2
error(0.05) D0
error(0.05) D2
";
    let _matching = compile_matching_from_dem(dem_text);
}

#[test]
fn matching_decodes_non_byte_aligned_bit_packed_shots() {
    let dem_text = "\
error(0.1) D0 D8 L8
error(0.05) D0
error(0.05) D8
";
    let mut matching = compile_matching_from_dem(dem_text);

    let result = matching.decode_shots_bit_packed(
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

    assert_eq!(result, vec![0b0000_0000, 0b0000_0001, 0b0000_0000, 0b0000_0000]);
}
