from fractions import Fraction
import json
from pathlib import Path
import unittest

import paper_proposal_check as checks
from paper_proposals import (CultivationProposal, ResidualClassProposal,
                             check_cultivation_model, check_residual_model)


class PaperProposalTests(unittest.TestCase):
    def test_finite_branches_and_deposited_logical_oracles_execute_completely(self):
        path = Path(__file__).parent/"paper/proposals.json"
        result = checks.check_finite(json.loads(path.read_text()))
        self.assertEqual(result["status"],"PASS")
        self.assertEqual(result["logical"]["patterns"],262144)
        self.assertEqual(sum(row["patterns"] for row in result["cultivation"]),304)
        self.assertEqual(result["decoder_runtime_integration"],"not_yet_validated")

    def test_missing_mixture_or_conditional_normalizer_is_detected(self):
        class Broken(CultivationProposal):
            def mass(self, pattern):
                _, _, _, _, normalizer = self.closing_law(pattern)
                # Regression: discard the ordinary branch's mass.
                return super().mass(pattern)*normalizer if normalizer else super().mass(pattern)
        model = Broken([[0,1,2],[1],[0,1,3],[2]],"1/1000",closing_sites=2)
        with self.assertRaisesRegex(ValueError,"mixture/conditional normalizer"):
            check_cultivation_model(model)

    def test_wrong_conditional_pattern_mass_and_lost_support_are_detected(self):
        class Broken(ResidualClassProposal):
            def mass(self, word):
                return 2*super().mass(word)
        model = Broken([0,1,0,1],[0,1,0,1],width=2)
        with self.assertRaisesRegex(ValueError,"wrong physical pattern"):
            check_residual_model(model,model.counts)
        model = ResidualClassProposal([0,1,0,1],[0,1,0,1],width=2)
        buckets = [[list(bucket) for bucket in row] for row in model.buckets]
        buckets[0][0].clear()
        model.buckets = buckets
        with self.assertRaisesRegex(ValueError,"pattern was removed"):
            check_residual_model(model,model.counts)

    def test_invalid_exact_inputs_sources_and_unsupported_laws_fail_before_sampling(self):
        for kwargs in [dict(physical_rate=.001),dict(physical_rate="1/2",extra_faults=3),
                       dict(physical_rate="1/1000",mixture=1),dict(physical_rate="1/1000",closing_sites=3)]:
            with self.subTest(kwargs=kwargs),self.assertRaises(ValueError):
                CultivationProposal([[0],[1],[0],[1]],**kwargs)
        model = ResidualClassProposal([0,1,0,1],[0,1,0,1],width=2)
        class Bad:
            def randrange(self, bound):return -1
        with self.assertRaisesRegex(ValueError,"out-of-range"):
            model.draw(Bad())
        with self.assertRaisesRegex(ValueError,"invalid Pauli"):
            model.mass(True)
        with self.assertRaisesRegex(ValueError,"reweighting"):
            model.reweight([],Fraction(1,1000))
        with self.assertRaisesRegex(ValueError,"reweighting"):
            model.reweight(iter(()),Fraction(1,1000))


if __name__ == "__main__":unittest.main()
