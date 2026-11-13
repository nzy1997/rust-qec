"""Offline provenance, independent coordinates and pre-import output guards."""
import contextlib
import hashlib
import importlib.util
import io
import itertools
import json
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("original_kernel_matrix_reference", HERE / "original_kernel_matrix_reference.py")
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)
FIXTURE = HERE.parents[1] / "rmagna/tests/fixtures/original_kernel_matrices.json"
FIXTURE_SHA256 = "2af46fd515691818616b160437e7978486417d4c66a247c2060c12fe01b973bc"


def fixture():
    return json.loads(FIXTURE.read_text())


class OriginalKernelMatrixReferenceTests(unittest.TestCase):
    def test_provenance_and_original_schedule_inventory(self):
        raw = FIXTURE.read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), FIXTURE_SHA256)
        data = json.loads(raw)
        self.assertEqual(data["generator_sha256"], hashlib.sha256((HERE / "original_kernel_matrix_reference.py").read_bytes()).hexdigest())
        self.assertEqual(data["upstream_commit"], generator.PIN)
        self.assertEqual(data["source_sha256"], generator.SOURCES)
        self.assertEqual(data["reference_manifest_sha256"], generator.reference.digest(generator.reference.MANIFEST))
        manifest = generator.reference.load_manifest()
        self.assertEqual(len(manifest["upstream"]["files"]), 475)
        for name, sha in generator.SOURCES.items():
            self.assertEqual(manifest["upstream"]["files"][name], sha)
        self.assertEqual(data["source_files"], 475)
        self.assertEqual(data["seed"], 26092393)
        self.assertEqual(data["paulis"], [[1, 0], [1, 1], [0, 1], [3, 2]])
        self.assertEqual(len(data["rows"]), 96)
        for trial in range(24):
            rows = data["rows"][trial*4:trial*4+4]
            self.assertEqual([row["true_values"] for row in rows], [dict(a=a, b=b) for a, b in itertools.product(range(2), repeat=2)])
            for row in rows:
                self.assertEqual(row["trial"], trial)
                self.assertEqual(row["inputs"], [0, 1, 2])
                self.assertEqual(row["outputs"], [1, 0])
                self.assertEqual(row["initial"], dict(width=3, base=5, columns=[3, 6], linear=[1, 3], upper=[2, 0], power=-2, phase=7))
                self.assertEqual([op["id"] for op in row["operations"] if op["id"].startswith("g")], [f"g{j}" for j in range(10)])
                self.assertEqual([fault["pauli"] for fault in row["faults"]], list(("XZ", "ZX", "Y", "")[trial % 4]))
                self.assertEqual(row["report_flips"], ["a", "a", "b"] if trial % 2 else [])
        self.assertRegex(data["native_frontend_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(data["numpy_version"], "2.4.6")
        self.assertEqual(data["python_version"], "3.12.14")

    def test_independent_standard_library_norms_paulis_and_phase_anchor(self):
        data = fixture()
        first = complex(*data["rows"][0]["vector"][0])
        self.assertLess(abs(first - complex(-0.125, -0.125)), 4e-13)
        for trial in range(24):
            total = 0.0
            for row in data["rows"][trial*4:trial*4+4]:
                self.assertEqual(len(row["vector"]), 8)
                vector = [complex(*v) for v in row["vector"]]
                self.assertTrue(all(math.isfinite(v.real) and math.isfinite(v.imag) for v in vector))
                norm = sum(v.real*v.real + v.imag*v.imag for v in vector)
                self.assertLess(abs(norm - row["probability"]), 4e-13)
                total += norm
                self.assertEqual(len(row["moments"]), 4)
                for (x, z), stored in zip(data["paulis"], row["moments"]):
                    moment = sum(vector[y ^ x].conjugate() * v * (1j)**((x & z).bit_count())
                                 * (-1)**((z & y).bit_count()) for y, v in enumerate(vector))
                    self.assertLess(abs(moment - complex(*stored)), 4e-13)
                    self.assertLess(abs(moment.imag), 4e-13)
            self.assertLess(abs(total - 1), 4e-13)

    def reject_output(self, source, output):
        with patch.object(sys, "argv", ["generator", "--upstream", str(source), "--out", str(output)]):
            with patch.object(generator, "capture", side_effect=AssertionError("optional imports reached")) as capture:
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                    generator.main()
                self.assertEqual(error.exception.code, 2)
                capture.assert_not_called()

    def test_output_guards_precede_optional_imports(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            existing = root / "existing.json"
            existing.write_text("keep")
            broken = root / "broken.json"
            broken.symlink_to(root / "absent")
            alias = root / "source-alias"
            alias.symlink_to(source, target_is_directory=True)
            protected = [generator.reference.HERE / "fresh-kernel-output.json",
                         generator.reference.HERE.parents[1] / "rmagna/tests/fixtures/fresh-kernel-output.json"]
            for path in protected:
                self.assertFalse(path.exists() or path.is_symlink())
            for path in [existing, broken, alias / "new.json", root / "missing-parent/out.json", *protected]:
                with self.subTest(path=path):
                    self.reject_output(source, path)
            for path in protected:
                self.assertFalse(path.exists() or path.is_symlink())
            self.assertEqual(existing.read_text(), "keep")
            self.assertTrue(broken.is_symlink())
            self.assertEqual(list(source.iterdir()), [])

    def test_changed_source_hash_precedes_native_context_or_output_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            path = source / "original-test.py"
            path.write_bytes(b"changed original test")
            destination = root / "new.json"
            # Small synthetic source isolates the post-inventory test-hash gate.
            # The real 475-entry inventory is checked during reference generation
            # and independently bound in the committed fixture test above.
            sources = {"original-test.py": hashlib.sha256(b"original test").hexdigest()}
            with patch.object(generator, "SOURCES", sources), patch.object(generator.reference, "verify_source", return_value={"source_files":475}):
                with patch.object(generator, "native_reference", side_effect=AssertionError("native imports reached")) as native:
                    with patch.object(sys, "argv", ["generator", "--upstream", str(source), "--out", str(destination)]):
                        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                            generator.main()
                        self.assertEqual(error.exception.code, 2)
                    native.assert_not_called()
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
