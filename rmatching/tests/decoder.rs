#![cfg(feature = "bench")]

use rmatching::{Matching, PackedDecodeError};
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
fn matching_rejects_dem_probability_one_with_non_finite_weight() {
    for probability in ["1", "-0.1", "1.1", "NaN"] {
        let error = match Matching::from_dem(&format!("error({probability}) D0 L0\n")) {
            Ok(_) => panic!("probability {probability} unexpectedly compiled"),
            Err(error) => error,
        };
        assert!(
            error.contains("finite error probabilities in the range 0 <= p < 1"),
            "unexpected error for probability {probability}: {error}"
        );
    }
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
fn matching_rejects_requested_obs_width_exceeding_graph() {
    let mut matching = compile_matching_from_dem(
        "\
error(0.1) D0 L0
error(0.05) D0
",
    );

    let num_dets: usize = 1;
    let requested_num_obs: usize = 9;
    let result =
        matching.try_decode_shots_bit_packed(&[0b0000_0001], 1, num_dets, requested_num_obs);

    assert_eq!(
        result,
        Err(PackedDecodeError::ObservableCountMismatch {
            expected: 1,
            actual: 9,
        })
    );
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

    let result = matching
        .try_decode_shots_bit_packed(&[0b0000_0001], 1, 1, 9)
        .unwrap();

    assert_eq!(result, vec![0b0000_0001, 0b0000_0000]);
}

#[test]
fn matching_accepts_declared_unused_detectors() {
    let mut matching = Matching::from_dem("detector D8\n").unwrap();

    let result = matching
        .try_decode_shots_bit_packed(&[0, 0, 0, 0], 2, 9, 0)
        .unwrap();

    assert!(result.is_empty());
}

#[test]
fn matching_zero_probability_errors_only_declare_dimensions() {
    let mut matching = Matching::from_dem("error(0) D8 L8\n").unwrap();

    let result = matching
        .try_decode_shots_bit_packed(&[0, 0], 1, 9, 9)
        .unwrap();

    assert_eq!(result, vec![0, 0]);
}

#[test]
fn matching_checked_decode_matches_legacy_api_for_valid_multibyte_batch() {
    let dem = "error(0.1) D0 D8 L8\nerror(0.05) D0\nerror(0.05) D8\n";
    let dets = [1, 1, 0, 0];
    let mut checked = compile_matching_from_dem(dem);
    let mut legacy = compile_matching_from_dem(dem);

    let checked_result = checked.try_decode_shots_bit_packed(&dets, 2, 9, 9).unwrap();
    let legacy_result = legacy.decode_shots_bit_packed(&dets, 2, 9, 9);

    assert_eq!(checked_result, legacy_result);
}

#[test]
fn matching_checked_decode_rejects_short_and_long_buffers() {
    let dem = "error(0.1) D0 D8 L0\nerror(0.05) D0\nerror(0.05) D8\n";
    let mut matching = compile_matching_from_dem(dem);

    assert_eq!(
        matching.try_decode_shots_bit_packed(&[0; 3], 2, 9, 1),
        Err(PackedDecodeError::DetectorBufferLengthMismatch {
            expected: 4,
            actual: 3,
        })
    );
    assert_eq!(
        matching.try_decode_shots_bit_packed(&[0; 5], 2, 9, 1),
        Err(PackedDecodeError::DetectorBufferLengthMismatch {
            expected: 4,
            actual: 5,
        })
    );
}

#[test]
fn matching_checked_decode_rejects_graph_dimension_mismatches() {
    let dem = "error(0.1) D0 D1 L0\nerror(0.05) D0\nerror(0.05) D1\n";
    let mut matching = compile_matching_from_dem(dem);

    assert_eq!(
        matching.try_decode_shots_bit_packed(&[0], 1, 1, 1),
        Err(PackedDecodeError::DetectorCountMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        matching.try_decode_shots_bit_packed(&[0], 1, 2, 0),
        Err(PackedDecodeError::ObservableCountMismatch {
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn matching_checked_decode_accepts_zero_sized_graph_and_batches() {
    let mut matching = Matching::new();

    assert_eq!(
        matching.try_decode_shots_bit_packed(&[], 3, 0, 0),
        Ok(Vec::new())
    );
    assert_eq!(
        matching.try_decode_shots_bit_packed(&[], 0, 0, 0),
        Ok(Vec::new())
    );
}

#[test]
fn matching_checked_decode_reports_batch_size_overflow() {
    let mut matching = Matching::from_dem("detector D8\n").unwrap();

    assert_eq!(
        matching.try_decode_shots_bit_packed(&[], usize::MAX, 9, 0),
        Err(PackedDecodeError::BufferSizeOverflow {
            num_shots: usize::MAX,
            row_bytes: 2,
        })
    );
}
