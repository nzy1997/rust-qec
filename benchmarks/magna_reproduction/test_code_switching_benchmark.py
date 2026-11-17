"""Source-free checks for the fixed schedule and pooled exact statistics."""

from collections import Counter
from fractions import Fraction
from pathlib import Path
import unittest

import code_switching_benchmark as benchmark
from decoder_workloads_statistics import code_switch_moments
import reference
from remaining_contracts import read_json


class CodeSwitchingBenchmarkTests(unittest.TestCase):
    def test_manifest_partitions_the_published_precision_grid(self):
        root = Path(__file__).resolve().parent
        pinned = reference.load_manifest()
        plan = read_json(root/'paper/code-switching.json')
        benchmark.validate_manifest(plan, pinned)
        cases = list(benchmark.cases(plan))
        timed = [case for case in cases if case['kind'] == 'timing']
        self.assertEqual([case['family'] for case in timed], ['dk'] * 3 + ['wg-T'] * 3)
        self.assertTrue(all(case['p'] == case['q'] and case['shots'] == 1024 for case in timed))
        precision = [case for case in cases if case['kind'] == 'precision']
        self.assertEqual(sum(case['shots'] for case in precision), 21_757_952)
        self.assertEqual(len({case['id'] for case in cases}), len(cases))
        for family, design in plan['families'].items():
            for point, rate in enumerate(design['grid']):
                start = 0
                for case in precision:
                    if (case['family'], case['point']) != (family, point):
                        continue
                    self.assertEqual((case['start'], case['noise_seed'], case['born_seed']),
                                     (start, rate['noise_seed'], rate['born_seed']))
                    start += case['shots']
                self.assertEqual(start, rate['draws'])

    def test_pool_raw_draws_before_computing_paired_covariance(self):
        zero, one = (Fraction(0), Fraction(0)), (Fraction(1), Fraction(0))
        first = Counter({(0, one, zero): 2})
        second = Counter({(1, one, one): 1, (1, zero, zero): 1})
        rate = Fraction(1, 1000)
        pooled = code_switch_moments(first + second, rate, rate, 2)
        self.assertEqual(pooled['completed'], 4)
        self.assertEqual(pooled['positive_error_draws'], 1)
        for key, expected in (('acceptance', Fraction(3, 4)), ('conditional_error', Fraction(1, 3)),
                              ('covariance', Fraction(1, 48)), ('variance_conditional_error', Fraction(8, 81))):
            (numerator, denominator), irrational = pooled['exact'][key]
            self.assertEqual(Fraction(int(numerator, 16), int(denominator, 16)), expected)
            self.assertEqual(irrational, ['0x0', '0x1'])


if __name__ == '__main__':
    unittest.main()
