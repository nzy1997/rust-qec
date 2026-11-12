"""Focused regression checks for the exact original matrix reference."""
import importlib.util
import json
from fractions import Fraction
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("original_file_matrix_reference", HERE / "original_file_matrix_reference.py")
reference = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reference)


class OriginalFileMatrixReferenceTests(unittest.TestCase):
    def test_exact_phase_and_square_root_relations(self):
        w = reference.OMEGA
        w2 = reference.multiply(w, w)
        self.assertEqual(reference.multiply(w2, w2), reference.scale(reference.ONE, -1))
        self.assertEqual(reference.multiply(w, reference.conjugate(w)), reference.ONE)
        self.assertEqual(reference.multiply(reference.SQRT_HALF, reference.SQRT_HALF),
                         reference.scale(reference.ONE, Fraction(1, 2)))

    def test_bell_pauli_oracle_retains_y_sign_and_little_endian_order(self):
        state = [reference.SQRT_HALF, reference.ZERO, reference.ZERO, reference.SQRT_HALF]
        expected = {(0, 0): 1, (3, 0): 1, (0, 3): 1, (3, 3): -1}
        for x in range(4):
            for z in range(4):
                self.assertEqual(reference.exact_moment(state, x, z),
                                 reference.scale(reference.ONE, expected.get((x, z), 0)))

    def test_unknown_components_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "outside the pinned"):
            reference.lift_component(1 / 3)
        with self.assertRaisesRegex(ValueError, "outside the pinned"):
            reference.lift_component(float("nan"))

    def test_changed_source_is_rejected_before_optional_dependency_imports(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "tests/core/test_file_api.py"
            source.parent.mkdir(parents=True)
            source.write_text("raise RuntimeError('source must remain unexecuted')\n")
            with patch.dict("sys.modules", {"numpy": None, "stim": None, "magna": None}):
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    reference.capture(root)

    def test_committed_inventory_is_complete_and_generator_bound(self):
        import hashlib
        fixture = json.loads((HERE.parents[1] / "rmagna/tests/data/original-file-matrices.json").read_text())
        self.assertEqual(fixture["source_sha256"], reference.SOURCE_SHA256)
        self.assertEqual(fixture["generator_sha256"],
                         hashlib.sha256((HERE / "original_file_matrix_reference.py").read_bytes()).hexdigest())
        self.assertEqual(len(fixture["cases"]), 28)
        self.assertEqual(sum(len(row["paulis"]) for row in fixture["cases"]), 312)
        for row in fixture["cases"]:
            masks = [[x, z] for x in range(1 << row["qubits"]) for z in range(1 << row["qubits"]) if x or z]
            self.assertEqual(row["paulis"], masks)
            self.assertEqual(len(row["exact"]), len(masks))
            self.assertEqual(len(row["numpy"]), len(masks))


if __name__ == "__main__":
    unittest.main()
