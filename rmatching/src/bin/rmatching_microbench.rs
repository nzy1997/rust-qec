#[cfg(feature = "bench")]
mod bench {
    use rmatching::Matching;
    use serde::{Deserialize, Serialize};
    use std::time::Instant;

    #[derive(Debug, Deserialize)]
    pub struct BenchmarkRequest {
        dem: String,
        syndromes: Vec<Vec<u8>>,
        warmup_rounds: usize,
        measure_rounds: usize,
    }

    #[derive(Debug, Serialize)]
    pub struct BenchmarkResponse {
        predictions: Vec<Vec<u8>>,
        build_us: f64,
        decode_latencies_us: Vec<f64>,
        mean_decode_us: f64,
        median_decode_us: f64,
        p95_decode_us: f64,
    }

    fn summarize_latencies(samples: &[f64]) -> (f64, f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let median = sorted[sorted.len() / 2];
        let p95_index = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let p95 = sorted[p95_index];
        (mean, median, p95)
    }

    pub fn run_request(req: BenchmarkRequest) -> BenchmarkResponse {
        let build_started = Instant::now();
        let mut matching = Matching::from_dem(&req.dem).expect("DEM must build");
        let build_us = build_started.elapsed().as_secs_f64() * 1e6;

        let mut predictions = Vec::new();
        matching.decode_batch_into(&req.syndromes, &mut predictions);
        let mut scratch_predictions = Vec::new();

        for _ in 0..req.warmup_rounds {
            matching.decode_batch_into(&req.syndromes, &mut scratch_predictions);
        }

        let mut decode_latencies_us = Vec::with_capacity(req.measure_rounds);
        for _ in 0..req.measure_rounds {
            let started = Instant::now();
            matching.decode_batch_into(&req.syndromes, &mut scratch_predictions);
            decode_latencies_us.push(started.elapsed().as_secs_f64() * 1e6);
        }

        let (mean_decode_us, median_decode_us, p95_decode_us) =
            summarize_latencies(&decode_latencies_us);

        BenchmarkResponse {
            predictions,
            build_us,
            decode_latencies_us,
            mean_decode_us,
            median_decode_us,
            p95_decode_us,
        }
    }

    pub fn process_request_json(input: &str) -> String {
        let req: BenchmarkRequest =
            serde_json::from_str(input).expect("benchmark request JSON must parse");
        let resp = run_request(req);
        serde_json::to_string(&resp).expect("benchmark response JSON must serialize")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn summarize_latencies_basic() {
            let (mean, median, p95) = summarize_latencies(&[4.0, 1.0, 3.0, 2.0, 5.0]);
            assert_eq!(mean, 3.0);
            assert_eq!(median, 3.0);
            assert_eq!(p95, 5.0);
        }

        #[test]
        fn run_request_decodes_square_case() {
            let req = BenchmarkRequest {
                dem: "error(0.1) D0 D1\nerror(0.1) D2 D3\nerror(0.1) D0 D2\nerror(0.1) D1 D3\nerror(0.1) D0 D3 L0\nerror(0.05) D0\nerror(0.05) D1\nerror(0.05) D2\nerror(0.05) D3\n"
                    .to_string(),
                syndromes: vec![vec![1, 0, 0, 1], vec![1, 1, 0, 0]],
                warmup_rounds: 1,
                measure_rounds: 3,
            };
            let resp = run_request(req);
            assert_eq!(resp.predictions, vec![vec![1], vec![0]]);
            assert_eq!(resp.decode_latencies_us.len(), 3);
            assert!(resp.build_us >= 0.0);
            assert!(resp.mean_decode_us >= 0.0);
        }

        #[test]
        fn process_request_json_serializes_predictions_and_stats() {
            let input = serde_json::json!({
                "dem": "error(0.1) D0 D1\nerror(0.05) D0\nerror(0.05) D1\n",
                "syndromes": [[0, 0], [1, 1]],
                "warmup_rounds": 0,
                "measure_rounds": 1,
            })
            .to_string();

            let output = process_request_json(&input);
            assert!(output.contains("\"predictions\""));
            assert!(output.contains("\"mean_decode_us\""));
        }
    }
}

#[cfg(not(feature = "bench"))]
fn main() {
    eprintln!("Build with --features bench to use rmatching_microbench");
    std::process::exit(1);
}

#[cfg(feature = "bench")]
fn main() {
    use bench::process_request_json;
    use std::io::{self, Read};

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    println!("{}", process_request_json(&input));
}
