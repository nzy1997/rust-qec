import csv
import json
import tempfile
import unittest
from pathlib import Path

from benchmarks.run_minimal_benchmark import run_rmatching
from benchmarks.run_surface_dem_benchmark import run_pymatching, summarize_case, write_outputs
from benchmarks.surface_dem_cases import build_cases


class RunSurfaceDemBenchmarkTest(unittest.TestCase):
    def test_rmatching_decodes_known_d17_regression_syndrome(self):
        case = build_cases(shots=64, seed=12345)[1]
        single_shot_case = type(
            "SingleShotCase",
            (),
            {"dem": case.dem, "syndromes": [case.syndromes[6]]},
        )
        py_stats = run_pymatching(single_shot_case, warmup_rounds=0, measure_rounds=1)
        stats = run_rmatching(single_shot_case, warmup_rounds=0, measure_rounds=1)

        self.assertEqual(len(stats["predictions"]), 1)
        self.assertEqual(stats["predictions"], py_stats["predictions"])

    def test_summarize_case_reports_accuracy_against_observables(self):
        row, mismatches = summarize_case(
            case_name="surface-d5-p0.001",
            distance=5,
            p=0.001,
            num_detectors=120,
            expected_predictions=[[0], [1], [0]],
            py_predictions=[[0], [1], [0]],
            rm_predictions=[[0], [0], [0]],
            py_stats={
                "build_us": 15.0,
                "mean_decode_us": 4.0,
                "median_decode_us": 4.0,
                "p95_decode_us": 5.0,
            },
            rm_stats={
                "build_us": 10.0,
                "mean_decode_us": 6.0,
                "median_decode_us": 6.0,
                "p95_decode_us": 8.0,
            },
            syndromes=[[1, 0], [0, 1], [1, 1]],
        )
        self.assertEqual(len(row), 2)
        self.assertEqual(row[0]["decoder"], "pymatching")
        self.assertEqual(row[0]["prediction_match_rate"], 1.0)
        self.assertEqual(row[1]["decoder"], "rmatching")
        self.assertEqual(row[1]["prediction_match_rate"], 2 / 3)
        self.assertEqual(row[1]["mismatch_cases"], 1)
        self.assertEqual(mismatches[0]["syndrome"], [0, 1])

    def test_write_outputs_writes_surface_artifacts(self):
        rows = [
            {
                "case_name": "surface-d5-p0.001",
                "decoder": "pymatching",
                "p": 0.001,
                "d": 5,
                "num_detectors": 120,
                "num_syndromes_tested": 8,
                "prediction_match_rate": 1.0,
                "mismatch_cases": 0,
                "build_us": 10.0,
                "mean_decode_us": 1.0,
                "median_decode_us": 1.0,
                "p95_decode_us": 2.0,
            }
        ]
        mismatches = []
        with tempfile.TemporaryDirectory() as tmpdir:
            results_path = Path(tmpdir) / "surface_dem_results.csv"
            mismatch_path = Path(tmpdir) / "surface_dem_mismatches.json"
            write_outputs(rows, mismatches, results_path=results_path, mismatch_path=mismatch_path)

            with results_path.open() as handle:
                written_rows = list(csv.DictReader(handle))
            self.assertEqual(written_rows[0]["case_name"], "surface-d5-p0.001")
            self.assertEqual(json.loads(mismatch_path.read_text()), [])


if __name__ == "__main__":
    unittest.main()
