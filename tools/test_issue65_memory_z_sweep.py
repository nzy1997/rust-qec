#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "issue65_memory_z_sweep.py"


def load_module():
    spec = importlib.util.spec_from_file_location("issue65_memory_z_sweep", SCRIPT)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class FakeAxis:
    def __init__(self) -> None:
        self.calls = []

    def errorbar(self, xs, ys, yerr, marker, label, color, linestyle, linewidth, capsize):
        self.calls.append(
            {
                "xs": xs,
                "ys": ys,
                "yerr": yerr,
                "marker": marker,
                "label": label,
                "color": color,
                "linestyle": linestyle,
                "linewidth": linewidth,
                "capsize": capsize,
            }
        )

    def set_title(self, title):
        self.title = title

    def set_xlabel(self, label):
        self.xlabel = label

    def set_ylabel(self, label):
        self.ylabel = label

    def set_yscale(self, scale):
        self.yscale = scale

    def grid(self, *args, **kwargs):
        self.grid_args = (args, kwargs)

    def legend(self, *args, **kwargs):
        self.legend_args = (args, kwargs)


class FakeFigure:
    def __init__(self) -> None:
        self.saved = []

    def suptitle(self, title):
        self.title = title

    def tight_layout(self):
        self.tight = True

    def savefig(self, out, dpi):
        self.saved.append((out, dpi))


class FakePyplot(types.ModuleType):
    def __init__(self) -> None:
        super().__init__("matplotlib.pyplot")
        self.subplots_calls = []
        self.figure = FakeFigure()
        self.axes = [FakeAxis(), FakeAxis(), FakeAxis()]

    def subplots(self, rows, cols, figsize=None, sharey=None):
        self.subplots_calls.append((rows, cols, figsize, sharey))
        if cols == 1:
            return self.figure, self.axes[0]
        return self.figure, self.axes[:cols]


class Issue65MemoryZSweepPlotTest(unittest.TestCase):
    def test_add_series_plots_logical_error_rate_per_round(self) -> None:
        sweep = load_module()
        row = {
            "distance": 3,
            "rounds": 9,
            "p": 0.008,
            "shots": 47_360,
            "logical_errors": 5_006,
            "logical_error_rate": 5_006 / 47_360,
        }
        axis = FakeAxis()

        sweep.add_series(axis, "Stim/PyMatching", [row], 3, "tab:blue", "o")

        plotted_y = axis.calls[0]["ys"][0]
        expected = sweep.shot_error_rate_to_piece_error_rate(
            row["logical_error_rate"],
            row["rounds"],
        )
        self.assertAlmostEqual(plotted_y, expected)
        self.assertLess(plotted_y, row["logical_error_rate"])

    def test_plot_compare_overlays_all_distances_on_one_axis(self) -> None:
        sweep = load_module()
        fake_matplotlib = types.ModuleType("matplotlib")
        fake_matplotlib.use = lambda backend: None
        fake_pyplot = FakePyplot()

        with tempfile.TemporaryDirectory() as tmp, mock.patch.dict(
            sys.modules,
            {"matplotlib": fake_matplotlib, "matplotlib.pyplot": fake_pyplot},
        ):
            root = Path(tmp)
            stim_fixture = root / "stim.json"
            rust_results = root / "rust.jsonl"
            out = root / "plot.png"
            stim_rows = []
            rust_rows = []
            for distance in [3, 5, 7]:
                for p in [0.008, 0.009]:
                    row = {
                        "distance": distance,
                        "rounds": distance * 3,
                        "p": p,
                        "shots": 10_000,
                        "logical_errors": distance * 10 + int(p * 1000),
                        "logical_error_rate": (distance * 10 + int(p * 1000)) / 10_000,
                    }
                    stim_rows.append(row)
                    rust_rows.append(
                        {
                            "params": {"distance": distance, "rounds": distance * 3, "p": p},
                            "metrics": {
                                "shots_used": 10_000,
                                "logical_errors": distance * 11 + int(p * 1000),
                                "logical_error_rate": (distance * 11 + int(p * 1000)) / 10_000,
                            },
                            "status": "ok",
                        }
                    )
            stim_fixture.write_text(json.dumps({"rows": stim_rows}) + "\n")
            rust_results.write_text("\n".join(json.dumps(row) for row in rust_rows) + "\n")

            sweep.plot_compare(stim_fixture, [rust_results], out)

        self.assertEqual(fake_pyplot.subplots_calls, [(1, 1, (7, 5), False)])
        axis = fake_pyplot.axes[0]
        self.assertEqual(len(axis.calls), 6)
        self.assertEqual(
            [call["label"] for call in axis.calls],
            [
                "Stim/PyMatching d=3",
                "RStim/rmatching d=3",
                "Stim/PyMatching d=5",
                "RStim/rmatching d=5",
                "Stim/PyMatching d=7",
                "RStim/rmatching d=7",
            ],
        )
        self.assertEqual(axis.ylabel, "logical error rate per round")


if __name__ == "__main__":
    unittest.main()
