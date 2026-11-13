import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import remaining_contracts as contracts


class RemainingContractsTests(unittest.TestCase):
    def setUp(self):
        self.catalog = contracts.read_json(contracts.HERE / "remaining-contracts.json")
        self.ledger = contracts.read_json(contracts.HERE / "coverage.json")
        self.reference = contracts.reference.load_manifest()

    def check(self, catalog=None, ledger=None):
        return contracts.check_contracts(catalog or self.catalog, ledger or self.ledger, self.reference)

    def test_complete_static_inventory_does_not_certify_parity(self):
        result = self.check()
        self.assertEqual(result["outstanding"], 130)
        self.assertEqual(result["categories"], {"G": 6, "P": 32, "V": 41, "E": 49, "U": 2})
        self.assertEqual((result["tasks"], result["paper_files"], result["roadmap_issues"]), (31, 435, 18))
        self.assertEqual(result["scope"], "static_requirements_only")
        self.assertFalse(result["upstream_verified"])
        self.assertEqual(result["ledger_snapshot"], {"verified": 222, "partial": 2, "unmapped": 128})

    def test_source_missing_method_and_unwitnessed_promotions_are_rejected(self):
        changed = copy.deepcopy(self.catalog)
        changed["methods"][0]["source"]["source_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "source identity"):
            self.check(changed)
        changed = copy.deepcopy(self.catalog)
        changed["methods"].pop()
        with self.assertRaisesRegex(ValueError, "outstanding method"):
            self.check(changed)
        changed = copy.deepcopy(self.catalog)
        next(r for r in changed["methods"] if r["ledger_status"] == "partial")["ledger_status"] = "verified"
        with self.assertRaisesRegex(ValueError, "status promotion"):
            self.check(changed)
        changed = copy.deepcopy(self.ledger)
        next(r for r in changed["cases"] if r["status"] == "partial")["status"] = "verified"
        with self.assertRaisesRegex(ValueError, "status counts changed"):
            self.check(ledger=changed)

    def test_dependency_cycles_helper_pins_and_paper_version_mixing_are_rejected(self):
        mutations = [
            (lambda d: d["roadmap"]["R01"]["depends_on"].append("B06"), "cyclic"),
            (lambda d: d["helper_source_scope"].pop(next(iter(d["helper_source_scope"]))), "closure"),
            (lambda d: d["paper"].update(primary_version="2610.22315v2"), "version"),
            (lambda d: d["paper"]["files"].pop(next(iter(d["paper"]["files"]))), "paper files"),
            (lambda d: d["methods"][0]["rust_owners"][0].update(path="../outside.rs"), "relative"),
            (lambda d: d["methods"][0]["rust_owners"][0].update(anchor="missing contract owner"), "owner missing"),
        ]
        for change, expected in mutations:
            with self.subTest(expected=expected):
                altered = copy.deepcopy(self.catalog)
                change(altered)
                with self.assertRaisesRegex(ValueError, expected):
                    self.check(altered)

    def test_whole_method_and_helper_context_bind_loops_beyond_assert_names(self):
        source = """import unittest
from fixture import oracle
def helper(x): return oracle(x)
class Suite(unittest.TestCase):
    def exhaustive(self, n): return helper(n)
    def test_values(self):
        for n in range(12):
            self.assertEqual(helper(n), self.exhaustive(n))
"""
        before = contracts.method_details(source, "test_fixture.Suite.test_values")
        self.assertIn("helper", before["module_helpers"])
        self.assertIn("Suite.exhaustive", before["module_helpers"])
        self.assertIn("from fixture import oracle", before["module_imports"])
        self.assertEqual(before["loops"][0]["iterable"], "range(12)")
        after = contracts.method_details(source.replace("range(12)", "range(1)"), "test_fixture.Suite.test_values")
        self.assertEqual(before["required_checks"], after["required_checks"])
        self.assertNotEqual(before["method_sha256"], after["method_sha256"])
        self.assertNotEqual(before["loops"], after["loops"])


if __name__ == "__main__":
    unittest.main()
