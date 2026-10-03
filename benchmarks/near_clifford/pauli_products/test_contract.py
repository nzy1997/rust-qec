"""Negative controls for independently validated support evidence."""
import copy
import json
from pathlib import Path
import unittest

from verify import verify

RESULT = Path(__file__).resolve().parent.parent/'results/apple-m4-pauli-support-2026-10-03.json'


class SupportContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls): cls.result=json.loads(RESULT.read_text())

    def rejects(self, edit, git_sources=False):
        result=copy.deepcopy(self.result)
        edit(result)
        with self.assertRaises((ValueError,KeyError)):
            verify(result,git_sources=git_sources)

    def test_complete_controls_pass(self):
        self.assertTrue(verify(self.result,git_sources=True).startswith('PASS: 14'))

    def test_incomplete_matrix_semantics_and_wrong_entry_fail(self):
        self.rejects(lambda r:r.pop('completed_utc'))
        self.rejects(lambda r:r['cases'].pop())
        self.rejects(lambda r:r['verification'].pop('candidate-diagnostic'))
        self.rejects(lambda r:r['inputs'].update(main_rs='0'*64))

    def test_physics_is_checked_even_when_all_payloads_are_changed_together(self):
        def change(r,key,value):
            for payloads in r['verification'].values():
                payloads['star_63']['physics'][0]['steps'][0][key]=value
        self.rejects(lambda r:change(r,'zero',.4))
        self.rejects(lambda r:change(r,'outcome',0))
        def remove(r):
            for payloads in r['verification'].values():
                payloads['star_63']['physics'][0]['final'].pop()
        self.rejects(remove)

    def test_final_phase_and_rng_continuation_are_required(self):
        def change(r):
            for payloads in r['verification'].values():
                payloads['star_63']['physics'][0]['final'][0]['zero']=.5
        self.rejects(change)
        def missing(r):
            for payloads in r['verification'].values():
                payloads['star_63']['physics'][0].pop('continuation')
        self.rejects(missing)

    def test_circuit_identity_warmup_raw_times_and_order_are_required(self):
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate'].update(circuit='H 0\nT 0\n'))
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate'].update(warmup_calls=0))
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate'].update(raw_ns=[True]*3))
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate'].update(median_ns=-1))
        self.rejects(lambda r:r['cases'][0]['runs'][0].update(order=['candidate','baseline']))

    def test_runtime_row_work_and_diagnostic_sources_are_bound(self):
        self.rejects(lambda r:r['diagnostics']['candidate']['star_63'].update(row_work=[3,189]))
        self.rejects(lambda r:r['sources']['candidate']['diagnostic'].update(near_clifford_source_sha256='0'*64),True)
        def wrong(r):
            for metadata in [r['sources']['candidate'],r['sources']['candidate']['diagnostic']]:
                metadata['tableau_source_sha256']='0'*64
        self.rejects(wrong,True)


if __name__=='__main__': unittest.main()
