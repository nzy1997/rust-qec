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

fn true_fraction(rows: &[Vec<bool>]) -> f64 {
    let ones = rows.iter().flatten().filter(|&&bit| bit).count();
    let total: usize = rows.iter().map(Vec::len).sum();
    ones as f64 / total as f64
}

#[test]
fn compiled_backend_matches_interpreted_for_repeat_circuit() {
    let instrs = parse_lines(
        "REPEAT 32 {\n  X_ERROR(0) 0\n  M 0\n  DETECTOR rec[-1]\n  OBSERVABLE_INCLUDE(0) rec[-1]\n}\n",
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
fn compiled_backend_samples_repeat_noise_distribution() {
    let instrs = parse_lines(
        "REPEAT 16 {\n  X_ERROR(0.1) 0\n  M 0\n  DETECTOR rec[-1]\n  OBSERVABLE_INCLUDE(0) rec[-1]\n}\n",
    )
    .unwrap();
    let mut rng = StdRng::seed_from_u64(7);

    let compiled = sample_batch_with_options(
        &instrs,
        512,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    let measurements = bit_table_rows(&compiled.measurements);
    let detections = bit_table_rows(&compiled.detections);
    let measurement_rate = true_fraction(&measurements);
    assert_eq!(detections, measurements);
    assert!(
        (0.06..=0.14).contains(&measurement_rate),
        "compiled repeat X_ERROR(0.1) measurement rate was {measurement_rate}"
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
