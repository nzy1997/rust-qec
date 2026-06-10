import subprocess
import unittest


class SurfaceDecoderRustPlotCliTest(unittest.TestCase):
    def test_rsinter_bench_plot_cli_help_is_available(self) -> None:
        completed = subprocess.run(
            [
                "cargo",
                "run",
                "-p",
                "rsinter",
                "--bin",
                "rsinter",
                "--",
                "bench",
                "plot",
                "--help",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0)
        self.assertIn("--spec", completed.stdout)
        self.assertIn("--input", completed.stdout)
        self.assertIn("--out", completed.stdout)


if __name__ == "__main__":
    unittest.main()
