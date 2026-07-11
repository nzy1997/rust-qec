#[cfg(debug_assertions)]
mod tests {
    use rand::rngs::StdRng;
    use rand::RngCore;
    use rand::SeedableRng;
    use rstim::compiled::compile_circuit;
    use rstim::parser::parse_lines;
    use rstim::sampler::{
        sample_batch_with_options, SampleOptions, SampleOutputMode, SamplingBackend,
    };
    use rstim::sim::bit_table::BitTable;
    use rstim::sim::frame::{
        decode_instruction_wide_event_index, one_qubit_noise_instruction_telemetry,
        reset_frame_noise_telemetry, reset_one_qubit_noise_instruction_telemetry,
        take_frame_noise_telemetry, FrameSimulator, OneQubitNoiseSamplingPath,
    };

    #[derive(Debug, Clone)]
    struct ScriptedRng {
        words: Vec<u64>,
        index: usize,
    }

    impl ScriptedRng {
        fn new(words: Vec<u64>) -> Self {
            Self { words, index: 0 }
        }
    }

    impl RngCore for ScriptedRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            let word = self.words.get(self.index).copied().unwrap_or(0);
            self.index += 1;
            word
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    fn targets(count: usize) -> String {
        (0..count)
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn program(instruction: &str, probability: f64, target_count: usize) -> String {
        let targets = targets(target_count);
        format!("{instruction}({probability}) {targets}\nM {targets}\n")
    }

    fn run_and_read_telemetry(
        instruction: &str,
        probability: f64,
        target_count: usize,
        shots: usize,
        backend: SamplingBackend,
    ) -> rstim::sim::frame::OneQubitNoiseInstructionTelemetry {
        reset_one_qubit_noise_instruction_telemetry();
        let instrs = parse_lines(&program(instruction, probability, target_count)).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let output = sample_batch_with_options(
            &instrs,
            shots,
            &mut rng,
            SampleOptions {
                backend,
                output_mode: SampleOutputMode::MeasurementsOnly,
                ..SampleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(output.measurements.num_major(), target_count);
        one_qubit_noise_instruction_telemetry()
    }

    fn assert_sparse_telemetry(context: &str, expected_attempt_count: usize) {
        let telemetry = one_qubit_noise_instruction_telemetry();
        assert_eq!(
            telemetry.sampling_path,
            OneQubitNoiseSamplingPath::Sparse,
            "{context}"
        );
        assert_eq!(telemetry.iterator_builds, 1, "{context}");
        assert_eq!(telemetry.attempt_count, expected_attempt_count, "{context}");
    }

    fn run_direct_measurements(
        program: &str,
        shots: usize,
        backend: SamplingBackend,
        rng: &mut impl RngCore,
    ) -> BitTable {
        let instrs = parse_lines(program).unwrap();
        let compiled = compile_circuit(&instrs).unwrap();
        let mut frame = FrameSimulator::new(compiled.num_qubits, shots);
        match backend {
            SamplingBackend::Interpreted => frame.run(&instrs, &[], rng).unwrap(),
            SamplingBackend::Compiled => frame
                .run_compiled_blocks(&compiled.blocks, &[], rng)
                .unwrap(),
            SamplingBackend::Auto => panic!("direct sparse test requires an explicit backend"),
        }
        let ref_sample = vec![false; frame.m_record.len()];
        frame.measurements(&ref_sample)
    }

    fn measurement_ones(measurements: &BitTable) -> Vec<(usize, usize)> {
        let mut ones = Vec::new();
        for measurement in 0..measurements.num_major() {
            for shot in 0..measurements.num_minor() {
                if measurements.get(measurement, shot) {
                    ones.push((measurement, shot));
                }
            }
        }
        ones
    }

    fn raw_for_sparse_skip(probability: f64, skip: usize) -> u64 {
        let one_minus_p = 1.0 - probability;
        let low = one_minus_p.powi((skip + 1) as i32);
        let high = one_minus_p.powi(skip as i32);
        let midpoint = (low + high) * 0.5;
        let mantissa = (midpoint * ((1u64 << 53) as f64)).ceil() as u64;
        assert!(mantissa > 0);
        assert!(mantissa < (1u64 << 53));
        mantissa << 11
    }

    fn scripted_sparse_rng(
        probability: f64,
        attempt_count: usize,
        event_indices: &[usize],
    ) -> ScriptedRng {
        let mut next_candidate = 0usize;
        let mut words = Vec::new();
        for &event_index in event_indices {
            assert!(event_index < attempt_count);
            assert!(event_index >= next_candidate);
            words.push(raw_for_sparse_skip(
                probability,
                event_index - next_candidate,
            ));
            next_candidate = event_index + 1;
        }
        if next_candidate < attempt_count {
            words.push(raw_for_sparse_skip(
                probability,
                attempt_count - next_candidate,
            ));
        }
        ScriptedRng::new(words)
    }

    #[derive(Debug, Default)]
    struct Depolarize1BranchCounts {
        x_only: usize,
        z_only: usize,
        y: usize,
    }

    fn depolarize1_branch_counts(
        z_basis_measurements: &BitTable,
        x_basis_measurements: &BitTable,
    ) -> Depolarize1BranchCounts {
        assert_eq!(
            z_basis_measurements.num_major(),
            x_basis_measurements.num_major()
        );
        assert_eq!(
            z_basis_measurements.num_minor(),
            x_basis_measurements.num_minor()
        );

        let mut counts = Depolarize1BranchCounts::default();
        for measurement in 0..z_basis_measurements.num_major() {
            for shot in 0..z_basis_measurements.num_minor() {
                match (
                    z_basis_measurements.get(measurement, shot),
                    x_basis_measurements.get(measurement, shot),
                ) {
                    (true, false) => counts.x_only += 1,
                    (false, true) => counts.z_only += 1,
                    (true, true) => counts.y += 1,
                    (false, false) => {}
                }
            }
        }
        counts
    }

    #[test]
    fn instruction_wide_index_decoder_matches_known_answers() {
        let cases = [
            (0, (0, 0)),
            (1023, (0, 1023)),
            (1024, (1, 0)),
            (102399, (99, 1023)),
        ];
        for (event_index, expected) in cases {
            assert_eq!(
                decode_instruction_wide_event_index(event_index, 1024),
                Some(expected),
                "event_index={event_index}"
            );
        }
    }

    #[test]
    fn sparse_x_error_places_events_on_instruction_wide_targets_and_shots() {
        let probability = 0.02;
        let shots = 16;
        let target_count = 3;
        let attempt_count = target_count * shots;
        let event_indices = [shots + 3, 2 * shots + 9];
        let program = format!("X_ERROR({probability}) 1 4 7\nM 1 4 7\n");

        for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
            reset_one_qubit_noise_instruction_telemetry();
            let mut rng = scripted_sparse_rng(probability, attempt_count, &event_indices);
            let measurements = run_direct_measurements(&program, shots, backend, &mut rng);
            assert_eq!(
                measurement_ones(&measurements),
                vec![(1, 3), (2, 9)],
                "{backend:?}"
            );
            assert_sparse_telemetry(&format!("X_ERROR {backend:?}"), attempt_count);
        }
    }

    #[test]
    fn sparse_depolarize1_outputs_x_z_and_y_branches() {
        let probability = 0.02;
        let target_count = 96;
        let shots = 512;
        let attempt_count = target_count * shots;
        let target_list = targets(target_count);
        let z_basis_program =
            format!("DEPOLARIZE1({probability}) {target_list}\nM {target_list}\n");
        let x_basis_program =
            format!("DEPOLARIZE1({probability}) {target_list}\nMX {target_list}\n");

        for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
            reset_one_qubit_noise_instruction_telemetry();
            let mut z_basis_rng = StdRng::seed_from_u64(0xD011_0C1E);
            let z_basis_measurements =
                run_direct_measurements(&z_basis_program, shots, backend, &mut z_basis_rng);
            assert_sparse_telemetry(&format!("DEPOLARIZE1 M {backend:?}"), attempt_count);

            reset_one_qubit_noise_instruction_telemetry();
            let mut x_basis_rng = StdRng::seed_from_u64(0xD011_0C1E);
            let x_basis_measurements =
                run_direct_measurements(&x_basis_program, shots, backend, &mut x_basis_rng);
            assert_sparse_telemetry(&format!("DEPOLARIZE1 MX {backend:?}"), attempt_count);

            let counts = depolarize1_branch_counts(&z_basis_measurements, &x_basis_measurements);
            assert!(
                counts.x_only > 100,
                "{backend:?} expected many X-only sparse branches, got {counts:?}"
            );
            assert!(
                counts.z_only > 100,
                "{backend:?} expected many Z-only sparse branches, got {counts:?}"
            );
            assert!(
                counts.y > 100,
                "{backend:?} expected many X+Z/Y sparse branches, got {counts:?}"
            );
        }
    }

    #[test]
    fn sparse_one_qubit_noise_uses_one_instruction_wide_iterator() {
        for instruction in ["X_ERROR", "DEPOLARIZE1"] {
            for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
                let telemetry = run_and_read_telemetry(instruction, 0.001, 100, 1024, backend);
                assert_eq!(
                    telemetry.sampling_path,
                    OneQubitNoiseSamplingPath::Sparse,
                    "{instruction} {backend:?}"
                );
                assert_eq!(telemetry.iterator_builds, 1, "{instruction} {backend:?}");
                assert_eq!(telemetry.attempt_count, 102400, "{instruction} {backend:?}");
            }
        }
    }

    #[test]
    fn sparse_one_qubit_noise_boundary_probability_uses_sparse_path() {
        let probability = 0.02;
        let target_count = 37;
        let shots = 129;
        let expected_attempt_count = target_count * shots;

        for instruction in ["X_ERROR", "DEPOLARIZE1"] {
            for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
                let telemetry =
                    run_and_read_telemetry(instruction, probability, target_count, shots, backend);
                assert_eq!(
                    telemetry.sampling_path,
                    OneQubitNoiseSamplingPath::Sparse,
                    "{instruction} {backend:?}"
                );
                assert_eq!(telemetry.iterator_builds, 1, "{instruction} {backend:?}");
                assert_eq!(
                    telemetry.attempt_count, expected_attempt_count,
                    "{instruction} {backend:?}"
                );
            }
        }
    }

    #[test]
    fn medium_probability_one_qubit_noise_uses_dense_path() {
        for instruction in ["X_ERROR", "DEPOLARIZE1"] {
            for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
                let telemetry = run_and_read_telemetry(instruction, 0.3, 100, 1024, backend);
                assert_eq!(
                    telemetry.sampling_path,
                    OneQubitNoiseSamplingPath::Dense,
                    "{instruction} {backend:?}"
                );
                assert_eq!(telemetry.iterator_builds, 0, "{instruction} {backend:?}");
                assert_eq!(telemetry.attempt_count, 102400, "{instruction} {backend:?}");
            }
        }
        println!("PASS instruction-wide one-qubit noise telemetry");
    }

    #[test]
    fn frame_noise_telemetry_accumulates_one_qubit_noise_operations() {
        let program = concat!(
            "X_ERROR(0.001) 0 1 2\n",
            "DEPOLARIZE1(0.3) 0 1 2\n",
            "M 0 1 2\n",
        );
        let expected_attempt_count = 3 * 17;

        for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
            reset_frame_noise_telemetry();
            let mut rng = StdRng::seed_from_u64(463);
            let measurements = run_direct_measurements(program, 17, backend, &mut rng);
            assert_eq!(measurements.num_major(), 3);

            let telemetry = take_frame_noise_telemetry();
            assert_eq!(telemetry.len(), 2, "{backend:?}");
            assert_eq!(telemetry[0].name, "X_ERROR", "{backend:?}");
            assert_eq!(telemetry[0].sampling_path, "sparse", "{backend:?}");
            assert_eq!(telemetry[0].iterator_builds, 1, "{backend:?}");
            assert_eq!(
                telemetry[0].attempt_count, expected_attempt_count,
                "{backend:?}"
            );
            assert_eq!(telemetry[1].name, "DEPOLARIZE1", "{backend:?}");
            assert_eq!(telemetry[1].sampling_path, "dense", "{backend:?}");
            assert_eq!(telemetry[1].iterator_builds, 0, "{backend:?}");
            assert_eq!(
                telemetry[1].attempt_count, expected_attempt_count,
                "{backend:?}"
            );
        }
    }
}
