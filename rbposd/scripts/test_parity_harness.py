import tempfile
import unittest
from pathlib import Path
from unittest import mock

from parity_harness import (
    build_entries,
    classify_mismatch,
    is_real_mismatch,
    iter_generated_cases,
    iter_lsd_fixture_cases,
    load_lsd_manifest,
    map_config_to_ldpc_kwargs,
    map_lsd_case_to_ldpc_kwargs,
    matrix_to_dense,
)


class ParityHarnessTests(unittest.TestCase):
    def test_classify_mismatch_exact_match(self) -> None:
        rust_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": False,
                "bp_iterations": 0,
                "used_osd": True,
                "residual_syndrome_weight": 0,
            },
        }
        python_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": False,
                "bp_iterations": 0,
                "used_osd": True,
                "residual_syndrome_weight": 0,
            },
        }

        self.assertEqual(classify_mismatch(rust_actual, python_actual), "exact_match")

    def test_classify_mismatch_correction_mismatch(self) -> None:
        rust_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": False,
                "bp_iterations": 0,
                "used_osd": True,
                "residual_syndrome_weight": 0,
            },
        }
        python_actual = {
            "status": "success",
            "correction": [False, True, False],
            "diagnostics": {
                "converged": False,
                "bp_iterations": 0,
                "used_osd": True,
                "residual_syndrome_weight": 0,
            },
        }

        self.assertEqual(
            classify_mismatch(rust_actual, python_actual), "correction_mismatch"
        )

    def test_classify_mismatch_diagnostics_mismatch(self) -> None:
        rust_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": False,
                "bp_iterations": 0,
                "used_osd": True,
                "residual_syndrome_weight": 0,
            },
        }
        python_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": True,
                "bp_iterations": 1,
                "used_osd": False,
                "residual_syndrome_weight": 0,
            },
        }
        self.assertEqual(
            classify_mismatch(rust_actual, python_actual), "diagnostics_mismatch"
        )

    def test_classify_mismatch_payload_mismatch(self) -> None:
        rust_actual = {"status": "unexpected_payload_shape"}
        python_actual = {"status": "unexpected_payload_shape"}
        self.assertEqual(
            classify_mismatch(rust_actual, python_actual), "payload_mismatch"
        )

    def test_payload_mismatch_counts_as_real_mismatch(self) -> None:
        self.assertTrue(is_real_mismatch("payload_mismatch"))

    def test_matrix_to_dense(self) -> None:
        matrix = {
            "num_checks": 2,
            "num_bits": 4,
            "rows": [[0, 3], [1, 2]],
        }
        self.assertEqual(
            matrix_to_dense(matrix),
            [[1, 0, 0, 1], [0, 1, 1, 0]],
        )

    def test_iter_generated_cases_includes_tiebreak_case(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            generated = list(iter_generated_cases(Path(tmp_dir)))
        names = [case["name"] for case in generated]
        self.assertIn("generated_osd_equal_reliability_tiebreak", names)

    def test_map_config_to_ldpc_kwargs_maps_contract_fields(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": True,
            "bp_variant": "minimum_sum",
            "schedule": "parallel",
            "osd_variant": "osd0",
        }
        self.assertEqual(
            map_config_to_ldpc_kwargs(config),
            {
                "max_iter": 30,
                "bp_method": "minimum_sum",
                "schedule": "parallel",
                "osd_method": "OSD_0",
                "osd_order": 0,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_config_to_ldpc_kwargs_maps_product_sum_serial_bp_method(self) -> None:
        config = {
            "max_bp_iterations": 7,
            "early_stop": True,
            "bp_variant": "product_sum",
            "schedule": "serial",
            "osd_variant": "osd0",
        }
        self.assertEqual(
            map_config_to_ldpc_kwargs(config),
            {
                "max_iter": 7,
                "bp_method": "product_sum",
                "schedule": "serial",
                "osd_method": "OSD_0",
                "osd_order": 0,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_lsd_case_to_ldpc_kwargs_maps_product_sum_serial_bp_method(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 9,
                "early_stop": True,
                "bp_variant": "product_sum",
                "schedule": "serial",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 1,
            },
        }

        self.assertEqual(
            map_lsd_case_to_ldpc_kwargs(case),
            {
                "max_iter": 9,
                "bp_method": "product_sum",
                "schedule": "serial",
                "lsd_method": "localized_statistics",
                "lsd_order": 1,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_config_to_ldpc_kwargs_rejects_unsupported_schedule(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": True,
            "bp_variant": "product_sum",
            "schedule": "flooding",
            "osd_variant": "osd0",
        }
        with self.assertRaisesRegex(ValueError, "Unsupported schedule: flooding"):
            map_config_to_ldpc_kwargs(config)

    def test_map_config_to_ldpc_kwargs_rejects_unsupported_early_stop(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": False,
            "bp_variant": "minimum_sum",
            "schedule": "parallel",
            "osd_variant": "osd0",
        }
        with self.assertRaisesRegex(ValueError, "Unsupported early_stop value"):
            map_config_to_ldpc_kwargs(config)

    def test_iter_lsd_fixture_cases_loads_manifest_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            (fixture_dir / "manifest.json").write_text(
                """
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "path": "lsd_small_sparse_code.json",
      "provenance": "unit test provenance",
      "verifier": "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
      "pass_condition": "unit test pass condition",
      "consumes": ["#90"]
    }
  ]
}
""",
                encoding="utf-8",
            )
            (fixture_dir / "lsd_small_sparse_code.json").write_text(
                """
{
  "id": "lsd_small_sparse_code",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[1, 2], [0]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "success"
  }
}
""",
                encoding="utf-8",
            )

            manifest = load_lsd_manifest(fixture_dir)
            cases = iter_lsd_fixture_cases(fixture_dir)

        self.assertEqual(manifest["fixtures"][0]["id"], "lsd_small_sparse_code")
        self.assertEqual(len(cases), 1)
        self.assertEqual(cases[0]["name"], "lsd_small_sparse_code")
        self.assertEqual(cases[0]["decoder"], "bp_lsd")
        self.assertEqual(cases[0]["lsd_config"]["method"], "localized_statistics")
        self.assertEqual(cases[0]["lsd_config"]["lsd_order"], 1)
        self.assertEqual(cases[0]["tags"], ["fixture", "lsd", "#90"])

    def test_iter_lsd_fixture_cases_rejects_empty_manifest_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            (fixture_dir / "manifest.json").write_text(
                """
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "path": "lsd_small_sparse_code.json",
      "provenance": "unit test provenance",
      "verifier": "",
      "pass_condition": "unit test pass condition",
      "consumes": ["#90"]
    }
  ]
}
""",
                encoding="utf-8",
            )
            (fixture_dir / "lsd_small_sparse_code.json").write_text(
                """
{
  "id": "lsd_small_sparse_code",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[1, 2], [0]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "success"
  }
}
""",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "verifier must not be empty"):
                iter_lsd_fixture_cases(fixture_dir)

    def test_map_lsd_case_to_ldpc_kwargs_maps_supported_lsd(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 1,
            },
        }

        self.assertEqual(
            map_lsd_case_to_ldpc_kwargs(case),
            {
                "max_iter": 30,
                "bp_method": "minimum_sum",
                "schedule": "parallel",
                "lsd_method": "localized_statistics",
                "lsd_order": 1,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_lsd_case_to_ldpc_kwargs_rejects_unsupported_lsd_order(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 2,
            },
        }

        with self.assertRaisesRegex(ValueError, "Unsupported lsd_order"):
            map_lsd_case_to_ldpc_kwargs(case)

    def test_build_entries_includes_lsd_cases_only_when_requested(self) -> None:
        lsd_case = {
            "name": "lsd_case",
            "decoder": "bp_lsd",
            "matrix": {"num_checks": 1, "num_bits": 1, "rows": [[0]]},
            "channel": {"kind": "bsc", "error_rate": 0.1},
            "syndrome": [True],
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {"method": "localized_statistics", "lsd_order": 1},
            "tags": ["fixture", "lsd", "#90"],
        }
        rust_report = {
            "actual": {
                "status": "success",
                "correction": [True],
                "diagnostics": {
                    "converged": False,
                    "bp_iterations": 30,
                    "used_osd": False,
                    "residual_syndrome_weight": 0,
                },
            }
        }
        python_actual = rust_report["actual"]

        with mock.patch("parity_harness.fixture_case_paths", return_value=[]):
            with mock.patch("parity_harness.iter_generated_cases", return_value=[]):
                with mock.patch("parity_harness.iter_lsd_fixture_cases", return_value=[lsd_case]):
                    with mock.patch("parity_harness.run_rust_case", return_value=rust_report):
                        with mock.patch(
                            "parity_harness.run_python_ldpc", return_value=python_actual
                        ):
                            without_lsd = build_entries(
                                repo_root=Path("."),
                                fixtures_dir=Path("."),
                                skip_generated=True,
                                case_limit=None,
                            )
                            with_lsd = build_entries(
                                repo_root=Path("."),
                                fixtures_dir=Path("."),
                                skip_generated=True,
                                case_limit=None,
                                include_lsd=True,
                                lsd_fixtures_dir=Path("lsd"),
                            )

        self.assertEqual(without_lsd, [])
        self.assertEqual(len(with_lsd), 1)
        self.assertEqual(with_lsd[0]["name"], "lsd_case")
        self.assertEqual(with_lsd[0]["source"], "lsd_fixture")
        self.assertEqual(with_lsd[0]["mismatch_classification"], "exact_match")

    def test_build_entries_diagnostics_drift_is_not_counted_as_mismatch(self) -> None:
        case = {
            "name": "fixture_case",
            "matrix": {"num_checks": 1, "num_bits": 1, "rows": [[0]]},
            "channel": {"kind": "bsc", "error_rate": 0.1},
            "syndrome": [True],
            "config": {
                "max_bp_iterations": 1,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "tags": ["fixture"],
        }
        rust_report = {
            "actual": {
                "status": "success",
                "correction": [True],
                "diagnostics": {
                    "converged": False,
                    "bp_iterations": 0,
                    "used_osd": True,
                    "residual_syndrome_weight": 0,
                },
            }
        }
        python_actual = {
            "status": "success",
            "correction": [True],
            "diagnostics": {
                "converged": True,
                "bp_iterations": 1,
                "used_osd": False,
                "residual_syndrome_weight": 0,
            },
        }
        with mock.patch("parity_harness.fixture_case_paths", return_value=[Path("a.json")]):
            with mock.patch("parity_harness.load_case", return_value=case):
                with mock.patch("parity_harness.run_rust_case", return_value=rust_report):
                    with mock.patch(
                        "parity_harness.run_python_ldpc", return_value=python_actual
                    ):
                        entries = build_entries(
                            repo_root=Path("."),
                            fixtures_dir=Path("."),
                            skip_generated=True,
                            case_limit=None,
                        )
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0]["mismatch_classification"], "diagnostics_mismatch")
        self.assertFalse(entries[0]["is_mismatch"])

    def test_build_entries_zero_iter_solution_drift_is_not_counted_as_mismatch(
        self,
    ) -> None:
        case = {
            "name": "fixture_case",
            "matrix": {"num_checks": 2, "num_bits": 3, "rows": [[0, 1], [1, 2]]},
            "channel": {
                "kind": "bit_flip_probabilities",
                "probabilities": [0.1, 0.2, 0.3],
            },
            "syndrome": [True, False],
            "config": {
                "max_bp_iterations": 0,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "tags": ["fixture"],
        }
        rust_report = {
            "actual": {
                "status": "success",
                "correction": [False, True, True],
                "diagnostics": {
                    "converged": False,
                    "bp_iterations": 0,
                    "used_osd": True,
                    "residual_syndrome_weight": 0,
                },
            }
        }
        python_actual = {
            "status": "success",
            "correction": [True, False, False],
            "diagnostics": {
                "converged": True,
                "bp_iterations": 2,
                "used_osd": False,
                "residual_syndrome_weight": 0,
            },
        }
        with mock.patch("parity_harness.fixture_case_paths", return_value=[Path("a.json")]):
            with mock.patch("parity_harness.load_case", return_value=case):
                with mock.patch("parity_harness.run_rust_case", return_value=rust_report):
                    with mock.patch(
                        "parity_harness.run_python_ldpc", return_value=python_actual
                    ):
                        entries = build_entries(
                            repo_root=Path("."),
                            fixtures_dir=Path("."),
                            skip_generated=True,
                            case_limit=None,
                        )
        self.assertEqual(len(entries), 1)
        self.assertEqual(
            entries[0]["mismatch_classification"], "zero_iter_semantics_mismatch"
        )
        self.assertFalse(entries[0]["is_mismatch"])


if __name__ == "__main__":
    unittest.main()
