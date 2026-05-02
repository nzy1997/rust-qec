# Benchmark Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `rmatching_bench` Rust binary and a `benchmarks/run_benchmark.py` Python driver to compare rstim+rmatching vs stim+PyMatching on the pre-existing surface code circuits, reporting decoding speed (µs/round) and logical error rate.

**Architecture:** A dedicated `rmatching_bench` binary handles the full Rust pipeline (rstim circuit parse → DEM generation → rmatching decode) and prints one CSV line per invocation. A Python script `benchmarks/run_benchmark.py` runs the PyMatching baseline natively, calls `rmatching_bench` via subprocess for each of the 36 circuits, and writes a combined `benchmarks/results.csv`.

**Tech Stack:** Rust 2024 edition, rstim (local path dep), rmatching, Python 3, stim, pymatching, numpy

---

### Task 1: Add `bench` feature and rstim path dependency to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Read current Cargo.toml**

```bash
cat Cargo.toml
```
Expected output shows `rstim = { git = "https://github.com/nzy1997/rstim.git", optional = true }`.

**Step 2: Add `bench` feature and workspace patch**

Edit `Cargo.toml` to:

```toml
[package]
name = "rmatching"
version = "0.1.0"
edition = "2024"

[workspace]

[features]
rsinter = ["dep:rsinter", "dep:rstim"]
bench = ["dep:rstim"]

[dependencies]
rsinter = { git = "https://github.com/nzy1997/rstim.git", optional = true }
rstim = { git = "https://github.com/nzy1997/rstim.git", optional = true }

[patch."https://github.com/nzy1997/rstim.git"]
rstim = { path = "../rstim" }
rsinter = { path = "../rsinter" }

[dev-dependencies]
```

**Step 3: Verify it compiles**

```bash
cargo check --features bench
```
Expected: no errors.

**Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "feat: add bench feature with rstim path dep for rmatching_bench binary"
```

---

### Task 2: Create `rmatching_bench` helper functions with unit tests

**Files:**
- Create: `src/bin/rmatching_bench.rs`

**Context:** The benchmark binary needs three pure helper functions before the `main()` plumbing:
- `parse_stim_filename(path) -> Option<(f64, usize)>` — extracts `(p, d)` from the filename
- `detections_to_syndromes(table, num_detectors) -> Vec<Vec<u8>>` — converts rstim's `BitTable` (major=shot, minor=detector) to rmatching's `Vec<Vec<u8>>`
- `count_logical_errors(predictions, obs_flips) -> usize` — counts shots where any predicted observable ≠ actual flip

**Step 1: Create the file with failing tests only**

```rust
// src/bin/rmatching_bench.rs
#[cfg(feature = "bench")]
mod bench {
    use rstim::sim::bit_table::BitTable;
    use std::path::Path;

    pub fn parse_stim_filename(path: &Path) -> Option<(f64, usize)> {
        todo!()
    }

    pub fn detections_to_syndromes(table: &BitTable, num_detectors: usize) -> Vec<Vec<u8>> {
        todo!()
    }

    pub fn count_logical_errors(
        predictions: &[Vec<u8>],
        obs_flips: &BitTable,
    ) -> usize {
        todo!()
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
            // 2 shots, 3 detectors
            let mut table = BitTable::new(2, 3);
            table.set(0, 0, true);  // shot 0, det 0
            table.set(1, 2, true);  // shot 1, det 2
            let syndromes = detections_to_syndromes(&table, 3);
            assert_eq!(syndromes.len(), 2);
            assert_eq!(syndromes[0], vec![1, 0, 0]);
            assert_eq!(syndromes[1], vec![0, 0, 1]);
        }

        #[test]
        fn test_count_logical_errors_none() {
            // 2 shots, 1 observable, all predicted correctly
            let mut obs = BitTable::new(2, 1);
            obs.set(0, 0, false);
            obs.set(1, 0, false);
            let preds = vec![vec![0u8], vec![0u8]];
            assert_eq!(count_logical_errors(&preds, &obs), 0);
        }

        #[test]
        fn test_count_logical_errors_one() {
            // 2 shots, 1 observable, shot 1 mispredicted
            let mut obs = BitTable::new(2, 1);
            obs.set(0, 0, false);
            obs.set(1, 0, true);  // actual flip
            let preds = vec![vec![0u8], vec![0u8]];  // predicted no flip for shot 1
            assert_eq!(count_logical_errors(&preds, &obs), 1);
        }

        #[test]
        fn test_count_logical_errors_multi_obs() {
            // 1 shot, 2 observables; shot counted once even if both wrong
            let mut obs = BitTable::new(1, 2);
            obs.set(0, 0, true);
            obs.set(0, 1, true);
            let preds = vec![vec![0u8, 0u8]];  // both wrong
            assert_eq!(count_logical_errors(&preds, &obs), 1);
        }
    }
}

fn main() {}
```

**Step 2: Run tests to confirm they fail**

```bash
cargo test --features bench --bin rmatching_bench 2>&1 | head -30
```
Expected: multiple `panicked at 'not yet implemented'` errors.

**Step 3: Implement the three functions**

Replace the `todo!()` stubs:

```rust
pub fn parse_stim_filename(path: &Path) -> Option<(f64, usize)> {
    let stem = path.file_stem()?.to_str()?;
    // Match e.g. "...p_0.001_d_5" or "...p_0.01_d_17"
    let p_start = stem.find("_p_")? + 3;
    let rest = &stem[p_start..];
    let d_pos = rest.find("_d_")?;
    let p_str = &rest[..d_pos];
    let after_d = &rest[d_pos + 3..];
    // d is the next token (until _ or end)
    let d_end = after_d.find('_').unwrap_or(after_d.len());
    let d_str = &after_d[..d_end];
    let p: f64 = p_str.parse().ok()?;
    let d: usize = d_str.parse().ok()?;
    Some((p, d))
}

pub fn detections_to_syndromes(table: &BitTable, num_detectors: usize) -> Vec<Vec<u8>> {
    let n_shots = table.num_major();
    (0..n_shots)
        .map(|shot| {
            (0..num_detectors)
                .map(|det| if table.get(shot, det) { 1u8 } else { 0u8 })
                .collect()
        })
        .collect()
}

pub fn count_logical_errors(
    predictions: &[Vec<u8>],
    obs_flips: &BitTable,
) -> usize {
    let num_obs = obs_flips.num_minor();
    predictions.iter().enumerate().filter(|(shot, pred)| {
        (0..num_obs.min(pred.len())).any(|obs| {
            let actual = obs_flips.get(*shot, obs);
            let predicted = pred[obs] != 0;
            actual != predicted
        })
    }).count()
}
```

**Step 4: Run tests to confirm they pass**

```bash
cargo test --features bench --bin rmatching_bench 2>&1 | tail -20
```
Expected: `test result: ok. 7 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/bin/rmatching_bench.rs
git commit -m "feat: add rmatching_bench helper functions with unit tests"
```

---

### Task 3: Implement the `main()` function for `rmatching_bench`

**Files:**
- Modify: `src/bin/rmatching_bench.rs`

**Context:** The `main()` function reads a `.stim` file path and shot count from CLI args, runs the full rstim→rmatching pipeline, times `decode_batch`, and prints a CSV line.

**Step 1: Replace the stub `main()` with the full implementation**

Replace `fn main() {}` at the bottom of the file with:

```rust
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

    // Parse circuit
    let instrs = rstim::parser::parse_lines(&circuit_text).unwrap_or_else(|e| {
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

    // Convert detections to syndromes
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
```

**Step 2: Build in release mode**

```bash
cargo build --release --features bench --bin rmatching_bench
```
Expected: `Compiling rmatching ...` then `Finished release [optimized] target(s)`.

**Step 3: Smoke test with one of the pre-existing .stim files**

```bash
./target/release/rmatching_bench \
  PyMatching/benchmarks/surface_codes/surface_code_rotated_memory_x_p_0.001_d_5_7_9_13_17_23_29_39_50_both_bases/surface_code_rotated_memory_x_p_0.001_d_5.stim \
  1000
```
Expected: one CSV line like `rmatching,0.001,5,0.1234,0.002300` (exact numbers will vary).

If the binary panics with a DEM parsing error, debug by printing `dem_text` to stderr and comparing to a DEM generated by `stim`:
```bash
python3 -c "
import stim
c = stim.Circuit.from_file('PyMatching/benchmarks/surface_codes/surface_code_rotated_memory_x_p_0.001_d_5_7_9_13_17_23_29_39_50_both_bases/surface_code_rotated_memory_x_p_0.001_d_5.stim')
print(c.detector_error_model(decompose_errors=True))
" | head -20
```
Compare the first few lines of both DEMs to verify they match structurally.

**Step 4: Run all circuits with 100 shots to check no panics**

```bash
for f in PyMatching/benchmarks/surface_codes/**/*.stim; do
  ./target/release/rmatching_bench "$f" 100 || echo "FAILED: $f"
done
```
Expected: 36 CSV lines, no `FAILED` lines.

**Step 5: Commit**

```bash
git add src/bin/rmatching_bench.rs
git commit -m "feat: implement rmatching_bench binary for rstim+rmatching pipeline"
```

---

### Task 4: Create `benchmarks/run_benchmark.py`

**Files:**
- Create: `benchmarks/run_benchmark.py`
- Create: `benchmarks/.gitignore` (to ignore results.csv)

**Step 1: Create the benchmarks directory and gitignore**

```bash
mkdir -p benchmarks
echo "results.csv" > benchmarks/.gitignore
```

**Step 2: Create the Python driver**

```python
#!/usr/bin/env python3
"""
benchmarks/run_benchmark.py

Compare stim+PyMatching vs rstim+rmatching on rotated surface code circuits.

Usage:
    python3 benchmarks/run_benchmark.py [--shots 10000]

Requirements:
    pip install stim pymatching numpy
    cargo build --release --features bench --bin rmatching_bench
"""

import argparse
import csv
import glob
import re
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
import pymatching
import stim


STIM_GLOB = "PyMatching/benchmarks/surface_codes/**/*.stim"
BENCH_BINARY = "./target/release/rmatching_bench"
RESULTS_CSV = "benchmarks/results.csv"
CSV_HEADER = ["decoder", "p", "d", "decode_us_per_round", "logical_error_rate"]


def parse_filename(path: Path):
    """Extract (p, d) from a surface code .stim filename."""
    m = re.search(r"_p_([\d.]+)_d_(\d+)\.stim$", path.name)
    if not m:
        return None
    return float(m.group(1)), int(m.group(2))


def run_pymatching(stim_path: Path, num_shots: int, p: float, d: int) -> dict:
    """Run the stim+PyMatching baseline pipeline."""
    circuit = stim.Circuit.from_file(str(stim_path))
    dem = circuit.detector_error_model(decompose_errors=True)
    matcher = pymatching.Matching.from_detector_error_model(dem)
    sampler = circuit.compile_detector_sampler()
    detections, obs_flips = sampler.sample(num_shots, separate_observables=True)

    t0 = time.perf_counter()
    predictions = matcher.decode_batch(detections)
    decode_s = time.perf_counter() - t0

    # A shot is a logical error if any observable prediction is wrong
    logical_errors = int(np.any(predictions != obs_flips, axis=1).sum())
    logical_error_rate = logical_errors / num_shots
    decode_us_per_round = decode_s * 1e6 / (num_shots * d)

    return {
        "decoder": "pymatching",
        "p": p,
        "d": d,
        "decode_us_per_round": round(decode_us_per_round, 4),
        "logical_error_rate": round(logical_error_rate, 6),
    }


def run_rmatching(stim_path: Path, num_shots: int) -> dict:
    """Run the rstim+rmatching pipeline via the rmatching_bench binary."""
    result = subprocess.run(
        [BENCH_BINARY, str(stim_path), str(num_shots)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"rmatching_bench failed for {stim_path.name}:\n{result.stderr}"
        )
    line = result.stdout.strip()
    decoder, p, d, us_per_round, ler = line.split(",")
    return {
        "decoder": decoder,
        "p": float(p),
        "d": int(d),
        "decode_us_per_round": float(us_per_round),
        "logical_error_rate": float(ler),
    }


def print_table(rows: list[dict]) -> None:
    """Print a formatted comparison table to stdout."""
    print(f"\n{'decoder':<12} {'p':>8} {'d':>4}  {'decode_us/round':>16}  {'logical_err_rate':>16}")
    print("-" * 65)
    for r in rows:
        print(
            f"{r['decoder']:<12} {r['p']:>8.4f} {r['d']:>4}  "
            f"{r['decode_us_per_round']:>16.4f}  {r['logical_error_rate']:>16.6f}"
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--shots", type=int, default=10000)
    args = parser.parse_args()
    num_shots = args.shots

    # Verify rmatching_bench binary exists
    if not Path(BENCH_BINARY).exists():
        print(
            f"ERROR: {BENCH_BINARY} not found.\n"
            "Build it with: cargo build --release --features bench --bin rmatching_bench",
            file=sys.stderr,
        )
        sys.exit(1)

    stim_files = sorted(glob.glob(STIM_GLOB, recursive=True))
    if not stim_files:
        print(f"ERROR: No .stim files found matching {STIM_GLOB}", file=sys.stderr)
        sys.exit(1)

    print(f"Found {len(stim_files)} circuits. Running {num_shots} shots each.")
    print(f"{'Progress':<10}", end="", flush=True)

    rows = []
    for i, path_str in enumerate(stim_files):
        stim_path = Path(path_str)
        parsed = parse_filename(stim_path)
        if not parsed:
            print(f"\nWARNING: skipping {stim_path.name} (cannot parse p/d)", file=sys.stderr)
            continue
        p, d = parsed

        print(f"\r[{i+1:2}/{len(stim_files)}] {stim_path.name}", end="", flush=True)

        # PyMatching baseline
        pm_row = run_pymatching(stim_path, num_shots, p, d)
        rows.append(pm_row)

        # rmatching
        rm_row = run_rmatching(stim_path, num_shots)
        rows.append(rm_row)

    print()  # newline after progress

    # Write CSV
    with open(RESULTS_CSV, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)

    print(f"\nResults saved to {RESULTS_CSV}")
    print_table(rows)


if __name__ == "__main__":
    main()
```

**Step 3: Verify Python dependencies are available**

```bash
python3 -c "import stim, pymatching, numpy; print('OK')"
```
Expected: `OK`. If not, install: `pip install stim pymatching numpy`.

**Step 4: Dry-run on a single circuit to verify the pipeline end-to-end**

Temporarily add `stim_files = stim_files[:1]` after the file list is built, run:
```bash
python3 benchmarks/run_benchmark.py --shots 100
```
Expected: 2 rows (one pymatching, one rmatching), a results.csv, and a printed table. Remove the limit after verifying.

**Step 5: Commit**

```bash
git add benchmarks/run_benchmark.py benchmarks/.gitignore
git commit -m "feat: add run_benchmark.py Python driver for stim+PyMatching vs rstim+rmatching"
```

---

### Task 5: Run the full benchmark

**Files:**
- Generates: `benchmarks/results.csv`

**Step 1: Ensure the release binary is up to date**

```bash
cargo build --release --features bench --bin rmatching_bench
```

**Step 2: Run the full benchmark**

```bash
python3 benchmarks/run_benchmark.py --shots 10000 2>&1 | tee benchmarks/run_log.txt
```
Expected: 36 lines of progress, then a printed table with 72 rows (36 circuits × 2 decoders), and `results.csv` written.

**Step 3: Spot-check results**

```bash
# Check that rmatching and pymatching logical error rates are close (within 2x)
python3 -c "
import csv
rows = list(csv.DictReader(open('benchmarks/results.csv')))
pm = {(r['p'], r['d']): float(r['logical_error_rate']) for r in rows if r['decoder']=='pymatching'}
rm = {(r['p'], r['d']): float(r['logical_error_rate']) for r in rows if r['decoder']=='rmatching'}
problems = [(k, pm[k], rm[k]) for k in pm if rm.get(k, 0) > 0 and abs(pm[k]-rm[k]) / max(pm[k],rm[k]) > 0.1]
if problems:
    print('LARGE DISCREPANCIES (>10%):')
    for k,a,b in problems:
        print(f'  p={k[0]} d={k[1]}: pymatching={a:.6f} rmatching={b:.6f}')
else:
    print('All logical error rates agree within 10%')
"
```
Expected: `All logical error rates agree within 10%`. Larger discrepancies indicate a bug in the rstim DEM generation or rmatching decode logic.

**Step 4: Commit results log (not results.csv, which is gitignored)**

```bash
git add benchmarks/run_log.txt
git commit -m "bench: add benchmark run log for rstim+rmatching vs stim+PyMatching"
```
