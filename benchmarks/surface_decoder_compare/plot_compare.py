from __future__ import annotations

import math
import csv
import os
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
import argparse

os.environ.setdefault("MPLCONFIGDIR", str(Path("/tmp") / "codex-mpl-cache"))

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR = 9.0
MIN_LOG_Y = 1e-10


@dataclass(frozen=True)
class BinomialFit:
    low: float
    best: float
    high: float


@dataclass(frozen=True)
class LogicalRateFitForPlot:
    low: float
    best: float | None
    high: float


def _log_binomial(p: float, n: int, hits: int) -> float:
    p = min(max(p, 0.0), 1.0)
    misses = n - hits
    if hits > 0 and p == 0.0:
        return float("-inf")
    if misses > 0 and p == 1.0:
        return float("-inf")
    result = 0.0
    if p > 0.0:
        result += math.log(p) * hits
    if p < 1.0:
        result += math.log(1.0 - p) * misses
    return (
        result
        + math.lgamma(n + 1.0)
        - math.lgamma(misses + 1.0)
        - math.lgamma(hits + 1.0)
    )


def _binary_search(func, min_x: int, max_x: int, target: float) -> int:
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
    hi_value = func(hi)
    lo_value = func(lo)
    hi_delta = 0.0 if hi_value == target else abs(hi_value - target)
    lo_delta = 0.0 if lo_value == target else abs(lo_value - target)
    return hi if hi_delta < lo_delta else lo


def _fit_binomial(
    num_shots: int,
    num_hits: int,
    max_likelihood_factor: float,
) -> BinomialFit:
    if num_shots == 0:
        return BinomialFit(low=0.0, best=0.5, high=1.0)
    best_p = num_hits / num_shots
    log_ml = _log_binomial(best_p, num_shots, num_hits)
    target = log_ml - math.log(max_likelihood_factor)
    accuracy = 100
    denominator = accuracy * num_shots
    low = _binary_search(
        lambda expected_errors: _log_binomial(
            expected_errors / denominator,
            num_shots,
            num_hits,
        ),
        0,
        num_hits * accuracy,
        target,
    )
    high = _binary_search(
        lambda expected_errors: -_log_binomial(
            expected_errors / denominator,
            num_shots,
            num_hits,
        ),
        num_hits * accuracy,
        num_shots * accuracy,
        -target,
    )
    return BinomialFit(
        low=low / denominator,
        best=best_p,
        high=high / denominator,
    )


def _decoder_family(decoder: str) -> str:
    families = {
        "pymatching": "mwpm",
        "rmatching": "mwpm",
        "ilpqec": "ilp",
        "rilpqec": "ilp",
        "ldpc": "bp",
        "rbposd": "bp",
    }
    return families.get(decoder, decoder)


def _line_style_for_decoder(decoder: str) -> str:
    dashed_decoders = {"rmatching", "rilpqec", "rbposd"}
    return "--" if decoder in dashed_decoders else "-"


def _load_ok_rows(results_path: Path) -> list[dict[str, str]]:
    with results_path.open() as handle:
        rows = list(csv.DictReader(handle))
    return [row for row in rows if row["status"] == "ok"]


def _logical_error_rate_fit_for_plot(
    row: dict[str, str],
    confidence_interval_likelihood_factor: float = DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR,
) -> LogicalRateFitForPlot:
    shots_used = int(row["shots_used"])
    if shots_used <= 0:
        raise ValueError("shots_used must be positive")
    logical_errors = int(row["logical_errors"])
    if logical_errors < 0:
        raise ValueError("logical_errors must be non-negative")
    if logical_errors > shots_used:
        raise ValueError("logical_errors must be <= shots_used")

    fit = _fit_binomial(
        num_shots=shots_used,
        num_hits=logical_errors,
        max_likelihood_factor=confidence_interval_likelihood_factor,
    )
    return LogicalRateFitForPlot(
        low=max(fit.low, MIN_LOG_Y),
        best=None if logical_errors == 0 else max(fit.best, MIN_LOG_Y),
        high=max(fit.high, MIN_LOG_Y),
    )


def render_axes(ax_left, ax_right, rows: list[dict[str, str]]) -> None:
    grouped: dict[tuple[str, int], list[dict[str, str]]] = defaultdict(list)
    distances = sorted({int(row["distance"]) for row in rows})
    families = sorted({_decoder_family(row["decoder"]) for row in rows})
    color_cycle = plt.rcParams["axes.prop_cycle"].by_key()["color"]
    colors = {
        (family, distance): color_cycle[
            (family_index * max(len(distances), 1) + distance_index) % len(color_cycle)
        ]
        for family_index, family in enumerate(families)
        for distance_index, distance in enumerate(distances)
    }

    for row in rows:
        grouped[(row["decoder"], int(row["distance"]))].append(row)

    for (decoder, distance), items in grouped.items():
        items = sorted(items, key=lambda row: float(row["p"]))
        fits = [_logical_error_rate_fit_for_plot(row) for row in items]
        x = [float(row["p"]) for row in items]
        y_left = [fit.best if fit.best is not None else math.nan for fit in fits]
        y_right = [float(row["decode_us_per_shot"]) for row in items]
        label = f"{decoder} d={distance}"
        family = _decoder_family(decoder)
        color = colors[(family, distance)]
        line_style = _line_style_for_decoder(decoder)
        ax_left.vlines(
            x,
            [fit.low for fit in fits],
            [fit.high for fit in fits],
            color=color,
            linestyle=line_style,
            linewidth=1.0,
        )
        ax_left.plot(
            x,
            y_left,
            color=color,
            linestyle=line_style,
            marker="o",
            label=label,
        )
        ax_right.plot(
            x,
            y_right,
            color=color,
            linestyle=line_style,
            marker="o",
            label=label,
        )

    ax_left.set_xlabel("p")
    ax_left.set_ylabel("logical_error_rate")
    ax_left.set_xscale("log")
    ax_left.set_yscale("log")
    ax_left.set_title("Logical Error Rate vs p")

    ax_right.set_xlabel("p")
    ax_right.set_ylabel("decode_us_per_shot")
    ax_right.set_xscale("log")
    ax_right.set_yscale("log")
    ax_right.set_title("Decode Time vs p")


def render_plot(results_path: Path, out_path: Path) -> None:
    rows = _load_ok_rows(results_path)
    fig, (ax_left, ax_right) = plt.subplots(
        1, 2, figsize=(14, 5), constrained_layout=True
    )
    render_axes(ax_left, ax_right, rows)

    handles, labels = ax_left.get_legend_handles_labels()
    fig.legend(handles, labels, loc="upper center", ncol=3)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_path, dpi=200)
    plt.close(fig)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=("smoke", "full"), required=True)
    parser.add_argument(
        "--results-root",
        type=Path,
        default=Path("benchmarks/surface_decoder_compare/results"),
    )
    args = parser.parse_args(argv)

    tier_dir = args.results_root / args.tier
    render_plot(
        tier_dir / "results.csv",
        tier_dir / "surface_decoder_compare.png",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
