import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from parity_harness import (
    build_entries,
    classify_mismatch,
    is_real_mismatch,
    iter_catalog_fixture_cases,
    iter_generated_cases,
    iter_lsd_fixture_cases,
    load_fixture_catalog,
    map_config_to_ldpc_kwargs,
    map_lsd_case_to_ldpc_kwargs,
    matrix_to_dense,
)


class ParityHarnessTests(unittest.TestCase):
    def write_catalog_fixture(self, root: Path) -> Path:
        catalog_path = root / "catalog.json"
        (root / "lsd").mkdir()
        (root / "parity").mkdir()
        catalog_path.write_text(
            """
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "kind": "lsd",
      "decoder": "bp_lsd",
      "path": "lsd/lsd_small_sparse_code.json",
      "matrix_path": "lsd/lsd_small_sparse_code.json#/matrix",
      "syndrome_path": "lsd/lsd_small_sparse_code.json#/syndrome",
      "provenance": "unit test provenance",
      "verifier": "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
      "pass_condition": "unit test pass condition",
      "consumes": ["#90", "#98"],
      "modes": ["decoder=bp_lsd", "lsd_order=1"]
    },
    {
      "id": "bp_product_sum_serial_sensitive",
      "kind": "bp_option",
      "decoder": "bp_osd",
      "path": "parity/bp_product_sum_serial_sensitive.json",
      "matrix_path": "parity/bp_product_sum_serial_sensitive.json#/matrix",
      "syndrome_path": "parity/bp_product_sum_serial_sensitive.json#/syndrome",
      "provenance": "unit test bp provenance",
      "verifier": "cargo test -p rbposd product_sum_serial_teeth_cases",
      "pass_condition": "unit test bp pass condition",
      "consumes": ["#97", "#98"],
      "modes": ["bp_variant=product_sum", "schedule=serial"]
    }
  ]
}
""",
            encoding="utf-8",
        )
        (root / "lsd" / "lsd_small_sparse_code.json").write_text(
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
        (root / "parity" / "bp_product_sum_serial_sensitive.json").write_text(
            """
{
  "name": "bp_product_sum_serial_sensitive",
  "matrix": {
    "num_checks": 3,
    "num_bits": 4,
    "rows": [[0, 1], [1, 2], [2, 3]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false, true],
  "config": {
    "max_bp_iterations": 30,
    "early_stop": true,
    "bp_variant": "product_sum",
    "schedule": "serial",
    "osd_variant": "osd0"
  },
  "expected": {
    "status": "success",
    "correction": [false, true, true, false],
    "diagnostics": {
      "converged": true,
      "bp_iterations": 3,
      "used_osd": false,
      "residual_syndrome_weight": 0
    }
  },
  "tags": ["static-baseline", "bp-only", "product-sum", "serial"]
}
""",
            encoding="utf-8",
        )
        return catalog_path

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

    def test_map_config_to_ldpc_kwargs_rejects_unsupported_bp_method(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": True,
            "bp_variant": "belief_propagation",
            "schedule": "serial",
            "osd_variant": "osd0",
        }
        with self.assertRaisesRegex(
            ValueError, "Unsupported bp_variant: belief_propagation"
        ):
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

    def test_iter_lsd_fixture_cases_loads_catalog_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            catalog_path = self.write_catalog_fixture(fixture_dir)
            catalog = load_fixture_catalog(catalog_path)
            cases = iter_lsd_fixture_cases(catalog_path)

        self.assertEqual(catalog["fixtures"][0]["id"], "lsd_small_sparse_code")
        self.assertEqual(len(cases), 1)
        self.assertEqual(cases[0]["name"], "lsd_small_sparse_code")
        self.assertEqual(cases[0]["decoder"], "bp_lsd")
        self.assertEqual(cases[0]["lsd_config"]["method"], "localized_statistics")
        self.assertEqual(cases[0]["lsd_config"]["lsd_order"], 1)
        self.assertEqual(cases[0]["tags"], ["fixture", "lsd", "#90", "#98"])

    def test_iter_lsd_fixture_cases_rejects_empty_catalog_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            catalog_path = self.write_catalog_fixture(fixture_dir)
            catalog = load_fixture_catalog(catalog_path)
            catalog["fixtures"][0]["verifier"] = ""
            catalog_path.write_text(json.dumps(catalog), encoding="utf-8")

            with self.assertRaisesRegex(
                ValueError,
                "Fixture catalog entry lsd_small_sparse_code verifier must not be empty",
            ):
                iter_lsd_fixture_cases(catalog_path)

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

    def test_map_lsd_case_to_ldpc_kwargs_rejects_unsupported_decoder_mode(self) -> None:
        case = {
            "decoder": "bp_osd",
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

        with self.assertRaisesRegex(
            ValueError, "Unsupported LSD decoder mode: bp_osd"
        ):
            map_lsd_case_to_ldpc_kwargs(case)

    def test_map_config_to_ldpc_kwargs_rejects_unsupported_osd_variant(self) -> None:
        config = {
            "max_bp_iterations": 30,
            "early_stop": True,
            "bp_variant": "minimum_sum",
            "schedule": "parallel",
            "osd_variant": "osd1",
        }

        with self.assertRaisesRegex(ValueError, "Unsupported osd_variant: osd1"):
            map_config_to_ldpc_kwargs(config)

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
                with mock.patch(
                    "parity_harness.iter_catalog_fixture_cases",
                    side_effect=lambda _catalog, include_lsd: [
                        {
                            "source": "catalog_fixture",
                            "case_path": None,
                            "case": lsd_case,
                            "catalog_path": "lsd/lsd_case.json",
                        }
                    ]
                    if include_lsd
                    else [],
                ):
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
                            )

        self.assertEqual(without_lsd, [])
        self.assertEqual(len(with_lsd), 1)
        self.assertEqual(with_lsd[0]["name"], "lsd_case")
        self.assertEqual(with_lsd[0]["source"], "catalog_fixture")
        self.assertEqual(with_lsd[0]["mismatch_classification"], "exact_match")

    def test_build_entries_uses_catalog_for_bp_option_fixture_without_duplicate(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            catalog_path = self.write_catalog_fixture(fixture_dir)
            catalog_items = iter_catalog_fixture_cases(
                catalog_path, include_lsd=False
            )
            fixture_path = fixture_dir / "parity" / "bp_product_sum_serial_sensitive.json"
            rust_report = {
                "actual": {
                    "status": "success",
                    "correction": [False, True, True, False],
                    "diagnostics": {
                        "converged": True,
                        "bp_iterations": 3,
                        "used_osd": False,
                        "residual_syndrome_weight": 0,
                    },
                }
            }
            python_actual = rust_report["actual"]

            with mock.patch(
                "parity_harness.iter_catalog_fixture_cases", return_value=catalog_items
            ):
                with mock.patch(
                    "parity_harness.fixture_case_paths", return_value=[fixture_path]
                ):
                    with mock.patch(
                        "parity_harness.run_rust_case", return_value=rust_report
                    ):
                        with mock.patch(
                            "parity_harness.run_python_ldpc", return_value=python_actual
                        ):
                            entries = build_entries(
                                repo_root=Path("."),
                                fixtures_dir=fixture_dir / "parity",
                                skip_generated=True,
                                case_limit=None,
                                fixture_catalog=catalog_path,
                            )

        names = [entry["name"] for entry in entries]
        self.assertEqual(names, ["bp_product_sum_serial_sensitive"])

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
            with mock.patch("parity_harness.iter_catalog_fixture_cases", return_value=[]):
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
            with mock.patch("parity_harness.iter_catalog_fixture_cases", return_value=[]):
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
