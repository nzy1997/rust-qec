import csv
import tempfile
import unittest
from pathlib import Path

from benchmarks.surface_decoder_compare.plot_compare import (
    _logical_error_display_rate,
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


if __name__ == "__main__":
    unittest.main()
