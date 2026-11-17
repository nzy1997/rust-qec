"""Render figures from the verified Magna paper data tables with Matplotlib."""

import argparse
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

from remaining_contracts import read_json


FIGURES = {
    "figure3": ("Bravyi–Haah distillation", ("conditional_error", "acceptance")),
    "figure4": ("Magic state cultivation", ("acceptance", "conditional_error")),
    "figure5": ("Logical 15-to-1 distillation", ("acceptance", "conditional_error")),
    "code-switching": ("Code switching", ("acceptance", "conditional_error")),
    "tqd": ("Twisted quantum double", ("full", "matched_physical", "matched_author", "paired_difference")),
}
METRICS = {
    "acceptance": ("Acceptance probability", "linear"),
    "conditional_error": ("Conditional infidelity", "log"),
    "full": ("Full physical infidelity", "log"),
    "matched_physical": ("Flux-matched physical infidelity", "log"),
    "matched_author": ("Flux-matched author approximation", "log"),
    "paired_difference": ("Physical minus author infidelity", "linear"),
}
ORIGINS = {
    "rust": ("Rust", "o"),
    "analytic": ("Exact", None),
    "deposit": ("Deposit", "s"),
    "literature": ("Literature", "*"),
}


def precision_figure(plt, rows, family_name, out, preview):
    from matplotlib.ticker import NullLocator

    title, metrics = FIGURES[family_name]
    fig, axes = plt.subplots(len(metrics)//2, 2, squeeze=False, figsize=(10, 4.2*(len(metrics)//2)),
                            layout="constrained")
    colors = plt.get_cmap("tab10")
    families = {family: colors(i) for i, family in enumerate(sorted({point["family"] for point in rows}))}
    for metric, axis in zip(metrics, axes.flat, strict=True):
        label, scale = METRICS[metric]
        chosen = defaultdict(list)
        for row in rows:
            if (row["metric"] == metric and row["estimate"] is not None
                    and (scale != "log" or row["estimate"] > 0)):
                chosen[row["family"], row["origin"]].append(row)
        for (family, origin), points in sorted(chosen.items()):
            points = sorted(points, key=lambda row: Fraction(row["p"]))
            name, marker = ORIGINS[origin]
            axis.errorbar([float(Fraction(row["p"])) for row in points],
                          [row["estimate"] for row in points],
                          yerr=[row["standard_error"] or 0 for row in points],
                          label=f"{family} · {name}", color=families[family], marker=marker, markersize=4,
                          capsize=2, linestyle="-" if origin == "analytic" else ":", linewidth=1)
        axis.set(xscale="log", yscale=scale, xlabel="Physical noise probability", ylabel=label)
        rates = sorted({float(Fraction(row["p"])) for row in rows if row["metric"] == metric})
        if rates and len(rates) <= 8:
            axis.set_xticks(rates, [f"{rate:.3g}" for rate in rates])
            axis.xaxis.set_minor_locator(NullLocator())
        if metric == "paired_difference":
            axis.axhline(0, color="0.65", linewidth=0.8)
        axis.grid(axis="y", which="major", alpha=0.25)
        if chosen:
            axis.legend(fontsize=7, ncols=2, loc="best")
    fig.suptitle(title + preview + "\nError bars show one standard error. The data table retains unresolved zero estimates.", fontsize=11)
    fig.savefig(out/f"{family_name}.png", dpi=180)
    plt.close(fig)


def bh_timing_figure(plt, timing, out, preview):
    fig, axes = plt.subplots(1, 2, figsize=(10, 4.2), layout="constrained")
    selected = defaultdict(list)
    for row in timing:
        if row["family"].startswith("bh"):
            size, mode = row["family"][2:].split("-")
            selected[row["backend"], mode].append((int(size), row))
    for axis, (key, label) in zip(axes, (("preparation_seconds", "Preparation (s)"),
                                        ("seconds_per_attempt", "Sampling per attempt (s)")), strict=True):
        for (backend, mode), records in sorted(selected.items()):
            order = sorted(records)
            centers = [row[key] for _, row in order]
            intervals = [row[key+"_range"] for _, row in order]
            errors = [[center-low for center, (low, _) in zip(centers, intervals, strict=True)],
                      [high-center for center, (_, high) in zip(centers, intervals, strict=True)]]
            axis.errorbar([size for size, _ in order], centers, yerr=errors, marker="o" if mode == "T" else "s",
                          markersize=4, capsize=2, label=f"{backend} · {mode}")
        axis.set(xlabel="Output count", ylabel=label, yscale="log")
        axis.grid(axis="y", which="major", alpha=0.25)
        axis.legend(fontsize=8)
    fig.suptitle("Bravyi–Haah timing" + preview + "\nBars span the observed ranges under separate host and timing protocols.", fontsize=11)
    fig.savefig(out/"figure3-timing.png", dpi=180)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, required=True, help="verified regenerated.json")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    data = read_json(args.data)
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    args.out.mkdir(parents=True, exist_ok=False)
    preview = "" if data["status"] == "PASS" else " (projection preflight)"
    for figure in FIGURES:
        precision_figure(plt, [point for point in data["points"] if point["figure"] == figure], figure, args.out, preview)
    bh_timing_figure(plt, data["timings"], args.out, preview)


if __name__ == "__main__":
    main()
