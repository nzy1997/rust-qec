"""Render figures from the verified Magna paper data tables with Matplotlib."""

import argparse
from collections import defaultdict
from fractions import Fraction
from math import isnan, nan
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
    "rust": ("Rust", "o", ":"),
    "analytic": ("Exact", None, "-"),
    "deposit": ("Deposit", "s", ":"),
    "literature": ("Literature", "*", ":"),
    "leading_order": ("Leading order", None, "--"),
    "tomography_table": ("Published tomography", "*", "none"),
    "tomography_fit": ("Published tomography fit", None, "--"),
}
CULTIVATION_FAMILIES = ("color3", "rp2-d3-T", "color5", "rp2-d5-T")
CULTIVATION_LABELS = ("Color d=3", "RP² d=3", "Color d=5", "RP² d=5")
CULTIVATION_BACKENDS = {
    "magna": ("Magna", "terms"),
    "symft": ("SymFT", "amplitudes"),
    "clifft": ("Clifft", "amplitudes"),
    "ppvm": ("PPVM", "coefficients"),
    "rust": ("Rust", "terms"),
}


def precision_figure(plt, rows, family_name, out, preview):
    from matplotlib.ticker import NullLocator

    title, metrics = FIGURES[family_name]
    fig, axes = plt.subplots(len(metrics)//2, 2, squeeze=False, figsize=(10, 5.2*(len(metrics)//2)),
                            layout="constrained")
    colors = plt.get_cmap("tab10")
    families = {family: colors(i) for i, family in enumerate(sorted({point["family"] for point in rows}))}
    for metric, axis in zip(metrics, axes.flat, strict=True):
        label, scale = METRICS[metric]
        chosen = defaultdict(list)
        for row in rows:
            if row["metric"] == metric:
                chosen[row["family"], row["origin"]].append(row)
        displayed = False
        unresolved_label = "Unresolved"
        for (family, origin), points in sorted(chosen.items()):
            points = sorted(points, key=lambda row: Fraction(row["p"]))
            name, marker, style = ORIGINS[origin]
            rates = [float(Fraction(row["p"])) for row in points]
            values = [row["estimate"] if row["precision_resolved"] and row["estimate"] is not None
                      and (scale != "log" or row["estimate"] > 0) else nan for row in points]
            if any(not isnan(value) for value in values):
                errors = [row["standard_error"] or 0 for row in points]
                axis.errorbar(rates, values, yerr=errors if any(errors) else None,
                              label=f"{family} · {name}", color=families[family], marker=marker, markersize=4,
                              capsize=2, linestyle=style, linewidth=1)
                displayed = True
            unresolved = [rate for rate, row in zip(rates, points, strict=True) if not row["precision_resolved"]]
            if unresolved:
                axis.scatter(unresolved, [0.035]*len(unresolved), transform=axis.get_xaxis_transform(),
                             marker="v", s=19, facecolors="none", edgecolors=families[family],
                             label=unresolved_label, zorder=4)
                displayed = True
                unresolved_label = "_nolegend_"
        axis.set(xscale="log", yscale=scale, xlabel="Physical noise probability", ylabel=label)
        rates = sorted({float(Fraction(row["p"])) for row in rows
                        if row["metric"] == metric and row["origin"] != "tomography_fit"})
        if rates and len(rates) <= 8:
            axis.set_xticks(rates, [f"{rate:.3g}" for rate in rates])
            axis.xaxis.set_minor_locator(NullLocator())
        if metric == "paired_difference":
            axis.axhline(0, color="0.65", linewidth=0.8)
        axis.grid(axis="y", which="major", alpha=0.25)
        if displayed:
            axis.legend(fontsize=7, ncols=2, loc="lower center", bbox_to_anchor=(0.5, 1.02))
    fig.suptitle(title + preview + "\nError bars show one standard error. Open triangles mark unresolved points.", fontsize=11)
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


def cultivation_comparison_figure(plt, representation, timing, out, preview):
    fig, axes = plt.subplots(1, 3, figsize=(13.4, 4.2), layout="constrained")
    sizes = {(row["family"], row["backend"]): row["count"] for row in representation
             if row["status"] in ("complete", "completed")}
    for index, (backend, (label, quantity)) in enumerate(CULTIVATION_BACKENDS.items()):
        offset = (index-2)*0.13
        coordinates = [(i+offset, sizes[family, backend]) for i, family in enumerate(CULTIVATION_FAMILIES)
                       if sizes.get((family, backend)) is not None]
        if coordinates:
            x, y = zip(*coordinates, strict=True)
            axes[0].scatter(x, y, s=21, color=f"C{index}", label=f"{label} {quantity}")
    axes[0].set(title="Representation size", ylabel="Count")
    axes[0].legend(fontsize=7, loc="upper left")

    for axis, protocol, title in zip(axes[1:], ("deposited-peer-timing", "same-host-matched-peers"),
                                      ("Deposited sampling", "Same-host matched sampling"), strict=True):
        selected = {(row["family"], row["backend"]): row["seconds_per_attempt"] for row in timing
                    if row["table"] == "table2" and row["timing_protocol"] == protocol}
        for index, (backend, (label, _)) in enumerate(CULTIVATION_BACKENDS.items()):
            offset = (index-2)*0.13
            coordinates = [(i+offset, 1000*selected[family, backend]) for i, family in enumerate(CULTIVATION_FAMILIES)
                           if selected.get((family, backend)) is not None]
            if coordinates:
                x, y = zip(*coordinates, strict=True)
                axis.scatter(x, y, s=21, color=f"C{index}", label=label)
        axis.set(title=title, ylabel="Sampling per attempt (ms)")
        axis.legend(fontsize=7, loc="upper left")

    for axis in axes:
        axis.set_xticks(range(len(CULTIVATION_FAMILIES)), CULTIVATION_LABELS, rotation=17)
        axis.set_yscale("log")
        axis.grid(axis="y", which="major", alpha=0.25)
    timing_limits = [axis.get_ylim() for axis in axes[1:]]
    for axis in axes[1:]:
        axis.set_ylim(min(low for low, _ in timing_limits), max(high for _, high in timing_limits))
    fig.suptitle("Cultivation resources and sampling" + preview, fontsize=11)
    fig.savefig(out/"figure4-comparison.png", dpi=180)
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
    cultivation_comparison_figure(plt, data["cultivation_representation"], data["timings"], args.out, preview)


if __name__ == "__main__":
    main()
