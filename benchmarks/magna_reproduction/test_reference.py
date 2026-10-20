"""Offline corruption controls for the pinned reference and completion receipt."""

import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("magna_reference", HERE / "reference.py")
reference = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reference)


class ReferenceTests(unittest.TestCase):
    def test_pinned_inventory_has_complete_task_and_test_coverage(self):
        data = reference.load_manifest()
        self.assertEqual(len(data["tasks"]), 31)
        self.assertEqual(len(set(data["tasks"])), 31)
        self.assertEqual(len(data["test_modules"]), 57)
        for name in data["tasks"]:
            self.assertIn(f"examples/{name}/run.json", data["upstream"]["files"])
        for name in data["test_modules"]:
            self.assertIn(f"tests/core/{name}", data["upstream"]["files"])
        for bundles in data["paper_items"].values():
            for name in bundles:
                self.assertIn(f"data/{name}", data["paper"]["files"])

    def test_missing_altered_and_symlinked_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            file = root / "circuit.magna"
            expected = {"circuit.magna": hashlib.sha256(b"H 0\n").hexdigest()}
            with self.assertRaisesRegex(reference.ReferenceError, "missing"):
                reference.verify_files(root, expected)
            file.write_bytes(b"H 0\n")
            self.assertEqual(reference.verify_files(root, expected), 1)
            file.write_bytes(b"T 0\n")
            with self.assertRaisesRegex(reference.ReferenceError, "mismatch"):
                reference.verify_files(root, expected)
            file.unlink()
            target = root / "target"
            target.write_bytes(b"H 0\n")
            file.symlink_to(target)
            with self.assertRaisesRegex(reference.ReferenceError, "symlink"):
                reference.verify_files(root, expected)

    def test_escaping_and_ambiguous_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in ["../secret", "/tmp/secret", "a/../secret", "a//b", "./a", "a\\b", ""]:
                with self.subTest(name=name), self.assertRaises(reference.ReferenceError):
                    reference.relative_file(Path(temporary).resolve(), name)

    def test_partial_or_failed_draws_cannot_be_declared_complete(self):
        valid = {"status": "complete", "requested": 8, "completed": 8, "failed": 0,
                 "not_attempted": 0, "failures": [], "estimates": {"acceptance": 0.75, "error": 0.125}}
        reference.check_case_result(valid, 8)
        for patch in [{"status": "failed"}, {"completed": 7}, {"requested": 9}, {"failed": 1},
                      {"not_attempted": 1}, {"failures": ["error"]}, {"estimates": {"acceptance": None}},
                      {"estimates": {"acceptance": 1, "error": float("nan")}},
                      {"estimates": {"acceptance": 1, "error": -0.1}}]:
            with self.subTest(patch=patch), self.assertRaises(reference.ReferenceError):
                reference.check_case_result({**valid, **patch}, 8)

    def test_capture_retains_failed_receipt_and_rejects_binary_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for directory in ["core", "scripts", "tests/core", "examples/a", "examples/b"]:
                (root / directory).mkdir(parents=True, exist_ok=True)
            files = {}
            for case in ["a", "b"]:
                name = f"examples/{case}/run.json"
                (root / name).write_text("{}", encoding="utf-8")
                files[name] = reference.digest(root / name)
            manifest = {"upstream": {"commit": "c" * 40, "files": files},
                        "tasks": ["a", "b"], "test_modules": []}
            self.assertEqual(reference.verify_source(root, manifest)["tasks"], 2)
            extra = root / "core/injected.py"
            extra.write_text("raise RuntimeError()", encoding="utf-8")
            with self.assertRaisesRegex(reference.ReferenceError, "unlisted"):
                reference.verify_source(root, manifest)
            extra.unlink()
            result = {"status": "complete", "requested": 8, "completed": 8, "failed": 0,
                      "not_attempted": 0, "failures": [], "estimates": {"acceptance": 0.75, "error": 0.125}}
            runs = [Namespace(returncode=0, stderr="", stdout=json.dumps({"result": result, "native_sha256": sha * 64}))
                    for sha in ["a", "b"]]
            args = Namespace(case=None, upstream=root, out=root / "output", python=Path("/usr/bin/python3"),
                             shots=8, seed=7, timeout=10)
            with patch.object(reference.subprocess, "run", side_effect=runs):
                with self.assertRaisesRegex(reference.ReferenceError, "changed between"):
                    reference.capture(args, manifest)
            receipt = json.loads((args.out / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "failed")
            self.assertEqual(len(receipt["runs"]), 1)
            self.assertTrue((args.out / "b.json").is_file())
            with self.assertRaises(FileExistsError):
                reference.capture(args, manifest)

    def test_capture_preserves_venv_interpreter_and_checks_offline_receipts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = {"upstream": {"commit": "c" * 40}, "tasks": ["a", "b"]}
            result = {"status": "complete", "requested": 8, "completed": 8, "failed": 0,
                      "not_attempted": 0, "failures": [], "estimates": {"acceptance": 1, "error": 0}}
            completed = Namespace(returncode=0, stderr="", stdout=json.dumps(
                {"result": result, "native_sha256": "a" * 64}))
            interpreter = root / "venv-python"
            interpreter.symlink_to(Path("/usr/bin/python3"))
            args = Namespace(case=None, upstream=root, out=root / "output", python=interpreter,
                             shots=8, seed=7, timeout=10)
            with patch.object(reference, "verify_source", return_value={}), patch.object(
                reference.subprocess, "run", return_value=completed
            ) as run:
                reference.capture(args, manifest)
                self.assertEqual(run.call_args.args[0][0], str(interpreter))
            self.assertEqual(reference.verify_capture(args.out, manifest, True)["tasks"], 2)
            receipt_file = args.out / "receipt.json"
            receipt = json.loads(receipt_file.read_text())
            for change in [{"status": "failed"}, {"runs": receipt["runs"][:-1]},
                           {"cases": ["a"]}, {"manifest_sha256": "b" * 64},
                           {"worker_sha256": "b" * 64}, {"shots": 9}]:
                receipt_file.write_text(json.dumps({**receipt, **change}), encoding="utf-8")
                with self.subTest(change=change), self.assertRaises(reference.ReferenceError):
                    reference.verify_capture(args.out, manifest, True)
            receipt_file.write_text(json.dumps(receipt), encoding="utf-8")
            (args.out / "a.json").write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(reference.ReferenceError, "SHA-256 mismatch"):
                reference.verify_capture(args.out, manifest, True)

    def test_timeout_retains_partial_output_and_failed_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = {"upstream": {"commit": "c" * 40}, "tasks": ["a"]}
            args = Namespace(case=None, upstream=root, out=root / "output", python=Path("/usr/bin/python3"),
                             shots=8, seed=7, timeout=10)
            timeout = subprocess.TimeoutExpired("python", 10, output=b"partial", stderr=b"diagnostic")
            with patch.object(reference, "verify_source", return_value={}), patch.object(
                reference.subprocess, "run", side_effect=timeout
            ):
                with self.assertRaisesRegex(reference.ReferenceError, "exceeded"):
                    reference.capture(args, manifest)
            self.assertEqual((args.out / "a.json").read_text(), "partial")
            self.assertEqual((args.out / "a.stderr.txt").read_text(), "diagnostic")
            self.assertEqual(json.loads((args.out / "receipt.json").read_text())["status"], "failed")


if __name__ == "__main__":
    unittest.main()
