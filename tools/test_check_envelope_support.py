"""Structural validation tests for tools/check_envelope_support.py."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from tools import check_envelope_support as checker

REPO_ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = REPO_ROOT / "docs/envelope-support.json"


def load_matrix() -> dict:
    return json.loads(MATRIX_PATH.read_text(encoding="utf-8"))


class MatrixStructureTest(unittest.TestCase):
    def test_committed_matrix_is_valid(self) -> None:
        checker.validate_matrix(load_matrix())

    def test_required_controls_cover_both_decoders(self) -> None:
        matrix = load_matrix()
        for decoder, required in checker.REQUIRED_CONTROLS.items():
            for control_id in required:
                control = next(c for c in matrix["controls"] if c["id"] == control_id)
                self.assertIn(decoder, control["decoders"], control_id)

    def test_fixture_controls_point_at_existing_datasets(self) -> None:
        matrix = load_matrix()
        fixtures = [
            c["dataset"]["path"]
            for c in matrix["controls"]
            if c["dataset"].get("type") == "fixture"
        ]
        self.assertGreaterEqual(len(fixtures), 3)
        for relative in fixtures:
            dataset = REPO_ROOT / relative
            for name in ("circuit.stim", "shots.b8", "manifest.json"):
                self.assertTrue((dataset / name).is_file(), f"{relative}/{name}")

    def test_removing_a_required_control_is_rejected(self) -> None:
        matrix = load_matrix()
        matrix["controls"] = [
            c for c in matrix["controls"] if c["id"] != "repeat-block-rejection"
        ]
        with self.assertRaises(checker.MatrixError):
            checker.validate_matrix(matrix)

    def test_rejection_controls_must_pin_output_rules(self) -> None:
        matrix = load_matrix()
        target = next(c for c in matrix["controls"] if c["id"] == "repeat-block-rejection")
        del target["expected"]["stats_written"]
        with self.assertRaises(checker.MatrixError):
            checker.validate_matrix(matrix)

    def test_success_controls_must_pin_predictions(self) -> None:
        matrix = load_matrix()
        target = next(c for c in matrix["controls"] if c["id"] == "mini-circuit-known-answer")
        target["expected"] = {"outcome": "success"}
        with self.assertRaises(checker.MatrixError):
            checker.validate_matrix(matrix)

    def test_unknown_decoder_declaration_is_rejected(self) -> None:
        matrix = load_matrix()
        target = copy.deepcopy(matrix["controls"][0])
        target["decoders"] = ["envelope-matching", "envelope-unknown"]
        matrix["controls"].append({**target, "id": "bogus"})
        with self.assertRaises(checker.MatrixError):
            checker.validate_matrix(matrix)

    def test_variant_expansion_produces_stable_ids(self) -> None:
        matrix = load_matrix()
        expanded = checker.expand_controls(matrix)
        variant_ids = [c["id"] for c in expanded if ":" in c["id"]]
        self.assertEqual(len(variant_ids), 5)
        self.assertEqual(len(set(variant_ids)), 5)
        self.assertGreater(len(expanded), len(matrix["controls"]))

    def test_controls_distinguish_timeout_infeasible_and_unsupported(self) -> None:
        matrix = load_matrix()
        by_id = {c["id"]: c for c in matrix["controls"]}
        self.assertEqual(by_id["mle-solve-timeout"]["expected"]["error_code"], "decode_timeout")
        self.assertEqual(by_id["mle-solve-timeout"]["expected"]["exit_code"], 3)
        self.assertTrue(by_id["mle-solve-timeout"]["expected"]["stats_written"])
        self.assertFalse(by_id["mle-solve-timeout"]["expected"]["predictions_written"])
        self.assertEqual(by_id["mle-infeasible-shot"]["expected"]["error_code"], "decode_infeasible")
        self.assertEqual(
            by_id["conventional-mle-candidate-explosion-rejection"]["expected"]["error_code"],
            "unsupported_circuit",
        )
        self.assertFalse(
            by_id["conventional-mle-candidate-explosion-rejection"]["expected"]["stats_written"]
        )


if __name__ == "__main__":
    unittest.main()
