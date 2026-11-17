from fractions import Fraction as F
import math
import unittest

import cultivation_benchmark as benchmark
import cultivation_peers as peers
import cultivation_statistics as stats


class CultivationBenchmarkTests(unittest.TestCase):
    def test_prescribed_paper_budget_disjoint_ranges_and_seeds(self):
        pinned = benchmark.reference.load_manifest()
        plan = benchmark.manifest(pinned)
        cases = list(benchmark.cases(plan))
        precision = [row for row in cases if row['kind'] != 'timing']
        streams = {}
        for row in precision:
            identifies = (row['family'], row['point'], row['kind'])
            ranges = streams.setdefault(row['seed'], (identifies, []))
            self.assertEqual(ranges[0], identifies)
            ranges[1].append((row['start'], row['start']+row['shots']))
        self.assertEqual(len(streams), 40)
        self.assertEqual(sum(row['shots'] for row in precision), 1384120320)
        self.assertEqual(len(cases)-len(precision), 12)
        for _, ranges in streams.values():
            self.assertEqual(ranges[0][0], 0)
            self.assertEqual([r[1] for r in ranges[:-1]], [r[0] for r in ranges[1:]])
        self.assertEqual(plan['families']['rp2-d5-T']['grid'][-1]['failure_seed'],2026101542)

    def test_exact_moments_and_variance_use_different_acceptance_and_failure_batches(self):
        no_closing = (0, 0, 0, False)
        acceptance = stats.moments({(no_closing,stats.ONE,stats.ZERO):3,
                                    ((1,0,0,False),stats.ZERO,stats.ZERO):1}, F(1,8),F(1,8),4,'acceptance')
        # Independent direct witness: exactly one of four observations equals
        # 1/2 with a P/Q of (1/8)/(7/8) * (7/8)^3/(1/8)^3 / (59/40).
        accepted_error = (F(1,2),F(0))
        key = (1, 3, 2, True)
        failure = stats.moments({(key,stats.ONE,accepted_error):1,
                                 ((3,3,0,False),stats.ZERO,stats.ZERO):3}, F(1,8),F(7,8),4,'failure')
        weighted = F(1,2)*49*F(40,59)
        self.assertEqual(failure['mean'],(weighted/4,0))
        self.assertEqual(failure['variance'],(weighted**2/16,0))
        self.assertEqual(acceptance['variance'],(F(1,16),0))
        combined = stats.two_estimator(acceptance,failure)
        estimate = (weighted/4)/F(3,4)
        expected_variance = (weighted**2/16 + estimate**2/16) / F(3,4)**2
        self.assertAlmostEqual(combined['conditional_error'],float(estimate))
        self.assertAlmostEqual(combined['conditional_error_variance'],float(expected_variance))
        self.assertAlmostEqual(combined['conditional_error_se'],math.sqrt(expected_variance))
        self.assertGreater(combined['conditional_error'],combined['accepted_failure'])

    def test_real_cyclotomic_values_and_impossible_closing_observations(self):
        exact = stats.real_field(['1/2','-1/4','0','1/4'])
        self.assertAlmostEqual(stats.as_float(exact),0.5-math.sqrt(2)/4)
        self.assertEqual(stats.sign(exact),1)
        self.assertEqual(stats.multiply(exact,(F(1,2),F(1,4))),(F(1,8),0))
        with self.assertRaisesRegex(ValueError,'not real'):
            stats.real_field(['1/2','1/4','0','1/4'])
        with self.assertRaisesRegex(ValueError,'closing normalizer'):
            stats.weight_key(dict(occurrences=3,tail_candidates=15,passing_candidates=3,passes=True),
                             (15,1,1,1),2,[])
        with self.assertRaisesRegex(ValueError,'offset parity'):
            stats.weight_key(dict(occurrences=0,tail_candidates=1,passing_candidates=0,passes=False),
                             (1,1,1,1),1,[])

    def test_sampled_peer_readout_preserves_signed_absolute_detector_rows(self):
        source = '''RX 0
T 0
MX(0.001) 0
DETECTOR(1, 0, 0) rec[-1]
CX rec[-1] 1
MPP X0*X1 Z0*Z1
DETECTOR rec[-3] rec[-1]
OBSERVABLE_INCLUDE(4) rec[-1]
POSTSELECT det[1] !det[0]
'''
        text, expected = peers.peer_circuit(source,dict(outputs=[1],ideal_state='target.magna'),'H 0\nT 0')
        self.assertIn('T 0\nMX(0.001) 0\nDETECTOR(1, 0, 0) rec[-1]\nCX rec[-1] 1\n',text)
        self.assertTrue(text.endswith('T_DAG 1\nH 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]\n'))
        self.assertNotIn('OBSERVABLE_INCLUDE(4)',text)
        self.assertEqual(peers.scoring_layout(text,expected),
                         dict(detectors=[[0],[0,2]],expected=[True,False],target_record=3))
        with self.assertRaisesRegex(ValueError,'every selected detector'):
            peers.peer_circuit(source.replace(' !det[0]',''),dict(outputs=[1]),'')
        with self.assertRaisesRegex(ValueError,'unsupported cultivation target'):
            peers.peer_circuit(source,dict(outputs=[1]),'H 0')

    def test_ppvm_inversion_lowering_restores_the_projected_qubit_and_record_position(self):
        source = 'M(0.01) 0 !1 2\nMX !3\nMY !4\nCX rec[-1] 5\n'
        self.assertEqual(peers.lower_inverted_measurements(source),
                         'M(0.01) 0\nX 1\nM(0.01) 1\nX 1\nM(0.01) 2\n'
                         'Z 3\nMX 3\nZ 3\nX 4\nMY 4\nX 4\nCX rec[-1] 5\n')


if __name__ == '__main__':
    unittest.main()
