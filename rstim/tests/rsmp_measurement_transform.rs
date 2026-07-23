use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rstim::ir::{circuit_to_string, StimInstr};
use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::{
    CheckedMeasurementLayout, EncodedMeasurementBlock, MeasurementTransform,
    MeasurementTransformError, MeasurementTransformLimits,
};
use rstim::output::{read_shots_b8, write_shots_b8};
use rstim::parser::parse_lines;
use rstim::sim::bit_table::{BitTable, BitTableAllocError};

const PROPERTY_SEED: u64 = 0x5253_4d50_0522_0001;
const PROPERTY_CASES: usize = 256;
const SHOT_COUNTS: [usize; 8] = [0, 1, 7, 8, 9, 63, 64, 65];

#[test]
fn rsmp_measurement_transform_contract() {
    let mut valid_cases = consume_required_catalog_roles();
    let known_answers = verify_catalog_known_answers();
    verify_fixed_known_answer_case();
    verify_public_error_and_accessor_contracts();
    valid_cases += verify_additional_valid_semantics();
    assert_eq!(valid_cases, 7);
    assert_eq!(known_answers, 4);

    let property_cases = verify_property_cases();
    assert_eq!(property_cases, PROPERTY_CASES);

    verify_negative_controls();
    let allocation_guards = verify_allocation_guards();
    assert_eq!(allocation_guards, 12);

    let benchmark_rank = verify_surface_benchmark_rank();
    assert_eq!(benchmark_rank, 12000);

    println!(
        "PASS rsmp measurement transform valid_cases=7 known_answers=4 property_cases=256 allocation_guards=12 benchmark_rank=12000"
    );
}

fn consume_required_catalog_roles() -> usize {
    let required_roles = [
        "nonzero_reference",
        "rank_zero",
        "dependent_detectors",
        "repeat_records",
        "observable_recovery",
        "loss_visible_measurements",
        "surface_d11_r100",
    ];
    let catalog = load_catalog();
    let cases = catalog["cases"].as_array().expect("catalog cases array");
    let mut consumed = 0;
    for role in required_roles {
        let case = cases
            .iter()
            .find(|case| has_role(case, role))
            .unwrap_or_else(|| panic!("missing semantic role {role}"));
        let id = case["id"].as_str().expect("case id");
        let circuit = read_case_circuit(case);
        let instrs =
            parse_lines(&circuit).unwrap_or_else(|err| panic!("{id}: parse failed: {err}"));
        let transform = MeasurementTransform::from_circuit(&instrs)
            .unwrap_or_else(|err| panic!("{id}: transform failed: {err}"));
        assert_catalog_dimensions(case, &transform);
        assert_identity_uses_canonical_circuit_sha(&instrs, &transform);
        assert_eq!(transform.limits(), MeasurementTransformLimits::default());

        if role == "surface_d11_r100" {
            assert_eq!(transform.num_measurements(), 12121);
            assert_eq!(transform.num_detectors(), 12000);
            assert_eq!(transform.num_observables(), 1);
            assert_eq!(transform.rank(), 12000);
            assert_eq!(transform.free_columns().len(), 121);
        } else {
            let shots = case["shots"].as_u64().expect("shots") as usize;
            let measurements = patterned_table(transform.num_measurements(), shots, id.as_bytes());
            assert_round_trip_matches_m2d(&instrs, &transform, &measurements);
        }
        consumed += 1;
    }
    consumed
}

fn verify_catalog_known_answers() -> usize {
    let catalog = load_catalog();
    let cases = catalog["cases"].as_array().expect("catalog cases array");
    let known_cases: Vec<&Value> = cases
        .iter()
        .filter(|case| case["known_answer"].as_bool().unwrap_or(false))
        .collect();
    assert_eq!(known_cases.len(), 4);

    for case in &known_cases {
        let id = case["id"].as_str().expect("case id");
        let circuit = read_case_circuit(case);
        let instrs =
            parse_lines(&circuit).unwrap_or_else(|err| panic!("{id}: parse failed: {err}"));
        let transform = MeasurementTransform::from_circuit(&instrs)
            .unwrap_or_else(|err| panic!("{id}: transform failed: {err}"));
        assert_catalog_dimensions(case, &transform);
        assert_identity_uses_canonical_circuit_sha(&instrs, &transform);
        assert_catalog_hashes_loaded_from_files(case);

        let shots = case["shots"].as_u64().expect("shots") as usize;
        let measurements = read_expected_table(case, "measurements_b8", shots);
        let expected_detectors = read_expected_table(case, "detectors_b8", shots);
        let expected_observables = read_expected_table(case, "observables_b8", shots);

        let decoded = transform
            .decode_block(
                &transform
                    .encode_block(&measurements)
                    .expect("encode known answer"),
            )
            .expect("decode known answer");
        assert_tables_eq(&measurements, &decoded.measurements, id, "measurements");
        assert_tables_eq(&expected_detectors, &decoded.detections, id, "detectors");
        assert_tables_eq(
            &expected_observables,
            &decoded.observable_flips,
            id,
            "observables",
        );

        let m2d = measurements_to_detections(&instrs, &measurements)
            .unwrap_or_else(|err| panic!("{id}: m2d failed: {err}"));
        assert_tables_eq(&m2d.detections, &decoded.detections, id, "m2d detectors");
        assert_tables_eq(
            &m2d.observable_flips,
            &decoded.observable_flips,
            id,
            "m2d observables",
        );
    }

    known_cases.len()
}

fn verify_fixed_known_answer_case() {
    let instrs = parse_lines("M 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n")
        .expect("fixed known-answer circuit parses");
    let transform = MeasurementTransform::from_circuit(&instrs).expect("fixed transform");
    assert_eq!(transform.selected_detector_rows(), &[0, 1]);
    assert_eq!(transform.pivot_columns(), &[1, 2]);
    assert_eq!(transform.free_columns(), &[0]);

    let mut measurements = BitTable::try_new(3, 1).expect("fixed measurement table");
    measurements.set(0, 0, true);
    measurements.set(2, 0, true);
    let encoded = transform.encode_block(&measurements).expect("fixed encode");
    assert_eq!(encoded.selected_detectors.num_major(), 2);
    assert_eq!(encoded.free_measurements.num_major(), 1);
    assert!(encoded.selected_detectors.get(0, 0));
    assert!(encoded.selected_detectors.get(1, 0));
    assert!(encoded.free_measurements.get(0, 0));

    let decoded = transform.decode_block(&encoded).expect("fixed decode");
    assert_tables_eq(
        &measurements,
        &decoded.measurements,
        "fixed",
        "measurements",
    );
}

fn verify_public_error_and_accessor_contracts() {
    assert_eq!(
        MeasurementTransformError::UnsupportedSweep.to_string(),
        "sweep-bit circuits are not supported"
    );
    assert_eq!(
        MeasurementTransformError::LimitExceeded {
            limit: "max_measurements"
        }
        .to_string(),
        "measurement transform limit exceeded: max_measurements"
    );
    assert_eq!(
        MeasurementTransformError::ShapeMismatch {
            detail: "bad shape".to_string()
        }
        .to_string(),
        "measurement transform shape mismatch: bad shape"
    );
    assert_eq!(
        MeasurementTransformError::from(BitTableAllocError::SizeOverflow).to_string(),
        "BitTable allocation failed: SizeOverflow"
    );
    assert_eq!(
        MeasurementTransformError::Reference {
            detail: "reference failed".to_string()
        }
        .to_string(),
        "reference sample construction failed: reference failed"
    );

    let instrs =
        parse_lines("X 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let layout =
        CheckedMeasurementLayout::from_circuit_with_limits(&instrs, permissive_limits()).unwrap();
    assert_eq!(layout.expanded_instructions(), 4);
    assert_eq!(layout.parity_terms(), 2);

    let transform = MeasurementTransform::from_circuit_with_limits(&instrs, permissive_limits())
        .expect("accessor transform");
    assert_eq!(transform.reference_bits(), &[true]);

    let err = transform
        .encode_block(&BitTable::try_new(0, 1).unwrap())
        .unwrap_err();
    assert!(matches!(
        err,
        MeasurementTransformError::ShapeMismatch { .. }
    ));
    assert!(err
        .to_string()
        .contains("measurement rows 0 do not match transform measurements 1"));

    let zero_shot_measurements = BitTable::try_new(1, 0).unwrap();
    let zero_shot_encoded = transform.encode_block(&zero_shot_measurements).unwrap();
    let zero_shot_decoded = transform.decode_block(&zero_shot_encoded).unwrap();
    assert_eq!(zero_shot_decoded.measurements.num_minor(), 0);

    assert_limit_guard(
        "direct_max_repeat_depth",
        "REPEAT 1 {\n  M 0\n}\n",
        |limits| limits.max_repeat_depth = 0,
        |limits| limits.max_repeat_depth = 1,
    );
    assert_limit_guard(
        "direct_max_expanded_instructions",
        "M 0\n",
        |limits| limits.max_expanded_instructions = 0,
        |limits| limits.max_expanded_instructions = 1,
    );
    assert_limit_guard(
        "repeat_preflight_max_measurements",
        "REPEAT 2 {\n  M 0\n}\n",
        |limits| limits.max_measurements = 1,
        |limits| limits.max_measurements = 2,
    );
    assert_limit_guard(
        "repeat_preflight_max_detectors",
        "M 0\nREPEAT 2 {\n  DETECTOR rec[-1]\n}\n",
        |limits| limits.max_detectors = 1,
        |limits| limits.max_detectors = 2,
    );
    assert_limit_guard(
        "repeat_preflight_max_observables",
        "M 0\nREPEAT 1 {\n  OBSERVABLE_INCLUDE(1) rec[-1]\n}\n",
        |limits| limits.max_observables = 1,
        |limits| limits.max_observables = 2,
    );
    assert_limit_guard(
        "repeat_preflight_max_parity_terms",
        "M 0\nREPEAT 2 {\n  DETECTOR rec[-1]\n}\n",
        |limits| limits.max_parity_terms = 1,
        |limits| limits.max_parity_terms = 2,
    );

    let invalid_observable = parse_lines("M 0\nOBSERVABLE_INCLUDE(nan) rec[-1]\n").unwrap();
    assert!(matches!(
        MeasurementTransform::from_circuit_with_limits(&invalid_observable, permissive_limits()),
        Err(MeasurementTransformError::InvalidRecordTarget { .. })
    ));
}

fn verify_additional_valid_semantics() -> usize {
    let nested = parse_lines("REPEAT 2 {\n  REPEAT 2 {\n    M 0\n    DETECTOR rec[-1]\n  }\n}\n")
        .expect("nested repeat parses");
    let transform = MeasurementTransform::from_circuit(&nested).expect("nested repeat transform");
    assert_eq!(transform.num_measurements(), 4);
    assert_eq!(transform.num_detectors(), 4);
    assert_eq!(transform.rank(), 4);
    let measurements = patterned_table(transform.num_measurements(), 9, b"nested-repeat");
    assert_round_trip_matches_m2d(&nested, &transform, &measurements);

    let full_rank = parse_lines("M 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n")
        .expect("full rank parses");
    let transform = MeasurementTransform::from_circuit(&full_rank).expect("full rank transform");
    assert_eq!(transform.rank(), transform.num_measurements());
    assert!(transform.free_columns().is_empty());
    let measurements = patterned_table(transform.num_measurements(), 8, b"full-rank");
    assert_round_trip_matches_m2d(&full_rank, &transform, &measurements);

    let producers = parse_lines(
        "MXX 0 1\nMYY 2 3\nMZZ 4 5\nMPAD 0 1\nHERALDED_ERASE(0.25) 6\nHERALDED_PAULI_CHANNEL_1(0.1,0.2,0.3,0.4) 7\nML 8\n",
    )
    .expect("producer coverage circuit parses");
    let transform =
        MeasurementTransform::from_circuit(&producers).expect("producer coverage transform");
    assert_eq!(transform.num_measurements(), 9);
    assert_eq!(transform.num_detectors(), 0);
    assert_eq!(transform.rank(), 0);
    let measurements = patterned_table(transform.num_measurements(), 7, b"producers");
    assert_round_trip_matches_m2d(&producers, &transform, &measurements);

    0
}

fn verify_property_cases() -> usize {
    let mut rng = StdRng::seed_from_u64(PROPERTY_SEED);
    for case_index in 0..PROPERTY_CASES {
        let generated = generated_property_case(&mut rng, case_index);
        let instrs = parse_lines(&generated.circuit).expect("property circuit parses");
        let transform = MeasurementTransform::from_circuit(&instrs).expect("property transform");
        let (oracle_selected, oracle_pivots, oracle_free) =
            oracle_highest_pivot_elimination(generated.measurement_count, &generated.detectors);
        assert_eq!(
            transform.selected_detector_rows(),
            oracle_selected.as_slice()
        );
        assert_eq!(transform.pivot_columns(), oracle_pivots.as_slice());
        assert_eq!(transform.free_columns(), oracle_free.as_slice());
        assert_eq!(transform.rank(), oracle_pivots.len());

        let shots = SHOT_COUNTS[case_index % SHOT_COUNTS.len()];
        let measurements = random_table(&mut rng, generated.measurement_count, shots);
        assert_round_trip_matches_m2d(&instrs, &transform, &measurements);
    }
    PROPERTY_CASES
}

fn verify_negative_controls() {
    assert!(matches!(
        MeasurementTransform::from_circuit(&parse_lines("DETECTOR rec[-1]\n").unwrap()),
        Err(MeasurementTransformError::InvalidRecordTarget { .. })
    ));
    assert!(matches!(
        MeasurementTransform::from_circuit(&parse_lines("M 0\nDETECTOR 0\n").unwrap()),
        Err(MeasurementTransformError::InvalidRecordTarget { .. })
    ));
    assert!(matches!(
        MeasurementTransform::from_circuit(&parse_lines("CX sweep[0] 0\nM 0\n").unwrap()),
        Err(MeasurementTransformError::UnsupportedSweep)
    ));

    let instrs =
        parse_lines("M 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n").unwrap();
    let transform = MeasurementTransform::from_circuit(&instrs).unwrap();
    let mut measurements = BitTable::try_new(3, 1).unwrap();
    measurements.set(0, 0, true);
    measurements.set(2, 0, true);
    let encoded = transform.encode_block(&measurements).unwrap();

    let wrong_selected = EncodedMeasurementBlock {
        selected_detectors: BitTable::try_new(1, 1).unwrap(),
        free_measurements: encoded.free_measurements.clone(),
    };
    assert!(matches!(
        transform.decode_block(&wrong_selected),
        Err(MeasurementTransformError::ShapeMismatch { .. })
    ));

    let wrong_free = EncodedMeasurementBlock {
        selected_detectors: encoded.selected_detectors.clone(),
        free_measurements: BitTable::try_new(2, 1).unwrap(),
    };
    assert!(matches!(
        transform.decode_block(&wrong_free),
        Err(MeasurementTransformError::ShapeMismatch { .. })
    ));

    let shot_mismatch = EncodedMeasurementBlock {
        selected_detectors: BitTable::try_new(2, 2).unwrap(),
        free_measurements: BitTable::try_new(1, 1).unwrap(),
    };
    assert!(matches!(
        transform.decode_block(&shot_mismatch),
        Err(MeasurementTransformError::ShapeMismatch { .. })
    ));

    let mut changed_free = encoded.clone();
    changed_free.free_measurements.toggle(0, 0);
    let decoded_changed_free = transform.decode_block(&changed_free).unwrap();
    assert_ne!(
        table_bits(&measurements),
        table_bits(&decoded_changed_free.measurements)
    );

    let mut changed_selected = encoded.clone();
    changed_selected.selected_detectors.toggle(0, 0);
    let decoded_changed_selected = transform.decode_block(&changed_selected).unwrap();
    assert_ne!(
        table_bits(&measurements),
        table_bits(&decoded_changed_selected.measurements)
    );
    assert_ne!(
        encoded.selected_detectors.get(0, 0),
        decoded_changed_selected
            .detections
            .get(transform.selected_detector_rows()[0], 0)
    );
}

fn verify_allocation_guards() -> usize {
    let mut guards = 0;

    let round_up = std::panic::catch_unwind(|| {
        let zero = BitTable::try_new(0, 0).unwrap();
        assert_eq!(zero.words_per_row(), 0);
        let one = BitTable::try_new(1, 1).unwrap();
        assert_eq!(one.words_per_row(), 1);
        let sixty_four = BitTable::try_new(1, 64).unwrap();
        assert_eq!(sixty_four.words_per_row(), 1);
        let sixty_five = BitTable::try_new(1, 65).unwrap();
        assert_eq!(sixty_five.words_per_row(), 2);
    });
    assert!(round_up.is_ok());
    assert!(matches!(
        BitTable::try_new(1, usize::MAX),
        Err(BitTableAllocError::SizeOverflow)
    ));
    guards += 1;

    let total_overflow = std::panic::catch_unwind(|| BitTable::try_new(usize::MAX, 65));
    assert!(matches!(
        total_overflow,
        Ok(Err(BitTableAllocError::SizeOverflow))
    ));
    guards += 1;

    let unrepresentable_capacity =
        std::panic::catch_unwind(|| BitTable::try_new(isize::MAX as usize / 8 + 1, 1));
    assert!(matches!(
        unrepresentable_capacity,
        Ok(Err(BitTableAllocError::ReservationFailed))
    ));
    guards += 1;

    assert_limit_guard(
        "max_measurements",
        "M 0 1\n",
        |limits| limits.max_measurements = 1,
        |limits| limits.max_measurements = 2,
    );
    guards += 1;

    assert_limit_guard(
        "max_detectors",
        "M 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n",
        |limits| limits.max_detectors = 1,
        |limits| limits.max_detectors = 2,
    );
    guards += 1;

    assert_limit_guard(
        "max_observables",
        "M 0\nOBSERVABLE_INCLUDE(1) rec[-1]\n",
        |limits| limits.max_observables = 1,
        |limits| limits.max_observables = 2,
    );
    guards += 1;

    assert_limit_guard(
        "max_repeat_depth",
        "REPEAT 1 {\n  REPEAT 1 {\n    M 0\n  }\n}\n",
        |limits| limits.max_repeat_depth = 1,
        |limits| limits.max_repeat_depth = 2,
    );
    guards += 1;

    assert_limit_guard(
        "max_expanded_instructions",
        "REPEAT 2 {\n  M 0\n}\n",
        |limits| limits.max_expanded_instructions = 1,
        |limits| limits.max_expanded_instructions = 2,
    );
    guards += 1;

    assert_limit_guard(
        "max_parity_terms",
        "M 0 1\nDETECTOR rec[-2] rec[-1]\n",
        |limits| limits.max_parity_terms = 1,
        |limits| limits.max_parity_terms = 2,
    );
    guards += 1;

    let mut limits = permissive_limits();
    limits.max_shots_per_block = 1;
    let transform =
        MeasurementTransform::from_circuit_with_limits(&parse_lines("M 0\n").unwrap(), limits)
            .unwrap();
    let one_shot = BitTable::try_new(1, 1).unwrap();
    let two_shots = BitTable::try_new(1, 2).unwrap();
    let encoded = transform.encode_block(&one_shot).unwrap();
    assert!(matches!(
        transform.encode_block(&two_shots),
        Err(MeasurementTransformError::LimitExceeded { .. })
    ));
    let two_shot_encoded = EncodedMeasurementBlock {
        selected_detectors: BitTable::try_new(0, 2).unwrap(),
        free_measurements: BitTable::try_new(1, 2).unwrap(),
    };
    assert!(matches!(
        transform.decode_block(&two_shot_encoded),
        Err(MeasurementTransformError::LimitExceeded { .. })
    ));
    transform.decode_block(&encoded).unwrap();
    guards += 1;

    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\n").unwrap();
    let mut pre_layout_low_limits = permissive_limits();
    pre_layout_low_limits.max_transform_working_bytes = 0;
    assert!(matches!(
        MeasurementTransform::from_circuit_with_limits(&instrs, pre_layout_low_limits),
        Err(MeasurementTransformError::LimitExceeded { .. })
    ));
    let high =
        MeasurementTransform::from_circuit_with_limits(&instrs, permissive_limits()).unwrap();
    let exact_bytes = high.transform_working_bytes();
    let mut exact_limits = permissive_limits();
    exact_limits.max_transform_working_bytes = exact_bytes;
    MeasurementTransform::from_circuit_with_limits(&instrs, exact_limits).unwrap();
    let mut low_limits = permissive_limits();
    low_limits.max_transform_working_bytes = exact_bytes.saturating_sub(1);
    assert!(matches!(
        MeasurementTransform::from_circuit_with_limits(&instrs, low_limits),
        Err(MeasurementTransformError::LimitExceeded { .. })
    ));
    guards += 1;

    let exact_block_bytes = high.estimate_block_working_bytes(1).unwrap();
    let mut exact_block_limits = permissive_limits();
    exact_block_limits.max_block_working_bytes = exact_block_bytes;
    let exact_block_transform =
        MeasurementTransform::from_circuit_with_limits(&instrs, exact_block_limits).unwrap();
    exact_block_transform
        .encode_block(&BitTable::try_new(1, 1).unwrap())
        .unwrap();
    let mut low_block_limits = permissive_limits();
    low_block_limits.max_block_working_bytes = exact_block_bytes.saturating_sub(1);
    let low_block_transform =
        MeasurementTransform::from_circuit_with_limits(&instrs, low_block_limits).unwrap();
    assert!(matches!(
        low_block_transform.encode_block(&BitTable::try_new(1, 1).unwrap()),
        Err(MeasurementTransformError::LimitExceeded { .. })
    ));
    guards += 1;

    guards
}

fn verify_surface_benchmark_rank() -> usize {
    let circuit = fs::read_to_string(repo_path(
        "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
    ))
    .expect("surface fixture");
    let instrs = parse_lines(&circuit).expect("surface fixture parses");
    let transform = MeasurementTransform::from_circuit(&instrs).expect("surface transform");
    assert_eq!(transform.num_measurements(), 12121);
    assert_eq!(transform.num_detectors(), 12000);
    assert_eq!(transform.num_observables(), 1);
    assert_eq!(transform.rank(), 12000);
    assert_eq!(transform.free_columns().len(), 121);
    transform.rank()
}

fn assert_limit_guard(
    limit_name: &'static str,
    circuit: &str,
    make_fail: impl FnOnce(&mut MeasurementTransformLimits),
    make_success: impl FnOnce(&mut MeasurementTransformLimits),
) {
    let instrs = parse_lines(circuit).unwrap();
    let mut fail_limits = permissive_limits();
    make_fail(&mut fail_limits);
    let err = MeasurementTransform::from_circuit_with_limits(&instrs, fail_limits).unwrap_err();
    assert!(
        matches!(err, MeasurementTransformError::LimitExceeded { .. }),
        "{limit_name}: {err:?}"
    );

    let mut success_limits = permissive_limits();
    make_success(&mut success_limits);
    MeasurementTransform::from_circuit_with_limits(&instrs, success_limits)
        .unwrap_or_else(|err| panic!("{limit_name}: at-limit success control failed: {err}"));
}

fn assert_round_trip_matches_m2d(
    instrs: &[StimInstr],
    transform: &MeasurementTransform,
    measurements: &BitTable,
) {
    let encoded = transform.encode_block(measurements).expect("encode");
    let decoded = transform.decode_block(&encoded).expect("decode");
    assert_tables_eq(
        measurements,
        &decoded.measurements,
        "round-trip",
        "measurements",
    );
    let m2d = measurements_to_detections(instrs, measurements).expect("m2d consistency");
    assert_tables_eq(
        &m2d.detections,
        &decoded.detections,
        "round-trip",
        "detections",
    );
    assert_tables_eq(
        &m2d.observable_flips,
        &decoded.observable_flips,
        "round-trip",
        "observables",
    );
}

fn assert_catalog_dimensions(case: &Value, transform: &MeasurementTransform) {
    let id = case["id"].as_str().expect("case id");
    assert_eq!(
        transform.num_measurements() as u64,
        case["measurement_count"]
            .as_u64()
            .expect("measurement_count"),
        "{id}: measurement_count"
    );
    assert_eq!(
        transform.num_detectors() as u64,
        case["detector_count"].as_u64().expect("detector_count"),
        "{id}: detector_count"
    );
    assert_eq!(
        transform.num_observables() as u64,
        case["observable_count"].as_u64().expect("observable_count"),
        "{id}: observable_count"
    );
    assert_eq!(
        transform.rank() as u64,
        case["rank_H"].as_u64().expect("rank_H"),
        "{id}: rank_H"
    );
}

fn assert_identity_uses_canonical_circuit_sha(
    instrs: &[StimInstr],
    transform: &MeasurementTransform,
) {
    let canonical = circuit_to_string(instrs);
    let expected: [u8; 32] = Sha256::digest(canonical.as_bytes()).into();
    assert_eq!(transform.identity().circuit_sha256, expected);
}

fn assert_catalog_hashes_loaded_from_files(case: &Value) {
    let expected_files = case["expected_files"]
        .as_object()
        .expect("known answer expected_files");
    for (name, file) in expected_files {
        let path = file["path"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: path"));
        let bytes = fs::read(repo_path(path)).unwrap_or_else(|err| panic!("{path}: {err}"));
        let digest = hex(&Sha256::digest(&bytes));
        assert_eq!(
            digest,
            file["sha256"].as_str().expect("expected sha"),
            "{path}"
        );
    }
}

fn read_expected_table(case: &Value, key: &str, shots: usize) -> BitTable {
    let file = &case["expected_files"][key];
    let path = file["path"]
        .as_str()
        .unwrap_or_else(|| panic!("{key}: path"));
    let bit_count = file["bit_count"]
        .as_u64()
        .unwrap_or_else(|| panic!("{key}: bit_count")) as usize;
    let bytes = fs::read(repo_path(path)).unwrap_or_else(|err| panic!("{path}: {err}"));
    let digest = hex(&Sha256::digest(&bytes));
    assert_eq!(
        digest,
        file["sha256"].as_str().expect("expected sha"),
        "{path}"
    );
    let table = read_shots_b8(&bytes, bit_count).unwrap_or_else(|err| panic!("{path}: {err}"));
    assert_eq!(table.num_minor(), shots, "{path}: shots");
    table
}

fn assert_tables_eq(left: &BitTable, right: &BitTable, case: &str, label: &str) {
    assert_eq!(left.num_major(), right.num_major(), "{case}: {label} rows");
    assert_eq!(left.num_minor(), right.num_minor(), "{case}: {label} shots");
    for row in 0..left.num_major() {
        for shot in 0..left.num_minor() {
            assert_eq!(
                left.get(row, shot),
                right.get(row, shot),
                "{case}: {label}[{row},{shot}]"
            );
        }
    }
    let mut left_bytes = Vec::new();
    let mut right_bytes = Vec::new();
    write_shots_b8(left, &mut left_bytes).unwrap();
    write_shots_b8(right, &mut right_bytes).unwrap();
    assert_eq!(
        hex(&Sha256::digest(&left_bytes)),
        hex(&Sha256::digest(&right_bytes))
    );
}

fn table_bits(table: &BitTable) -> Vec<bool> {
    let mut out = Vec::new();
    for row in 0..table.num_major() {
        for shot in 0..table.num_minor() {
            out.push(table.get(row, shot));
        }
    }
    out
}

fn patterned_table(bits: usize, shots: usize, salt: &[u8]) -> BitTable {
    let mut table = BitTable::try_new(bits, shots).expect("patterned table allocates");
    for bit in 0..bits {
        for shot in 0..shots {
            let salt_bit = salt[(bit + shot) % salt.len()] & 1 == 1;
            if ((bit * 17 + shot * 31 + salt.len()) & 1 == 1) ^ salt_bit {
                table.set(bit, shot, true);
            }
        }
    }
    table
}

fn random_table(rng: &mut StdRng, bits: usize, shots: usize) -> BitTable {
    let mut table = BitTable::try_new(bits, shots).expect("random table allocates");
    for bit in 0..bits {
        for shot in 0..shots {
            if rng.r#gen::<bool>() {
                table.set(bit, shot, true);
            }
        }
    }
    table
}

struct GeneratedPropertyCase {
    circuit: String,
    measurement_count: usize,
    detectors: Vec<Vec<usize>>,
}

fn generated_property_case(rng: &mut StdRng, case_index: usize) -> GeneratedPropertyCase {
    let measurement_count = 1 + rng.gen_range(0..8);
    let detector_count: usize = rng.gen_range(0..=measurement_count + 3);
    let mut circuit = String::new();
    circuit.push('M');
    for q in 0..measurement_count {
        circuit.push_str(&format!(" {q}"));
    }
    circuit.push('\n');

    let mut detectors = Vec::new();
    for d in 0..detector_count {
        let mut row = Vec::new();
        let term_count = rng.gen_range(0..=measurement_count + 2);
        circuit.push_str("DETECTOR");
        for _ in 0..term_count {
            let abs = rng.gen_range(0..measurement_count);
            row.push(abs);
            let lookback = measurement_count - abs;
            circuit.push_str(&format!(" rec[-{lookback}]"));
        }
        if case_index % 17 == 0 && d == detector_count.saturating_sub(1) {
            for abs in 0..measurement_count {
                row.push(abs);
                let lookback = measurement_count - abs;
                circuit.push_str(&format!(" rec[-{lookback}]"));
            }
        }
        circuit.push('\n');
        detectors.push(normalize_terms(row));
    }
    if case_index % 5 == 0 {
        circuit.push_str("OBSERVABLE_INCLUDE(0)");
        for abs in 0..measurement_count {
            if abs % 2 == case_index % 2 {
                let lookback = measurement_count - abs;
                circuit.push_str(&format!(" rec[-{lookback}]"));
            }
        }
        circuit.push('\n');
    }

    GeneratedPropertyCase {
        circuit,
        measurement_count,
        detectors,
    }
}

fn oracle_highest_pivot_elimination(
    measurement_count: usize,
    detector_rows: &[Vec<usize>],
) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let mut selected = Vec::new();
    let mut pivots = Vec::new();
    let mut equations: Vec<Vec<bool>> = Vec::new();
    for (detector_index, detector_row) in detector_rows.iter().enumerate() {
        let mut row = vec![false; measurement_count];
        for &term in detector_row {
            row[term] ^= true;
        }
        for (equation, &pivot) in equations.iter().zip(pivots.iter()) {
            if row[pivot] {
                for (dst, &src) in row.iter_mut().zip(equation.iter()) {
                    *dst ^= src;
                }
            }
        }
        if let Some(pivot) = (0..measurement_count).rev().find(|&col| row[col]) {
            selected.push(detector_index);
            pivots.push(pivot);
            equations.push(row);
        }
    }
    let pivot_set: BTreeSet<usize> = pivots.iter().copied().collect();
    let free = (0..measurement_count)
        .filter(|col| !pivot_set.contains(col))
        .collect();
    (selected, pivots, free)
}

fn normalize_terms(mut terms: Vec<usize>) -> Vec<usize> {
    terms.sort_unstable();
    let mut out = Vec::new();
    let mut i = 0;
    while i < terms.len() {
        let value = terms[i];
        let mut count = 0;
        while i < terms.len() && terms[i] == value {
            count += 1;
            i += 1;
        }
        if count % 2 == 1 {
            out.push(value);
        }
    }
    out
}

fn has_role(case: &Value, role: &str) -> bool {
    case["semantic_roles"]
        .as_array()
        .expect("semantic_roles")
        .iter()
        .any(|value| value.as_str() == Some(role))
}

fn read_case_circuit(case: &Value) -> String {
    let path = case["circuit_path"].as_str().expect("circuit_path");
    fs::read_to_string(repo_path(path)).unwrap_or_else(|err| panic!("{path}: {err}"))
}

fn load_catalog() -> Value {
    let path = repo_path("rstim/tests/fixtures/rsmp/catalog.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).expect("catalog json")
}

fn repo_path(rel: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(rel)
}

fn permissive_limits() -> MeasurementTransformLimits {
    MeasurementTransformLimits {
        max_measurements: 1_000_000,
        max_detectors: 1_000_000,
        max_observables: 1_000_000,
        max_repeat_depth: 1_000,
        max_expanded_instructions: 10_000_000,
        max_parity_terms: 10_000_000,
        max_shots_per_block: 4096,
        max_transform_working_bytes: 1 << 30,
        max_block_working_bytes: 1 << 30,
    }
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
