import tempfile
import unittest
from pathlib import Path

from benchmarks.python_runners.surface_decoder.run import main


class PythonSurfaceRunnerTest(unittest.TestCase):
    def test_main_writes_manifest_and_results(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            out_root = Path(tmpdir)
            exit_code = main(
                [
                    "--spec",
                    "benchmarks/surface_decoder/spec.toml",
                    "--language",
                    "python",
                    "--out",
                    str(out_root),
                ]
            )

            self.assertEqual(exit_code, 0)
            manifests = list(out_root.rglob("run_manifest.json"))
            results = list(out_root.rglob("results.jsonl"))
            self.assertTrue(manifests)
            self.assertTrue(results)


if __name__ == "__main__":
    unittest.main()
