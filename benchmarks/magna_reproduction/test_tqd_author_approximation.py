"""Retained-output scoring and compatibility with sealed three-color captures."""

from copy import deepcopy
from fractions import Fraction
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

try:
    import numpy as np
except ImportError:
    np = None

from tqd_author_approximation import AuthorApproximation, LEGACY_SCHEMA, SCHEMA, evaluate_payload, fault_identity
from tqd_paper_statistics import RNG_PROTOCOL, fault_stream, paired_rows


@unittest.skipIf(np is None, 'requires the pinned reference runtime installed by Magna CI')
class RetainedOutputScoringTests(unittest.TestCase):
    def setUp(self):
        # A three-edge closed cycle has zero syndrome and nonzero Z winding.
        # Its matching correction is identity, leaving a known logical residual.
        self.module = AuthorApproximation.__new__(AuthorApproximation)
        self.module.np = np
        self.module.size, self.module.depth = 3, 1
        self.module.masked = set()
        self.module.full = np.array([[1, 0, 1], [1, 1, 0], [0, 1, 1]], dtype=np.uint8)
        self.module.matching = SimpleNamespace(decode=lambda syndrome: np.zeros(3, dtype=np.uint8))
        self.module.endpoints = self.module.lookup = self.module.prefixes = self.module.prefix_matchings = None

        def twisted(loops, *args, **kwargs):
            np.random.randint(0, 2, size=3)
            return {color: np.zeros(3, dtype=np.uint8) for color in ('b', 'g', 'r')}

        self.module.author = SimpleNamespace(
            decoder=SimpleNamespace(jit_decode_full=lambda size, depth, x, *args: np.zeros_like(x),
                                    is_logical_error=lambda residual, *args, **kwargs: bool(np.any(residual))),
            twisted=SimpleNamespace(generate_twisted_z_errors=Mock(side_effect=twisted)))
        self.sites = [dict(kind='Z', color=color, edge=edge) for color in ('b', 'g', 'r') for edge in range(3)]

    def test_green_logical_z_is_discarded_while_blue_and_red_still_fail(self):
        for colors in ((), ('g',), ('b',), ('r',), ('b', 'g'), ('g', 'r'), ('b', 'r'), ('b', 'g', 'r')):
            with self.subTest(colors=colors):
                selected = [index for index, site in enumerate(self.sites) if site['color'] in colors]
                result = self.module.evaluate(self.sites, selected, 202610420000, 16)
                failed = 'b' in colors or 'r' in colors
                self.assertTrue(result['flux_success'])
                self.assertEqual(result['nested_attempted'], 32)
                self.assertEqual(result['nested_logical_errors'], 32 if failed else 0)
                self.assertEqual(result['matched_infidelity'], '24/25' if failed else '0')

    def test_green_x_logical_error_still_flags_the_sample_before_nested_scoring(self):
        sites = [dict(kind='X', color='g', edge=edge) for edge in range(3)]
        result = self.module.evaluate(sites, [0, 1, 2], 123, 7)
        self.assertEqual(result['flux_failures'], dict(b=False, g=True, r=False))
        self.assertFalse(result['flux_success'])
        self.assertEqual(result['nested_attempted'], 0)
        self.assertIsNone(result['nested_logical_errors'])
        self.module.author.twisted.generate_twisted_z_errors.assert_not_called()

    def test_green_x_residuals_still_generate_blue_twisted_z_errors(self):
        sites = [dict(kind='X', color='g', edge=edge) for edge in range(3)]
        self.module.author.decoder.is_logical_error = lambda residual, *args, error_type, **kwargs: (
            error_type == 'z' and bool(np.any(residual)))

        def twisted(loops, *args, **kwargs):
            np.testing.assert_array_equal(loops['g'], np.ones(3, dtype=np.uint8))
            return dict(b=loops['g'].copy(), g=np.zeros(3, dtype=np.uint8), r=np.zeros(3, dtype=np.uint8))

        self.module.author.twisted.generate_twisted_z_errors.side_effect = twisted
        result = self.module.evaluate(sites, [0, 1, 2], 123, 7)
        self.assertTrue(result['flux_success'])
        self.assertEqual(result['nested_logical_errors'], 32)
        self.assertEqual(result['matched_infidelity'], '24/25')
        self.assertEqual(self.module.author.twisted.generate_twisted_z_errors.call_count, 32)

    def test_green_only_twisted_z_does_not_fail_the_retained_outputs(self):
        sites = [dict(kind='X', color='b', edge=edge) for edge in range(3)]
        self.module.author.decoder.is_logical_error = lambda residual, *args, error_type, **kwargs: (
            error_type == 'z' and bool(np.any(residual)))

        def twisted(loops, *args, **kwargs):
            return dict(b=np.zeros(3, dtype=np.uint8), g=loops['b'].copy(), r=np.zeros(3, dtype=np.uint8))

        self.module.author.twisted.generate_twisted_z_errors.side_effect = twisted
        result = self.module.evaluate(sites, [0, 1, 2], 123, 7)
        self.assertTrue(result['flux_success'])
        self.assertEqual(result['nested_attempted'], 32)
        self.assertEqual(result['nested_logical_errors'], 0)
        self.assertEqual(result['matched_infidelity'], '0')

    def test_scoring_restores_numpy_random_state_on_success_and_failure(self):
        original = np.random.get_state()
        try:
            for failure in (False, True):
                with self.subTest(failure=failure):
                    np.random.seed(704)
                    expected = np.random.randint(0, 1000, size=8)
                    np.random.seed(704)
                    if failure:
                        self.module.author.twisted.generate_twisted_z_errors.side_effect = RuntimeError('decoder failed')
                        with self.assertRaisesRegex(RuntimeError, 'decoder failed'):
                            self.module.evaluate(self.sites, [3, 4, 5], 42, 9)
                    else:
                        self.module.evaluate(self.sites, [3, 4, 5], 42, 9)
                    np.testing.assert_array_equal(np.random.randint(0, 1000, size=8), expected)
        finally:
            np.random.set_state(original)


class ScoringVersionTests(unittest.TestCase):
    def test_new_payload_identifies_retained_output_scoring(self):
        evaluator = SimpleNamespace(hashes={}, evaluate=Mock(return_value=dict(index=0)))
        physical = dict(configuration=dict(size=3, seed=123), fault_map_sha256='map', rows=[dict(index=0, faults=[])])
        with patch('tqd_author_approximation.AuthorApproximation', return_value=evaluator):
            payload = evaluate_payload(Path('.'), dict(families=dict(tqd3=dict(entries=[], depth=5))), physical)
        self.assertEqual(payload['schema'], SCHEMA)
        self.assertEqual(payload['logical_z_colors'], ['b', 'r'])
        self.assertEqual(payload['logical_z_infidelity'], '24/25')
        self.assertEqual(payload['rows'], [dict(index=0, fault_sha256=fault_identity([]))])

    def test_legacy_scores_are_preserved_and_version_color_mismatches_are_rejected(self):
        seed, sites = 202610380000, 756
        rows = [dict(index=index, faults=fault_stream(seed, index, Fraction(3, 1000), sites),
                     accepted=['5/8', '0', '0', '0'], error=['0', '0', '0', '0'],
                     history_weight=['1', '0', '0', '0'], odd_terminal_charge=dict(b=False, g=False, r=False))
                for index in range(2)]
        physical = dict(schema='rmagna.paper-tqd-physical-run.v1', status='complete', failure=None,
                        configuration=dict(size=3, seed=seed, shots=2, start=0, p='3/1000'),
                        completed=2, active_sites=sites, rng_protocol=RNG_PROTOCOL, charge_depth=6,
                        fault_map_sha256='map', rows=rows)
        legacy = dict(schema=LEGACY_SCHEMA, size=3, depth=5, noise_seed=seed, nested_draws=32,
                      author_source_sha256={}, logical_z_infidelity='24/25', fault_map_sha256='map',
                      rows=[dict(index=row['index'], fault_sha256=fault_identity(row['faults']), flux_success=True,
                                 flux_failures=dict(b=False, g=False, r=False), nested_attempted=32,
                                 nested_logical_errors=32, matched_infidelity='24/25') for row in rows])
        self.assertEqual([row[3] for row in paired_rows(physical, legacy, {})], [(Fraction(24, 25), Fraction(0))]*2)
        current = dict(deepcopy(legacy), schema=SCHEMA, logical_z_colors=['b', 'r'])
        for row in current['rows']:
            row.update(nested_logical_errors=0, matched_infidelity='0')
        self.assertEqual([row[3] for row in paired_rows(physical, current, {})], [(Fraction(0), Fraction(0))]*2)
        for schema, colors in ((SCHEMA, ['b', 'g', 'r']), (SCHEMA, None),
                               (LEGACY_SCHEMA, ['b', 'r']), ('unknown', ['b', 'r'])):
            with self.subTest(schema=schema, colors=colors):
                invalid = dict(current, schema=schema, logical_z_colors=colors)
                with self.assertRaisesRegex(ValueError, 'schema or logical Z output colors'):
                    paired_rows(physical, invalid, {})


if __name__ == '__main__':
    unittest.main()
