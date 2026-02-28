use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::SeedableRng;

pub fn sample_lines(program: &str, shots: usize, seed: Option<u64>) -> String {
    let instrs = rstim::parser::parse_lines(program).unwrap();
    let mut out = String::new();
    for s in 0..shots {
        let mut ex = rstim::executor::Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = seeded_rng(seed, s as u64);
        let result = ex.run(&mut rng).unwrap();
        let line: String = result
            .measurements
            .iter()
            .map(|b| if *b { '1' } else { '0' })
            .collect();
        out.push_str(&line);
        out.push('\n');
    }
    out
}

pub fn deviation(sample_content: &str, expected: &HashMap<&str, f32>) -> String {
    let actual = line_freq(sample_content);
    let mut actual_total = 0usize;
    for (k, v) in actual.iter() {
        if !expected.contains_key(k.as_str()) {
            return format!("Sampled {} which was not expected.", k);
        }
        actual_total += *v;
    }
    if actual_total == 0 {
        return "No samples.".to_string();
    }
    let expected_unity: f32 = expected.values().sum();
    if (expected_unity - 1.0).abs() > 1e-5 {
        return "Expected distribution doesn't add up to 1.".to_string();
    }
    for (k, expected_rate) in expected.iter() {
        let allowed_variation = 5.0 * ((*expected_rate * (1.0 - *expected_rate) / actual_total as f32).sqrt());
        if *expected_rate - allowed_variation < 0.0 || *expected_rate + allowed_variation > 1.0 {
            return "Not enough samples to bound results away from extremes.".to_string();
        }
        let actual_rate = *actual.get(&k.to_string()).unwrap_or(&0) as f32 / actual_total as f32;
        if (*expected_rate - actual_rate).abs() > allowed_variation {
            return format!(
                "Actual rate {} of sample '{}' is more than 5 standard deviations from expected rate {}",
                actual_rate, k, expected_rate
            );
        }
    }
    String::new()
}

fn line_freq(data: &str) -> HashMap<String, usize> {
    let mut result = HashMap::new();
    for line in data.trim().split('\n') {
        if line.is_empty() {
            continue;
        }
        *result.entry(line.to_string()).or_insert(0) += 1;
    }
    result
}

fn seeded_rng(seed: Option<u64>, shot: u64) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s + shot),
        None => StdRng::from_entropy(),
    }
}
