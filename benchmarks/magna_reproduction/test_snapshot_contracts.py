import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
import snapshot_contracts as contracts


class SnapshotContractsTests(unittest.TestCase):
    def setUp(self):
        self.decisions = contracts.ledger_tools.read_json(contracts.HERE / "rust-snapshot-contracts.json")
        self.ledger = contracts.ledger_tools.read_json(contracts.HERE / "coverage.json")
        self.manifest = contracts.reference.load_manifest()

    def check(self, decisions=None):
        return contracts.check_decisions(decisions or self.decisions, self.ledger, self.manifest)

    def test_all27_decisions_keep_six_partial_and_one_unmapped_gap_visible(self):
        result = self.check()
        self.assertEqual(result["methods"], 27)
        self.assertEqual(result["decisions"], {"verified": 20, "partial": 6, "unmapped": 1})
        self.assertFalse(result["upstream_verified"])

    def test_hidden_gap_promotions_and_architecture_exclusions_are_rejected(self):
        altered = copy.deepcopy(self.decisions)
        next(r for r in altered["methods"] if r["status"] == "verified")["remaining_gap"] = "still missing"
        with self.assertRaisesRegex(ValueError, "cannot be verified"):
            self.check(altered)
        altered = copy.deepcopy(self.decisions)
        altered["methods"][0]["status"] = "architecture_specific"
        with self.assertRaisesRegex(ValueError, "architecture exclusions"):
            self.check(altered)

    def test_source_witness_fixture_and_consumer_drift_are_rejected(self):
        for mutate, expected in [
            (lambda d: d["methods"][0]["source"].update(source_sha256="0" * 64), "source identity"),
            (lambda d: d["methods"][0]["rust_tests"][0].update(test="fabricated"), "witnesses differ"),
            (lambda d: d["fixtures"].update({next(iter(d["fixtures"])): "0" * 64}), "fixture changed"),
            (lambda d: d["consumers"]["R09"].update(issue=0), "consumer issue"),
        ]:
            with self.subTest(expected=expected):
                altered = copy.deepcopy(self.decisions)
                mutate(altered)
                with self.assertRaisesRegex(ValueError, expected):
                    self.check(altered)


if __name__ == "__main__":
    unittest.main()
