#!/usr/bin/env python3

import argparse
import csv
import json
import os
import sys
import time
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", str(Path("/tmp") / "codex-mpl-cache"))

import numpy as np
import pymatching
import stim

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from benchmarks.run_minimal_benchmark import (
    measure_batch_decode,
    normalize_predictions,
    run_rmatching,
)
from benchmarks.surface_dem_cases import DEFAULT_SEED, DEFAULT_SHOTS, build_cases


REPO_ROOT = Path(__file__).resolve().parent.parent
RESULTS_CSV = REPO_ROOT / "benchmarks/surface_dem_results.csv"
MISMATCH_JSON = REPO_ROOT / "benchmarks/surface_dem_mismatches.json"
CSV_HEADER = [
    "case_name",
    "decoder",
    "status",
    "error",
    "p",
    "d",
    "num_detectors",
    "num_errors",
    "num_syndromes_tested",
    "prediction_match_rate",
    "mismatch_cases",
    "build_us",
    "mean_decode_us",
    "median_decode_us",
    "p95_decode_us",
]


def run_pymatching(case, warmup_rounds: int, measure_rounds: int):
    build_started = time.perf_counter()
    matcher = pymatching.Matching.from_detector_error_model(stim.DetectorErrorModel(case.dem))
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


def summarize_case(
    case_name,
    distance,
    p,
    num_detectors,
    expected_predictions,
    py_predictions,
    rm_predictions,
    py_stats,
    rm_stats,
    syndromes,
    num_errors=0,
):
    rows = []
    mismatches = []
    for decoder, predictions, stats in (
        ("pymatching", py_predictions, py_stats),
        ("rmatching", rm_predictions, rm_stats),
    ):
        decoder_mismatches = []
        for syndrome, expected, actual in zip(syndromes, expected_predictions, predictions):
            if expected != actual:
                decoder_mismatches.append(
                    {
                        "case_name": case_name,
                        "decoder": decoder,
                        "syndrome": syndrome,
                        "expected_prediction": expected,
                        "actual_prediction": actual,
                    }
                )

        total = len(syndromes)
        match_rate = 1.0 if total == 0 else (total - len(decoder_mismatches)) / total
        rows.append(
            {
                "case_name": case_name,
                "decoder": decoder,
                "status": "ok",
                "error": "",
                "p": p,
                "d": distance,
                "num_detectors": num_detectors,
                "num_errors": num_errors,
                "num_syndromes_tested": total,
                "prediction_match_rate": match_rate,
                "mismatch_cases": len(decoder_mismatches),
                "build_us": stats["build_us"],
                "mean_decode_us": stats["mean_decode_us"],
                "median_decode_us": stats["median_decode_us"],
                "p95_decode_us": stats["p95_decode_us"],
            }
        )
        mismatches.extend(decoder_mismatches)

    return rows, mismatches


def write_outputs(rows, mismatches, results_path=RESULTS_CSV, mismatch_path=MISMATCH_JSON):
    with open(results_path, "w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)

    with open(mismatch_path, "w") as handle:
        json.dump(mismatches, handle, indent=2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--shots", type=int, default=DEFAULT_SHOTS)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--warmup-rounds", type=int, default=5)
    parser.add_argument("--measure-rounds", type=int, default=20)
    args = parser.parse_args()

    rows = []
    mismatches = []
    for case in build_cases(shots=args.shots, seed=args.seed):
        py_stats = run_pymatching(case, args.warmup_rounds, args.measure_rounds)
        rows.append(
            {
                "case_name": case.name,
                "decoder": "pymatching",
                "status": "ok",
                "error": "",
                "p": case.p,
                "d": case.distance,
                "num_detectors": case.num_detectors,
                "num_errors": case.num_errors,
                "num_syndromes_tested": case.num_syndromes,
                "prediction_match_rate": sum(
                    1 for expected, actual in zip(case.observables, py_stats["predictions"]) if expected == actual
                ) / case.num_syndromes,
                "mismatch_cases": sum(
                    1 for expected, actual in zip(case.observables, py_stats["predictions"]) if expected != actual
                ),
                "build_us": py_stats["build_us"],
                "mean_decode_us": py_stats["mean_decode_us"],
                "median_decode_us": py_stats["median_decode_us"],
                "p95_decode_us": py_stats["p95_decode_us"],
            }
        )
        try:
            rm_stats = run_rmatching(case, args.warmup_rounds, args.measure_rounds)
        except RuntimeError as exc:
            rows.append(
                {
                    "case_name": case.name,
                    "decoder": "rmatching",
                    "status": "error",
                    "error": str(exc),
                    "p": case.p,
                    "d": case.distance,
                    "num_detectors": case.num_detectors,
                    "num_errors": case.num_errors,
                    "num_syndromes_tested": case.num_syndromes,
                    "prediction_match_rate": "",
                    "mismatch_cases": "",
                    "build_us": "",
                    "mean_decode_us": "",
                    "median_decode_us": "",
                    "p95_decode_us": "",
                }
            )
            continue

        case_rows, case_mismatches = summarize_case(
            case_name=case.name,
            distance=case.distance,
            p=case.p,
            num_detectors=case.num_detectors,
            num_errors=case.num_errors,
            expected_predictions=case.observables,
            py_predictions=py_stats["predictions"],
            rm_predictions=rm_stats["predictions"],
            py_stats=py_stats,
            rm_stats=rm_stats,
            syndromes=case.syndromes,
        )
        rows[-1] = case_rows[0]
        rows.extend(case_rows[1:])
        mismatches.extend(case_mismatches)

    write_outputs(rows, mismatches)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
