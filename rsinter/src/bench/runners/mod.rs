use std::collections::BTreeMap;
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::output::write_shots_b8;
use rstim::sampler::sample_batch;

use crate::bench::circuit_source::build_circuit_for_point;
use crate::bench::registry::{BenchCasePoint, BenchRunContext};
use crate::bench::result::{BenchmarkResultRow, MetricMap, PairMapExt};
use crate::decode::Decoder;
use crate::failure::classify_completed;

pub(crate) mod params;
pub mod rbposd;
pub mod rilpqec;
pub mod rmatching;

fn under_wall_budget(total_seconds: f64, max_wall_seconds: Option<f64>) -> bool {
    match max_wall_seconds {
        Some(max_seconds) => total_seconds < max_seconds,
        None => true,
    }
}

pub(crate) fn run_decoder_point(
    runner_name: &'static str,
    decoder: &dyn Decoder,
    point: &BenchCasePoint,
    ctx: &BenchRunContext,
    decoder_params: &crate::bench::result::ParamMap,
) -> Result<BenchmarkResultRow, String> {
    let built = build_circuit_for_point(point, &ctx.spec_dir)?;
    let circuit = built.circuit;
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)?;

    let compile_started = Instant::now();
    let compiled = decoder.compile_for_dem(&dem);
    let compile_us = compile_started.elapsed().as_secs_f64() * 1e6;

    let max_shots = usize::try_from(point.max_shots)
        .map_err(|_| "max_shots exceeds supported usize range".to_string())?;
    let max_errors = usize::try_from(point.max_errors)
        .map_err(|_| "max_errors exceeds supported usize range".to_string())?;
    let num_dets = dem.effective_num_detectors();
    let num_obs = dem.num_observables();
    let obs_bytes = num_obs.div_ceil(8);

    let mut rng = StdRng::seed_from_u64(ctx.seed);
    let mut shots_used = 0usize;
    let mut logical_errors = 0usize;
    let mut generated_shots = 0usize;
    let mut total_decode_us = 0.0;
    let mut wall_seconds = 0.0;

    while shots_used < max_shots
        && logical_errors < max_errors
        && under_wall_budget(wall_seconds, point.max_wall_seconds)
    {
        let batch_started = Instant::now();
        let batch_shots = point.batch_size.min(max_shots - shots_used);
        let batch = sample_batch(&circuit, batch_shots, &mut rng)?;
        generated_shots += batch_shots;

        let mut dets = Vec::new();
        write_shots_b8(&batch.detections, &mut dets).map_err(|e| e.to_string())?;
        let mut obs = Vec::new();
        write_shots_b8(&batch.observable_flips, &mut obs).map_err(|e| e.to_string())?;

        let decode_started = Instant::now();
        let predictions = compiled.decode_shots_bit_packed(&dets, batch_shots, num_dets, num_obs);
        total_decode_us += decode_started.elapsed().as_secs_f64() * 1e6;

        let expected_len = batch_shots * obs_bytes;
        if predictions.len() != expected_len {
            return Err(format!(
                "decoder {runner_name} produced {} bytes, expected {expected_len}",
                predictions.len()
            ));
        }
        if obs.len() != expected_len {
            return Err(format!(
                "sampler produced {} observable bytes, expected {expected_len}",
                obs.len()
            ));
        }

        for shot in 0..batch_shots {
            let start = shot * obs_bytes;
            let end = start + obs_bytes;
            if predictions[start..end] != obs[start..end] {
                logical_errors += 1;
            }
            shots_used += 1;
            if shots_used >= max_shots || logical_errors >= max_errors {
                break;
            }
        }
        wall_seconds += batch_started.elapsed().as_secs_f64();
    }

    let mut result_params = built.params;
    for (key, value) in decoder_params {
        result_params.insert(key.clone(), value.clone());
    }
    let timed_out =
        matches!(point.max_wall_seconds, Some(max_seconds) if wall_seconds >= max_seconds)
            && shots_used < max_shots
            && logical_errors < max_errors;

    Ok(BenchmarkResultRow {
        benchmark: ctx.benchmark_name.clone(),
        runner: ctx.runner_name.clone(),
        language: ctx.language.clone(),
        status: "ok".into(),
        failure_kind: classify_completed(logical_errors as u64, timed_out),
        params: result_params,
        case_summary: {
            let mut summary = built.case_summary;
            summary.insert("num_dets".into(), serde_json::json!(num_dets));
            summary.insert("num_obs".into(), serde_json::json!(num_obs));
            summary.insert(
                "num_shots_generated".into(),
                serde_json::json!(generated_shots),
            );
            summary
        },
        metrics: MetricMap::from_pairs([
            ("shots_used", shots_used as f64),
            ("logical_errors", logical_errors as f64),
            (
                "logical_error_rate",
                if shots_used == 0 {
                    0.0
                } else {
                    logical_errors as f64 / shots_used as f64
                },
            ),
            ("compile_us", compile_us),
            ("total_decode_us", total_decode_us),
            ("wall_seconds", wall_seconds),
            (
                "decode_us_per_shot",
                if shots_used == 0 {
                    0.0
                } else {
                    total_decode_us / shots_used as f64
                },
            ),
        ]),
        artifacts: BTreeMap::new(),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use rstim::dem::DetectorErrorModel;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::decode::{CompiledDecoder, Decoder};
    use crate::failure::FailureKind;

    struct EmptyPredictionDecoder;

    struct EmptyPredictionCompiled;

    impl Decoder for EmptyPredictionDecoder {
        fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
            Box::new(EmptyPredictionCompiled)
        }
    }

    impl CompiledDecoder for EmptyPredictionCompiled {
        fn decode_shots_bit_packed(
            &self,
            _dets: &[u8],
            _num_shots: usize,
            _num_dets: usize,
            _num_obs: usize,
        ) -> Vec<u8> {
            Vec::new()
        }
    }

    struct SlowPredictionDecoder {
        sleep: Duration,
    }

    struct SlowPredictionCompiled {
        sleep: Duration,
    }

    impl Decoder for SlowPredictionDecoder {
        fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
            Box::new(SlowPredictionCompiled { sleep: self.sleep })
        }
    }

    impl CompiledDecoder for SlowPredictionCompiled {
        fn decode_shots_bit_packed(
            &self,
            _dets: &[u8],
            num_shots: usize,
            _num_dets: usize,
            num_obs: usize,
        ) -> Vec<u8> {
            thread::sleep(self.sleep);
            let obs_bytes = num_obs.div_ceil(8);
            vec![0u8; num_shots * obs_bytes]
        }
    }

    #[test]
    fn run_decoder_point_rejects_prediction_buffers_with_wrong_length() {
        let point = BenchCasePoint {
            input_type: "surface_rotated_memory_x".into(),
            code_id: None,
            distance: Some(3),
            rounds: 3,
            p: 0.002,
            basis: None,
            schedule: None,
            hx_path: None,
            hz_path: None,
            observables_path: None,
            max_shots: 4,
            max_errors: 2,
            max_wall_seconds: None,
            batch_size: 2,
            decoder_params: BTreeMap::new(),
        };
        let ctx = BenchRunContext {
            benchmark_name: "surface_decoder".into(),
            runner_name: "fake".into(),
            language: "rust".into(),
            seed: 12_345,
            spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        };

        let decoder_params = crate::bench::result::ParamMap::new();
        let err = run_decoder_point(
            "fake",
            &EmptyPredictionDecoder,
            &point,
            &ctx,
            &decoder_params,
        )
        .unwrap_err();

        assert!(err.contains("decoder fake produced 0 bytes"));
    }

    #[test]
    fn run_decoder_point_reports_zero_rates_when_no_shots_are_requested() {
        let point = BenchCasePoint {
            input_type: "surface_rotated_memory_x".into(),
            code_id: None,
            distance: Some(3),
            rounds: 3,
            p: 0.002,
            basis: None,
            schedule: None,
            hx_path: None,
            hz_path: None,
            observables_path: None,
            max_shots: 0,
            max_errors: 2,
            max_wall_seconds: None,
            batch_size: 2,
            decoder_params: BTreeMap::new(),
        };
        let ctx = BenchRunContext {
            benchmark_name: "surface_decoder".into(),
            runner_name: "fake".into(),
            language: "rust".into(),
            seed: 12_345,
            spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        };

        let decoder_params = crate::bench::result::ParamMap::new();
        let row = run_decoder_point(
            "fake",
            &EmptyPredictionDecoder,
            &point,
            &ctx,
            &decoder_params,
        )
        .unwrap();

        assert_eq!(row.metrics["shots_used"], 0.0);
        assert_eq!(row.metrics["logical_error_rate"], 0.0);
        assert_eq!(row.metrics["decode_us_per_shot"], 0.0);
    }

    #[test]
    fn run_decoder_point_respects_wall_clock_budget() {
        let point = BenchCasePoint {
            input_type: "surface_rotated_memory_x".into(),
            code_id: None,
            distance: Some(3),
            rounds: 3,
            p: 0.0,
            basis: None,
            schedule: None,
            hx_path: None,
            hz_path: None,
            observables_path: None,
            max_shots: 20,
            max_errors: 20,
            max_wall_seconds: Some(0.09),
            batch_size: 1,
            decoder_params: BTreeMap::new(),
        };
        let ctx = BenchRunContext {
            benchmark_name: "surface_decoder".into(),
            runner_name: "fake".into(),
            language: "rust".into(),
            seed: 12_345,
            spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        };

        let decoder = SlowPredictionDecoder {
            sleep: Duration::from_millis(35),
        };
        let decoder_params = crate::bench::result::ParamMap::new();
        let row = run_decoder_point("fake", &decoder, &point, &ctx, &decoder_params).unwrap();

        let shots_used = row.metrics["shots_used"];
        let wall_seconds = row
            .metrics
            .get("wall_seconds")
            .copied()
            .expect("wall_seconds metric is recorded");

        assert!(shots_used > 0.0, "shots_used={shots_used}");
        assert!(shots_used < 20.0, "shots_used={shots_used}");
        assert_eq!(row.failure_kind, FailureKind::Timeout);
        assert!(wall_seconds >= 0.09, "wall_seconds={wall_seconds}");
        assert!(wall_seconds.is_finite(), "wall_seconds={wall_seconds}");
    }
}
