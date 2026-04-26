import tempfile
import unittest
from pathlib import Path

from parity_harness import classify_mismatch, iter_generated_cases, matrix_to_dense


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


if __name__ == "__main__":
    unittest.main()
