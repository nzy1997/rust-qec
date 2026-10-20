"""Real-process cache isolation and provenance for witness generation."""

import hashlib
import importlib.util
import json
from pathlib import Path
import py_compile
import shutil
import subprocess
import sys
import tempfile
import unittest


HERE = Path(__file__).resolve().parent


class ScalarVectorTests(unittest.TestCase):
    def test_generator_uses_verified_source_and_retains_binary_and_manifest_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            harness = root / "harness"
            source = root / "upstream"
            harness.mkdir()
            for name in ("reference.py", "witness_environment.py", "scalar_vectors.py"):
                shutil.copy2(HERE / name, harness / name)
            (source / "core/magna").mkdir(parents=True)
            (source / "examples/a").mkdir(parents=True)
            (source / "tests/core").mkdir(parents=True)
            (source / "scripts").mkdir()
            native = source / "core/magna/_native.test.so"
            native.write_bytes(b"test-only identity; no actual extension is loaded")
            # This stand-in tests import isolation and provenance, not algebra.
            module = source / "core/magna/__init__.py"
            module.write_text("from pathlib import Path\nfrom types import SimpleNamespace\n"
                              "_native = SimpleNamespace(__file__=str(Path(__file__).parent / '_native.test.so'), "
                              "algebra_scalar=lambda operation, a, b=None: a)\n", encoding="utf-8")
            task = source / "examples/a/run.json"
            task.write_text("{}", encoding="utf-8")
            manifest = {"schema": "rstim.magna-reference.v1", "upstream": {"commit": "c" * 40,
                        "files": {p.relative_to(source).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                                  for p in (module, task)}}, "tasks": ["a"], "test_modules": []}
            manifest_file = harness / "reference.json"
            manifest_file.write_text(json.dumps(manifest), encoding="utf-8")
            malicious = root / "malicious.py"
            malicious.write_text("raise RuntimeError('unverified bytecode executed')\n", encoding="utf-8")
            cache = Path(importlib.util.cache_from_source(str(module), optimization=""))
            cache.parent.mkdir()
            py_compile.compile(str(malicious), cfile=str(cache), optimize=0, doraise=True,
                               invalidation_mode=py_compile.PycInvalidationMode.UNCHECKED_HASH)
            probe = subprocess.run([sys.executable, "-I", "-c",
                f"import sys; sys.path.insert(0, {str(source / 'core')!r}); import magna"],
                capture_output=True, text=True)
            self.assertNotEqual(probe.returncode, 0)
            self.assertIn("unverified bytecode executed", probe.stderr)
            output = root / "witnesses.json"
            command = [sys.executable, "-I", str(harness / "scalar_vectors.py"),
                       "--upstream", str(source), "--out", str(output)]
            run = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            data = json.loads(output.read_text())
            self.assertEqual(data["native_sha256"], hashlib.sha256(native.read_bytes()).hexdigest())
            self.assertEqual(data["reference_manifest_sha256"], hashlib.sha256(manifest_file.read_bytes()).hexdigest())
            self.assertEqual(data["upstream_commit"], "c" * 40)
            self.assertEqual(data["seed"], 20261020)
            retained = output.read_bytes()
            rerun = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(rerun.returncode, 0)
            self.assertEqual(output.read_bytes(), retained)
            for target, message in [(task, "reference SHA-256 mismatch"), (native, "native extension changed")]:
                with self.subTest(target=target.name):
                    task.write_text("{}", encoding="utf-8")
                    native.write_bytes(b"original stand-in binary")
                    module.write_text("from pathlib import Path\nfrom types import SimpleNamespace\n"
                        f"def mutate(operation, a, b=None):\n    Path({str(target)!r}).write_bytes(b'changed')\n    return a\n"
                        "_native = SimpleNamespace(__file__=str(Path(__file__).parent / '_native.test.so'), algebra_scalar=mutate)\n",
                        encoding="utf-8")
                    for p in (module, task):
                        manifest["upstream"]["files"][p.relative_to(source).as_posix()] = hashlib.sha256(p.read_bytes()).hexdigest()
                    manifest_file.write_text(json.dumps(manifest), encoding="utf-8")
                    failed_output = root / f"failed-{target.name}.json"
                    failed = subprocess.run([*command[:-1], str(failed_output)], capture_output=True, text=True)
                    self.assertNotEqual(failed.returncode, 0)
                    self.assertIn(message, failed.stderr)
                    self.assertFalse(failed_output.exists())


if __name__ == "__main__":
    unittest.main()
