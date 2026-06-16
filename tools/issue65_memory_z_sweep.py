#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path

DISTANCES = (3, 5, 7)
NOISES = (0.008, 0.009, 0.010, 0.011, 0.012)
MAX_SHOTS = 1_000_000
MAX_ERRORS = 5_000
MAX_LIKELIHOOD_FACTOR = 10_000.0
ACC = 100


def log_binomial(p: float, shots: int, hits: int) -> float:
    p = min(1.0, max(0.0, p))
    misses = shots - hits
    if hits > 0 and p == 0.0:
        return float("-inf")
    if misses > 0 and p == 1.0:
        return float("-inf")

    result = 0.0
    if p > 0.0:
        result += math.log(p) * hits
    if p < 1.0:
        result += math.log1p(-p) * misses
    result += math.lgamma(shots + 1) - math.lgamma(misses + 1) - math.lgamma(hits + 1)
    return result


def binary_search(func, min_x: int, max_x: int, target: float) -> int:
    lo = min_x
    hi = max_x
    while hi > lo + 1:
        mid = lo + (hi - lo) // 2
        value = func(mid)
        if value < target:
            lo = mid
        elif value > target:
            hi = mid
        else:
            return mid
    f_hi = func(hi)
    f_lo = func(lo)
    d_hi = 0.0 if f_hi == target else abs(f_hi - target)
    d_lo = 0.0 if f_lo == target else abs(f_lo - target)
    return hi if d_hi < d_lo else lo


def fit_binomial(shots: int, hits: int) -> tuple[float, float, float]:
    if shots == 0:
        return 0.0, 0.5, 1.0
    best = hits / shots
    log_ml = log_binomial(best, shots, hits)
    target = log_ml - math.log(MAX_LIKELIHOOD_FACTOR)

    low = binary_search(
        lambda exp_err: log_binomial(exp_err / (ACC * shots), shots, hits),
        0,
        hits * ACC,
        target,
    )
    high = binary_search(
        lambda exp_err: -log_binomial(exp_err / (ACC * shots), shots, hits),
        hits * ACC,
        shots * ACC,
        -target,
    )
    return low / (ACC * shots), best, high / (ACC * shots)


def shot_error_rate_to_piece_error_rate(shot_error_rate: float, pieces: int | float) -> float:
    if not 0.0 <= shot_error_rate <= 1.0:
        raise ValueError(f"shot error rate must be in [0, 1], got {shot_error_rate}")
    if pieces <= 0:
        raise ValueError(f"pieces must be positive, got {pieces}")
    if pieces == 1:
        return shot_error_rate
    if shot_error_rate > 0.5:
        return 1.0 - shot_error_rate_to_piece_error_rate(1.0 - shot_error_rate, pieces)

    randomize_rate = 2.0 * shot_error_rate
    piece_randomize_rate = 1.0 - (1.0 - randomize_rate) ** (1.0 / pieces)
    piece_error_rate = piece_randomize_rate / 2.0
    if piece_error_rate == 0.0:
        return shot_error_rate / pieces
    return piece_error_rate


def make_circuit(distance: int, rounds: int, noise: float):
    import stim

    return stim.Circuit.generated(
        "surface_code:rotated_memory_z",
        rounds=rounds,
        distance=distance,
        after_clifford_depolarization=noise,
        after_reset_flip_probability=noise,
        before_measure_flip_probability=noise,
        before_round_data_depolarization=noise,
    )


def import_sinter_version() -> str | None:
    try:
        import sinter
    except ModuleNotFoundError:
        return None
    return getattr(sinter, "__version__", "unknown")


def count_mismatched_rows(pred, obs) -> int:
    import numpy as np

    if pred.shape != obs.shape:
        raise ValueError(f"prediction shape {pred.shape} does not match observable shape {obs.shape}")
    if pred.ndim == 1:
        return int(np.count_nonzero(pred != obs))
    return int(np.count_nonzero(np.any(pred != obs, axis=1)))


def collect_stim_reference(out: Path) -> None:
    import pymatching
    import stim

    rows = []
    for distance in DISTANCES:
        rounds = distance * 3
        for noise in NOISES:
            circuit = make_circuit(distance, rounds, noise)
            dem = circuit.detector_error_model(decompose_errors=True)
            matcher = pymatching.Matching.from_detector_error_model(dem)
            sampler = circuit.compile_detector_sampler()
            shots = 0
            errors = 0
            while shots < MAX_SHOTS and errors < MAX_ERRORS:
                batch = min(256, MAX_SHOTS - shots)
                dets, obs = sampler.sample(
                    shots=batch,
                    separate_observables=True,
                    bit_packed=True,
                )
                pred = matcher.decode_batch(
                    dets,
                    bit_packed_shots=True,
                    bit_packed_predictions=True,
                )
                errors += count_mismatched_rows(pred, obs)
                shots += batch

            low, best, high = fit_binomial(shots, errors)
            row = {
                "distance": distance,
                "rounds": rounds,
                "p": noise,
                "shots": shots,
                "logical_errors": errors,
                "logical_error_rate": best,
                "ci_low": low,
                "ci_high": high,
                "num_detectors": circuit.num_detectors,
                "num_observables": circuit.num_observables,
            }
            rows.append(row)
            print(
                "stim "
                f"d={distance} r={rounds} p={noise:.3f} "
                f"shots={shots} errors={errors} ler={best:.6g} "
                f"ci=[{low:.6g}, {high:.6g}]",
                file=sys.stderr,
                flush=True,
            )

    payload = {
        "metadata": {
            "generator": "tools/issue65_memory_z_sweep.py collect-stim",
            "created_at_unix": int(time.time()),
            "stim_version": stim.__version__,
            "pymatching_version": pymatching.__version__,
            "sinter_version": import_sinter_version(),
            "max_shots": MAX_SHOTS,
            "max_errors": MAX_ERRORS,
            "max_likelihood_factor": MAX_LIKELIHOOD_FACTOR,
            "case_count": len(rows),
        },
        "rows": rows,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def load_rust_rows(paths: list[Path]) -> list[dict[str, object]]:
    rows = []
    for path in paths:
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            rows.append(
                {
                    "distance": int(row["params"]["distance"]),
                    "rounds": int(row["params"]["rounds"]),
                    "p": float(row["params"]["p"]),
                    "shots": int(row["metrics"]["shots_used"]),
                    "logical_errors": int(row["metrics"]["logical_errors"]),
                    "logical_error_rate": float(row["metrics"]["logical_error_rate"]),
                    "status": row["status"],
                }
            )
    return rows


def add_series(
    axis,
    label: str,
    rows: list[dict[str, object]],
    distance: int,
    color: str,
    marker: str,
    linestyle: str = "-",
) -> None:
    selected = sorted(
        [row for row in rows if int(row["distance"]) == distance],
        key=lambda row: float(row["p"]),
    )
    if not selected:
        return
    xs = [float(row["p"]) for row in selected]
    per_round_ys = []
    low_err = []
    high_err = []
    for row in selected:
        y = float(row["logical_error_rate"])
        low, _best, high = fit_binomial(int(row["shots"]), int(row["logical_errors"]))
        rounds = int(row["rounds"])
        per_round_y = shot_error_rate_to_piece_error_rate(y, rounds)
        per_round_low = shot_error_rate_to_piece_error_rate(low, rounds)
        per_round_high = shot_error_rate_to_piece_error_rate(high, rounds)
        per_round_ys.append(per_round_y)
        low_err.append(max(0.0, per_round_y - per_round_low))
        high_err.append(max(0.0, per_round_high - per_round_y))
    axis.errorbar(
        xs,
        per_round_ys,
        yerr=[low_err, high_err],
        marker=marker,
        label=label,
        color=color,
        linestyle=linestyle,
        linewidth=1.4,
        capsize=3,
    )


def plot_compare(stim_fixture: Path, rust_results: list[Path], out: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    stim_rows = json.loads(stim_fixture.read_text())["rows"]
    rust_rows = load_rust_rows(rust_results)
    fig, axis = plt.subplots(1, 1, figsize=(7, 5), sharey=False)

    colors = {3: "tab:blue", 5: "tab:green", 7: "tab:red"}
    for distance in DISTANCES:
        color = colors[distance]
        add_series(axis, f"Stim/PyMatching d={distance}", stim_rows, distance, color, "o", "-")
        add_series(
            axis,
            f"RStim/rmatching d={distance}",
            rust_rows,
            distance,
            color,
            "s",
            "--",
        )
    axis.set_title("d=3,5,7; rounds=3d")
    axis.set_xlabel("p")
    axis.set_yscale("log")
    axis.grid(True, alpha=0.3)

    axis.set_ylabel("logical error rate per round")
    axis.legend(ncol=2)
    fig.suptitle("Issue 65 rotated memory-Z sweep")
    fig.tight_layout()
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=180)


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    collect = sub.add_parser("collect-stim")
    collect.add_argument("--out", type=Path, required=True)
    plot = sub.add_parser("plot")
    plot.add_argument("--stim-fixture", type=Path, required=True)
    plot.add_argument("--rust-results", type=Path, action="append", required=True)
    plot.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    if args.cmd == "collect-stim":
        collect_stim_reference(args.out)
    elif args.cmd == "plot":
        plot_compare(args.stim_fixture, args.rust_results, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
