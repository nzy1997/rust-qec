use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::CompiledLossMeasurementSampler;
use rstim::data_path::ReferenceSampleMode;
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};

const ATOM_LOSS_FIXTURE: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
);

#[test]
fn compiled_loss_sampler_reuses_its_plan_and_reference() {
    let circuit = parse_lines("X 0\nLOSS(1) 0\nCX 0 2\nM 0 1 2 3\n").unwrap();
    let mut sampler =
        CompiledLossMeasurementSampler::compile(&circuit, ReferenceSampleMode::SimulateNoiseless)
            .unwrap();
    let mut rng = StdRng::seed_from_u64(0xc0ffee);

    for _ in 0..2 {
        let output = sampler
            .sample(64, &mut rng, SampleOutputMode::MeasurementsOnly)
            .unwrap();
        for shot in 0..64 {
            assert!(output.measurements.get(0, shot));
            assert!(!output.measurements.get(2, shot));
        }
    }

    let diagnostics = sampler.diagnostics();
    assert_eq!(diagnostics.compiled_ir_builds, 1);
    assert_eq!(diagnostics.reference_builds, 1);
    assert_eq!(diagnostics.sample_calls, 2);
}

#[test]
fn loss_before_cx_skips_the_gate_for_interpreted_and_auto_sampling() {
    let circuit = parse_lines("X 0\nLOSS(1) 0\nCX 0 2\nM 0 1 2 3\n").unwrap();

    for backend in [SamplingBackend::Interpreted, SamplingBackend::Auto] {
        let mut rng = StdRng::seed_from_u64(0xc0ffee);
        let output = sample_batch_with_options(
            &circuit,
            64,
            &mut rng,
            SampleOptions {
                reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
                backend,
                output_mode: SampleOutputMode::MeasurementsOnly,
            },
        )
        .unwrap();

        for shot in 0..64 {
            assert!(output.measurements.get(0, shot), "lost q0 reads as one");
            assert!(!output.measurements.get(1, shot), "untouched q1 stays zero");
            assert!(
                !output.measurements.get(2, shot),
                "q2 stays zero because CX is skipped when q0 is lost"
            );
            assert!(!output.measurements.get(3, shot), "untouched q3 stays zero");
        }
    }
}

#[test]
fn atom_loss_fixture_conditional_tableau_batch_has_the_declared_shape() {
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
fn safe_bit_parallel_loss_marginals_and_correlations_match_legacy_executor() {
    const SHOTS: usize = 20_000;
    const TOLERANCE: f64 = 0.025;
    let circuit = parse_lines(
        "R 0 1 2\n\
         H 0\n\
         CX 0 1 1 2\n\
         DEPOLARIZE1(0.12) 0 1 2\n\
         DEPOLARIZE2(0.18) 0 1 1 2\n\
         LOSS(0.2) 0 1 2\n\
         M 0 1\n\
         MR 2\n\
         M 2\n",
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

#[test]
fn conditional_tableau_loss_marginals_and_correlations_match_legacy_executor() {
    const SHOTS: usize = 20_000;
    const TOLERANCE: f64 = 0.025;
    let circuit = parse_lines(
        "R 0 1 2\n\
         H 0\n\
         CX 0 1 1 2\n\
         DEPOLARIZE1(0.12) 0 1 2\n\
         DEPOLARIZE2(0.18) 0 1 1 2\n\
         LOSS(0.2) 0 1 2\n\
         CX 0 1 1 2\n\
         M 0 1\n\
         MR 2\n\
         M 2\n",
    )
    .unwrap();
    assert_sampling_matches_legacy(circuit, SHOTS, TOLERANCE);
}

#[test]
fn conditional_css_frame_handles_measurement_followed_by_more_cliffords() {
    let circuit = parse_lines(
        "R 0 1 2\n\
         H 0\n\
         CX 0 1\n\
         M 0\n\
         H 0\n\
         LOSS(0.2) 1\n\
         CX 1 2\n\
         M 0 1 2\n",
    )
    .unwrap();
    assert_sampling_matches_legacy(circuit, 20_000, 0.025);
}

fn assert_sampling_matches_legacy(
    circuit: Vec<rstim::ir::StimInstr>,
    shots: usize,
    tolerance: f64,
) {
    let width = rstim::stats::num_measurements(&circuit);

    let mut optimized_rng = StdRng::seed_from_u64(0xc01d_17a1);
    let optimized = sample_batch_with_options(
        &circuit,
        shots,
        &mut optimized_rng,
        SampleOptions {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
        },
    )
    .unwrap();

    let mut legacy_rng = StdRng::seed_from_u64(0x1e9a_c7);
    let mut executor = Executor::from_instrs(circuit).unwrap();
    let mut legacy_rows = Vec::with_capacity(shots);
    for _ in 0..shots {
        legacy_rows.push(executor.run(&mut legacy_rng).unwrap().measurements);
    }

    for first in 0..width {
        let optimized_rate = (0..shots)
            .filter(|&shot| optimized.measurements.get(first, shot))
            .count() as f64
            / shots as f64;
        let legacy_rate = legacy_rows.iter().filter(|row| row[first]).count() as f64 / shots as f64;
        assert_rate_close(optimized_rate, legacy_rate, tolerance, first, None);

        for second in first + 1..width {
            let optimized_xor_rate = (0..shots)
                .filter(|&shot| {
                    optimized.measurements.get(first, shot)
                        ^ optimized.measurements.get(second, shot)
                })
                .count() as f64
                / shots as f64;
            let legacy_xor_rate = legacy_rows
                .iter()
                .filter(|row| row[first] ^ row[second])
                .count() as f64
                / shots as f64;
            assert_rate_close(
                optimized_xor_rate,
                legacy_xor_rate,
                tolerance,
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
