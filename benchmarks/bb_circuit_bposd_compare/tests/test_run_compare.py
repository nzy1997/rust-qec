import csv
import io
import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import ModuleType
from unittest import mock

from benchmarks.bb_circuit_bposd_compare.cases import (
    DIAGNOSTIC_CASES,
    HARD_REPLAY_CASES,
    SMOKE_CASES,
)
from benchmarks.bb_circuit_bposd_compare.run_compare import (
    _python_row,
    main,
    run_diagnostic_suite,
    run_hard_replay_suite,
    run_suite,
)
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import (
    verify_rows as verify_diagnostic_rows,
)
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


def fake_diagnostic_export(case):
    export = fake_export(case)
    export["rust_result"]["profile"].update(
        {
            "bp_seconds": 0.12,
            "osd_seconds": 0.10,
            "decode_call_count": 2,
            "bp_iteration_count": 20000,
            "osd_use_count": 1,
            "osd_candidate_count": 16,
            "gf2_solve_count": 1,
            "gf2_full_elimination_count": 1,
        }
    )
    return export


def fake_run_rust_export(case, rust_binary=None, osd_method=None):
    return fake_export(case)


FAKE_HARD_LOGICAL = [False, True, False, True, False, False, False, True]


def fake_hard_fixture():
    return {
        "case_id": HARD_REPLAY_CASES[0].case_id,
        "basis": "Z",
        "syndrome_support": [0, 2, 3],
        "expected_sampled_logical": FAKE_HARD_LOGICAL,
    }


def fake_hard_export(case):
    return {
        "code_id": "bb90",
        "physical_error_rate": 0.006,
        "num_cycles": 10,
        "num_trials": 1,
        "seed": 12345,
        "max_bp_iterations": 10000,
        "osd_order": 7,
        "rust_result": {
            "num_failed_trials": 0,
            "profile": {"setup_seconds": 0.11, "decode_seconds": 0.22},
        },
        "z_model": {
            "num_checks": 4,
            "num_bits": 4,
            "sparse_rows": [[0], [1], [2], [3]],
            "augmented_columns": [[], [], [5], [7, 11]],
            "channel_probs": [0.1, 0.1, 0.1, 0.1],
            "first_logical_row": 4,
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
                "z_syndrome": [True, False, True, True],
                "x_syndrome": [False],
                "z_logical": FAKE_HARD_LOGICAL,
                "x_logical": [False],
                "z_correction": [True, False, True, True],
                "x_correction": None,
                "z_logical_prediction": FAKE_HARD_LOGICAL,
                "x_logical_prediction": [False],
                "z_profile": {
                    "setup_seconds": 0.0,
                    "sample_seconds": 0.0,
                    "decode_seconds": 0.22,
                    "bp_seconds": 0.12,
                    "osd_seconds": 0.10,
                    "decode_call_count": 1,
                    "z_decode_call_count": 1,
                    "x_decode_call_count": 0,
                    "bp_iteration_count": 10000,
                    "osd_use_count": 1,
                    "osd_candidate_count": 4100,
                    "gf2_solve_count": 4101,
                    "gf2_full_elimination_count": 1,
                },
                "x_profile": None,
            }
        ],
    }


class FakeHardMatrix:
    def __init__(self, shape):
        rows, cols = shape
        self.rows = [[0 for _ in range(cols)] for _ in range(rows)]

    def __setitem__(self, key, value):
        row_index, column_index = key
        self.rows[row_index][column_index] = value


class FakeHardNumpy(ModuleType):
    uint8 = "uint8"

    def __init__(self):
        super().__init__("numpy")

    def zeros(self, shape, dtype=None):
        return FakeHardMatrix(shape)

    def asarray(self, values, dtype=None):
        return list(values)


class FakeHardVector:
    def __init__(self, values):
        self._values = list(values)

    def tolist(self):
        return list(self._values)


class FakeHardDecoder:
    def __init__(self, matrix, **kwargs):
        self.kwargs = kwargs

    def decode(self, syndrome):
        return FakeHardVector([True, False, True, True])


class FakeHardMismatchDecoder(FakeHardDecoder):
    def decode(self, syndrome):
        return FakeHardVector([False, False, False, True])


class FakeDiagnosticDecoder:
    def __init__(self, matrix, **kwargs):
        self.kwargs = kwargs

    def decode(self, syndrome):
        return FakeHardVector([False])


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
            self.assertEqual(decoder.kwargs["ms_scaling_factor"], 0)
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

    def test_run_suite_reraises_unrelated_import_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir)
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                side_effect=ImportError("cannot import name 'frobnicate' from 'internal_helpers'"),
            ):
                with self.assertRaisesRegex(ImportError, "frobnicate"):
                    run_suite(output_dir=output_dir, rust_exporter=fake_export)

            self.assertFalse((output_dir / "results.csv").exists())

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

    def test_main_prints_missing_dependency_error_to_stderr(self) -> None:
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._run_rust_export",
                new=fake_run_rust_export,
            ):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                    side_effect=ModuleNotFoundError("No module named 'ldpc'"),
                ):
                    with mock.patch("sys.stderr", stderr):
                        status = main(["--tier", "smoke", "--output-dir", tmpdir])

        self.assertNotEqual(status, 0)
        self.assertIn(
            "python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
            stderr.getvalue(),
        )

    def test_main_does_not_print_missing_dependency_error_when_allowed(self) -> None:
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._run_rust_export",
                new=fake_run_rust_export,
            ):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                    side_effect=ModuleNotFoundError("No module named 'ldpc'"),
                ):
                    with mock.patch("sys.stderr", stderr):
                        status = main(
                            [
                                "--tier",
                                "smoke",
                                "--output-dir",
                                tmpdir,
                                "--allow-missing-python",
                            ]
                        )

        self.assertEqual(status, 0)
        self.assertEqual(stderr.getvalue(), "")

    def test_diagnostic_suite_writes_paired_high_p_rows(self) -> None:
        fake_ldpc = ModuleType("ldpc")
        fake_ldpc.BpOsdDecoder = FakeDiagnosticDecoder

        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch.dict(
                "sys.modules",
                {"numpy": FakeHardNumpy(), "ldpc": fake_ldpc},
            ):
                status = run_diagnostic_suite(
                    Path(tmpdir),
                    rust_exporter=fake_diagnostic_export,
                )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        self.assertEqual(status, 0)
        self.assertEqual(len(rows), 4)
        self.assertEqual(
            [case.case_id for case in DIAGNOSTIC_CASES],
            [rows[0]["case_id"], rows[2]["case_id"]],
        )
        self.assertEqual(verify_diagnostic_rows(rows), [])
        rust_rows = [row for row in rows if row["decoder_impl"] == "rbposd"]
        self.assertTrue(all(row["gf2_solve_count"] == "1" for row in rust_rows))

    def test_diagnostic_suite_records_skipped_python_dependency_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
                side_effect=ModuleNotFoundError("No module named 'ldpc'"),
            ):
                status = run_diagnostic_suite(
                    Path(tmpdir),
                    rust_exporter=fake_diagnostic_export,
                )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        self.assertNotEqual(status, 0)
        python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
        self.assertEqual(len(python_rows), len(DIAGNOSTIC_CASES))
        self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
        self.assertIn(
            "Python ldpc_bposd diagnostic row is skipped",
            "\n".join(verify_diagnostic_rows(rows)),
        )

    def test_main_accepts_diagnostic_tier(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare.run_diagnostic_suite",
                return_value=0,
            ) as run_diagnostic:
                status = main(["--tier", "diagnostic", "--output-dir", tmpdir])

        self.assertEqual(status, 0)
        run_diagnostic.assert_called_once()

    def test_main_diagnostic_validation_failure_does_not_read_missing_csv(self) -> None:
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare.validate_diagnostic_cases",
                return_value=["broken diagnostic catalog"],
            ):
                with mock.patch("sys.stderr", stderr):
                    status = main(["--tier", "diagnostic", "--output-dir", tmpdir])

        self.assertEqual(status, 1)
        self.assertIn("broken diagnostic catalog", stderr.getvalue())

    def test_main_diagnostic_validation_failure_ignores_stale_results_csv(self) -> None:
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as tmpdir:
            results_path = Path(tmpdir) / "results.csv"
            results_path.write_text(
                "decoder_impl,status,error\n"
                "ldpc_bposd,skipped,python dependency unavailable for ldpc_bposd replay: stale\n"
            )
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare.validate_diagnostic_cases",
                return_value=["broken diagnostic catalog"],
            ):
                with mock.patch("sys.stderr", stderr):
                    status = main(["--tier", "diagnostic", "--output-dir", tmpdir])

        self.assertEqual(status, 1)
        self.assertIn("broken diagnostic catalog", stderr.getvalue())
        self.assertNotIn("stale", stderr.getvalue())

    def test_hard_replay_suite_writes_paired_prediction_rows(self) -> None:
        fake_ldpc = ModuleType("ldpc")
        fake_ldpc.BpOsdDecoder = FakeHardDecoder

        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch.dict("sys.modules", {"numpy": FakeHardNumpy(), "ldpc": fake_ldpc}):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._load_hard_replay_fixture",
                    side_effect=fake_hard_fixture,
                ):
                    status = run_hard_replay_suite(
                        Path(tmpdir),
                        rust_exporter=fake_hard_export,
                    )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))
            trace = json.loads((Path(tmpdir) / "hard_replay_trace.json").read_text())

        self.assertEqual(status, 0)
        self.assertEqual([row["decoder_impl"] for row in rows], ["rbposd", "ldpc_bposd"])
        self.assertEqual(rows[0]["case_id"], HARD_REPLAY_CASES[0].case_id)
        self.assertEqual(rows[0]["basis"], "Z")
        self.assertEqual(rows[0]["osd_method"], "osd_cs")
        self.assertEqual(rows[0]["logical_prediction"], rows[1]["logical_prediction"])
        self.assertEqual(json.loads(rows[0]["logical_prediction"]), FAKE_HARD_LOGICAL)
        self.assertEqual(trace["case_id"], HARD_REPLAY_CASES[0].case_id)
        self.assertEqual(trace["basis"], "Z")
        self.assertEqual(trace["classification"], "matched")
        self.assertEqual(trace["syndrome_support"], [0, 2, 3])
        self.assertEqual(
            [entry["decoder_impl"] for entry in trace["decoders"]],
            ["rbposd", "ldpc_bposd"],
        )
        rust_trace, python_trace = trace["decoders"]
        self.assertEqual(rust_trace["correction_support"], [0, 2, 3])
        self.assertEqual(rust_trace["correction_weight"], 3)
        self.assertTrue(rust_trace["residual_syndrome_matches"])
        self.assertEqual(rust_trace["profile"]["osd_candidate_count"], 4100)
        self.assertEqual(python_trace["correction_support"], [0, 2, 3])
        self.assertEqual(python_trace["correction_weight"], 3)
        self.assertTrue(python_trace["residual_syndrome_matches"])
        self.assertEqual(rows[0]["syndrome_support"], "[0,2,3]")
        self.assertEqual(rows[0]["osd_candidate_count"], "4100")
        self.assertEqual(rows[0]["gf2_solve_count"], "4101")

    def test_hard_replay_suite_writes_logical_prediction_mismatch_trace(self) -> None:
        fake_ldpc = ModuleType("ldpc")
        fake_ldpc.BpOsdDecoder = FakeHardMismatchDecoder

        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch.dict("sys.modules", {"numpy": FakeHardNumpy(), "ldpc": fake_ldpc}):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._load_hard_replay_fixture",
                    side_effect=fake_hard_fixture,
                ):
                    status = run_hard_replay_suite(
                        Path(tmpdir),
                        rust_exporter=fake_hard_export,
                    )
            trace = json.loads((Path(tmpdir) / "hard_replay_trace.json").read_text())

        self.assertEqual(status, 0)
        self.assertEqual(trace["classification"], "logical_prediction_mismatch")

    def test_hard_replay_suite_records_skipped_python_dependency_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_hard_replay_decode",
                side_effect=ModuleNotFoundError("No module named 'ldpc'"),
            ):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._load_hard_replay_fixture",
                    side_effect=fake_hard_fixture,
                ):
                    status = run_hard_replay_suite(
                        Path(tmpdir),
                        rust_exporter=fake_hard_export,
                    )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))
            trace = json.loads((Path(tmpdir) / "hard_replay_trace.json").read_text())

        self.assertNotEqual(status, 0)
        self.assertEqual(rows[1]["decoder_impl"], "ldpc_bposd")
        self.assertEqual(rows[1]["status"], "skipped")
        self.assertIn("No module named 'ldpc'", rows[1]["error"])
        self.assertEqual(trace["classification"], "incomplete")
        self.assertEqual(trace["decoders"][1]["status"], "skipped")


if __name__ == "__main__":
    unittest.main()
