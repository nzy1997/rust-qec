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
fn matching_decodes_shots() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 D1 L0
error(0.1) D1 D2
error(0.05) D0
error(0.05) D2
",
    );

    let num_dets: usize = 3;
    let num_obs: usize = 1;
    let num_shots: usize = 2;
    let obs_bytes = num_obs.div_ceil(8);
    let dets = vec![0x03u8, 0x00u8];

    let result = matching.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), num_shots * obs_bytes);
    assert_eq!(result[0] & 1, 1, "shot 1 should flip L0");
    assert_eq!(result[1] & 1, 0, "shot 2 should not flip L0");
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
        &[0b0000_0001, 0b0000_0001, 0b0000_0000, 0b1111_1110],
        2,
        9,
        9,
    );

    assert_eq!(
        result,
        vec![0b0000_0000, 0b0000_0001, 0b0000_0000, 0b0000_0000]
    );
}

#[test]
fn matching_preserves_lsb_order_for_non_byte_aligned_detector_width() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D8 L0
error(0.05) D0
error(0.05) D8
",
    );

    let num_dets: usize = 9;
    let num_obs: usize = 1;
    let num_shots: usize = 3;
    let dets = vec![
        0b0000_0000,
        0b0000_0001, // shot 1: only D8 fires
        0b0000_0001,
        0b0000_0000, // shot 2: only D0 fires
        0b0000_0000,
        0b1000_0000, // shot 3: only a padding bit after D8 is set
    ];

    let result = matching.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), 3);
    assert_eq!(
        result[0] & 1,
        1,
        "D8 must decode through the second input byte"
    );
    assert_eq!(result[1] & 1, 0, "D0 should not flip L0 in this DEM");
    assert_eq!(
        result[2] & 1,
        0,
        "padding bits beyond num_dets must be ignored"
    );
}

#[test]
fn matching_packs_multiple_observables_without_touching_padding_bits() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 L0
error(0.1) D1 L8
error(0.05) D0
error(0.05) D1
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

    let result = matching.decode_shots_bit_packed(&dets, num_shots, num_dets, num_obs);

    assert_eq!(result.len(), num_shots * obs_bytes);
    assert_eq!(result[0], 0b0000_0001);
    assert_eq!(result[1], 0b0000_0000);
    assert_eq!(result[2], 0b0000_0000);
    assert_eq!(result[3], 0b0000_0001);
}

#[test]
fn matching_repeated_calls_keep_return_shape_stable() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 L0
error(0.1) D1 L8
error(0.05) D0
error(0.05) D1
",
    );

    let num_dets: usize = 2;
    let num_obs: usize = 9;
    let first_dets = vec![0b0000_0001, 0b0000_0010];
    let second_dets = vec![0b0000_0010];

    let first = matching.decode_shots_bit_packed(&first_dets, 2, num_dets, num_obs);
    let second = matching.decode_shots_bit_packed(&second_dets, 1, num_dets, num_obs);

    assert_eq!(
        first,
        vec![0b0000_0001, 0b0000_0000, 0b0000_0000, 0b0000_0001]
    );
    assert_eq!(second.len(), 2);
    assert_eq!(second, vec![0b0000_0000, 0b0000_0001]);
}

#[test]
fn matching_zero_fills_when_requested_obs_width_exceeds_dem() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 L0
error(0.05) D0
",
    );

    let num_dets: usize = 1;
    let requested_num_obs: usize = 9;
    let result = matching.decode_shots_bit_packed(&[0b0000_0001], 1, num_dets, requested_num_obs);

    assert_eq!(result.len(), requested_num_obs.div_ceil(8));
    assert_eq!(result, vec![0b0000_0001, 0b0000_0000]);
}

#[test]
fn matching_zero_fills_declared_but_unused_observable_width() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 L0
error(0.05) D0
logical_observable L8
",
    );

    let result = matching.decode_shots_bit_packed(&[0b0000_0001], 1, 1, 9);

    assert_eq!(result.len(), 2);
    assert_eq!(result, vec![0b0000_0001, 0b0000_0000]);
}
