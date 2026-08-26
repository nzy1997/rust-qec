use rstim::m2d::{
    measurements_to_loss_aware_detections, measurements_to_loss_aware_detections_with_loss_mask,
    measurements_to_loss_aware_detections_with_loss_mask_and_limits, CompiledLossAwareM2d,
    LossAwareDetectorCheck, LossAwareM2dLimits,
};
use rstim::measurement_transform::{CheckedMeasurementLayout, MeasurementTransformLimits};
use rstim::parser::parse_lines;
use rstim::sim::bit_table::BitTable;

fn single_shot(bits: &[bool]) -> BitTable {
    let mut table = BitTable::new(bits.len(), 1);
    for (index, &value) in bits.iter().enumerate() {
        table.set(index, 0, value);
    }
    table
}

fn loss_mask(bits: usize, lost: &[usize]) -> BitTable {
    let mut table = BitTable::new(bits, 1);
    for &index in lost {
        table.set(index, 0, true);
    }
    table
}

#[test]
fn shared_lost_measurement_becomes_placeholder_invariant_supercheck() {
    let circuit =
        parse_lines("M 0\nM 0\nM 0\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n").unwrap();
    let mask = loss_mask(3, &[1]);

    let placeholder_zero = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[true, false, false]),
        &mask,
    )
    .unwrap();
    let placeholder_one = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[true, true, false]),
        &mask,
    )
    .unwrap();

    assert_eq!(placeholder_zero, placeholder_one);
    assert_eq!(placeholder_zero.shots[0].lost_measurements, [1]);
    assert_eq!(placeholder_zero.shots[0].detector_valid, [false, false]);
    assert_eq!(
        placeholder_zero.shots[0].checks,
        [LossAwareDetectorCheck {
            source_detectors: vec![0, 1],
            value: true,
        }]
    );
    assert!(placeholder_zero.shots[0].checks[0].is_supercheck());
    println!("PASS loss-aware-detectors placeholder_invariant=true superchecks=1");
}

#[test]
fn no_loss_preserves_all_original_detectors_as_singletons() {
    let circuit =
        parse_lines("M 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nDETECTOR rec[-2] rec[-1]\n")
            .unwrap();
    let output = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[true, false]),
        &loss_mask(2, &[]),
    )
    .unwrap();

    assert_eq!(output.shots[0].detector_valid, [true, true, true]);
    assert_eq!(output.shots[0].checks.len(), 3);
    assert_eq!(output.shots[0].checks[0].source_detectors, [0]);
    assert_eq!(output.shots[0].checks[1].source_detectors, [1]);
    assert_eq!(output.shots[0].checks[2].source_detectors, [2]);
    assert_eq!(
        output.shots[0]
            .checks
            .iter()
            .map(|check| check.value)
            .collect::<Vec<_>>(),
        [true, false, true]
    );
}

#[test]
fn independent_losses_remove_independent_detector_degrees_of_freedom() {
    let circuit = parse_lines("M 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n").unwrap();
    let output = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[false, true]),
        &loss_mask(2, &[0, 1]),
    )
    .unwrap();

    assert_eq!(output.shots[0].detector_valid, [false, false]);
    assert!(output.shots[0].checks.is_empty());
}

#[test]
fn embedded_loss_flags_derive_a_distinct_mask_for_each_shot() {
    let circuit = parse_lines("ML 0\nDETECTOR rec[-1]\n").unwrap();
    let mut measurements = BitTable::new(2, 3);
    measurements.set(0, 1, true);
    measurements.set(1, 1, true);
    measurements.set(0, 2, true);

    let output = measurements_to_loss_aware_detections(&circuit, &measurements).unwrap();

    assert!(output.shots[0].lost_measurements.is_empty());
    assert_eq!(output.shots[0].detector_valid, [true]);
    assert_eq!(output.shots[1].lost_measurements, [1]);
    assert_eq!(output.shots[1].detector_valid, [false]);
    assert_eq!(output.shots[2].lost_measurements, [1]);
    assert_eq!(output.shots[1].checks, output.shots[2].checks);
}

#[test]
fn compiled_transform_reuses_the_validated_layout_and_reference() {
    let circuit = parse_lines("ML 0\nDETECTOR rec[-1]\n").unwrap();
    let mut measurements = BitTable::new(2, 2);
    measurements.set(0, 0, true);
    measurements.set(1, 0, true);
    measurements.set(1, 1, true);

    let compiled = CompiledLossAwareM2d::new(&circuit).unwrap();
    assert_eq!(compiled.layout().num_measurements(), 2);
    let reused = compiled.convert(&measurements).unwrap();
    let convenience = measurements_to_loss_aware_detections(&circuit, &measurements).unwrap();

    assert_eq!(reused, convenience);
}

#[test]
fn explicit_mask_is_unioned_with_authoritative_embedded_flags() {
    let circuit = parse_lines("ML 0\nDETECTOR rec[-1]\n").unwrap();
    let empty_mask = loss_mask(2, &[]);

    let placeholder_zero = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[true, false]),
        &empty_mask,
    )
    .unwrap();
    let placeholder_one = measurements_to_loss_aware_detections_with_loss_mask(
        &circuit,
        &single_shot(&[true, true]),
        &empty_mask,
    )
    .unwrap();

    assert_eq!(placeholder_zero, placeholder_one);
    assert_eq!(placeholder_zero.shots[0].lost_measurements, [1]);
    assert_eq!(placeholder_zero.shots[0].detector_valid, [false]);
    assert!(placeholder_zero.shots[0].checks.is_empty());
}

#[test]
fn repeat_expansion_preserves_loss_visible_record_pairs() {
    let circuit = parse_lines("REPEAT 2 {\n  ML 0\n  DETECTOR rec[-1]\n}\n").unwrap();
    let measurements = single_shot(&[true, true, false, true]);

    let output = measurements_to_loss_aware_detections(&circuit, &measurements).unwrap();

    assert_eq!(output.shots[0].lost_measurements, [1]);
    assert_eq!(output.shots[0].detector_valid, [false, true]);
    assert_eq!(output.shots[0].checks.len(), 1);
    assert_eq!(output.shots[0].checks[0].source_detectors, [1]);
}

#[test]
fn nested_repeats_and_multi_target_measurements_keep_all_flag_value_pairs() {
    let circuit = parse_lines("REPEAT 2 {\n  REPEAT 2 {\n    MRL 0 1\n  }\n}\n").unwrap();
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        &circuit,
        MeasurementTransformLimits::default(),
    )
    .unwrap();

    assert_eq!(layout.num_measurements(), 16);
    assert_eq!(
        layout
            .loss_visible_measurements()
            .iter()
            .map(|pair| (pair.flag, pair.value))
            .collect::<Vec<_>>(),
        [
            (0, 1),
            (2, 3),
            (4, 5),
            (6, 7),
            (8, 9),
            (10, 11),
            (12, 13),
            (14, 15),
        ]
    );
}

#[test]
fn rejects_loss_flag_terms_and_malformed_explicit_masks() {
    let references_flag = parse_lines("ML 0\nDETECTOR rec[-2]\n").unwrap();
    let error =
        measurements_to_loss_aware_detections(&references_flag, &single_shot(&[true, true]))
            .unwrap_err();
    assert!(error.contains("detector 0 references loss-flag record 0"));

    let valid = parse_lines("ML 0\nDETECTOR rec[-1]\n").unwrap();
    let error = measurements_to_loss_aware_detections_with_loss_mask(
        &valid,
        &single_shot(&[true, true]),
        &single_shot(&[true, false]),
    )
    .unwrap_err();
    assert!(error.contains("marks loss-flag record 0 as lost"));

    let wrong_shape = BitTable::new(1, 1);
    let error = measurements_to_loss_aware_detections_with_loss_mask(
        &valid,
        &single_shot(&[false, false]),
        &wrong_shape,
    )
    .unwrap_err();
    assert!(error.contains("measurement_loss_mask has 1 bits"));

    let wrong_measurement_width = BitTable::new(1, 1);
    let error =
        measurements_to_loss_aware_detections(&valid, &wrong_measurement_width).unwrap_err();
    assert!(error.contains("meas_table has 1 bits but circuit has 2 measurements"));

    let error = measurements_to_loss_aware_detections_with_loss_mask(
        &valid,
        &wrong_measurement_width,
        &BitTable::new(2, 1),
    )
    .unwrap_err();
    assert!(error.contains("meas_table has 1 bits but circuit has 2 measurements"));

    let error = measurements_to_loss_aware_detections_with_loss_mask(
        &valid,
        &single_shot(&[false, false]),
        &BitTable::new(2, 2),
    )
    .unwrap_err();
    assert!(error.contains("measurement_loss_mask has 2 shots but meas_table has 1 shots"));

    let observable_references_flag = parse_lines("ML 0\nOBSERVABLE_INCLUDE(0) rec[-2]\n").unwrap();
    let error = measurements_to_loss_aware_detections(
        &observable_references_flag,
        &single_shot(&[false, false]),
    )
    .unwrap_err();
    assert!(error.contains("observable 0 references loss-flag record 0"));
}

#[test]
fn configurable_work_limits_fail_before_unbounded_elimination() {
    let circuit =
        parse_lines("M 0\nM 0\nM 0\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n").unwrap();
    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &circuit,
        &single_shot(&[false, false, false]),
        &loss_mask(3, &[1]),
        LossAwareM2dLimits {
            max_pivots_per_shot: 0,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("max_pivots_per_shot"));

    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &circuit,
        &single_shot(&[false, false, false]),
        &loss_mask(3, &[1]),
        LossAwareM2dLimits {
            max_elimination_steps: 0,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("max_elimination_steps"));

    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &circuit,
        &single_shot(&[false, false, false]),
        &loss_mask(3, &[1]),
        LossAwareM2dLimits {
            max_materialized_terms: 0,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("max_materialized_terms"));

    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &circuit,
        &single_shot(&[false, false, false]),
        &loss_mask(3, &[]),
        LossAwareM2dLimits {
            max_detectors_per_shot: 1,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("max_detectors_per_shot"));

    let empty_circuit = parse_lines("").unwrap();
    let zero_width_many_shots = BitTable::new(0, 2);
    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &empty_circuit,
        &zero_width_many_shots,
        &zero_width_many_shots,
        LossAwareM2dLimits {
            max_shots_per_batch: 1,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("max_shots_per_batch"));

    let error = measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        &circuit,
        &single_shot(&[false, false, false]),
        &loss_mask(3, &[]),
        LossAwareM2dLimits {
            max_batch_table_bits: 2,
            ..LossAwareM2dLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("measurement table exceeded max_batch_table_bits"));
}
