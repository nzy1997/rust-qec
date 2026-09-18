#!/usr/bin/env python3
"""Offline plan-validation suite for the MLE candidate support scope (issue #721).

    python3 -m unittest tools.test_envelope_mle_scope

Feeds a representative declared MLE case and out-of-scope circuits to the
scope evaluator, resolves every required case ID to its pinned provenance or
an explicit pending requirement, and proves the validator rejects a hollowed,
prematurely promoted, or silently widened plan.
"""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
import sys
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import envelope_mle_scope as scope  # noqa: E402

PLAN_PATH = REPO_ROOT / "docs/envelope-mle-scope.json"


def load_raw_plan() -> dict:
    return json.loads(PLAN_PATH.read_text(encoding="utf-8"))


def in_domain_descriptor() -> dict:
    """A representative declared MLE case: midswap d=3/r=2, loss 0.002, 1,024 shots."""
    return {
        "circuit": {
            "family": "midswap",
            "contract_valid": True,
            "flat": True,
            "readout_basis": "Z",
            "observables": 1,
            "sweep_bits": 0,
            "instructions": [
                "R", "H", "CX", "LOSS", "ML", "DETECTOR",
                "OBSERVABLE_INCLUDE",
            ],
            "distance": 3,
            "rounds": 2,
            "loss_rate": 0.002,
        },
        "shots": 1024,
    }


class DeclaredDomainTests(unittest.TestCase):
    def setUp(self):
        self.plan = scope.load_plan(PLAN_PATH)

    def test_declared_mle_case_is_in_domain(self):
        verdict = scope.evaluate_case(self.plan, in_domain_descriptor())
        self.assertEqual(verdict["status"], "in-domain")
        self.assertEqual(
            verdict["point"], "midswap-d3-r2-loss-0.002-batches-1024-16384")

    def test_second_declared_point_is_in_domain(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["rounds"] = 1
        descriptor["circuit"]["loss_rate"] = 0.01
        descriptor["shots"] = 16384
        verdict = scope.evaluate_case(self.plan, descriptor)
        self.assertEqual(verdict["status"], "in-domain")
        self.assertEqual(
            verdict["point"], "midswap-d3-r1-loss-0.01-batches-1024-16384")

    def test_verdict_is_a_support_boundary_not_a_decoder_prediction(self):
        verdict = scope.evaluate_case(self.plan, in_domain_descriptor())
        self.assertIn(verdict["status"], ("in-domain", "outside-supported-domain"))
        self.assertNotIn(verdict["status"], ("success", "fail", "pass", "decode-ok"))


class OutOfScopeTests(unittest.TestCase):
    def setUp(self):
        self.plan = scope.load_plan(PLAN_PATH)

    def assert_outside(self, descriptor, needle):
        verdict = scope.evaluate_case(self.plan, descriptor)
        self.assertEqual(verdict["status"], "outside-supported-domain")
        self.assertFalse(any(
            word in verdict["status"] for word in ("success", "fail")))
        self.assertTrue(
            any(needle in reason for reason in verdict["reasons"]),
            f"expected a reason containing {needle!r}: {verdict['reasons']}")

    def test_conventional_family_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["family"] = "conventional-stim-annotated"
        self.assert_outside(descriptor, "family")

    def test_larger_distance_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["distance"] = 5
        self.assert_outside(descriptor, "no declared domain point")

    def test_more_rounds_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["rounds"] = 3
        self.assert_outside(descriptor, "no declared domain point")

    def test_higher_loss_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["loss_rate"] = 0.02
        self.assert_outside(descriptor, "no declared domain point")

    def test_unmeasured_lower_loss_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["loss_rate"] = 0.001
        self.assert_outside(descriptor, "exact measured")

    def test_larger_batch_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["shots"] = 65536
        self.assert_outside(descriptor, "no declared domain point")

    def test_unmeasured_smaller_batch_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["shots"] = 512
        self.assert_outside(descriptor, "exact measured")

    def test_x_basis_readout_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["readout_basis"] = "X"
        self.assert_outside(descriptor, "readout basis")

    def test_repeat_blocks_are_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["flat"] = False
        self.assert_outside(descriptor, "flat")

    def test_missing_contract_attestation_is_outside(self):
        descriptor = in_domain_descriptor()
        del descriptor["circuit"]["contract_valid"]
        self.assert_outside(descriptor, "contract_valid")

    def test_missing_contract_field_is_outside(self):
        descriptor = in_domain_descriptor()
        del descriptor["circuit"]["readout_basis"]
        self.assert_outside(descriptor, "readout basis")

    def test_disallowed_instruction_is_outside(self):
        descriptor = in_domain_descriptor()
        descriptor["circuit"]["instructions"].append("MXL")
        self.assert_outside(descriptor, "instructions outside")

    def test_missing_instruction_inventory_is_outside(self):
        descriptor = in_domain_descriptor()
        del descriptor["circuit"]["instructions"]
        self.assert_outside(descriptor, "explicit non-empty list")


class RequirementResolutionTests(unittest.TestCase):
    def test_every_required_case_resolves(self):
        plan = scope.load_plan(PLAN_PATH)
        resolved = scope.resolve_requirements(plan, REPO_ROOT / "docs/envelope-support.json")
        self.assertEqual(set(resolved), {c["id"] for c in plan["required_cases"]})
        for cid, record in resolved.items():
            with self.subTest(case=cid):
                self.assertIn(record["status"], ("pinned", "pending"))
                if record["status"] == "pinned":
                    self.assertTrue(record["input"], f"{cid}: missing pinned input")
                    self.assertTrue(record["oracle"], f"{cid}: missing oracle provenance")
                else:
                    self.assertTrue(record["requirement"],
                                    f"{cid}: pending case needs an explicit requirement")


class NegativeControlTests(unittest.TestCase):
    def assert_plan_rejected(self, mutate, needle):
        plan = load_raw_plan()
        mutate(plan)
        with self.assertRaises(scope.ScopeError) as raised:
            scope.validate_plan(plan)
        self.assertIn(needle, str(raised.exception))

    def test_removing_a_required_case_fails(self):
        def mutate(plan):
            plan["required_cases"] = [
                c for c in plan["required_cases"]
                if c["id"] != "correctness:midswap-d3-r2-fixture"
            ]
        self.assert_plan_rejected(mutate, "required case(s) missing")

    def test_removing_a_resource_case_fails(self):
        def mutate(plan):
            plan["required_cases"] = [
                c for c in plan["required_cases"]
                if c["id"] != "resource:mle-d3r1-p010-b16384"
            ]
        self.assert_plan_rejected(mutate, "required case(s) missing")

    def test_compilation_covered_by_solve_timeout_fails(self):
        def mutate(plan):
            plan["limits_and_semantics"]["timeout"]["scope"] = "compile-and-solve"
        self.assert_plan_rejected(mutate, "solve phase")

    def test_premature_maturity_promotion_fails(self):
        def mutate(plan):
            plan["maturity"]["current"] = "supported"
        self.assert_plan_rejected(mutate, "promoted before the evidence exists")

    def test_correctness_identity_must_be_pinned(self):
        def mutate(plan):
            del plan["required_cases"][0]["evidence_identity"]["circuit_sha256"]
        self.assert_plan_rejected(mutate, "must pin evidence_identity")

    def test_widening_the_grid_without_evidence_fails(self):
        def mutate(plan):
            plan["domain"]["points"].append({
                "id": "midswap-d3-r3-loss-le-0.01-batch-le-16384",
                "distance": 3,
                "rounds": 3,
                "loss_rates": [0.01],
                "batches": [16384],
                "justification": "smuggled in without measured coverage",
                "required_evidence": ["correctness:midswap-d3-r3-generated"],
            })
        self.assert_plan_rejected(mutate, "no independent end-to-end correctness case")

    def test_widening_with_borrowed_evidence_fails(self):
        """Reusing another point's measurements does not cover a new grid point."""

        def mutate(plan):
            plan["domain"]["points"].append({
                "id": "midswap-d3-r3-borrowed",
                "distance": 3,
                "rounds": 3,
                "loss_rates": [0.01],
                "batches": [1024],
                "justification": "borrows the r=1 measurements",
                "required_evidence": [
                    "correctness:midswap-d3-r1-generated",
                    "resource:mle-d3r1-p010-b1024",
                ],
            })
        self.assert_plan_rejected(mutate, "circuit shape")

    def test_compiler_only_substitution_fails(self):
        """A compiler-only case must not satisfy a point's end-to-end need."""

        def mutate(plan):
            point = plan["domain"]["points"][0]
            point["required_evidence"] = [
                cid for cid in point["required_evidence"]
                if cid != "correctness:midswap-d3-r2-p002-generated"
            ] + ["correctness:midswap-d3-r3-generated"]
        self.assert_plan_rejected(mutate, "no independent end-to-end correctness case")

    def test_adding_unmeasured_batch_fails(self):
        def mutate(plan):
            plan["domain"]["points"][0]["batches"].append(65536)
        self.assert_plan_rejected(mutate, "missing exact loss/batch")

    def test_adding_unmeasured_loss_fails(self):
        def mutate(plan):
            plan["domain"]["points"][0]["loss_rates"].append(0.0025)
        self.assert_plan_rejected(mutate, "missing exact loss/batch")

    def test_correctness_at_a_different_loss_does_not_cover_a_point(self):
        def mutate(plan):
            case = next(
                c for c in plan["required_cases"]
                if c["id"] == "correctness:midswap-d3-r2-p002-generated"
            )
            case["circuit_params"]["loss_rate"] = 0.003
        self.assert_plan_rejected(mutate, "exact declared loss point")

    def test_dropping_conventional_exclusion_fails(self):
        def mutate(plan):
            plan["exclusions"] = [
                e for e in plan["exclusions"]
                if e["id"] != "conventional-family-candidate-limit"
            ]
        self.assert_plan_rejected(mutate, "conventional-family candidate-limit exclusion")


class CommittedPlanTests(unittest.TestCase):
    def test_committed_plan_passes_and_documents_the_domain(self):
        """One consistent scoped plan must pass, so unconditional failure cannot win."""
        plan = scope.load_plan(PLAN_PATH)
        self.assertEqual(plan["maturity"]["current"], "beta")
        table = scope.domain_table(plan)
        self.assertIn("midswap-d3-r2-loss-0.002-batches-1024-16384", table)
        self.assertIn("midswap-d3-r1-loss-0.01-batches-1024-16384", table)
        print("\n" + table)


if __name__ == "__main__":
    unittest.main()
