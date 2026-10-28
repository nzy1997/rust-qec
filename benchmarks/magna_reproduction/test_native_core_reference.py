"""Refusal controls run without importing the simulator or native extension."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("native_core_reference", HERE / "native_core_reference.py")
capture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(capture)


class NativeCoreCaptureControls(unittest.TestCase):
    def test_altered_manifest_is_rejected_before_any_oracle_or_native_import(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = json.loads((HERE / "reference.json").read_text())
            manifest["upstream"]["files"] = {}
            path = root / "reference.json"
            path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "changed pinned reference manifest"):
                capture.capture(root, path)

    def test_changed_pinned_source_is_rejected_before_imports(self):
        manifest_path = HERE / "reference.json"
        manifest = json.loads(manifest_path.read_text())
        first = next(iter(manifest["upstream"]["files"]))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / first
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# deliberately changed pinned source\n")
            with self.assertRaisesRegex(ValueError, "changed pinned source"):
                capture.capture(root, manifest_path)

    def test_symlinked_pinned_source_is_rejected_even_inside_input_root(self):
        manifest_path = HERE / "reference.json"
        manifest = json.loads(manifest_path.read_text())
        first = next(iter(manifest["upstream"]["files"]))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_text("# symlink refusal precedes content verification\n")
            path = root / first
            path.parent.mkdir(parents=True, exist_ok=True)
            path.symlink_to(target)
            with self.assertRaisesRegex(ValueError, "symlink in source"):
                capture.capture(root, manifest_path)

    def test_missing_pinned_source_is_an_error_not_an_empty_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileNotFoundError):
                capture.capture(Path(directory), HERE / "reference.json")


if __name__ == "__main__":
    unittest.main()
