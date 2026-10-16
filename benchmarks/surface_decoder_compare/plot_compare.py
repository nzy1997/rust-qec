from __future__ import annotations

import csv
import os
from collections import defaultdict
from pathlib import Path
import argparse

os.environ.setdefault("MPLCONFIGDIR", str(Path("/tmp") / "codex-mpl-cache"))

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from sinter import fit_binomial


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


def _logical_error_display_rate(row: dict[str, str]) -> float:
    logical_error_rate = float(row["logical_error_rate"])
    if logical_error_rate > 0:
        return logical_error_rate

    shots_used = int(row["shots_used"])
    logical_errors = int(row["logical_errors"])
    fit = fit_binomial(
        num_shots=shots_used,
        num_hits=logical_errors,
        max_likelihood_factor=1e3,
    )
    if fit.high is not None and fit.high > 0:
        return fit.high
    return 1 / max(shots_used, 1)


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
        x = [float(row["p"]) for row in items]
        y_left = [_logical_error_display_rate(row) for row in items]
        y_right = [float(row["decode_us_per_shot"]) for row in items]
        label = f"{decoder} d={distance}"
        family = _decoder_family(decoder)
        color = colors[(family, distance)]
        line_style = _line_style_for_decoder(decoder)
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
