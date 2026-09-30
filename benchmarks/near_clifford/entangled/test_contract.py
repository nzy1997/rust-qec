"""Negative controls for the retained entangled evidence contract."""
import copy
import json
import hashlib
from pathlib import Path
import unittest
from verify import verify, MODES
from run import adapt_oracle

RESULT = Path(__file__).resolve().parent.parent/'results/apple-m4-entangled-2026-09-30.json'

class EvidenceContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.result=json.loads(RESULT.read_text())

    def rejects(self,edit):
        changed=copy.deepcopy(self.result)
        edit(changed)
        with self.assertRaises(ValueError): verify(changed)

    def test_oracle_guard_adapter_rejects_missing_or_duplicate_anchors(self):
        anchor=b'assert!(num_qubits <= 10, "dense oracle is limited to 10 qubits");'
        with self.assertRaises(ValueError): adapt_oracle(b'not the expected oracle')
        with self.assertRaises(ValueError): adapt_oracle(anchor+anchor)

    def test_retained_complete_bundle_passes(self):
        self.assertTrue(verify(self.result).startswith('PASS: 28'))

    def test_smoke_and_missing_completion_are_rejected(self):
        self.rejects(lambda r:r.update(quick=True))
        self.rejects(lambda r:r.pop('completed_utc'))

    def test_missing_or_duplicate_configuration_is_rejected(self):
        self.rejects(lambda r:r['cases'].pop())
        self.rejects(lambda r:r['cases'].__setitem__(1,copy.deepcopy(r['cases'][0])))

    def test_changed_driver_or_lock_is_rejected(self):
        self.rejects(lambda r:r['entangled_inputs'].update({'fixtures.rs':'0'*64}))
        self.rejects(lambda r:r['sources']['candidate'].update(lock_sha256='0'*64))

    def test_wrong_timed_circuit_is_rejected(self):
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate'].update(circuit='H 0\nM 0\n'))

    def test_wrong_median_and_missing_mode_are_rejected(self):
        mode=sorted(MODES)[0]
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate']['measurements'][0][mode].update(median_ns=-1))
        self.rejects(lambda r:r['cases'][0]['runs'][0]['candidate']['measurements'][0].pop(mode))

    def test_wrong_semantic_payload_is_rejected(self):
        name=self.result['matrix'][0][0]
        self.rejects(lambda r:r['verification_results']['candidate'][name].update(continuation=0))
        self.rejects(lambda r:r['verification'][name].update(output_sha256='0'*64))

    def test_missing_diagnostic_or_unsafe_cache_budget_is_rejected(self):
        name=self.result['matrix'][0][0]
        self.rejects(lambda r:r['diagnostics']['candidate'].pop(name))
        self.rejects(lambda r:r['diagnostics']['candidate'][name]['after_probe'].__setitem__(2,10**9))

    def test_diagnostic_source_and_warmup_counter_gaps_are_rejected(self):
        name=self.result['matrix'][0][0]
        self.rejects(lambda r:r['sources']['candidate'].pop('diagnostic'))
        self.rejects(lambda r:r['sources']['candidate']['diagnostic'].update(revision='0'*40))
        self.rejects(lambda r:r['diagnostics']['candidate'][name].pop('warmup_counters'))
        self.rejects(lambda r:r['diagnostics']['candidate'][name]['warmup_counters'].__setitem__(0,True))
        changed=copy.deepcopy(self.result)
        changed['sources']['candidate']['diagnostic']['near_clifford_source_sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'diagnostic overlay differs'):
            verify(changed,git_sources=True)

    def test_alternating_order_and_counter_definitions_are_required(self):
        self.rejects(lambda r:r['cases'][0]['runs'][1].update(order=['baseline','candidate']))
        self.rejects(lambda r:r.update(counter_names=['incorrect']*8))
        self.rejects(lambda r:r.update(snapshot_names=['incorrect']*5))

    def test_oracle_coverage_cannot_be_relabelled_as_full_width(self):
        name='brick_12_193_3'
        def change(r,field,value):
            for label in r['verification_results']:
                r['verification_results'][label][name]['physics'][field]=value
            payload=r['verification_results']['baseline'][name]
            r['verification'][name]['output_sha256']=hashlib.sha256(json.dumps(payload,sort_keys=True).encode()).hexdigest()
        self.rejects(lambda r:change(r,'full_width_oracle',True))
        self.rejects(lambda r:change(r,'reference_fixture',name))
        self.rejects(lambda r:change(r,'born_probability_comparisons',1))

if __name__=='__main__':unittest.main()
