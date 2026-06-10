import unittest
from pathlib import Path


class SurfaceDecoderCompatibilityTest(unittest.TestCase):
    def test_new_surface_benchmark_spec_exists(self) -> None:
        self.assertTrue(Path("benchmarks/surface_decoder/spec.toml").exists())

    def test_python_runner_entrypoint_exists(self) -> None:
        self.assertTrue(Path("benchmarks/python_runners/surface_decoder/run.py").exists())


if __name__ == "__main__":
    unittest.main()
