import csv
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import matplotlib.colors as mcolors

from benchmarks.surface_decoder_compare.plot_compare import (
    _logical_error_display_rate,
    _load_ok_rows,
    _decoder_family,
    _line_style_for_decoder,
    main,
    render_axes,
    render_plot,
)


class PlotCompareTest(unittest.TestCase):
    def test_zero_logical_error_rate_uses_positive_sinter_upper_bound(self) -> None:
        display = _logical_error_display_rate(
            {
                "shots_used": "2000",
                "logical_errors": "0",
                "logical_error_rate": "0.0",
            }
        )
        self.assertGreater(display, 0.0)

    def test_render_axes_uses_log_scaled_x_axis(self) -> None:
        import matplotlib.pyplot as plt

        fig, (ax_left, ax_right) = plt.subplots(1, 2)
        try:
            render_axes(
                ax_left,
                ax_right,
                rows=[
                    {
                        "decoder": "pymatching",
                        "distance": "3",
                        "p": "0.001",
                        "logical_errors": "0",
                        "logical_error_rate": "0.0",
                        "shots_used": "2000",
                        "decode_us_per_shot": "0.05",
                        "status": "ok",
                    },
                    {
                        "decoder": "pymatching",
                        "distance": "3",
                        "p": "0.002",
                        "logical_errors": "1",
                        "logical_error_rate": "0.0005",
                        "shots_used": "2000",
                        "decode_us_per_shot": "0.06",
                        "status": "ok",
                    },
                ],
            )
            self.assertEqual(ax_left.get_xscale(), "log")
            self.assertEqual(ax_right.get_xscale(), "log")
        finally:
            plt.close(fig)

    def test_decoder_family_shares_color_within_family(self) -> None:
        self.assertEqual(_decoder_family("pymatching"), "mwpm")
        self.assertEqual(_decoder_family("rmatching"), "mwpm")
        self.assertEqual(_decoder_family("ilpqec"), "ilp")
        self.assertEqual(_decoder_family("rilpqec"), "ilp")
        self.assertEqual(_decoder_family("ldpc"), "bp")
        self.assertEqual(_decoder_family("rbposd"), "bp")

    def test_line_style_distinguishes_matching_decoder_pairs(self) -> None:
        self.assertEqual(_line_style_for_decoder("ldpc"), "-")
        self.assertEqual(_line_style_for_decoder("rbposd"), "--")
        self.assertEqual(_line_style_for_decoder("pymatching"), "-")
        self.assertEqual(_line_style_for_decoder("rmatching"), "--")

    def test_render_axes_uses_same_color_for_same_family_at_same_distance(self) -> None:
        import matplotlib.pyplot as plt

        fig, (ax_left, ax_right) = plt.subplots(1, 2)
        try:
            render_axes(
                ax_left,
                ax_right,
                rows=[
                    {
                        "decoder": "ldpc",
                        "distance": "3",
                        "p": "0.002",
                        "logical_errors": "1",
                        "logical_error_rate": "0.0005",
                        "shots_used": "2000",
                        "decode_us_per_shot": "1.0",
                        "status": "ok",
                    },
                    {
                        "decoder": "rbposd",
                        "distance": "3",
                        "p": "0.002",
                        "logical_errors": "2",
                        "logical_error_rate": "0.0010",
                        "shots_used": "2000",
                        "decode_us_per_shot": "2.0",
                        "status": "ok",
                    },
                ],
            )
            left_lines = ax_left.get_lines()
            self.assertEqual(len(left_lines), 2)
            self.assertEqual(
                mcolors.to_hex(left_lines[0].get_color()),
                mcolors.to_hex(left_lines[1].get_color()),
            )
            self.assertNotEqual(
                left_lines[0].get_linestyle(),
                left_lines[1].get_linestyle(),
            )
        finally:
            plt.close(fig)

    def test_render_plot_writes_a_png(self) -> None:
        rows = [
            {
                "tier": "smoke",
                "decoder": "pymatching",
                "backend": "native",
                "distance": 3,
                "rounds": 3,
                "p": 0.001,
                "seed": 12345,
                "num_dets": 8,
                "num_obs": 1,
                "shots_budget": 2000,
                "errors_budget": 20,
                "shots_used": 2000,
                "logical_errors": 0,
                "logical_error_rate": 0.0,
                "compile_us": 10.0,
                "total_decode_us": 100.0,
                "decode_us_per_shot": 0.05,
                "status": "ok",
                "error": "",
            },
            {
                "tier": "smoke",
                "decoder": "pymatching",
                "backend": "native",
                "distance": 5,
                "rounds": 5,
                "p": 0.001,
                "seed": 12345,
                "num_dets": 32,
                "num_obs": 1,
                "shots_budget": 2000,
                "errors_budget": 20,
                "shots_used": 2000,
                "logical_errors": 14,
                "logical_error_rate": 0.007,
                "compile_us": 11.0,
                "total_decode_us": 140.0,
                "decode_us_per_shot": 0.07,
                "status": "ok",
                "error": "",
            },
            {
                "tier": "smoke",
                "decoder": "pymatching",
                "backend": "native",
                "distance": 3,
                "rounds": 3,
                "p": 0.002,
                "seed": 12345,
                "num_dets": 8,
                "num_obs": 1,
                "shots_budget": 2000,
                "errors_budget": 20,
                "shots_used": 2000,
                "logical_errors": 3,
                "logical_error_rate": 0.0015,
                "compile_us": 10.0,
                "total_decode_us": 110.0,
                "decode_us_per_shot": 0.055,
                "status": "ok",
                "error": "",
            },
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            results_path = root / "results.csv"
            with results_path.open("w", newline="") as handle:
                writer = csv.DictWriter(handle, fieldnames=list(rows[0].keys()))
                writer.writeheader()
                writer.writerows(rows)

            out_path = root / "surface_decoder_compare.png"
            render_plot(results_path, out_path)
            self.assertTrue(out_path.exists())
            self.assertGreater(out_path.stat().st_size, 0)

    def test_load_ok_rows_filters_out_errors(self) -> None:
        rows = [
            {"decoder": "a", "status": "ok"},
            {"decoder": "b", "status": "error"},
        ]
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "results.csv"
            with path.open("w", newline="") as handle:
                writer = csv.DictWriter(handle, fieldnames=["decoder", "status"])
                writer.writeheader()
                writer.writerows(rows)
            loaded = _load_ok_rows(path)
        self.assertEqual(loaded, [{"decoder": "a", "status": "ok"}])

    @mock.patch("benchmarks.surface_decoder_compare.plot_compare.render_plot")
    def test_main_renders_requested_tier(self, render_plot_mock: mock.Mock) -> None:
        exit_code = main(["--tier", "full"])
        self.assertEqual(exit_code, 0)
        results_path, out_path = render_plot_mock.call_args.args
        self.assertEqual(
            results_path,
            Path("benchmarks/surface_decoder_compare/results/full/results.csv"),
        )
        self.assertEqual(
            out_path,
            Path("benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png"),
        )


if __name__ == "__main__":
    unittest.main()
