"""Actual graph-adapter controls and retained chain-report corruption tests."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

from . import chain_reference, run
from .chain_contract import ROWS, compare_reports, verify_chain


class ChainContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.report = chain_reference.run(run.ROOT/'target/release/rustqec',
                                        run.ROOT/'target/release/examples/export_matching_benchmark')

    def reject(self, change):
        report = copy.deepcopy(self.report)
        change(report)
        with self.assertRaises(ValueError):
            verify_chain(report)

    def test_actual_healthy_graph_adapters_and_three_mutants(self):
        verify_chain(self.report)
        self.assertEqual(self.report['witness_generation'], 'stim-reference-sample')
        self.assertEqual(self.report['measurement_sha256'],
                         'f179f618bd3a18bcab28565103c4717c0111d3ae3a534d326200698e8d118cea')
        for backend in chain_reference.GRAPH_BACKENDS:
            self.assertEqual(self.report['backends'][backend]['rejected_rows'], [])
            for mutation in chain_reference.GRAPH_MUTATIONS:
                record = self.report['graph_adapter_controls'][mutation][backend]
                if mutation == 'empty_edges' and record['outcome'] == 'decoder_error':
                    continue
                self.assertEqual(record['outcome'], 'oracle_rejected')
                self.assertGreater(len(record['rejected_rows']), 0)

    def test_complete_backend_objective_and_mutation_inventory_required(self):
        for section in ('backends', 'allowed_answers', 'graph_adapter_controls',
                        'compiler_output_mutations_rejected'):
            for name in self.report[section]:
                with self.subTest(section=section, missing=name):
                    self.reject(lambda r, s=section, n=name: r[s].pop(n))
            self.reject(lambda r, s=section: r[s].update(unexpected={}))
        for mutation in chain_reference.GRAPH_MUTATIONS:
            for backend in chain_reference.GRAPH_BACKENDS:
                self.reject(lambda r, m=mutation, b=backend: r['graph_adapter_controls'][m].pop(b))

    def test_raw_rows_cannot_be_truncated_or_replaced_with_nonbinary_values(self):
        for name in self.report['backends']:
            self.reject(lambda r, n=name: r['backends'][n]['predictions'].pop())
            for value in (2, -1, True, 0.0, None):
                self.reject(lambda r, n=name, v=value: r['backends'][n]['predictions'].__setitem__(0, v))
        for name in self.report['allowed_answers']:
            self.reject(lambda r, n=name: r['allowed_answers'][n].pop())
            for value in ([], [0, 0], [1, 0], [True], [0.0], [2], None):
                self.reject(lambda r, n=name, v=value: r['allowed_answers'][n].__setitem__(0, v))
        self.reject(lambda r: r.update(rows=ROWS-1))
        self.reject(lambda r: r.update(rows=float(ROWS)))
        for field in ('measurement_sha256', 'witness_generation'):
            self.reject(lambda r, f=field: r.pop(f))
        for value in ('invalid-hash', 'g'*64, None, 0):
            self.reject(lambda r, v=value: r.update(measurement_sha256=v))
        self.reject(lambda r: r.update(witness_generation='seeded-iid-sample'))

    def test_all_stored_summaries_and_hashes_are_recomputed(self):
        for name, original in self.report['backends'].items():
            for field, value in original.items():
                if field == 'predictions':
                    continue
                changed = not value if type(value) is bool else value+1 if type(value) is int else [0] if type(value) is list else '0'*64
                with self.subTest(backend=name, field=field):
                    self.reject(lambda r, n=name, f=field, v=changed: r['backends'][n].__setitem__(f, v))
        self.reject(lambda r: r['backends']['envelope-matching'].__setitem__('placeholder_invariance', 1))
        self.reject(lambda r: r['compiler_output_mutations_rejected'].__setitem__('pauli_weight', 1))

    def test_corrupted_predictions_fail_even_after_hash_is_resealed(self):
        for name in self.report['backends']:
            def change(report):
                record = report['backends'][name]
                record['predictions'][0] ^= 1
                record['prediction_sha256'] = hashlib.sha256(bytes(record['predictions'])).hexdigest()
            self.reject(change)
        self.reject(lambda r: r['allowed_answers'].__setitem__('envelope-matching', [[0, 1]]*ROWS))

    def test_mutant_outcomes_need_actual_complete_wrong_predictions(self):
        for mutation in chain_reference.GRAPH_MUTATIONS:
            for backend in chain_reference.GRAPH_BACKENDS:
                def control(report):
                    return report['graph_adapter_controls'][mutation][backend]
                self.reject(lambda r: control(r).__setitem__('outcome', 'accepted'))
                self.reject(lambda r: control(r).__setitem__('rejected_rows', [ROWS]))
                if mutation != 'empty_edges':
                    self.reject(lambda r: control(r).update(outcome='decoder_error', predictions=None, rejected_rows=[]))
                if control(self.report)['outcome'] == 'oracle_rejected':
                    self.reject(lambda r: control(r)['predictions'].pop())
                    self.reject(lambda r: control(r).update(predictions=r['backends']['envelope-matching']['predictions'], rejected_rows=[]))
                else:
                    self.reject(lambda r: control(r).__setitem__('predictions', []))

    def test_actual_constant_zero_python_adapter_fails_the_positive_oracle(self):
        def wrong(graph, conditioned):
            return np.zeros(len(graph['syndromes']), dtype=np.uint8), {}
        with patch.object(run, 'python_decode', wrong):
            report = chain_reference.run(run.ROOT/'target/release/rustqec',
                                         run.ROOT/'target/release/examples/export_matching_benchmark')
        self.assertEqual(report['status'], 'FAIL')
        self.assertGreater(len(report['backends']['pymatching-envelope']['rejected_rows']), 0)
        self.assertEqual(report['backends']['pymatching-envelope']['predicted_ones'], 0)
        with self.assertRaises(ValueError):
            verify_chain(report)

    def test_contract_works_without_site_packages_and_with_python_optimization(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/'report.json'
            program = ('import json,sys; from benchmarks.atom_loss.chain_contract import verify_chain; '
                       'verify_chain(json.load(open(sys.argv[1])))')
            path.write_text(json.dumps(self.report))
            healthy = subprocess.run([sys.executable, '-S', '-O', '-c', program, str(path)],
                                     cwd=run.ROOT, capture_output=True, text=True)
            self.assertEqual(healthy.returncode, 0, healthy.stderr)
            report = copy.deepcopy(self.report)
            report['backends']['pymatching-envelope']['predicted_ones'] += 1
            path.write_text(json.dumps(report))
            rejected = subprocess.run([sys.executable, '-S', '-O', '-c', program, str(path)],
                                      cwd=run.ROOT, capture_output=True, text=True)
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn('Chain report contract', rejected.stderr)

    def test_cli_compare_rejects_a_different_published_report(self):
        with tempfile.TemporaryDirectory() as temp:
            temp = Path(temp)
            different = copy.deepcopy(self.report)
            different['method'] += ' An altered experiment definition.'
            published = temp/'published.json'; published.write_text(json.dumps(different))
            result = subprocess.run([sys.executable, '-m', 'benchmarks.atom_loss.chain_reference',
                                     '--out', str(temp/'out.json'), '--compare', str(published)],
                                    cwd=run.ROOT, capture_output=True, text=True, timeout=120)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Fresh chain oracle definitions differ from published evidence', result.stderr)

    def test_fresh_comparison_accepts_different_legal_tied_optima(self):
        alternate = copy.deepcopy(self.report)
        choices = alternate['allowed_answers']['envelope-mle']
        row = next(i for i, values in enumerate(choices[:ROWS//2]) if len(values) == 2)
        record = alternate['backends']['envelope-mle']
        for i in (row, row+ROWS//2):
            record['predictions'][i] ^= 1
        record['predicted_ones'] = sum(record['predictions'])
        record['prediction_sha256'] = hashlib.sha256(bytes(record['predictions'])).hexdigest()
        self.assertNotEqual(record['prediction_sha256'], self.report['backends']['envelope-mle']['prediction_sha256'])
        verify_chain(alternate)
        compare_reports(alternate, self.report)

    def test_fresh_comparison_rejects_changed_allowed_sets_and_metadata(self):
        changed = copy.deepcopy(self.report)
        changed['method'] += ' Different definition.'
        verify_chain(changed)
        with self.assertRaisesRegex(ValueError, 'oracle definitions differ.*method'):
            compare_reports(changed, self.report)
        changed = copy.deepcopy(self.report)
        changed['measurement_sha256'] = '0'*64
        verify_chain(changed)
        with self.assertRaisesRegex(ValueError, 'oracle definitions differ.*measurement_sha256'):
            compare_reports(changed, self.report)
        changed = copy.deepcopy(self.report)
        # Remove only the unchosen answer from a tied row and its placeholder
        # counterpart, keeping this report internally consistent and valid.
        choices = changed['allowed_answers']['envelope-mle']
        row = next(i for i, values in enumerate(choices[:ROWS//2]) if len(values) == 2)
        record = changed['backends']['envelope-mle']
        for i in (row, row+ROWS//2):
            choices[i] = [record['predictions'][i]]
        record['unique_optimum_rows'] += 2
        verify_chain(changed)
        with self.assertRaisesRegex(ValueError, 'oracle definitions differ.*allowed_answers'):
            compare_reports(changed, self.report)


if __name__ == '__main__':
    unittest.main()
