"""Actual small-oracle observations and adversarial publication-contract checks."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from . import decoder_reference
from .decoder_contract import compare_reports, verify_decoder

ROOT = Path(__file__).resolve().parents[2]


class DecoderContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.report = decoder_reference.run(ROOT/'target/release/rustqec',
                                          ROOT/'target/release/examples/export_matching_benchmark')

    def reject(self, change):
        report = copy.deepcopy(self.report)
        change(report)
        with self.assertRaises(ValueError):
            verify_decoder(report)

    def test_actual_decoder_observations_pass_independent_contract(self):
        verify_decoder(self.report)
        self.assertEqual([case['rows_checked'] for case in self.report['cases']], [64, 1024])
        witness = self.report['cases'][1]['strict_witness']
        self.assertEqual(witness['logical_order'], [0, 1])
        self.assertEqual(witness['fixed_costs'], [2, 3])
        self.assertEqual(witness['conditioned_costs'], [2, 1.5])

    def test_original_explicit_failure_missing_backend_and_wrong_witness_attacks(self):
        for field in ('hand_derived_graph_pass', 'loss_mapping_pass', 'placeholder_invariance_pass'):
            self.reject(lambda r, f=field: r['cases'][0].__setitem__(f, False))
        self.reject(lambda r: r['cases'][1].update(status='FAIL'))
        self.reject(lambda r: r['cases'][1].update(rejected_rows={}))
        for name in ('native_prediction', 'pymatching_prediction'):
            self.reject(lambda r, n=name: r['cases'][1]['strict_witness'].__setitem__(n, 0))

    def test_exact_report_case_graph_and_backend_inventory(self):
        self.reject(lambda r: r['cases'].pop())
        self.reject(lambda r: r['cases'].reverse())
        for key in self.report:
            self.reject(lambda r, k=key: r.pop(k))
        for key in self.report['cases'][0]:
            self.reject(lambda r, k=key: r['cases'][0].pop(k))
        for section in ('graph', 'raw_predictions', 'rejected_rows'):
            for key in self.report['cases'][0][section]:
                self.reject(lambda r, s=section, k=key: r['cases'][0][s].pop(k))
            self.reject(lambda r, s=section: r['cases'][0][s].update(unexpected=[]))
        self.reject(lambda r: r['cases'][1].update(rows_checked=1024.0))
        self.reject(lambda r: r['cases'][0].update(wires=True))

    def test_all_raw_predictions_require_complete_integer_bits(self):
        for backend in self.report['cases'][0]['raw_predictions']:
            self.reject(lambda r, n=backend: r['cases'][0]['raw_predictions'][n].pop())
            for value in (True, 0.0, 2, -1, None):
                self.reject(lambda r, n=backend, v=value: r['cases'][0]['raw_predictions'][n].__setitem__(0, v))
        for backend in ('native', 'pymatching'):
            self.reject(lambda r, n=backend: r['cases'][0]['raw_predictions'][n].__setitem__(0, 1))

    def test_graph_structure_weights_mapping_and_all_transform_rows(self):
        self.reject(lambda r: r['cases'][0]['graph']['edges'].pop())
        self.reject(lambda r: r['cases'][0]['graph']['edges'][0].update(weight=0.))
        self.reject(lambda r: r['cases'][0]['graph']['edges'][0].update(weight=float('nan')))
        self.reject(lambda r: r['cases'][0]['graph']['edges'][0].update(loss_factor=.25))
        self.reject(lambda r: r['cases'][0]['graph']['edges'][0].update(u=True))
        self.reject(lambda r: r['cases'][0]['graph']['edges'][0].update(observables=[True]))
        self.reject(lambda r: r['cases'][0]['graph'].update(mean_weight=0.))
        self.reject(lambda r: r['cases'][0]['graph']['loss_edges'].__setitem__(0, []))
        self.reject(lambda r: r['cases'][0]['graph']['loss_edges'].__setitem__(0, [True]))
        self.reject(lambda r: r['cases'][0]['graph']['loss_edges'].reverse())
        for section in ('syndromes', 'losses'):
            self.reject(lambda r, s=section: r['cases'][0]['graph'][s].pop())
            self.reject(lambda r, s=section: r['cases'][0]['graph'][s].__setitem__(1, []))
            self.reject(lambda r, s=section: r['cases'][0]['graph'][s].__setitem__(0, [True]))

    def test_mutant_predictions_and_rejection_summaries_are_recomputed(self):
        self.reject(lambda r: r['cases'][0]['rejected_rows'].update(native=[0]))
        self.reject(lambda r: r['cases'][0].update(flipped_prediction_rejected_rows=[]))
        self.reject(lambda r: r['cases'][0].update(flipped_prediction_rejected_rows=[False]))
        self.reject(lambda r: r['cases'][1].update(ignored_conditioning_rejected_rows=[21]))
        self.reject(lambda r: r['cases'][1]['raw_predictions']['ignored_conditioning'].__setitem__(0, 1))
        self.reject(lambda r: r['cases'][0]['raw_predictions'].update(
            flipped_native=r['cases'][0]['raw_predictions']['native'].copy()))
        self.reject(lambda r: r['cases'][0].update(placeholder_invariance_pass=1))

    def test_every_witness_field_has_an_independent_or_raw_source(self):
        original = self.report['cases'][1]['strict_witness']
        for key in original:
            self.reject(lambda r, k=key: r['cases'][1]['strict_witness'].pop(k))
        for key, value in {
            'packed_row': True, 'logical_order': [1, 0], 'fixed_optimum': [1],
            'conditioned_optimum': [0], 'fixed_decoder_prediction': 1,
            'native_prediction': True, 'pymatching_prediction': 0,
            'fixed_costs': [3, 2], 'conditioned_costs': [1.5, 2],
        }.items():
            self.reject(lambda r, k=key, v=value: r['cases'][1]['strict_witness'].__setitem__(k, v))
        self.reject(lambda r: r['cases'][0].update(strict_witness=original))

    def test_fresh_comparison_allows_legal_ties_and_equivalent_graph_order(self):
        alternate = copy.deepcopy(self.report)
        case = alternate['cases'][0]
        row = next(raw for raw in range(64) if len(decoder_reference.oracle(raw, wires=3)[0]) == 2)
        canonical = lambda raw: raw | sum(1 << (2*q+1) for q in range(3) if raw >> (2*q) & 1)
        for raw in range(64):
            if canonical(raw) == canonical(row):
                case['raw_predictions']['pymatching'][raw] ^= 1
        self.assertNotEqual(case['raw_predictions']['pymatching'], self.report['cases'][0]['raw_predictions']['pymatching'])
        graph = case['graph'];count = len(graph['edges'])
        graph['edges'].reverse()
        graph['loss_edges'] = [[count-1-i for i in indices] for indices in graph['loss_edges']]
        for edge in graph['edges']:
            if edge['v'] is not None:
                edge['u'], edge['v'] = edge['v'], edge['u']
            edge['weight'] += 2e-13
        graph['mean_weight'] += 2e-13
        compare_reports(alternate, self.report)
        alternate['method'] += ' Changed experiment definition.'
        with self.assertRaisesRegex(ValueError, 'Fresh decoder oracle definitions differ'):
            compare_reports(alternate, self.report)

    def test_tied_optimum_still_requires_placeholder_invariance(self):
        case = self.report['cases'][0]
        row = next(raw for raw in range(64) if len(decoder_reference.oracle(raw, wires=3)[0]) == 2
                   and any(raw >> (2*q) & 1 and not raw >> (2*q+1) & 1 for q in range(3)))
        self.reject(lambda r: r['cases'][0]['raw_predictions']['pymatching'].__setitem__(
            row, case['raw_predictions']['pymatching'][row] ^ 1))

    def test_contract_works_without_site_packages_and_under_optimization(self):
        program = ('import json,sys; from benchmarks.atom_loss.decoder_contract import verify_decoder; '
                   'verify_decoder(json.load(open(sys.argv[1])))')
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'report.json'
            for flags in (['-S'], ['-S', '-O']):
                path.write_text(json.dumps(self.report))
                good = subprocess.run([sys.executable, *flags, '-c', program, str(path)], cwd=ROOT,
                                      capture_output=True, text=True)
                self.assertEqual(good.returncode, 0, good.stderr)
                for kind in ('wrong_witness', 'failed_graph', 'missing_backend'):
                    report = copy.deepcopy(self.report)
                    if kind == 'wrong_witness':
                        report['cases'][1]['strict_witness']['native_prediction'] = 0
                    elif kind == 'failed_graph':
                        report['cases'][0]['hand_derived_graph_pass'] = False
                    else:
                        report['cases'][0]['rejected_rows'] = {}
                    path.write_text(json.dumps(report))
                    bad = subprocess.run([sys.executable, *flags, '-c', program, str(path)], cwd=ROOT,
                                         capture_output=True, text=True)
                    self.assertNotEqual(bad.returncode, 0, kind)
                    self.assertIn('Decoder report contract', bad.stderr)


if __name__ == '__main__':
    unittest.main()
