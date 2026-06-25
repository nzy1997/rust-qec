import csv
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import ModuleType
from unittest import mock

from benchmarks.bb_circuit_bposd_compare.cases import SMOKE_CASES
from benchmarks.bb_circuit_bposd_compare.run_compare import _python_row, run_suite
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
    def _run_suite_with_python_failure(
        self,
        error: Exception,
        *,
        allow_missing_python: bool = False,
    ) -> tuple[int, list[dict[str, str]]]:
        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir)
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                side_effect=error,
            ):
                status = run_suite(
                    output_dir=output_dir,
                    allow_missing_python=allow_missing_python,
                    rust_exporter=fake_export,
                )

            with (output_dir / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))
        return status, rows

    def test_python_row_uses_pinned_upstream_settings(self) -> None:
        case = replace(
            SMOKE_CASES[0],
            seed=999,
            bp_method="ps",
            max_iter=17,
            osd_method="osd0",
            osd_order=3,
        )

        class FakeVector:
            def __init__(self, values):
                self._values = list(values)

            def tolist(self):
                return list(self._values)

        class FakeMatrix:
            def __init__(self, shape):
                rows, cols = shape
                self.rows = [[0 for _ in range(cols)] for _ in range(rows)]

            def __setitem__(self, key, value):
                row_index, column_index = key
                self.rows[row_index][column_index] = value

        class FakeNumpy(ModuleType):
            uint8 = "uint8"

            def __init__(self):
                super().__init__("numpy")

            def zeros(self, shape, dtype=None):
                return FakeMatrix(shape)

            def asarray(self, values, dtype=None):
                return list(values)

        class FakeDecoder:
            calls = []

            def __init__(self, matrix, **kwargs):
                self.matrix = matrix
                self.kwargs = kwargs
                FakeDecoder.calls.append(self)

            def decode(self, syndrome):
                return FakeVector([0])

        fake_numpy = FakeNumpy()
        fake_ldpc = ModuleType("ldpc")
        fake_ldpc.BpOsdDecoder = FakeDecoder

        with mock.patch.dict("sys.modules", {"numpy": fake_numpy, "ldpc": fake_ldpc}):
            row = _python_row(case, fake_export(case))

        self.assertEqual(row["decoder_impl"], "ldpc_bposd")
        self.assertEqual(row["status"], "ok")
        self.assertEqual(row["seed"], "12345")
        self.assertEqual(row["bp_method"], "ms")
        self.assertEqual(row["max_iter"], "10000")
        self.assertEqual(row["osd_method"], "osd_cs")
        self.assertEqual(row["osd_order"], "7")
        self.assertEqual(len(FakeDecoder.calls), 2)
        for decoder in FakeDecoder.calls:
            self.assertEqual(decoder.kwargs["bp_method"], "ms")
            self.assertEqual(decoder.kwargs["max_iter"], 10000)
            self.assertEqual(decoder.kwargs["osd_method"], "osd_cs")
            self.assertEqual(decoder.kwargs["osd_order"], 7)
            self.assertEqual(decoder.kwargs["input_vector_type"], "syndrome")

    def test_run_suite_writes_skipped_python_rows_and_returns_nonzero(self) -> None:
        status, rows = self._run_suite_with_python_failure(
            ModuleNotFoundError("No module named 'ldpc'"),
        )
        self.assertNotEqual(status, 0)

        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(SMOKE_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertTrue(all(row["error"] for row in python_rows))
        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(rows)),
        )

    def test_run_suite_skips_import_error_dependency_failure(self) -> None:
        status, rows = self._run_suite_with_python_failure(
            ImportError("cannot import name 'BpOsdDecoder' from 'ldpc'"),
        )
        self.assertNotEqual(status, 0)

        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(SMOKE_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertTrue(
            all("ldpc" in row["error"] or "BpOsdDecoder" in row["error"] for row in python_rows)
        )
        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(rows)),
        )

    def test_run_suite_allows_missing_python_when_requested(self) -> None:
        status, rows = self._run_suite_with_python_failure(
            ModuleNotFoundError("No module named 'ldpc'"),
            allow_missing_python=True,
        )
        self.assertEqual(status, 0)

        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(SMOKE_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(rows)),
        )


if __name__ == "__main__":
    unittest.main()
