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


_REPO_ROOT = Path(__file__).resolve().parent.parent
STIM_GLOB = str(_REPO_ROOT / "PyMatching/benchmarks/surface_codes/**/*.stim")
BENCH_BINARY = str(_REPO_ROOT / "target/release/rmatching_bench")
RESULTS_CSV = str(_REPO_ROOT / "benchmarks/results.csv")
CSV_HEADER = ["decoder", "p", "d", "decode_us_per_round", "logical_error_rate"]


def parse_filename(path: Path):
    """Extract (p, d) from a surface code .stim filename."""
    m = re.search(r"_p_([\d.]+)_d_(\d+)\.stim$", path.name)
    if not m:
        # Actual PyMatching benchmark filenames use format: {name}_{d}_{p}.stim
        # e.g. "surface_code_rotated_memory_x_5_0.001.stim" -> (p=0.001, d=5)
        m2 = re.search(r"_(\d+)_([\d.]+)\.stim$", path.name)
        if m2:
            return float(m2.group(2)), int(m2.group(1))
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
    parts = line.split(",")
    if len(parts) != 5:
        raise RuntimeError(f"Unexpected output from rmatching_bench: {line!r}")
    decoder, p, d, us_per_round, ler = parts
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
        try:
            rm_row = run_rmatching(stim_path, num_shots)
            rows.append(rm_row)
        except RuntimeError as e:
            print(f"\n  [rmatching FAILED: {e}]", file=sys.stderr)

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
