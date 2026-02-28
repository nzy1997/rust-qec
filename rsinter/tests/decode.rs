use rsinter::decode::{Decoder, VacuousDecoder};
use rstim::dem::DetectorErrorModel;

#[test]
fn vacuous_decoder_returns_all_zeros() {
    let dem = DetectorErrorModel::parse("error(0.1) D0").unwrap();
    let decoder = VacuousDecoder;
    let compiled = decoder.compile_for_dem(&dem);
    // 2 shots, 1 detector, 1 observable → dets_bytes=1 per shot, obs_bytes=1 per shot
    let dets = vec![0b1, 0b0]; // shot0: D0 fired, shot1: nothing
    let predictions = compiled.decode_shots_bit_packed(&dets, 2, 1, 1);
    // Vacuous always predicts 0
    assert_eq!(predictions, vec![0u8, 0u8]);
}

#[test]
fn vacuous_decoder_multi_obs() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0 L1").unwrap();
    let decoder = VacuousDecoder;
    let compiled = decoder.compile_for_dem(&dem);
    let dets = vec![0b1]; // 1 shot
    let predictions = compiled.decode_shots_bit_packed(&dets, 1, 1, 2);
    assert_eq!(predictions, vec![0u8]);
}
