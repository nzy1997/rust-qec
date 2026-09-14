"""Raw-public-row workload checks and fully resealed publication attacks."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile

from .plot import render
from .source_contract import ROOT
from .workload_contract import measurement_flags, public_workload, verify_case, verify_workloads


SOURCE = ROOT/'site/static/data/atom-loss'


def defects():
    def counts(case):
        case['graph']['loss_patterns'] = 1
        for run in case['decoders']['pymatching-envelope']['runs']:
            run['graph_builds'] = run['batch_calls'] = 1
        for run in case['decoders']['envelope-matching-offline']['runs']:
            run['batch']['graph_builds'] = run['stats']['matching_graph_builds'] = 1
    def set_run(backend, field, value, nested=None):
        def mutate(case):
            run = case['decoders'][backend]['runs'][0]
            (run if nested is None else run[nested])[field] = value
        return mutate
    return {
        'coherent-pattern-count': counts,
        'conditioned-calls': set_run('pymatching-envelope', 'batch_calls', 1),
        'fixed-graph-count': set_run('pymatching-fixed', 'graph_builds', 193),
        'fixed-loop-cache': set_run('pymatching-fixed-loop', 'cache_hits', 5000),
        'python-api': set_run('pymatching-envelope', 'graph_api', 'per-edge loop'),
        'python-policy': set_run('pymatching-envelope', 'execution', 'unrecorded cache'),
        'offline-stats-mirror': set_run('envelope-matching-offline', 'matching_graph_builds', 1, 'stats'),
        'offline-policy': set_run('envelope-matching-offline', 'policy', 'FIFO', 'batch'),
        'offline-boundary': set_run('envelope-matching-offline', 'boundary', 'output excluded', 'batch'),
        'offline-shots': set_run('envelope-matching-offline', 'shots', 4999, 'batch'),
        'native-decoder': set_run('envelope-matching', 'decoder', 'envelope-mle', 'stats'),
        'native-schema': set_run('envelope-matching', 'schema_version', 'unknown', 'stats'),
        'native-circuit': set_run('envelope-matching', 'circuit_sha256', '0'*64, 'stats'),
        'native-shot-count': set_run('envelope-matching', 'shot_count', 4999, 'stats'),
        'native-exact-pattern-count': set_run('envelope-matching', 'distinct_loss_patterns', 1, 'stats'),
        'native-cache-accounting': set_run('envelope-matching', 'cache_hits', 0, 'stats'),
        'native-bool-counter': set_run('envelope-matching', 'primitive_probe_count', True, 'stats'),
        'native-failed-run': set_run('envelope-matching', 'status', 'failed'),
        'native-exit-code': set_run('envelope-matching', 'exit_code', 1),
        'native-unknown-counter': set_run('envelope-matching', 'unvalidated_counter', 1, 'stats'),
        'native-mle-counter': set_run('envelope-matching', 'mle_model_builds', 1, 'stats'),
        'timing-order': lambda c: c['timing_order'][0].reverse(),
        'graph-detectors': lambda c: c['graph'].__setitem__('detectors', 999),
        'graph-observables': lambda c: c['graph'].__setitem__('num_observables', 2),
        'graph-source': lambda c: c['graph'].__setitem__('source', 'private answers'),
        'logical-x-support': lambda c: c.__setitem__('logical_x_support', '1'),
        'workload-rounds': lambda c: c.__setitem__('rounds', 3),
        'missing-run-field': lambda c: c['decoders']['pymatching-envelope']['runs'][0].pop('batch_calls'),
    }


def reseal(root, case, *, redraw=False):
    """Update all copies/hashes so failure cannot be ordinary integrity mismatch."""
    data = (json.dumps(case, indent=2)+'\n').encode()
    (root/'tradeoff.json').write_bytes(data)
    archive = root/'shot-data-v1.zip'
    with zipfile.ZipFile(archive) as z:
        payload = {name: z.read(name) for name in z.namelist()}
    payload['tradeoff.json'] = data
    index = json.loads(payload['index.json'])
    index['sha256']['tradeoff.json'] = hashlib.sha256(data).hexdigest()
    payload['index.json'] = (json.dumps(index, indent=2)+'\n').encode()
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as z:
        for name, value in payload.items():
            z.writestr(name, value)
    if redraw:
        render(root)
    manifest = json.loads((root/'bundle.json').read_text())
    manifest['sha256'] = {name: hashlib.sha256((root/name).read_bytes()).hexdigest()
                          for name in manifest['sha256']}
    (root/'bundle.json').write_text(json.dumps(manifest))


class WorkloadTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.case = json.loads((SOURCE/'tradeoff.json').read_text())
        with zipfile.ZipFile(SOURCE/'shot-data-v1.zip') as z:
            cls.workload = public_workload(lambda name: z.read('tradeoff/'+name))

    def test_actual_complete_bundle_agrees_with_public_rows_and_fixed_policy(self):
        verify_workloads(SOURCE)
        self.assertEqual(self.workload['loss_patterns'], 193)

    def test_flag_positions_expand_nested_repeats_and_skip_plain_measurements(self):
        text = 'R 0 1\nM 0\nREPEAT 2 {\nML 1\nREPEAT 2 {\nMRL 0\n}\nM 1\n}\n'
        bits, detectors, observables, flags = measurement_flags(text)
        self.assertEqual((bits, detectors, observables, flags),
                         (15, 0, {}, [1, 3, 5, 8, 10, 12]))

    def test_all_fixed_metadata_mutations_fail(self):
        for label, mutate in defects().items():
            with self.subTest(defect=label):
                case = copy.deepcopy(self.case)
                mutate(case)
                # The public evidence remains the independent authority.
                with self.assertRaises((ValueError, KeyError)):
                    verify_case(case, self.workload, tradeoff=True)

    def test_fully_resealed_attacks_fail_full_verifier_in_both_python_modes(self):
        selected = ['coherent-pattern-count', 'fixed-loop-cache', 'offline-stats-mirror',
                    'native-circuit', 'native-bool-counter', 'timing-order', 'python-policy']
        with tempfile.TemporaryDirectory() as temp:
            for label in selected:
                root = Path(temp)/label
                shutil.copytree(SOURCE, root)
                case = copy.deepcopy(self.case)
                defects()[label](case)
                # Reproduce the originally successful complete, chart-changing attack.
                reseal(root, case, redraw=label == 'coherent-pattern-count')
                for flags in [[], ['-O']]:
                    with self.subTest(defect=label, flags=flags):
                        result = subprocess.run([sys.executable, *flags, '-m',
                                                 'benchmarks.atom_loss.verify', str(root)],
                                                cwd=ROOT, text=True, capture_output=True)
                        self.assertNotEqual(result.returncode, 0)
                        # Dirty source/provenance or another unrelated check is not success.
                        self.assertIn('ValueError: Workload report contract:', result.stderr)


if __name__ == '__main__':
    unittest.main()
