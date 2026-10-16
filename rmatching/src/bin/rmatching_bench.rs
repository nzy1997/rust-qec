#[cfg(feature = "bench")]
mod bench {
    use rstim::sim::bit_table::BitTable;
    use std::path::Path;

    pub fn parse_stim_filename(path: &Path) -> Option<(f64, usize)> {
        let stem = path.file_stem()?.to_str()?;
        // Try format 1: "...p_0.001_d_5" or "...p_0.01_d_17"
        if let Some(p_idx) = stem.rfind("_p_") {
            let p_start = p_idx + 3;
            let rest = &stem[p_start..];
            let d_pos = rest.find("_d_")?;
            let p_str = &rest[..d_pos];
            let after_d = &rest[d_pos + 3..];
            let d_end = after_d.find('_').unwrap_or(after_d.len());
            let d_str = &after_d[..d_end];
            let p: f64 = p_str.parse().ok()?;
            let d: usize = d_str.parse().ok()?;
            return Some((p, d));
        }
        // Try format 2: "surface_code_rotated_memory_x_{d}_{p}" e.g. "surface_code_rotated_memory_x_5_0.001"
        // The last two underscore-separated tokens are d (integer) and p (float)
        let parts: Vec<&str> = stem.split('_').collect();
        if parts.len() >= 2 {
            let p_str = parts[parts.len() - 1];
            let d_str = parts[parts.len() - 2];
            if let (Ok(p), Ok(d)) = (p_str.parse::<f64>(), d_str.parse::<usize>()) {
                return Some((p, d));
            }
        }
        None
    }

    pub fn detections_to_syndromes(table: &BitTable, num_detectors: usize) -> Vec<Vec<u8>> {
        // The FrameSimulator BitTable layout is transposed:
        // major = detector index, minor = shot index.
        let n_shots = table.num_minor();
        let n_dets = num_detectors.min(table.num_major());
        (0..n_shots)
            .map(|shot| {
                (0..n_dets)
                    .map(|det| if table.get(det, shot) { 1u8 } else { 0u8 })
                    .collect()
            })
            .collect()
    }

    /// Remove spaces and tabs that appear inside parentheses so that
    /// "DETECTOR(2, 4, 0) rec[-1]" becomes "DETECTOR(2,4,0) rec[-1]".
    /// This lets the rstim parser keep argument lists as single whitespace tokens.
    pub fn normalize_paren_spaces(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut depth = 0usize;
        for ch in text.chars() {
            match ch {
                '(' => { depth += 1; out.push(ch); }
                ')' => { depth = depth.saturating_sub(1); out.push(ch); }
                ' ' | '\t' if depth > 0 => { /* skip spaces inside parens */ }
                _ => out.push(ch),
            }
        }
        out
    }

    pub fn count_logical_errors(
        predictions: &[Vec<u8>],
        obs_flips: &BitTable,
    ) -> usize {
        // obs_flips layout: major = observable index, minor = shot index.
        let num_obs = obs_flips.num_major();
        let n_shots = obs_flips.num_minor();
        (0..predictions.len()).filter(|&shot| {
            if shot >= n_shots { return false; }
            let pred = &predictions[shot];
            (0..num_obs.min(pred.len())).any(|obs| {
                let actual = obs_flips.get(obs, shot);
                let predicted = pred[obs] != 0;
                actual != predicted
            })
        }).count()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::Path;

        #[test]
        fn test_parse_stim_filename_basic() {
            let p = Path::new("surface_code_rotated_memory_x_p_0.001_d_5.stim");
            assert_eq!(parse_stim_filename(p), Some((0.001, 5)));
        }

        #[test]
        fn test_parse_stim_filename_larger() {
            let p = Path::new("surface_code_rotated_memory_x_p_0.01_d_17.stim");
            assert_eq!(parse_stim_filename(p), Some((0.01, 17)));
        }

        #[test]
        fn test_parse_stim_filename_no_match() {
            let p = Path::new("not_a_surface_code.stim");
            assert_eq!(parse_stim_filename(p), None);
        }

        #[test]
        fn test_detections_to_syndromes_basic() {
            // Layout: major = detector, minor = shot (matches FrameSimulator output).
            // 3 detectors, 2 shots.
            let mut table = BitTable::new(3, 2);
            table.set(0, 0, true);  // det 0 fires in shot 0
            table.set(2, 1, true);  // det 2 fires in shot 1
            let syndromes = detections_to_syndromes(&table, 3);
            assert_eq!(syndromes.len(), 2);
            assert_eq!(syndromes[0], vec![1, 0, 0]);
            assert_eq!(syndromes[1], vec![0, 0, 1]);
        }

        #[test]
        fn test_count_logical_errors_none() {
            // Layout: major = observable, minor = shot.
            // 1 observable, 2 shots, all predicted correctly.
            let mut obs = BitTable::new(1, 2);
            obs.set(0, 0, false);
            obs.set(0, 1, false);
            let preds = vec![vec![0u8], vec![0u8]];
            assert_eq!(count_logical_errors(&preds, &obs), 0);
        }

        #[test]
        fn test_count_logical_errors_one() {
            // 1 observable, 2 shots; shot 1 mispredicted.
            let mut obs = BitTable::new(1, 2);
            obs.set(0, 0, false);
            obs.set(0, 1, true);  // actual flip in shot 1
            let preds = vec![vec![0u8], vec![0u8]];  // predicted no flip for shot 1
            assert_eq!(count_logical_errors(&preds, &obs), 1);
        }

        #[test]
        fn test_count_logical_errors_multi_obs() {
            // 2 observables, 1 shot; shot counted once even if both wrong.
            let mut obs = BitTable::new(2, 1);
            obs.set(0, 0, true);
            obs.set(1, 0, true);
            let preds = vec![vec![0u8, 0u8]];  // both wrong
            assert_eq!(count_logical_errors(&preds, &obs), 1);
        }

        #[test]
        fn test_normalize_paren_spaces_basic() {
            assert_eq!(
                normalize_paren_spaces("DETECTOR(2, 4, 0) rec[-1]"),
                "DETECTOR(2,4,0) rec[-1]"
            );
        }

        #[test]
        fn test_normalize_paren_spaces_no_parens() {
            let s = "H 0 1 2\nCNOT 0 1";
            assert_eq!(normalize_paren_spaces(s), s);
        }

        #[test]
        fn test_normalize_paren_spaces_preserves_newlines() {
            let s = "DETECTOR(1, 2) rec[-1]\nDETECTOR(3, 4) rec[-2]";
            assert_eq!(
                normalize_paren_spaces(s),
                "DETECTOR(1,2) rec[-1]\nDETECTOR(3,4) rec[-2]"
            );
        }
    }
}

#[cfg(not(feature = "bench"))]
fn main() {
    eprintln!("Build with --features bench to use rmatching_bench");
    std::process::exit(1);
}

#[cfg(feature = "bench")]
fn main() {
    use bench::*;
    use rmatching::Matching;
    use rstim::error_analyzer::ErrorAnalyzer;
    use rstim::sampler::sample_batch;
    use std::time::Instant;

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: rmatching_bench <stim_file> <num_shots>");
        std::process::exit(1);
    }
    let stim_path = std::path::Path::new(&args[1]);
    let num_shots: usize = args[2].parse().unwrap_or_else(|_| {
        eprintln!("num_shots must be a positive integer");
        std::process::exit(1);
    });

    let (p, d) = parse_stim_filename(stim_path).unwrap_or_else(|| {
        eprintln!("Cannot parse (p, d) from filename: {}", stim_path.display());
        std::process::exit(1);
    });

    let circuit_text = std::fs::read_to_string(stim_path).unwrap_or_else(|e| {
        eprintln!("Failed to read stim file: {e}");
        std::process::exit(1);
    });

    // Parse circuit.
    // The rstim parser tokenizes on whitespace before handling parenthesised
    // argument lists, so "DETECTOR(2, 4, 0) rec[-1]" is split into the token
    // "DETECTOR(2," which fails.  Pre-process the text to remove spaces inside
    // parentheses (e.g. "(2, 4, 0)" -> "(2,4,0)") so the whole arg list stays
    // in one whitespace token.
    let stripped = normalize_paren_spaces(&circuit_text);
    let instrs = rstim::parser::parse_lines(&stripped).unwrap_or_else(|e| {
        eprintln!("Failed to parse circuit: {e}");
        std::process::exit(1);
    });

    // Generate decomposed DEM (equivalent to decompose_errors=True)
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&instrs).unwrap_or_else(|e| {
        eprintln!("Failed to generate DEM: {e}");
        std::process::exit(1);
    });
    let num_detectors = dem.num_detectors();
    let dem_text = dem.to_string();

    // Build matching decoder (not timed)
    let mut matching = Matching::from_dem(&dem_text).unwrap_or_else(|e| {
        eprintln!("Failed to build Matching from DEM: {e}");
        std::process::exit(1);
    });

    // Sample from circuit (not timed)
    let mut rng = rand::thread_rng();
    let output = sample_batch(&instrs, num_shots, &mut rng).unwrap_or_else(|e| {
        eprintln!("Failed to sample circuit: {e}");
        std::process::exit(1);
    });

    // Convert detections to syndromes (layout: major=detector, minor=shot)
    let sampler_dets = output.detections.num_major();
    if sampler_dets != num_detectors {
        eprintln!(
            "Warning: DEM declares {} detectors but sampler produced {}",
            num_detectors, sampler_dets
        );
    }
    let syndromes = detections_to_syndromes(&output.detections, num_detectors);

    // Decode (timed)
    let t0 = Instant::now();
    let predictions = matching.decode_batch(&syndromes);
    let decode_s = t0.elapsed().as_secs_f64();

    // Compute metrics
    let logical_errors = count_logical_errors(&predictions, &output.observable_flips);
    let logical_error_rate = logical_errors as f64 / num_shots as f64;
    let decode_us_per_round = decode_s * 1e6 / (num_shots as f64 * d as f64);

    println!(
        "rmatching,{p},{d},{decode_us_per_round:.4},{logical_error_rate:.6}"
    );
}
