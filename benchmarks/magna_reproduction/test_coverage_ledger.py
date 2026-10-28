"""Small synthetic source fixtures exercise refusal paths without Magna imports."""

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SPEC = importlib.util.spec_from_file_location("coverage_ledger", Path(__file__).with_name("coverage_ledger.py"))
coverage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(coverage)
rust = coverage.rust


class CoverageLedgerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.upstream = self.root / "upstream"
        self.repo = self.root / "repo"
        for path in ("core", "scripts", "examples", "tests/core", "tests/fixtures/example"):
            (self.upstream / path).mkdir(parents=True, exist_ok=True)
        (self.repo / "rmagna/tests").mkdir(parents=True)
        (self.repo / "Cargo.toml").write_text("synthetic manifest\n")
        (self.repo / "Cargo.lock").write_text("synthetic lockfile\n")
        source = "import unittest\nclass Cases(unittest.TestCase):\n    def test_first(self):\n        pass\n    def test_second(self):\n        pass\n"
        fixture = "import unittest\nclass Fixture(unittest.TestCase):\n    def test_example(self):\n        pass\n"
        self.put("tests/core/test_example.py", source)
        self.put("tests/fixtures/example/test_example.py", fixture)
        self.test_source = self.repo / "rmagna/tests/witness.rs"
        self.test_source.write_text("#[test]\nfn checks_contract() { assert_eq!(1, 1); }\n")
        self.manifest = {"upstream": {"commit": "synthetic-reference", "files": {
            name: rust.sha(self.upstream / name) for name in
            ("tests/core/test_example.py", "tests/fixtures/example/test_example.py")}},
            "tasks": [], "test_modules": ["test_example.py"]}
        self.ledger = {"schema": "rstim.magna-coverage.v1", "reference_sha256": "synthetic-manifest-hash",
                       "upstream_commit": "synthetic-reference",
                       "modules": {"test_example.py": rust.sha(self.upstream / "tests/core/test_example.py")},
                       "fixture_exclusions": [{"id": "tests.fixtures.example.test_example.Fixture.test_example",
                                               "path": "tests/fixtures/example/test_example.py",
                                               "source_sha256": rust.sha(self.upstream / "tests/fixtures/example/test_example.py"),
                                               "reason": "Example fixture outside the core denominator."}],
                       "cases": [{"id": "test_example.Cases.test_first", "status": "verified",
                                  "contract": "Check the synthetic literal equality contract.",
                                  "rust_tests": [{"package": "rmagna", "target": "witness", "test": "checks_contract",
                                                  "source_sha256": rust.sha(self.test_source)}]},
                                 {"id": "test_example.Cases.test_second", "status": "unmapped",
                                  "reason": "No assertion mapping has been reviewed."}]}
        # This is deliberately synthetic parser input, never execution evidence.
        binary = self.root / "synthetic-binary-bytes"
        binary.write_bytes(b"not an executable; unit fixture only\n")
        self.run = {"schema": "rstim.magna-rust-tests.v1", "source_files": rust.source_identity(self.repo),
                    "targets": [{"package": "rmagna", "target": "witness",
                                 "binary": {"path": str(binary), "sha256": rust.sha(binary)},
                                 "list": {"stdout": "checks_contract: test\n\n1 test, 0 benchmarks\n", "stderr": "", "exit_code": 0},
                                 "run": {"stdout": "running 1 test\ntest checks_contract ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
                                         "stderr": "", "exit_code": 0}}]}

    def put(self, name, value):
        (self.upstream / name).write_text(value)

    def check(self, ledger=None, run=None):
        return coverage.validate(self.upstream, self.ledger if ledger is None else ledger, run,
                                 root=self.repo, manifest=self.manifest, reference_sha256="synthetic-manifest-hash")

    def complete_mapping(self):
        row = copy.deepcopy(self.ledger["cases"][0])
        row["id"] = self.ledger["cases"][1]["id"]
        self.ledger["cases"][1] = row

    def test_inventory_valid_is_separate_from_parity(self):
        report = self.check(run=self.run)
        self.assertTrue(report["inventory_valid"])
        self.assertFalse(report["parity_complete"])
        self.assertEqual(report["status_counts"], {"verified": 1, "unmapped": 1, "architecture_specific": 0})
        self.assertEqual(report["core_cases"], 2)
        self.assertEqual(len(report["fixture_exclusions"]), 1)
        self.assertEqual(report["modules"]["test_example.py"]["total"], 2)

    def test_completion_requires_all_mappings_and_observed_passes(self):
        self.complete_mapping()
        self.assertFalse(self.check()["parity_complete"])
        self.assertTrue(self.check(run=self.run)["parity_complete"])

    def test_missing_duplicate_and_invented_ids_are_reported_exactly(self):
        for kind in ("missing", "duplicate", "invented", "nested"):
            with self.subTest(kind=kind):
                ledger = copy.deepcopy(self.ledger)
                if kind == "missing":
                    ledger["cases"].pop()
                    field, value = "missing_ids", "test_example.Cases.test_second"
                elif kind == "duplicate":
                    ledger["cases"].append(copy.deepcopy(ledger["cases"][0]))
                    field, value = "duplicate_ids", "test_example.Cases.test_first"
                else:
                    value = "test_example.Cases.inner.test_fake" if kind == "nested" else "test_fake.Cases.test_fake"
                    ledger["cases"][1]["id"] = value
                    field = "unknown_ids"
                report = self.check(ledger)
                self.assertFalse(report["inventory_valid"])
                self.assertIn(value, report[field])

    def test_reference_module_and_fixture_hash_corruption(self):
        for key in ("reference_sha256", "upstream_commit", "modules", "fixture_exclusions"):
            with self.subTest(key=key):
                ledger = copy.deepcopy(self.ledger)
                if key == "modules":
                    ledger[key]["test_example.py"] = "0" * 64
                elif key == "fixture_exclusions":
                    ledger[key][0]["source_sha256"] = "0" * 64
                else:
                    ledger[key] = "wrong"
                self.assertFalse(self.check(ledger)["inventory_valid"])

    def test_altered_or_unlisted_upstream_source_is_rejected(self):
        self.put("tests/core/test_example.py", "# altered\n")
        self.assertFalse(self.check()["inventory_valid"])

    def test_unlisted_upstream_source_is_rejected(self):
        self.put("tests/core/extra.py", "# unlisted\n")
        self.assertFalse(self.check()["inventory_valid"])

    def test_verified_without_concrete_contract_or_witness_is_rejected(self):
        for key, value in (("contract", " \n"), ("rust_tests", []), ("rust_tests", "checks_contract")):
            with self.subTest(key=key, value=value):
                ledger = copy.deepcopy(self.ledger)
                ledger["cases"][0][key] = value
                self.assertFalse(self.check(ledger)["inventory_valid"])

    def test_unknown_completion_flags_cannot_override_validation(self):
        for key in ("parity_complete", "inventory_valid", "completed"):
            ledger = copy.deepcopy(self.ledger)
            ledger[key] = True
            self.assertFalse(self.check(ledger)["inventory_valid"])

    def test_nonexistent_nested_duplicate_and_wrong_hash_rust_witness(self):
        for key, value in (("test", "invented"), ("test", "inner::checks_contract"),
                           ("target", "../witness"), ("package", "rstim"), ("source_sha256", "0" * 64)):
            ledger = copy.deepcopy(self.ledger)
            ledger["cases"][0]["rust_tests"][0][key] = value
            self.assertFalse(self.check(ledger)["inventory_valid"])
        self.ledger["cases"][0]["rust_tests"] *= 2
        self.assertFalse(self.check()["inventory_valid"])

    def test_fixture_cannot_remove_core_cases_or_go_missing(self):
        for rows in ([], self.ledger["fixture_exclusions"] * 2,
                     [{**self.ledger["fixture_exclusions"][0], "id": self.ledger["cases"][0]["id"]}]):
            ledger = copy.deepcopy(self.ledger)
            ledger["fixture_exclusions"] = rows
            self.assertFalse(self.check(ledger)["inventory_valid"])

    def test_architecture_specific_requires_rationale_and_still_blocks_full_parity(self):
        self.ledger["cases"][1] = {"id": self.ledger["cases"][1]["id"], "status": "architecture_specific",
                                   "platform": "synthetic platform", "reason": "A reviewed platform-specific fixture.",
                                   "contract": "The exact source assertion requiring this platform."}
        report = self.check(run=self.run)
        self.assertTrue(report["inventory_valid"])
        self.assertFalse(report["parity_complete"])
        self.ledger["cases"][1]["reason"] = ""
        self.assertFalse(self.check()["inventory_valid"])

    def test_source_mismatch_and_binary_corruption_retain_raw_diagnostics(self):
        self.complete_mapping()
        for kind in ("source", "binary"):
            run = copy.deepcopy(self.run)
            if kind == "source":
                run["source_files"]["Cargo.lock"] = "0" * 64
            else:
                run["targets"][0]["binary"]["sha256"] = "0" * 64
            report = self.check(run=run)
            self.assertTrue(report["inventory_valid"])
            self.assertFalse(report["parity_complete"])
            self.assertEqual(report["rust_execution"]["raw"], run)
            self.assertTrue(report["rust_execution"]["errors"])

    def test_failure_ignored_and_missing_execution_block_completion(self):
        self.complete_mapping()
        for outcome in ("FAILED", "ignored", "missing"):
            run = copy.deepcopy(self.run)
            observed = run["targets"][0]["run"]
            if outcome == "missing":
                observed["stdout"] = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n"
            else:
                observed["stdout"] = f"running 1 test\ntest checks_contract ... {outcome}\n\ntest result: {'FAILED' if outcome == 'FAILED' else 'ok'}. 0 passed; {int(outcome == 'FAILED')} failed; {int(outcome == 'ignored')} ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
                observed["exit_code"] = 101 if outcome == "FAILED" else 0
                observed["stderr"] = "synthetic panic diagnostic" if outcome == "FAILED" else ""
            report = self.check(run=run)
            self.assertFalse(report["parity_complete"])
            key = "failed" if outcome == "FAILED" else "missing"
            self.assertEqual(report["rust_execution"][key], ["rmagna::witness::checks_contract"])
            self.assertEqual(report["rust_execution"]["raw"], run)

    def test_malformed_or_fabricated_run_counts_cannot_complete(self):
        self.complete_mapping()
        for kind in ("counts", "exit", "list", "duplicate", "bool", "unknown"):
            run = copy.deepcopy(self.run)
            target = run["targets"][0]
            if kind == "counts":
                target["run"]["stdout"] = target["run"]["stdout"].replace("1 passed", "2 passed")
            elif kind == "exit":
                target["run"]["exit_code"] = 101
            elif kind == "list":
                target["list"]["stdout"] = "invented: test\n\n1 test, 0 benchmarks\n"
            elif kind == "duplicate":
                run["targets"] *= 2
            elif kind == "bool":
                target["run"]["exit_code"] = False
            else:
                run["parity_complete"] = True
            self.assertFalse(self.check(run=run)["parity_complete"])

    def test_duplicate_json_keys_and_nonfinite_values_are_rejected(self):
        path = self.root / "input.json"
        for source in ('{"cases":[],"cases":[]}', '{"value":NaN}', '{"value":Infinity}'):
            path.write_text(source)
            with self.assertRaises(coverage.CoverageError):
                coverage.read_json(path)

    def test_nested_duplicate_and_async_upstream_declarations_are_rejected(self):
        for body in ("    def test_a(self):\n        def test_nested(): pass\n",
                     "    def test_a(self): pass\n    def test_a(self): pass\n",
                     "    async def test_a(self): pass\n"):
            with self.assertRaises(coverage.CoverageError):
                coverage.declarations("class Cases(unittest.TestCase):\n" + body, "test_x", "test_x.py", "x")

    def test_rust_literals_comments_macros_modules_and_attributes_do_not_invent_tests(self):
        source = '''/* #[test] fn comment() {} /* nested */ */
const TEXT: &str = r###"#[test] fn raw() {}"###;
const OTHER: &str = "#[test] fn quoted() {}";
macro_rules! fake { () => { #[test] fn generated() {} } }
fake!(#[test] fn in_macro() {});
mod nested { #[test] fn inside() {} }
#[cfg(any())] #[test] fn disabled() {}
#[test] #[ignore] fn skipped() {}
#[test] fn real() { let _ch = '}'; let _s = b"["; }
'''
        self.assertEqual(rust.flat_tests(source), {"real"})
        for source in ("#[test] fn a() {} #[test] fn a() {}", "/* unclosed", '#![cfg(any())]\n#[test] fn a() {}'):
            with self.assertRaises(coverage.CoverageError):
                rust.flat_tests(source)

    def test_source_symlinks_are_rejected(self):
        link = self.repo / "rmagna/tests/link.rs"
        link.symlink_to(self.test_source)
        with self.assertRaises(coverage.CoverageError):
            rust.source_identity(self.repo)


if __name__ == "__main__":
    unittest.main()
