"""Offline controls for affine comparison evidence; not quantum witnesses."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import affine_task_compare as driver


def scalar(value=1):
    return dict(coefficients=[str(value), "0", "0", "0"], power=0)


def expected():
    return dict(raw_acceptance=scalar(), raw_error=scalar(0), raw_target_success=scalar(),
                history_mass=scalar(), target_normalization=scalar())


def valid(strategy):
    return dict(configuration=dict(max_terms=16), compilation=dict(strategy=strategy), **expected())


class AffineComparisonTests(unittest.TestCase):
    def test_compiler_validation_retains_raw_output_stderr_and_native_reference(self):
        for compiler, strategy in [("shared", "SharedCommutingLayer"), ("parity_copies", "ParityCopies")]:
            for compilation in [None, [], {}, dict(strategy="wrong"), dict(strategy=strategy)]:
                with self.subTest(compiler=compiler, compilation=compilation):
                    actual = valid(strategy)
                    actual.update(compilation=compilation, unique_marker="affine-payload")
                    payload = json.dumps(actual)
                    row = driver.compare_affine_result(subprocess.CompletedProcess([], 0, payload, "affine-stderr"),
                                                       expected(), 16, compiler)
                    self.assertNotIn("constant_word", row)
                    self.assertEqual(row["reference"], expected())
                    if compilation == dict(strategy=strategy):
                        self.assertEqual(row["status"], "matched")
                        self.assertEqual(row["rust"], actual)
                    else:
                        self.assertEqual(row["status"], "failed")
                        self.assertEqual(row["returncode"], 0)
                        self.assertEqual(row["stdout"], payload)
                        self.assertEqual(row["stderr"], "affine-stderr")
                        self.assertIn("compiler strategy", row["message"])
                        self.assertEqual(row["message"], row["error"])

    def test_bad_first_or_second_task_keeps_later_cases_inputs_and_partial_nonzero_receipt(self):
        @contextlib.contextmanager
        def native_reference(root):
            yield types.SimpleNamespace(manifest=dict(tasks=["first", "second", "external"], upstream=dict(commit="a" * 40)),
                                        manifest_sha256="b" * 64, native_sha256="c" * 64)
        def native_scalar(value=1):
            return dict(coefficients=[value, 0, 0, 0], power=0)
        original = types.SimpleNamespace(sample=lambda *args, **kwargs: dict(
            decoder_calls=[], recovery_decoder_calls=[], sampled_parities=[], exact=dict(
                acceptance=native_scalar(), error=native_scalar(0), target_success=native_scalar(), history_mass=native_scalar())))
        modules = {"witness_environment": types.SimpleNamespace(native_reference=native_reference),
                   "magna": types.SimpleNamespace(compile_circuit=lambda *args, **kwargs: original),
                   "magna.file_api": types.SimpleNamespace(NoiseDraw=lambda *args: None)}
        for compiler, strategy in [("shared", "SharedCommutingLayer"), ("parity_copies", "ParityCopies")]:
            for broken in [0, 1]:
                with self.subTest(compiler=compiler, broken=broken), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory).resolve()
                    binary = root / "binary"
                    binary.write_text("offline unchanged binary identity")
                    for name in ["first", "second", "external"]:
                        task = root / "examples" / name
                        task.mkdir(parents=True)
                        (task / "physical").write_text("offline physical input")
                        (task / "task").write_text("{}")
                        run = dict(circuit="physical", task="task", options=dict(max_terms=16))
                        if name == "external":
                            run["controllers"] = "controllers.json"
                        (task / "run.json").write_text(json.dumps(run))
                    out = root / "receipt.json"
                    calls = []
                    payloads = []
                    def rust(command, **kwargs):
                        self.assertEqual(command[command.index("--compiler") + 1], compiler)
                        index = len(calls)
                        calls.append(command)
                        actual = valid(strategy)
                        if index == broken:
                            actual["compilation"] = dict(strategy="wrong") if broken == 0 else []
                        actual["unique_marker"] = f"task-{index}"
                        payload = json.dumps(actual)
                        payloads.append(payload)
                        return subprocess.CompletedProcess(command, 0, payload, f"stderr-{index}")
                    argv = ["affine", "--upstream", str(root), "--rust-example", str(binary),
                            "--compiler", compiler, "--out", str(out)]
                    with patch.dict(sys.modules, modules), patch.object(sys, "argv", argv), patch.object(sys, "path", sys.path[:]), patch.object(driver.subprocess, "run", side_effect=rust), contextlib.redirect_stdout(io.StringIO()):
                        with self.assertRaises(SystemExit) as failure:
                            driver.main()
                    self.assertEqual(failure.exception.code, 1)
                    receipt = json.loads(out.read_text())
                    self.assertFalse(receipt["affine_subset_complete"])
                    self.assertEqual(receipt["status"], "partial")
                    self.assertEqual(receipt["compiler"], compiler)
                    self.assertEqual(receipt["native_sha256"], "c" * 64)
                    self.assertEqual(len(calls), 2)
                    self.assertEqual(len(receipt["cases"]), 3)
                    for index, row in enumerate(receipt["cases"][:2]):
                        self.assertEqual(row["status"], "failed" if index == broken else "matched")
                        self.assertEqual(row["reference"], expected())
                        self.assertEqual(len(row["inputs"]), 3)
                        self.assertTrue(all(len(value) == 64 for value in row["inputs"].values()))
                        if index == broken:
                            self.assertEqual(row["stdout"], payloads[index])
                            self.assertEqual(row["stderr"], f"stderr-{index}")
                    self.assertEqual(receipt["cases"][2]["status"], "unsupported")
