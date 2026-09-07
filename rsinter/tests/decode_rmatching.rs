#![cfg(feature = "rmatching-runner")]

use rsinter::decode::{Decoder, RmatchingDemDecoder};
use rstim::dem::DetectorErrorModel;

#[test]
fn rmatching_dem_decoder_compiles_for_dem() {
    let dem =
        DetectorErrorModel::parse("error(0.1) D0 D1 L0\nerror(0.05) D0\nerror(0.05) D1\n").unwrap();
    let decoder = RmatchingDemDecoder;

    let compiled = decoder.compile_for_dem(&dem).unwrap();
    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0011], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn rmatching_dem_decoder_handles_non_byte_aligned_widths() {
    let dem =
        DetectorErrorModel::parse("error(0.1) D0 D8 L8\nerror(0.05) D0\nerror(0.05) D8\n").unwrap();
    let decoder = RmatchingDemDecoder;
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(
            &[0b0000_0001, 0b0000_0001, 0b0000_0000, 0b1111_1110],
            2,
            9,
            9,
        )
        .unwrap();

    assert_eq!(
        predictions,
        vec![0b0000_0000, 0b0000_0001, 0b0000_0000, 0b0000_0000]
    );
}

#[test]
fn rmatching_dem_decoder_returns_invalid_batch_as_error() {
    let dem =
        DetectorErrorModel::parse("error(0.1) D0 D1 L0\nerror(0.05) D0\nerror(0.05) D1\n").unwrap();
    let compiled = RmatchingDemDecoder.compile_for_dem(&dem).unwrap();

    let error = compiled.decode_shots_bit_packed(&[], 1, 2, 1).unwrap_err();

    assert!(error.contains("detector buffer length mismatch"));
}

#[test]
fn rmatching_dem_decoder_returns_dimension_mismatch_as_error() {
    let dem =
        DetectorErrorModel::parse("error(0.1) D0 D1 L0\nerror(0.05) D0\nerror(0.05) D1\n").unwrap();
    let compiled = RmatchingDemDecoder.compile_for_dem(&dem).unwrap();

    let error = compiled.decode_shots_bit_packed(&[0], 1, 1, 1).unwrap_err();

    assert!(error.contains("detector count does not match the graph"));
}

#[test]
fn rmatching_dem_decoder_preserves_minimum_declared_dimensions() {
    let mut dem = DetectorErrorModel::new();
    dem.set_min_counts(72, 12);
    let compiled = RmatchingDemDecoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0; 18], 2, 72, 12)
        .unwrap();

    assert_eq!(predictions, vec![0; 4]);
}

#[test]
fn rmatching_dem_decoder_applies_minimum_detector_count_before_trailing_shift() {
    let mut dem = DetectorErrorModel::new();
    dem.set_min_counts(72, 1);
    dem.add_shift_detectors(72, Vec::new());
    let compiled = RmatchingDemDecoder.compile_for_dem(&dem).unwrap();

    assert_eq!(
        compiled.decode_shots_bit_packed(&[0; 9], 1, 72, 1).unwrap(),
        vec![0]
    );
    let error = compiled
        .decode_shots_bit_packed(&[0; 18], 1, 144, 1)
        .unwrap_err();
    assert!(error.contains("detector count does not match the graph"));
}
