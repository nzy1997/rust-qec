use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::executor::{Executor, reference_sample};
use rstim::interactive_shot::{EditableShot, ExpansionLimits, NoiseSiteKind};
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SamplingBackend, sample_batch_with_options};

fn sample(program: &str, backend: SamplingBackend, shots: usize) -> rstim::sampler::BatchOutput {
    let circuit = parse_lines(program).unwrap();
    sample_batch_with_options(
        &circuit,
        shots,
        &mut StdRng::seed_from_u64(314159),
        SampleOptions {
            backend,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn noisy_readout_flips_records_without_changing_the_measured_state() {
    for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
        for (reset, measurement) in [("R", "M"), ("RX", "MX")] {
            let out = sample(
                &format!("{reset} 0 1 2\n{measurement}(1) 0 1 2\n{measurement} 0 1 2\n"),
                backend,
                67,
            );
            for shot in 0..67 {
                for row in 0..3 {
                    assert!(
                        out.measurements.get(row, shot),
                        "{measurement} noisy row {row}"
                    );
                    assert!(
                        !out.measurements.get(row + 3, shot),
                        "readout must not change state"
                    );
                }
            }
        }
    }
}

#[test]
fn reset_readout_noise_is_independent_of_the_reset_and_reference_sample() {
    for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
        for (reset, measurement, later) in [("R", "MR", "M"), ("RX", "MRX", "MX")] {
            let program = format!(
                "{reset} 0 1 2\n{measurement}(1) 0 1 2\n{later} 0 1 2\nDETECTOR rec[-6]\nOBSERVABLE_INCLUDE(0) rec[-5]\n"
            );
            assert_eq!(
                reference_sample(&parse_lines(&program).unwrap()).unwrap(),
                vec![false; 6]
            );
            let out = sample(&program, backend, 67);
            for shot in 0..67 {
                for row in 0..3 {
                    assert!(out.measurements.get(row, shot));
                    assert!(!out.measurements.get(row + 3, shot));
                }
                assert!(out.detections.get(0, shot));
                assert!(out.observable_flips.get(0, shot));
            }
        }
    }
}

#[test]
fn single_shot_readout_noise_preserves_state_and_reset() {
    for (reset, measurement, later) in [
        ("R", "M", "M"),
        ("RX", "MX", "MX"),
        ("RY", "MY", "MY"),
        ("R", "MR", "M"),
        ("RX", "MRX", "MX"),
        ("RY", "MRY", "MY"),
    ] {
        let circuit = parse_lines(&format!(
            "{reset} 0 1 2\n{measurement}(1) 0 1 2\n{later} 0 1 2\n"
        ))
        .unwrap();
        let mut executor = Executor::from_instrs(circuit).unwrap();
        let output = executor.run(&mut StdRng::seed_from_u64(42)).unwrap();
        assert_eq!(
            output.measurements,
            vec![true, true, true, false, false, false],
            "{measurement}"
        );
    }
}

#[test]
fn y_basis_readout_flips_all_targets_in_both_batch_backends() {
    for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
        for measurement in ["MY", "MRY"] {
            // The compiled sampler does not support the S gate needed to
            // prepare a Y eigenstate. Compare the same intrinsic trajectory
            // with and without certain readout inversion instead.
            let plain = sample(&format!("R 0 1 2\n{measurement} 0 1 2\n"), backend, 67);
            let out = sample(&format!("R 0 1 2\n{measurement}(1) 0 1 2\n"), backend, 67);
            for row in 0..3 {
                for shot in 0..67 {
                    assert_ne!(
                        out.measurements.get(row, shot),
                        plain.measurements.get(row, shot)
                    );
                }
            }
        }
    }

    let prepared = sample(
        "R 0 1 2\nH 0 1 2\nS 0 1 2\nMY(1) 0 1 2\n",
        SamplingBackend::Interpreted,
        67,
    );
    for row in 0..3 {
        for shot in 0..67 {
            assert!(prepared.measurements.get(row, shot));
        }
    }
}

#[test]
fn classical_feedback_and_trace_use_the_flipped_readout() {
    let circuit = parse_lines("R 0 1 2\nM(1) 0\nCX rec[-1] 1\nM 1 2\n").unwrap();
    let mut executor = Executor::from_instrs(circuit).unwrap();
    let (output, trace) = executor
        .run_with_trace(&mut StdRng::seed_from_u64(42))
        .unwrap();
    assert_eq!(output.measurements, vec![true, true, false]);
    assert_eq!(trace.noise_events.len(), 1);
    assert_eq!(trace.noise_events[0].branch_label.as_deref(), Some("flip"));
}

#[test]
fn partial_readout_noise_is_sampled_independently_for_each_target() {
    for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
        let out = sample("R 0 1 2\nM(0.25) 0 1 2\n", backend, 16384);
        for row in 0..3 {
            let count = (0..16384)
                .filter(|&shot| out.measurements.get(row, shot))
                .count();
            assert!((3700..4500).contains(&count), "row {row}: {count}");
        }
        let both = (0..16384)
            .filter(|&shot| out.measurements.get(0, shot) && out.measurements.get(1, shot))
            .count();
        assert!(
            (800..1250).contains(&both),
            "independent joint count: {both}"
        );
    }
}

#[test]
fn product_and_pair_measurements_apply_one_readout_flip_per_record() {
    for instruction in ["MPP(1) Z0*Z1 Z1*Z2", "MZZ(1) 0 1 1 2"] {
        let out = sample(
            &format!("R 0 1 2\n{instruction}\nM 0 1 2\n"),
            SamplingBackend::Interpreted,
            67,
        );
        for shot in 0..67 {
            assert!(out.measurements.get(0, shot));
            assert!(out.measurements.get(1, shot));
            for row in 2..5 {
                assert!(!out.measurements.get(row, shot));
            }
        }
    }
}

#[test]
fn zero_readout_probability_preserves_seeded_measurements() {
    for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
        let plain = sample("R 0 1 2\nDEPOLARIZE1(0.2) 0 1 2\nM 0 1 2\n", backend, 67);
        let explicit = sample("R 0 1 2\nDEPOLARIZE1(0.2) 0 1 2\nM(0) 0 1 2\n", backend, 67);
        for row in 0..3 {
            for shot in 0..67 {
                assert_eq!(
                    plain.measurements.get(row, shot),
                    explicit.measurements.get(row, shot)
                );
            }
        }
    }
}

#[test]
fn interactive_sampling_catalogs_readout_flips_including_aliases_and_repeats() {
    for (reset, measurement, later) in [
        ("R", "M", "M"),
        ("R", "MZ", "MZ"),
        ("RX", "MX", "MX"),
        ("RY", "MY", "MY"),
        ("R", "MR", "M"),
        ("R", "MRZ", "MZ"),
        ("RX", "MRX", "MX"),
        ("RY", "MRY", "MY"),
    ] {
        let program = format!("{reset} 0 1\nREPEAT 2 {{\n{measurement}(1) 0 1\n}}\n{later} 0 1\n");
        let mut shot = EditableShot::open(&program, ExpansionLimits::default(), 23).unwrap();
        let catalog = shot.session().catalog();
        assert_eq!(catalog.events().len(), 4, "{measurement}");
        assert!(
            catalog
                .sites()
                .iter()
                .all(|site| site.kind == NoiseSiteKind::MeasurementFlip && !site.editable)
        );
        assert!(
            shot.summary()
                .result
                .measurements
                .iter()
                .all(|row| !row.bit)
        );
        shot.sample(23).unwrap();
        let result = shot.summary().result;
        assert_eq!(
            result
                .measurements
                .iter()
                .map(|row| row.bit)
                .collect::<Vec<_>>(),
            vec![true, true, true, true, false, false],
            "{measurement}"
        );
        assert_eq!(result.noise_events.len(), 4);
        // Snapshot construction also consumes the trace and must resolve all sites.
        shot.view_snapshot().unwrap();
        shot.clear(23).unwrap();
        assert!(
            shot.summary()
                .result
                .measurements
                .iter()
                .all(|row| !row.bit)
        );
    }
}
