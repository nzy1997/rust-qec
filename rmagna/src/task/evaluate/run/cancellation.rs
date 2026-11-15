pub use crate::cancellation::CancellationToken;

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::Path;
    struct Counted {
        stream: SplitMix64,
        words: usize,
    }
    impl RandomSource for Counted {
        fn next_u64(&mut self) -> u64 {
            self.words += 1;
            self.stream.next_u64()
        }
    }
    #[test]
    fn observed_cancellation_consumes_no_new_rng_word_or_decoder_work() {
        for sampler in [
            crate::runtime::HistorySampler::ExactMarginal,
            crate::runtime::HistorySampler::AffineSupport,
        ] {
            let token = CancellationToken::new();
            let captured = token.clone();
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = std::sync::Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", move |bits: &[bool]| {
                    observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    captured.cancel();
                    Ok(bits.to_vec())
                })
                .unwrap();
            let parse = crate::circuit::ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            };
            let circuit = crate::circuit::parse_circuit(
                "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\n",
                &parse,
            )
            .unwrap();
            let modules = crate::task::parse_task(
                r#"{"outputs":[1]}"#,
                Path::new("."),
                &crate::task::TaskOptions::default(),
            )
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
            let mut options = EvaluationOptions::default();
            options.runtime.history_sampler = sampler;
            let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
            let run = TaskRunOptions {
                shots: 3,
                ..Default::default()
            };
            let mut random = Counted {
                stream: SplitMix64::new(8),
                words: 0,
            };
            let result = evaluator
                .run_batch_with_rng(
                    &run,
                    &ExecutionMeter::new(run.execution_limits),
                    None,
                    Some(&token),
                    &mut random,
                )
                .unwrap();
            assert_eq!(result.status, RunStatus::Cancelled);
            assert_eq!(
                (result.completed, result.failed, result.not_attempted),
                (1, 0, 2)
            );
            assert_eq!(random.words, 1);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            let state = random.stream.state();
            let costs = result.execution_costs;
            let again = evaluator
                .run_batch_with_rng(
                    &run,
                    &ExecutionMeter::new(run.execution_limits),
                    None,
                    Some(&token),
                    &mut random,
                )
                .unwrap();
            assert_eq!(again.completed, 0);
            assert_eq!(again.execution_costs.query.attempted, 0);
            assert_eq!(again.execution_costs.decoder.attempted, 0);
            assert_eq!(random.words, 1);
            assert_eq!(random.stream.state(), state);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_eq!(costs.decoder.succeeded, 1);
        }
    }
}
