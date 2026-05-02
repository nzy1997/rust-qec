#![cfg(feature = "rsinter")]

use rsinter::decode::Decoder;
use rstim::dem::DetectorErrorModel;

use rmatching::decoder::MwpmDecoder;

/// Compile MwpmDecoder for a simple DEM and verify it returns a CompiledDecoder.
#[test]
fn mwpm_decoder_compiles_for_dem() {
    let dem_text = "\
error(0.1) d0 d1 l0
error(0.1) d1 d2
error(0.05) d0
error(0.05) d2
";
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    let _compiled = MwpmDecoder.compile_for_dem(&dem);
}

/// Decode bit-packed shots and verify predictions.
#[test]
fn mwpm_decoder_decodes_shots() {
    let dem_text = "\
error(0.1) d0 d1 l0
error(0.1) d1 d2
error(0.05) d0
error(0.05) d2
";
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    let compiled = MwpmDecoder.compile_for_dem(&dem);

    let num_dets = 3;
    let num_obs = 1;
    let _det_bytes = (num_dets + 7) / 8; // 1
    let obs_bytes = (num_obs + 7) / 8; // 1

    // Shot 1: D0=1, D1=1, D2=0 => bit-packed: 0b011 = 0x03
    // Shot 2: D0=0, D1=0, D2=0 => bit-packed: 0b000 = 0x00
    let dets = vec![0x03u8, 0x00u8];
    let num_shots = 2;

    let result = compiled.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), num_shots * obs_bytes);
    // Shot 1: D0 and D1 fire => matched via D0-D1 edge carrying L0 => L0=1
    assert_eq!(result[0] & 1, 1, "Shot 1: expected L0 flipped");
    // Shot 2: no detections => no observable flips
    assert_eq!(result[1] & 1, 0, "Shot 2: expected no flips");
}
