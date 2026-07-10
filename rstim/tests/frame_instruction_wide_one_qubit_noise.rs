#[cfg(debug_assertions)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rstim::parser::parse_lines;
    use rstim::sampler::{
        SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options,
    };
    use rstim::sim::frame::{
        OneQubitNoiseSamplingPath, decode_instruction_wide_event_index,
        one_qubit_noise_instruction_telemetry, reset_one_qubit_noise_instruction_telemetry,
    };

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
}
