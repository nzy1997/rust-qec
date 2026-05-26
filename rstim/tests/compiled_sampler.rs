use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch_with_options, SampleOptions, SamplingBackend};
use rstim::sim::bit_table::BitTable;

fn bit_table_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|major| {
            (0..table.num_minor())
                .map(|minor| table.get(major, minor))
                .collect()
        })
        .collect()
}

#[test]
fn compiled_backend_matches_interpreted_for_repeat_circuit() {
    let instrs = parse_lines(
        "REPEAT 32 {\n  X_ERROR(0.001) 0\n  M 0\n  DETECTOR rec[-1]\n  OBSERVABLE_INCLUDE(0) rec[-1]\n}\n",
    )
    .unwrap();

    let mut interpreted_rng = StdRng::seed_from_u64(7);
    let mut compiled_rng = StdRng::seed_from_u64(7);

    let interpreted = sample_batch_with_options(
        &instrs,
        16,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )
    .unwrap();
    let compiled = sample_batch_with_options(
        &instrs,
        16,
        &mut compiled_rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    assert_eq!(
        bit_table_rows(&compiled.measurements),
        bit_table_rows(&interpreted.measurements)
    );
    assert_eq!(
        bit_table_rows(&compiled.detections),
        bit_table_rows(&interpreted.detections)
    );
    assert_eq!(
        bit_table_rows(&compiled.observable_flips),
        bit_table_rows(&interpreted.observable_flips)
    );
}

#[test]
fn compiled_backend_rejects_loss_circuits_with_routing_reason() {
    let instrs = parse_lines("LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(13);

    let err = sample_batch_with_options(
        &instrs,
        16,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .err()
    .expect("compiled backend should reject loss circuits");

    assert_eq!(err, "loss instructions require the interpreted path");
}

#[test]
fn compiled_backend_rejects_feedback_circuits_with_routing_reason() {
    let instrs = parse_lines("M 0\nCX rec[-1] 0\n").unwrap();
    let mut rng = StdRng::seed_from_u64(17);

    let err = sample_batch_with_options(
        &instrs,
        16,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .err()
    .expect("compiled backend should reject feedback circuits");

    assert_eq!(err, "feedback instructions require the interpreted path");
}

#[test]
fn auto_backend_keeps_loss_circuit_on_interpreted_path() {
    let instrs = parse_lines("LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n").unwrap();

    let mut auto_rng = StdRng::seed_from_u64(11);
    let mut interpreted_rng = StdRng::seed_from_u64(11);

    let auto = sample_batch_with_options(
        &instrs,
        16,
        &mut auto_rng,
        SampleOptions {
            backend: SamplingBackend::Auto,
            ..SampleOptions::default()
        },
    )
    .unwrap();
    let interpreted = sample_batch_with_options(
        &instrs,
        16,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    assert_eq!(
        bit_table_rows(&auto.measurements),
        bit_table_rows(&interpreted.measurements)
    );
    assert_eq!(
        bit_table_rows(&auto.detections),
        bit_table_rows(&interpreted.detections)
    );
    assert_eq!(
        bit_table_rows(&auto.observable_flips),
        bit_table_rows(&interpreted.observable_flips)
    );
}

#[test]
fn compiled_backend_matches_interpreted_for_observable_with_nonzero_reference_sample() {
    let instrs = parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();

    let mut interpreted_rng = StdRng::seed_from_u64(19);
    let mut compiled_rng = StdRng::seed_from_u64(19);

    let interpreted = sample_batch_with_options(
        &instrs,
        16,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )
    .unwrap();
    let compiled = sample_batch_with_options(
        &instrs,
        16,
        &mut compiled_rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    assert_eq!(
        bit_table_rows(&compiled.measurements),
        bit_table_rows(&interpreted.measurements)
    );
    assert_eq!(
        bit_table_rows(&compiled.observable_flips),
        bit_table_rows(&interpreted.observable_flips)
    );
}
