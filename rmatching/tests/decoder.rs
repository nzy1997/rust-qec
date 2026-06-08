#![cfg(feature = "rsinter")]

use rsinter::decode::Decoder;
use rstim::dem::DetectorErrorModel;

use rmatching::decoder::MwpmDecoder;

fn compile_test_decoder(dem_text: &str) -> Box<dyn rsinter::decode::CompiledDecoder> {
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    MwpmDecoder.compile_for_dem(&dem)
}

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

#[test]
fn mwpm_decoder_decodes_shots() {
    let compiled = compile_test_decoder(
        "\
error(0.1) d0 d1 l0
error(0.1) d1 d2
error(0.05) d0
error(0.05) d2
",
    );

    let num_dets: usize = 3;
    let num_obs: usize = 1;
    let num_shots: usize = 2;
    let obs_bytes = num_obs.div_ceil(8);
    let dets = vec![0x03u8, 0x00u8];

    let result = compiled.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), num_shots * obs_bytes);
    assert_eq!(result[0] & 1, 1, "shot 1 should flip L0");
    assert_eq!(result[1] & 1, 0, "shot 2 should not flip L0");
}

#[test]
fn mwpm_decoder_preserves_lsb_order_for_non_byte_aligned_detector_width() {
    let compiled = compile_test_decoder(
        "\
error(0.1) d8 l0
error(0.05) d0
error(0.05) d8
",
    );

    let num_dets: usize = 9;
    let num_obs: usize = 1;
    let num_shots: usize = 3;
    let dets = vec![
        0b0000_0000, 0b0000_0001, // shot 1: only D8 fires
        0b0000_0001, 0b0000_0000, // shot 2: only D0 fires
        0b0000_0000, 0b1000_0000, // shot 3: only a padding bit after D8 is set
    ];

    let result = compiled.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), 3);
    assert_eq!(result[0] & 1, 1, "D8 must decode through the second input byte");
    assert_eq!(result[1] & 1, 0, "D0 should not flip L0 in this DEM");
    assert_eq!(result[2] & 1, 0, "padding bits beyond num_dets must be ignored");
}

#[test]
fn mwpm_decoder_packs_multiple_observables_without_touching_padding_bits() {
    let compiled = compile_test_decoder(
        "\
error(0.1) d0 l0
error(0.1) d1 l8
error(0.05) d0
error(0.05) d1
",
    );

    let num_dets: usize = 2;
    let num_obs: usize = 9;
    let num_shots: usize = 2;
    let obs_bytes = num_obs.div_ceil(8);
    let dets = vec![
        0b0000_0001, // shot 1: D0
        0b0000_0010, // shot 2: D1
    ];

    let result = compiled.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), num_shots * obs_bytes);
    assert_eq!(result[0], 0b0000_0001);
    assert_eq!(result[1], 0b0000_0000);
    assert_eq!(result[2], 0b0000_0000);
    assert_eq!(result[3], 0b0000_0001);
}

#[test]
fn mwpm_decoder_repeated_calls_keep_return_shape_stable() {
    let compiled = compile_test_decoder(
        "\
error(0.1) d0 l0
error(0.1) d1 l8
error(0.05) d0
error(0.05) d1
",
    );

    let num_dets: usize = 2;
    let num_obs: usize = 9;
    let first_dets = vec![0b0000_0001, 0b0000_0010];
    let second_dets = vec![0b0000_0010];

    let first = compiled.decode_shots_bit_packed(&first_dets, 2, num_dets, num_obs);
    let second = compiled.decode_shots_bit_packed(&second_dets, 1, num_dets, num_obs);

    assert_eq!(first, vec![0b0000_0001, 0b0000_0000, 0b0000_0000, 0b0000_0001]);
    assert_eq!(second.len(), 2);
    assert_eq!(second, vec![0b0000_0000, 0b0000_0001]);
}
