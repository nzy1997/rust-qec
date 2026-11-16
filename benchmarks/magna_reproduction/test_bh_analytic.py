from fractions import Fraction
from pathlib import Path
import json
import hashlib
import unittest

import bh_analytic as bh

HERE = Path(__file__).resolve().parent


class BhAnalyticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads((HERE/'paper/bh-code-reference.json').read_text())
        cls.codes = {row['k']: [int(v, 16) for v in row['rows_hex']]
                     for row in cls.fixture['codes']}

    def test_literal_encoding_and_all_published_exact_points_agree(self):
        prefix = self.fixture['codes'][0]['encoding_prefix']
        self.assertEqual(bh.matrix(2, prefix), self.codes[2])
        for row in self.fixture['curves']:
            actual = bh.exact_from_rows(row['k'], self.codes[row['k']], str(row['p']))
            for key, value in row['reference'].items():
                self.assertAlmostEqual(float(actual[key]), value, delta=1e-15)
        for row in self.fixture['codes']:
            if row['published_exact_reference'] is not None:
                actual = bh.exact_from_rows(row['k'], self.codes[row['k']], '1/1000')
                for key, value in row['published_exact_reference'].items():
                    self.assertAlmostEqual(float(actual[key]), value, delta=1e-15)
        self.assertEqual(bh.exact_from_rows(100, self.codes[100], 0)['joint_success'], 1)
        self.assertEqual(bh.exact_from_rows(100, self.codes[100], 1)['joint_success'], 0)

    def test_complete_t_and_s_runner_inputs_match_the_original_source_pins(self):
        root = HERE.parent.parent
        fixture = json.loads((root/'rmagna/tests/fixtures/bh-native-inputs.json').read_text())
        pinned = json.loads((HERE/'reference.json').read_text())
        self.assertEqual(fixture['upstream_commit'],pinned['upstream']['commit'])
        for variant, case in fixture['cases'].items():
            self.assertEqual(bh.matrix(2,case['inputs']['physical.magna']),self.codes[2])
            for name, text in case['inputs'].items():
                path=f'examples/bh2-{variant}/{name}'
                self.assertEqual(hashlib.sha256(text.encode()).hexdigest(),pinned['upstream']['files'][path])
                self.assertEqual(case['sha256'][path],pinned['upstream']['files'][path])

    def test_dual_sum_matches_complete_independent_fault_enumeration_and_joint_target(self):
        rows = self.codes[2]
        n = 14
        for p in [Fraction(0), Fraction(1, 8), Fraction(1)]:
            by_weight = [p**w*(1-p)**(n-w) for w in range(n+1)]
            acceptance = good = one_output = Fraction(0)
            for fault in range(1 << n):
                if any((row & fault).bit_count() % 2 for row in rows[2:]):
                    continue
                mass = by_weight[fault.bit_count()]
                acceptance += mass
                if not (rows[0] & fault).bit_count() % 2:
                    one_output += mass
                if not any((row & fault).bit_count() % 2 for row in rows[:2]):
                    good += mass
            actual = bh.exact_from_rows(2, rows, p)
            self.assertEqual(actual['acceptance'], acceptance)
            self.assertEqual(actual['joint_success'], good)
            self.assertEqual(actual['accepted_failure'], acceptance-good)
            if p == Fraction(1, 8):
                self.assertNotEqual(good, one_output, 'single-output fidelity must not pass a joint target check')

    def test_invalid_or_non_symmetric_codes_and_changed_prefixes_are_rejected(self):
        prefix = self.fixture['codes'][0]['encoding_prefix']
        for text in ['', prefix.replace('CX 0 14', 'CX 0 999'),
                     prefix.replace('S 0', 'CZ 0'), prefix.replace('H 16', 'T 16')]:
            with self.subTest(text=text[:50]), self.assertRaises(ValueError):
                bh.matrix(2, text)
        for k, rows, p in [(1, self.codes[2], 0), (2, self.codes[2][:-1], 0),
                           (2, self.codes[2], '-1/8'), (2, self.codes[2], '9/8'),
                           (2, [0]*5, 0), (2, [1 << 15]+self.codes[2][1:], 0)]:
            with self.subTest(k=k, p=p), self.assertRaises(ValueError):
                bh.exact_from_rows(k, rows, p)
        changed = list(self.codes[6])
        changed[0] ^= 1
        with self.assertRaises(ValueError):
            bh.exact_from_rows(6, changed, '1/8')


if __name__ == '__main__':
    unittest.main()
