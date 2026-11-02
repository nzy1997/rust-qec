"""All witness entrypoints refuse unsafe outputs before a mocked capture."""

from contextlib import redirect_stderr, redirect_stdout
import ast
import io
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest import mock

import native_core_contract_reference as contracts
import native_core_reference as core
import original_noise_reference as noise
import original_ratio_rounding_vectors as ratio
import witness_paths


CLIENTS = {"ratio": ratio, "noise": noise, "contracts": contracts, "core": core}


def snapshot(root):
    result = {}
    for path in root.rglob("*"):
        name = path.relative_to(root).as_posix()
        if path.is_symlink():
            result[name] = ("link", os.readlink(path))
        elif path.is_file():
            result[name] = ("file", path.read_bytes())
        else:
            result[name] = ("directory",)
    return result


class WitnessDestinationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.serial = 0

    def layout(self):
        self.serial += 1
        root = self.root / str(self.serial)
        root.mkdir()
        upstream, rust, helper_source, helper_bin = [root / name for name in
            ("fake-upstream", "fake-rust-inputs", "fake-helper-source", "fake-helper-bin")]
        for path in (upstream, rust, helper_source, helper_bin):
            path.mkdir()
            (path / "preserved-marker").write_bytes(b"isolated fake input; preserve bytes")
        helper = helper_bin / "placeholder.so"
        helper.write_bytes(b"never imported or executed")
        record = root / "helper-record.json"
        record.write_text(json.dumps({"source_root": str(helper_source)}))
        return SimpleNamespace(root=root, upstream=upstream, rust=rust,
                               helper_source=helper_source, helper_bin=helper_bin,
                               helper=helper, record=record,
                               output=root / "new-witness", receipt=root / "new-receipt.json")

    def argv(self, kind, layout, output, receipt=None):
        args = [kind, "--upstream", str(layout.upstream), "--out", str(output)]
        if kind == "ratio":
            args += ["--receipt", str(receipt), "--helper", str(layout.helper),
                     "--helper-record", str(layout.record)]
        elif kind == "core":
            args += ["--manifest", str(layout.root / "fake-manifest.json")]
        return args

    def run_main(self, kind, layout, output, receipt=None):
        with mock.patch.object(sys, "argv", self.argv(kind, layout, output, receipt)), \
             mock.patch.object(witness_paths, "RUST_INPUT_ROOT", layout.rust), \
             redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            return CLIENTS[kind].main()

    def bad_destination(self, layout, kind):
        root = layout.root
        if kind == "upstream":
            return layout.upstream / "new-output.json"
        if kind == "rust":
            return layout.rust / "new-output.json"
        if kind in {"upstream-parent-link", "rust-parent-link"}:
            target = layout.upstream if kind.startswith("upstream") else layout.rust
            alias = root / "source-alias"
            alias.symlink_to(target, target_is_directory=True)
            return alias / "new-output.json"
        if kind == "existing-file":
            path = root / "existing.json"
            path.write_bytes(b"preserve existing witness bytes")
            return path
        if kind == "existing-directory":
            path = root / "existing-directory"
            path.mkdir()
            return path
        if kind in {"existing-file-link", "broken-link", "broken-link-into-source"}:
            target = root / "link-target.json"
            if kind == "existing-file-link":
                target.write_bytes(b"preserve existing link target")
            elif kind == "broken-link-into-source":
                target = layout.upstream / "absent-target.json"
            path = root / "output-link"
            path.symlink_to(target)
            return path
        if kind == "missing-parent":
            return root / "missing-parent" / "output.json"
        if kind == "broken-parent-link":
            path = root / "broken-parent"
            path.symlink_to(root / "absent-parent", target_is_directory=True)
            return path / "output.json"
        if kind == "file-parent":
            path = root / "not-a-directory"
            path.write_bytes(b"preserve parent file")
            return path / "output.json"
        raise AssertionError(kind)

    def test_all_entrypoints_reject_bad_destinations_before_capture(self):
        kinds = ("upstream", "rust", "upstream-parent-link", "rust-parent-link",
                 "existing-file", "existing-directory", "existing-file-link",
                 "broken-link", "broken-link-into-source", "missing-parent",
                 "broken-parent-link", "file-parent")
        for client in CLIENTS:
            for role in ("output", "receipt") if client == "ratio" else ("output",):
                for kind in kinds:
                    with self.subTest(client=client, role=role, destination=kind):
                        layout = self.layout()
                        bad = self.bad_destination(layout, kind)
                        output = bad if role == "output" else layout.output
                        receipt = bad if role == "receipt" else layout.receipt
                        before = snapshot(layout.root)
                        modules = {name for name in sys.modules
                                   if name == "magna" or name.startswith("magna.")}
                        with mock.patch.object(CLIENTS[client], "capture",
                                               side_effect=AssertionError("capture entered")) as capture:
                            try:
                                status = self.run_main(client, layout, output, receipt)
                            except ValueError:
                                pass
                            except SystemExit as error:
                                self.assertNotIn(error.code, (None, 0))
                            else:
                                self.assertNotEqual(status, 0)
                                self.assertIsNotNone(status)
                            capture.assert_not_called()
                        self.assertEqual(snapshot(layout.root), before)
                        self.assertEqual({name for name in sys.modules
                                          if name == "magna" or name.startswith("magna.")}, modules)

    def test_ratio_rejects_each_helper_tree_for_both_outputs(self):
        for role in ("output", "receipt"):
            for tree in ("helper_source", "helper_bin"):
                with self.subTest(role=role, tree=tree):
                    layout = self.layout()
                    bad = getattr(layout, tree) / "new-output.json"
                    before = snapshot(layout.root)
                    with mock.patch.object(ratio, "capture") as capture:
                        with self.assertRaises(ValueError):
                            self.run_main("ratio", layout,
                                          bad if role == "output" else layout.output,
                                          bad if role == "receipt" else layout.receipt)
                        capture.assert_not_called()
                    self.assertEqual(snapshot(layout.root), before)

    def test_ratio_rejects_equal_and_resolved_alias_destinations(self):
        for alias in (False, True):
            with self.subTest(symlink_parent_alias=alias):
                layout = self.layout()
                receipt = layout.output
                if alias:
                    parent = layout.root / "output-parent-alias"
                    parent.symlink_to(layout.root, target_is_directory=True)
                    receipt = parent / layout.output.name
                before = snapshot(layout.root)
                with mock.patch.object(ratio, "capture") as capture:
                    with self.assertRaisesRegex(ValueError, "distinct"):
                        self.run_main("ratio", layout, layout.output, receipt)
                    capture.assert_not_called()
                self.assertEqual(snapshot(layout.root), before)

    def test_ratio_basic_refusal_precedes_helper_record_loading(self):
        for role in ("output", "receipt"):
            layout = self.layout()
            bad = layout.upstream / "new-output.json"
            layout.record.write_text("deliberately malformed local helper record")
            before = snapshot(layout.root)
            with mock.patch.object(ratio.json, "loads", side_effect=AssertionError("record loaded")), \
                 mock.patch.object(ratio, "capture") as capture:
                with self.assertRaisesRegex(ValueError, "protected input trees"):
                    self.run_main("ratio", layout,
                                  bad if role == "output" else layout.output,
                                  bad if role == "receipt" else layout.receipt)
                capture.assert_not_called()
            self.assertEqual(snapshot(layout.root), before)

    def fake_capture(self, client):
        if client == "ratio":
            return ({"sentinel": "fresh witness"}, {"counts": {"mock_cases": 1}})
        if client == "noise":
            return {"rows": [], "sentinel": "fresh witness"}
        if client == "contracts":
            return ({"sentinel": "fresh witness"},
                    {"mock-family": (None, [{"value": 7}])}, "fake license", [])
        return {name: [] for name in ("contractions", "overlaps", "zero_overlap_skips",
                                     "gauss", "scalars", "boundaries", "malformed")}

    def test_fresh_outputs_publish_and_second_invocation_cannot_capture_or_overwrite(self):
        for client in CLIENTS:
            with self.subTest(client=client):
                layout = self.layout()
                alias = layout.root / "safe-output-parent"
                alias.symlink_to(layout.root, target_is_directory=True)
                output = alias / layout.output.name
                inputs = [snapshot(path) for path in
                          (layout.upstream, layout.rust, layout.helper_source, layout.helper_bin)]
                with mock.patch.object(CLIENTS[client], "capture",
                                       return_value=self.fake_capture(client)) as capture:
                    self.assertIn(self.run_main(client, layout, output, layout.receipt), (None, 0))
                    capture.assert_called_once()
                    if client == "contracts":
                        self.assertEqual(json.loads((layout.output / "mock-family.json").read_text())["vectors"],
                                         [{"value": 7}])
                        self.assertTrue((layout.output / "receipt.json").is_file())
                    else:
                        self.assertTrue(layout.output.is_file())
                    if client == "ratio":
                        data = layout.output.read_bytes()
                        receipt = json.loads(layout.receipt.read_text())
                        self.assertEqual(receipt["fixture"]["bytes"], len(data))
                        self.assertEqual(receipt["fixture"]["sha256"], hashlib.sha256(data).hexdigest())
                        self.assertEqual(json.loads(data), {"sentinel": "fresh witness"})
                    before = snapshot(layout.root)
                    try:
                        status = self.run_main(client, layout, output, layout.receipt)
                    except ValueError:
                        pass
                    except SystemExit as error:
                        self.assertNotIn(error.code, (None, 0))
                    else:
                        self.assertNotEqual(status, 0)
                        self.assertIsNotNone(status)
                    capture.assert_called_once()
                self.assertEqual(snapshot(layout.root), before)
                self.assertEqual([snapshot(path) for path in
                                  (layout.upstream, layout.rust, layout.helper_source, layout.helper_bin)], inputs)
                self.assertTrue(alias.is_symlink())

    def test_core_contract_oracles_do_not_import_the_original_test_module_or_loader(self):
        layout = self.layout()
        path = layout.upstream / "tests/core/test_native_core.py"
        path.parent.mkdir(parents=True)
        path.write_text("import tests.support.native_testing\n"
                        "raise AssertionError('test module body must never run')\n"
                        "def dense(value): return ExactScalar(value)\n"
                        "def exact(value): return scalar(value, 2, 3)\n")
        package, support, exact = [ModuleType(name) for name in
                                   ("tests", "tests.support", "tests.support.exact_scalar")]
        package.__path__ = support.__path__ = []
        exact.ExactScalar = lambda value: ("exact", value)
        exact.scalar = lambda *values: ("scalar", *values)
        with mock.patch.dict(sys.modules, {"tests": package, "tests.support": support,
                                          "tests.support.exact_scalar": exact}):
            result = contracts.literal_oracles(layout.upstream)
            self.assertEqual(result.dense(5), ("exact", 5))
            self.assertEqual(result.exact(7), ("scalar", 7, 2, 3))
            self.assertNotIn("tests.support.native_testing", sys.modules)

    def test_current_core_math_matches_the_authenticated_historical_fixture_producer(self):
        fixtures = Path(__file__).resolve().parents[2] / "rmagna/tests/fixtures"
        frozen = (fixtures / "native_core_producer.py.source").read_bytes()
        fixture = json.loads((fixtures / "native_core.json").read_text())
        self.assertEqual(hashlib.sha256(frozen).hexdigest(), fixture["producer_sha256"])

        def numerical_definitions(source):
            return {node.name: ast.dump(node) for node in ast.parse(source).body
                    if isinstance(node, ast.FunctionDef) and node.name != "main"}

        self.assertEqual(numerical_definitions(Path(core.__file__).read_bytes()),
                         numerical_definitions(frozen))


if __name__ == "__main__":
    unittest.main()
