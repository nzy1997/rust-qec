import csv
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.bb_circuit_bposd_compare.cases import SMOKE_CASES
from benchmarks.bb_circuit_bposd_compare.run_compare import run_suite
from benchmarks.bb_circuit_bposd_compare.verify_smoke import verify_rows


def fake_export(case):
    return {
        "code_id": case.code_id,
        "physical_error_rate": case.p,
        "num_cycles": case.num_cycles,
        "num_trials": case.num_trials,
        "seed": case.seed,
        "max_bp_iterations": case.max_iter,
        "osd_order": case.osd_order,
        "rust_result": {
            "num_failed_trials": 0,
            "profile": {"setup_seconds": 0.1, "decode_seconds": 0.2},
        },
        "z_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[]],
            "augmented_columns": [[]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "x_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[]],
            "augmented_columns": [[]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "trials": [
            {
                "z_syndrome": [False],
                "x_syndrome": [False],
                "z_logical": [False],
                "x_logical": [False],
            }
        ],
    }


class RunCompareTest(unittest.TestCase):
    def test_run_suite_writes_skipped_python_rows_and_returns_nonzero(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir)
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                side_effect=ModuleNotFoundError("No module named 'ldpc'"),
            ):
                status = run_suite(output_dir=output_dir, rust_exporter=fake_export)

            self.assertNotEqual(status, 0)

            with (output_dir / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(SMOKE_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertTrue(all(row["error"] for row in python_rows))
        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(rows)),
        )

    def test_run_suite_allows_missing_python_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir)
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                side_effect=ModuleNotFoundError("No module named 'ldpc'"),
            ):
                status = run_suite(
                    output_dir=output_dir,
                    allow_missing_python=True,
                    rust_exporter=fake_export,
                )

            self.assertEqual(status, 0)

            with (output_dir / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(SMOKE_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(rows)),
        )


if __name__ == "__main__":
    unittest.main()
