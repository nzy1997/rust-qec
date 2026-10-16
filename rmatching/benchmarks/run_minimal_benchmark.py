#!/usr/bin/env python3

import argparse
import csv
import json
import math
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", str(Path("/tmp") / "codex-mpl-cache"))

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from benchmarks.minimal_cases import build_cases


REPO_ROOT = Path(__file__).resolve().parent.parent
RESULTS_CSV = REPO_ROOT / "benchmarks/minimal_results.csv"
MISMATCH_JSON = REPO_ROOT / "benchmarks/minimal_mismatches.json"
RELEASE_BINARY = REPO_ROOT / "target/release/rmatching_microbench"
DEBUG_BINARY = REPO_ROOT / "target/debug/rmatching_microbench"

CSV_HEADER = [
    "case_name",
    "decoder",
    "num_detectors",
    "num_edges",
    "num_syndromes_tested",
    "prediction_match_rate",
    "mismatch_cases",
    "build_us",
    "mean_decode_us",
    "median_decode_us",
    "p95_decode_us",
]


def normalize_fault_ids(observables):
    return {int(value) for value in observables}


def dem_to_pymatching(dem_text: str):
    import pymatching

    matcher = pymatching.Matching()
    for line in dem_text.strip().splitlines():
        line = line.strip()
        if not line or not line.startswith("error("):
            continue
        probability = float(line.split("(")[1].split(")")[0])
        tokens = line.split()[1:]
        detectors = [int(token[1:]) for token in tokens if token.startswith("D")]
        observables = [int(token[1:]) for token in tokens if token.startswith("L")]
        if probability in (0.0, 1.0):
            weight = 0.0
        else:
            weight = math.log((1 - probability) / probability)
        if len(detectors) == 2:
            matcher.add_edge(
                detectors[0],
                detectors[1],
                fault_ids=normalize_fault_ids(observables),
                weight=weight,
                error_probability=probability,
            )
        elif len(detectors) == 1:
            matcher.add_boundary_edge(
                detectors[0],
                fault_ids=normalize_fault_ids(observables),
                weight=weight,
                error_probability=probability,
            )
        else:
            raise ValueError(f"Unsupported DEM line: {line}")
    return matcher


def normalize_predictions(array_like):
    import numpy as np

    arr = np.asarray(array_like, dtype=np.uint8)
    if arr.ndim == 1:
        return [[int(value)] for value in arr.tolist()]
    return [[int(value) for value in row] for row in arr.tolist()]


def measure_batch_decode(
    decode_fn,
    normalize_fn,
    syndromes,
    warmup_rounds: int,
    measure_rounds: int,
):
    predictions = normalize_fn(decode_fn(syndromes))

    for _ in range(warmup_rounds):
        decode_fn(syndromes)

    latencies_us = []
    for _ in range(measure_rounds):
        started = time.perf_counter()
        decode_fn(syndromes)
        latencies_us.append((time.perf_counter() - started) * 1e6)

    ordered = sorted(latencies_us)
    if ordered:
        mean_decode_us = sum(ordered) / len(ordered)
        median_decode_us = ordered[len(ordered) // 2]
        p95_decode_us = ordered[max(math.ceil(len(ordered) * 0.95) - 1, 0)]
    else:
        mean_decode_us = 0.0
        median_decode_us = 0.0
        p95_decode_us = 0.0

    return {
        "predictions": predictions,
        "build_us": 0.0,
        "decode_latencies_us": latencies_us,
        "mean_decode_us": mean_decode_us,
        "median_decode_us": median_decode_us,
        "p95_decode_us": p95_decode_us,
    }


def run_pymatching(case, warmup_rounds: int, measure_rounds: int):
    import numpy as np
    import pymatching

    build_started = time.perf_counter()
    _ = pymatching
    matcher = dem_to_pymatching(case.dem)
    build_us = (time.perf_counter() - build_started) * 1e6
    syndromes = np.asarray(case.syndromes, dtype=np.uint8)

    def decode_batch(batch):
        return matcher.decode_batch(batch)

    stats = measure_batch_decode(
        decode_fn=decode_batch,
        normalize_fn=normalize_predictions,
        syndromes=syndromes,
        warmup_rounds=warmup_rounds,
        measure_rounds=measure_rounds,
    )
    stats["build_us"] = build_us
    return stats


def run_rmatching(case, warmup_rounds: int, measure_rounds: int):
    request = {
        "dem": case.dem,
        "syndromes": case.syndromes,
        "warmup_rounds": warmup_rounds,
        "measure_rounds": measure_rounds,
    }

    if RELEASE_BINARY.exists():
        cmd = [str(RELEASE_BINARY)]
    elif DEBUG_BINARY.exists():
        cmd = [str(DEBUG_BINARY)]
    elif shutil.which("cargo"):
        cmd = [
            "cargo",
            "run",
            "--quiet",
            "--features",
            "bench",
            "--bin",
            "rmatching_microbench",
        ]
    else:
        raise RuntimeError("Neither rmatching_microbench binary nor cargo is available")

    result = subprocess.run(
        cmd,
        input=json.dumps(request),
        text=True,
        capture_output=True,
        cwd=REPO_ROOT,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "rmatching_microbench failed")
    return json.loads(result.stdout)


def summarize_case(
    case_name,
    num_detectors,
    num_edges,
    py_predictions,
    rm_predictions,
    rm_stats,
    syndromes,
):
    mismatches = []
    for syndrome, py_pred, rm_pred in zip(syndromes, py_predictions, rm_predictions):
        if py_pred != rm_pred:
            mismatches.append(
                {
                    "case_name": case_name,
                    "syndrome": syndrome,
                    "pymatching_prediction": py_pred,
                    "rmatching_prediction": rm_pred,
                }
            )

    total = len(syndromes)
    match_rate = 1.0 if total == 0 else (total - len(mismatches)) / total
    row = {
        "case_name": case_name,
        "decoder": "rmatching",
        "num_detectors": num_detectors,
        "num_edges": num_edges,
        "num_syndromes_tested": total,
        "prediction_match_rate": match_rate,
        "mismatch_cases": len(mismatches),
        "build_us": rm_stats["build_us"],
        "mean_decode_us": rm_stats["mean_decode_us"],
        "median_decode_us": rm_stats["median_decode_us"],
        "p95_decode_us": rm_stats["p95_decode_us"],
    }
    return row, mismatches


def pymatching_row(case, stats):
    return {
        "case_name": case.name,
        "decoder": "pymatching",
        "num_detectors": case.num_detectors,
        "num_edges": case.num_edges,
        "num_syndromes_tested": len(case.syndromes),
        "prediction_match_rate": 1.0,
        "mismatch_cases": 0,
        "build_us": stats["build_us"],
        "mean_decode_us": stats["mean_decode_us"],
        "median_decode_us": stats["median_decode_us"],
        "p95_decode_us": stats["p95_decode_us"],
    }


def write_outputs(rows, mismatches, results_path=RESULTS_CSV, mismatch_path=MISMATCH_JSON):
    with open(results_path, "w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)

    with open(mismatch_path, "w") as handle:
        json.dump(mismatches, handle, indent=2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--warmup-rounds", type=int, default=5)
    parser.add_argument("--measure-rounds", type=int, default=20)
    args = parser.parse_args()

    rows = []
    mismatches = []
    for case in build_cases():
        py_stats = run_pymatching(case, args.warmup_rounds, args.measure_rounds)
        rm_stats = run_rmatching(case, args.warmup_rounds, args.measure_rounds)
        rows.append(pymatching_row(case, py_stats))
        row, case_mismatches = summarize_case(
            case_name=case.name,
            num_detectors=case.num_detectors,
            num_edges=case.num_edges,
            py_predictions=py_stats["predictions"],
            rm_predictions=rm_stats["predictions"],
            rm_stats=rm_stats,
            syndromes=case.syndromes,
        )
        rows.append(row)
        mismatches.extend(case_mismatches)

    write_outputs(rows, mismatches)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
