//! rmatching CLI: decode syndromes from a DEM file
//!
//! Usage: rmatching_cli <dem_file>
//! Stdin:  one syndrome per line, space-separated 0/1 per detector
//! Stdout: one prediction per line, space-separated 0/1 per observable

use rmatching::Matching;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rmatching_cli <dem_file>");
        std::process::exit(1);
    }

    let dem_text = std::fs::read_to_string(&args[1])
        .unwrap_or_else(|e| { eprintln!("Failed to read DEM file: {e}"); std::process::exit(1); });

    let mut matching = Matching::from_dem(&dem_text)
        .unwrap_or_else(|e| { eprintln!("Failed to parse DEM: {e}"); std::process::exit(1); });

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() { continue; }

        let syndrome: Vec<u8> = line.split_whitespace()
            .map(|s| s.parse::<u8>().expect("syndrome values must be 0 or 1"))
            .collect();

        let pred = matching.decode(&syndrome);
        let pred_str: Vec<String> = pred.iter().map(|b| b.to_string()).collect();
        writeln!(out, "{}", pred_str.join(" ")).unwrap();
    }
}
