use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::data_path::ReferenceSampleMode;
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};

const ATOM_LOSS_FIXTURE: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
);

#[test]
fn optimized_atom_loss_fixture_has_the_declared_batch_shape() {
    let circuit = parse_lines(ATOM_LOSS_FIXTURE).expect("atom-loss fixture parses");
    let mut rng = StdRng::seed_from_u64(7);
    let optimized = sample_batch_with_options(
        &circuit,
        2,
        &mut rng,
        SampleOptions {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
        },
    )
    .expect("optimized atom-loss sample succeeds");

    assert_eq!(optimized.measurements.num_major(), 12_121);
    assert_eq!(optimized.measurements.num_minor(), 2);
}

#[test]
fn optimized_loss_marginals_and_correlations_match_legacy_executor() {
    const SHOTS: usize = 20_000;
    const TOLERANCE: f64 = 0.025;
    let circuit = parse_lines(
        "R 0 1 2\n\
         REPEAT 3 {\n\
             H 0\n\
             CX 0 1 1 2\n\
             DEPOLARIZE1(0.12) 0 1 2\n\
             DEPOLARIZE2(0.18) 0 1 1 2\n\
             LOSS(0.2) 0 1 2\n\
             MR 1 2\n\
         }\n\
         M 0 1 2\n",
    )
    .unwrap();
    let width = rstim::stats::num_measurements(&circuit);

    let mut optimized_rng = StdRng::seed_from_u64(0xa70_1055);
    let optimized = sample_batch_with_options(
        &circuit,
        SHOTS,
        &mut optimized_rng,
        SampleOptions {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
        },
    )
    .unwrap();

    let mut legacy_rng = StdRng::seed_from_u64(0x1e9ac7);
    let mut executor = Executor::from_instrs(circuit).unwrap();
    let mut legacy_rows = Vec::with_capacity(SHOTS);
    for _ in 0..SHOTS {
        legacy_rows.push(executor.run(&mut legacy_rng).unwrap().measurements);
    }

    for first in 0..width {
        let optimized_rate = (0..SHOTS)
            .filter(|&shot| optimized.measurements.get(first, shot))
            .count() as f64
            / SHOTS as f64;
        let legacy_rate = legacy_rows.iter().filter(|row| row[first]).count() as f64 / SHOTS as f64;
        assert_rate_close(optimized_rate, legacy_rate, TOLERANCE, first, None);

        for second in first + 1..width {
            let optimized_xor_rate = (0..SHOTS)
                .filter(|&shot| {
                    optimized.measurements.get(first, shot)
                        ^ optimized.measurements.get(second, shot)
                })
                .count() as f64
                / SHOTS as f64;
            let legacy_xor_rate = legacy_rows
                .iter()
                .filter(|row| row[first] ^ row[second])
                .count() as f64
                / SHOTS as f64;
            assert_rate_close(
                optimized_xor_rate,
                legacy_xor_rate,
                TOLERANCE,
                first,
                Some(second),
            );
        }
    }
}

fn assert_rate_close(
    optimized: f64,
    legacy: f64,
    tolerance: f64,
    first: usize,
    second: Option<usize>,
) {
    assert!(
        (optimized - legacy).abs() <= tolerance,
        "rate mismatch for ({first}, {second:?}): optimized={optimized}, legacy={legacy}",
    );
}

#[test]
fn optimized_atom_loss_batch_is_repeatable_for_a_fixed_seed() {
    let circuit =
        parse_lines("R 0 1\nH 0\nCX 0 1\nDEPOLARIZE2(0.1) 0 1\nLOSS(0.2) 0 1\nM 0 1\n").unwrap();
    let options = SampleOptions {
        reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
        backend: SamplingBackend::Interpreted,
        output_mode: SampleOutputMode::MeasurementsOnly,
    };
    let mut first_rng = StdRng::seed_from_u64(0x5eed);
    let mut second_rng = StdRng::seed_from_u64(0x5eed);
    let first = sample_batch_with_options(&circuit, 257, &mut first_rng, options).unwrap();
    let second = sample_batch_with_options(&circuit, 257, &mut second_rng, options).unwrap();

    for measurement in 0..first.measurements.num_major() {
        for shot in 0..first.measurements.num_minor() {
            assert_eq!(
                first.measurements.get(measurement, shot),
                second.measurements.get(measurement, shot),
            );
        }
    }
}
