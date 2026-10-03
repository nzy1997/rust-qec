"""Negative controls for the retained scale and entangled row-operation evidence."""
import copy
import json
from pathlib import Path
import unittest
from unittest import mock
import hashlib
import subprocess
from verify import verify, source_bytes, ROOT

BASE = Path(__file__).resolve().parent.parent
FILES = [BASE / 'results' / f'apple-m4-row-ops-{suite}-2026-10-03.json'
         for suite in ['scale', 'entangled']]


class EvidenceContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.results = [json.loads(path.read_text()) for path in FILES]

    def rejects(self, edit):
        for result in self.results:
            changed = copy.deepcopy(result)
            edit(changed)
            with self.assertRaises((ValueError, KeyError)):
                verify(changed)

    def test_complete_campaigns_pass(self):
        for result in self.results:
            self.assertTrue(verify(result, git_sources=True).startswith('PASS:'))

    def test_wrong_entry_or_schema_is_rejected(self):
        self.rejects(lambda r: r.update(row_ops_entry_sha256='0' * 64))
        self.rejects(lambda r: r.update(row_ops_schema='unknown'))
        self.rejects(lambda r: r.update(row_ops_suite='unknown'))

    def test_smoke_incomplete_or_missing_configuration_is_rejected(self):
        self.rejects(lambda r: r.update(quick=True))
        self.rejects(lambda r: r.pop('completed_utc'))
        self.rejects(lambda r: r['cases'].pop())

    def test_missing_or_changed_tableau_binding_is_rejected(self):
        self.rejects(lambda r: r['sources']['candidate'].pop('tableau_source_sha256'))
        self.rejects(lambda r: r['sources']['candidate']['diagnostic'].update(tableau_source_sha256='0' * 64))
        for result in self.results:
            changed = copy.deepcopy(result)
            for metadata in [changed['sources']['candidate'], changed['sources']['candidate']['diagnostic']]:
                metadata['tableau_source_sha256'] = '0' * 64
            with self.assertRaisesRegex(ValueError, 'git source differs'):
                verify(changed, git_sources=True)

    def test_wrong_diagnostic_source_or_missing_warmup_is_rejected(self):
        self.rejects(lambda r: r['sources']['candidate']['diagnostic'].update(revision='0' * 40))
        self.rejects(lambda r: r['sources']['candidate']['diagnostic'].update(near_clifford_source_sha256='invalid'))
        self.rejects(lambda r: next(iter(r['diagnostics']['candidate'].values())).pop('warmup_counters'))

    def test_wrong_timing_median_modes_and_order_are_rejected(self):
        self.rejects(lambda r: r['cases'][0]['runs'][0]['candidate']['measurements'][0]['warm_prepared_flat'].update(median_ns=-1))
        self.rejects(lambda r: r['cases'][0]['runs'][0]['candidate']['measurements'][0].pop('warm_prepared_flat'))
        self.rejects(lambda r: r['cases'][0]['runs'][1].update(order=['baseline', 'candidate']))

    def test_scale_semantic_hash_and_circuit_identity_are_required(self):
        result = self.results[0]
        changed = copy.deepcopy(result)
        for value in changed['verification'].values():
            value.pop('output_sha256')
        with self.assertRaises((ValueError, KeyError)):
            verify(changed)
        changed = copy.deepcopy(result)
        for case in changed['cases']:
            for pair in case['runs']:
                for label in ['baseline', 'candidate']:
                    pair[label]['circuit'] = 'H 0\nM 0\n'
        with self.assertRaisesRegex(ValueError, 'historical scale circuit differs'):
            verify(changed)
        changed = copy.deepcopy(result)
        name = next(iter(changed['verification']))
        for values in changed['row_ops_verification_results'].values():
            values[name]['shots'][0]['m'] = [0]
        with self.assertRaisesRegex(ValueError, 'invalid measurement/detector bits'):
            verify(changed)

    def test_missing_squashed_source_requires_exact_head_equivalence(self):
        path = 'rstim/src/sim/tableau.rs'
        raw = subprocess.check_output(['git', 'show', 'HEAD:' + path], cwd=ROOT)
        expected = hashlib.sha256(raw).hexdigest()
        real_run = subprocess.run
        def missing_selected(cmd, *args, **kwargs):
            if cmd[:2] == ['git', 'show'] and cmd[2].startswith('missing:'):
                return subprocess.CompletedProcess(cmd, 128, b'', b'missing')
            return real_run(cmd, *args, **kwargs)
        with mock.patch('verify.subprocess.run', side_effect=missing_selected):
            self.assertEqual(source_bytes('missing', path, expected), raw)
            with self.assertRaisesRegex(ValueError, 'not HEAD-equivalent'):
                source_bytes('missing', path, '0' * 64)

    def test_unknown_counters_and_unsafe_cache_are_rejected(self):
        self.rejects(lambda r: r.update(counter_names=['invalid'] * 8))
        self.rejects(lambda r: next(iter(r['diagnostics']['candidate'].values()))['after_probe'].__setitem__(2, 10**9))


if __name__ == '__main__':
    unittest.main()
