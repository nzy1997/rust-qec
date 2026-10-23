"""Offline controls for comparison evidence retention, not quantum witnesses."""
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import types
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

import controller_task_compare as driver


def scalar(coefficient=1):
    return dict(coefficients=[str(coefficient), "0", "0", "0"], power=0)


def expected(bit=False):
    return dict(raw_acceptance=scalar(), raw_error=scalar(0), raw_target_success=scalar(),
                history_mass=scalar(), target_normalization=scalar(),
                controller_calls=[dict(name="copy", inputs=[bit], outputs=[bit])])


def valid(bit=False):
    return dict(configuration=dict(max_terms=16, warm_replays=0, insertion_preparation="direct",
                max_insertion_table_bytes=None, max_insertion_catalog_entries=None),
                warm_replays=None, compile_ms=0, query_ms=0.25, **expected(bit))


class ControllerComparisonTests(unittest.TestCase):
    def test_insertion_admission_keeps_raw_failure_evidence_for_both_modes(self):
        from argparse import Namespace
        import insertion_options
        for mode in ["direct", "prepared"]:
            args = Namespace(insertion_preparation=mode, max_insertion_table_bytes=None,
                             max_insertion_catalog_entries=None)
            base = valid(False)
            base["configuration"].update(insertion_options.resolved(args))
            base["prepared_insertions"] = (dict(catalog_entries=2, estimated_peak_bytes=19)
                                           if mode == "prepared" else None)
            values = [base, dict(base, configuration={}), dict(base, prepared_insertions={}),
                      dict(base, prepared_insertions=dict(catalog_entries=True, estimated_peak_bytes=19))]
            for index, actual in enumerate(values):
                with self.subTest(mode=mode, index=index):
                    payload = json.dumps(actual)
                    result = subprocess.CompletedProcess([], 0, payload, "insertion-stderr")
                    row = driver.compare_result(result, expected(), 7, 16, False, insertion_args=args)
                    self.assertEqual(row["reference"], expected())
                    self.assertEqual(row["status"], "matched" if index == 0 else "failed")
                    if index:
                        self.assertEqual(row["returncode"], 0)
                        self.assertEqual(row["stdout"], payload)
                        self.assertEqual(row["stderr"], "insertion-stderr")
                        self.assertTrue(row["error"])

    def test_malformed_schema_budget_and_fields_retain_raw_output_and_reference(self):
        malformed = ["unique-malformed-output", "[]", "null"]
        for key, value in [("configuration", []), ("configuration", {"max_terms": 15}),
                           ("configuration", {"max_terms": True}), ("raw_acceptance", []),
                           ("raw_acceptance", dict(coefficients=["x"] * 4, power=0)),
                           ("history_mass", dict(coefficients=["1"] * 4, power=False)),
                           ("controller_calls", [dict(name="copy", inputs=[1], outputs=[True])])]:
            altered = valid()
            altered[key] = value
            malformed.append(json.dumps(altered))
        missing = valid()
        del missing["history_mass"]
        malformed.append(json.dumps(missing))
        for payload in malformed:
            with self.subTest(payload=payload):
                result = subprocess.CompletedProcess(["fake"], 0, payload, "unique-stderr")
                row = driver.compare_result(result, expected(), 0, 16, False)
                self.assertEqual(row["status"], "failed")
                self.assertEqual(row["returncode"], 0)
                self.assertEqual(row["stdout"], payload)
                self.assertEqual(row["stderr"], "unique-stderr")
                self.assertEqual(row["reference"], expected())
                self.assertTrue(row["error_type"])
                self.assertTrue(row["error"])
        wrong = valid()
        wrong["raw_acceptance"] = scalar(2)
        row = driver.compare_result(subprocess.CompletedProcess([], 0, json.dumps(wrong), ""), expected(), 0, 16, False)
        self.assertEqual(row["status"], "mismatch")
        self.assertEqual(row["rust"], wrong)

    def test_compiler_strategy_is_verified_without_losing_malformed_history_evidence(self):
        for compiler, strategy in [("shared", "SharedCommutingLayer"), ("parity_copies", "ParityCopies")]:
            for compilation in [None, [], {}, {"strategy": "wrong"}, {"strategy": strategy}]:
                with self.subTest(compiler=compiler, compilation=compilation):
                    actual = valid()
                    actual["compilation"] = compilation
                    payload = json.dumps(actual)
                    row = driver.compare_result(subprocess.CompletedProcess([], 0, payload, "diagnostic"),
                                                expected(), 0, 16, False, compiler=compiler)
                    if compilation == {"strategy": strategy}:
                        self.assertEqual(row["status"], "matched")
                        self.assertEqual(row["rust"], actual)
                    else:
                        self.assertEqual(row["status"], "failed")
                        self.assertEqual(row["stdout"], payload)
                        self.assertEqual(row["stderr"], "diagnostic")
                        self.assertEqual(row["reference"], expected())
                        self.assertIn("compiler strategy", row["error"])

    def test_prepared_configuration_requires_actual_mode_and_exact_integer_search_rank(self):
        for mode in ["cold", "trace", "fixed"]:
            for rank in [0, 16]:
                required_rank = rank if mode == "fixed" else None
                configurations = [dict(probability_preparation=mode, fiber_search_rank=required_rank),
                                  dict(probability_preparation="wrong", fiber_search_rank=required_rank),
                                  dict(probability_preparation=mode, fiber_search_rank=False),
                                  dict(probability_preparation=mode, fiber_search_rank="0")]
                for index, config in enumerate(configurations):
                    with self.subTest(mode=mode, rank=rank, config=config):
                        actual = valid()
                        actual["configuration"].update(config)
                        payload = json.dumps(actual)
                        row = driver.compare_result(subprocess.CompletedProcess([], 0, payload, "prepared-stderr"),
                                                    expected(), 0, 16, False,
                                                    probability_preparation=mode, fiber_search_rank=rank)
                        self.assertEqual(row["reference"], expected())
                        if index == 0:
                            self.assertEqual(row["status"], "matched")
                            self.assertEqual(row["rust"], actual)
                        else:
                            self.assertEqual(row["status"], "failed")
                            self.assertEqual(row["stdout"], payload)
                            self.assertEqual(row["stderr"], "prepared-stderr")

    def test_malformed_first_or_second_history_keeps_both_cases_and_task_provenance(self):
        # Stub only the native boundary to keep this failure-control test offline.
        # The real native differential pilot separately verifies quantum values.
        class Registry:
            def register(self, name, callback, **kwargs):
                self.callback = callback
        def compile_circuit(*args, controllers, **kwargs):
            def sample(rng, noise):
                bit = bool(rng.getrandbits(1))
                controllers.callback([bit], {})
                return dict(sampled_records=[], exact={
                    "acceptance":dict(coefficients=[1, 0, 0, 0], power=0),
                    "error":dict(coefficients=[0, 0, 0, 0], power=0),
                    "target_success":dict(coefficients=[1, 0, 0, 0], power=0),
                    "history_mass":dict(coefficients=[1, 0, 0, 0], power=0)})
            return types.SimpleNamespace(sample=sample, task={"raw": {}})
        captured = types.SimpleNamespace(manifest={"upstream": {"commit": "a" * 40}},
                                         native_sha256="b" * 64, manifest_sha256="c" * 64)
        @contextlib.contextmanager
        def native_reference(root):
            yield captured
        modules = {"magna":types.SimpleNamespace(ControllerRegistry=Registry, compile_circuit=compile_circuit),
                   "magna.file_api":types.SimpleNamespace(NoiseDraw=lambda *args: None),
                   "witness_environment":types.SimpleNamespace(native_reference=native_reference),
                   "offline_decoder":types.SimpleNamespace(decode=lambda bits, config: bits)}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            task = root / "examples/dk"
            task.mkdir(parents=True)
            (task / "controllers.json").write_text(json.dumps({"bindings": {"copy": {"import": "offline_decoder:decode", "version": "offline"}}}))
            (task / "run.json").write_text(json.dumps({"options": {"max_terms": 16}, "circuit": "circuit", "task": "task"}))
            (task / "circuit").write_text("offline fixture")
            (task / "task").write_text("{}")
            binary = root / "binary"
            binary.write_text("unchanged offline identity")
            args = Namespace(upstream=root, one="dk", timeout=1, compiler=None,
                             probability_preparation=None, density_preparation=None,
                             fiber_search_rank=16, warm_replays=0, insertion_preparation="direct",
                             max_insertion_table_bytes=None, max_insertion_catalog_entries=None)
            for broken, malformed_kind in [(i, kind) for i in [0, 1] for kind in ["schema", "warm", "insertion"]]:
                calls = []
                def rust(command, **kwargs):
                    history = json.loads(Path(command[command.index("--history") + 1]).read_text())
                    index = len(calls)
                    calls.append(history)
                    self.assertEqual(command[command.index("--warm-replays") + 1], "0")
                    actual = valid(bool(index))
                    actual["unique_marker"] = f"unique-malformed-history-{index}"
                    if index == broken and malformed_kind == "insertion":
                        actual["configuration"]["insertion_preparation"] = "prepared"
                    if index == broken and malformed_kind == "warm":
                        actual["configuration"]["warm_replays"] = False
                    payload = (f"unique-malformed-history-{index}" if index == broken and malformed_kind == "schema"
                               else json.dumps(actual))
                    return subprocess.CompletedProcess(command, 0, payload, f"stderr-{index}")
                with patch.dict(sys.modules, modules), patch.object(sys, "path", sys.path[:]), patch.object(driver.subprocess, "run", side_effect=rust):
                    row = driver.one(args, binary)
                self.assertEqual(calls, [[[False]], [[True]]])
                self.assertEqual(row["status"], "failed")
                self.assertEqual(row["upstream_commit"], "a" * 40)
                self.assertEqual(row["native_sha256"], "b" * 64)
                self.assertEqual(row["reference_manifest_sha256"], "c" * 64)
                self.assertEqual(len(row["rust_binary_sha256"]), 64)
                self.assertEqual(len(row["inputs"]), 4)
                self.assertEqual(len(row["cases"]), 2)
                for index, case in enumerate(row["cases"]):
                    self.assertEqual(case["reference"], expected(bool(index)))
                    self.assertEqual(case["status"], "failed" if index == broken else "matched")
                self.assertIn(f"unique-malformed-history-{broken}", row["cases"][broken]["stdout"])
                self.assertEqual(row["cases"][broken]["stderr"], f"stderr-{broken}")

    def test_parent_keeps_partial_nonzero_completion_for_retained_failed_cases(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "binary"
            binary.write_text("unchanged binary")
            output = root / "report.json"
            failed = dict(task="dk", status="failed", cases=[dict(status="matched"), dict(status="failed", stdout="malformed")],
                          native_sha256="b" * 64)
            process = types.SimpleNamespace(returncode=0, pid=12345,
                communicate=lambda **kwargs: (json.dumps(failed), ""))
            argv = ["compare", "--upstream", str(root), "--rust-example", str(binary), "--tasks", "dk", "--out", str(output)]
            with patch.object(sys, "argv", argv), patch.object(sys, "path", sys.path[:]), patch.object(driver.subprocess, "Popen", return_value=process), patch.object(driver.os, "killpg"), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(SystemExit) as failure:
                    driver.main()
            self.assertEqual(failure.exception.code, 1)
            receipt = json.loads(output.read_text())
            self.assertEqual(receipt["status"], "partial")
            self.assertEqual(receipt["cases"], [failed])


if __name__ == "__main__":
    unittest.main()
