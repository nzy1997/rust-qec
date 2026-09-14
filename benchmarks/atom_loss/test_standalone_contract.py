"""Exercise the downloadable stdlib-only checker against coherently resealed ZIPs."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

from .shot_data import original_plan

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT/'site/static/data/atom-loss'
SCRIPT = Path(__file__).with_name('shot_data.py')
PAIR_KEYS = ('disagreements_with_native', 'paired_native_only_wrong', 'paired_python_only_wrong')


def resealed(payload, path):
    index = json.loads(payload['index.json'])
    index['sha256'] = {name: hashlib.sha256(data).hexdigest()
                       for name, data in payload.items() if name != 'index.json'}
    payload = {**payload, 'index.json': json.dumps(index).encode()}
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in payload.items():
            archive.writestr(name, data)


class StandaloneTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with zipfile.ZipFile(SOURCE/'shot-data-v1.zip') as archive:
            cls.original = {name: archive.read(name) for name in archive.namelist()}
        with zipfile.ZipFile(SOURCE/'accuracy-seeds.zip') as archive:
            cls.seeded = {name: archive.read(name) for name in archive.namelist()}

    def check_cli(self, archive, command, *, accepted):
        for optimize in ([], ['-O']):
            with self.subTest(optimize=optimize):
                # Copy the single file to a directory without the repository or dependencies.
                script = archive.parent/'rescore.py'
                script.write_bytes(SCRIPT.read_bytes())
                result = subprocess.run([sys.executable, '-I', '-S', *optimize, str(script), command, str(archive)],
                                        cwd=archive.parent, text=True, capture_output=True)
                if accepted:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn('198 prediction files' if command == 'rescore' else '147 predictions', result.stdout)
                else:
                    self.assertNotEqual(result.returncode, 0, result.stdout)
                    self.assertIn('Dataset contract:', result.stderr)
                    self.assertNotIn('ModuleNotFoundError', result.stderr)

    def test_complete_archives_pass_without_repository_or_site_packages(self):
        with tempfile.TemporaryDirectory() as temp:
            for name, payload, command in [('original.zip', self.original, 'rescore'),
                                           ('seeds.zip', self.seeded, 'rescore-seeds')]:
                path = Path(temp)/name
                resealed(payload, path)
                self.check_cli(path, command, accepted=True)

    def test_removed_comparator_and_prediction_members_cannot_redefine_completeness(self):
        payload = self.original.copy()
        for filename in ('decoding.json', 'tradeoff.json'):
            data = json.loads(payload[filename])
            for case in data if isinstance(data, list) else [data]:
                case['decoders'].pop('pymatching-envelope')
            payload[filename] = json.dumps(data).encode()
        for name in list(payload):
            if '/pymatching-envelope-' in name:
                del payload[name]
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/'missing-comparator.zip'
            resealed(payload, path)
            self.check_cli(path, 'rescore', accepted=False)

    def test_fixed_plan_rejects_unknown_missing_duplicate_or_relabelled_settings(self):
        sweep = json.loads(self.original['decoding.json'])
        tradeoff = json.loads(self.original['tradeoff.json'])
        cases = [(f"d{c['distance']}-p{c['loss_probability']}", c) for c in sweep]+[('tradeoff', tradeoff)]
        for changed in [cases[:-1], cases[:-1]+[cases[0]], [('d9-p0.01', cases[0][1])]+cases[1:]]:
            with self.subTest(labels=[label for label, _ in changed]), self.assertRaises(ValueError):
                original_plan(changed)
        # A wrong distance must fail even when its archive label is left intact.
        altered = json.loads(json.dumps(cases))
        altered[0][1]['distance'] = 9
        with self.assertRaises(ValueError):
            original_plan(altered)

    def test_resealed_original_score_plan_and_pair_attacks_fail_in_both_modes(self):
        def set_score(key, value):
            return lambda c: c['decoders']['envelope-mle'].__setitem__(key, value)
        def set_pair(key, value):
            return lambda c: c['decoders']['pymatching-envelope'].__setitem__(key, value)
        mutations = {
            'wilson-zero': set_score('wilson_95', [0, 0]),
            'wilson-missing': lambda c: c['decoders']['envelope-mle'].pop('wilson_95'),
            'wilson-nan': set_score('wilson_95', [0, float('nan')]),
            'missing-paired': lambda c: [c['decoders']['pymatching-envelope'].pop(k) for k in PAIR_KEYS],
            'missing-one-pair': lambda c: c['decoders']['pymatching-envelope'].pop(PAIR_KEYS[1]),
            'bool-pair': set_pair('paired_native_only_wrong', False),
            'float-pair': set_pair('paired_native_only_wrong', 0.0),
            'wrong-rounds': lambda c: c.__setitem__('rounds', 999),
            'wrong-seed': lambda c: c.__setitem__('seed', 20260911),
            'wrong-shots': lambda c: c.__setitem__('shots', 5000.0),
            'wrong-loss': lambda c: c.__setitem__('loss_probability', .001),
            'wrong-pauli': lambda c: c.__setitem__('pauli_probability', .002),
            'bool-errors': set_score('errors', True),
            'float-errors': set_score('errors', 1.0),
            'float-score-shots': set_score('shots', 5000.0),
            'bool-rate': set_score('logical_error_rate', False),
            'string-runs': set_score('runs', 'abc'),
            'missing-repetition': lambda c: c['decoders']['envelope-mle']['runs'].pop(),
            'empty-repetition': lambda c: c['decoders']['envelope-mle']['runs'].__setitem__(0, {}),
            'failed-repetition': lambda c: c['decoders']['envelope-mle']['runs'][0].__setitem__('status', 'failed'),
            'incomplete-shot-outcomes': lambda c: c['decoders']['envelope-mle']['runs'][0]['stats'].__setitem__('attempted_shot_count', 4999),
            'python-repetition-identity': lambda c: c['decoders']['pymatching-envelope']['runs'][0].__setitem__('export_repetition', 1),
            'arbitrary-python-record': lambda c: c['decoders']['pymatching-envelope']['runs'].__setitem__(0, {'invented': 1}),
        }
        with tempfile.TemporaryDirectory() as temp:
            for label, mutate in mutations.items():
                with self.subTest(attack=label):
                    payload = self.original.copy()
                    case = json.loads(payload['tradeoff.json'])
                    mutate(case)
                    payload['tradeoff.json'] = json.dumps(case).encode()
                    path = Path(temp)/(label+'.zip')
                    resealed(payload, path)
                    self.check_cli(path, 'rescore', accepted=False)

    def test_resealed_seed_score_types_and_mandatory_pairs_fail_in_both_modes(self):
        mutations = {
            'bool-rate': lambda r: r['cases'][0]['decoders']['envelope-matching'].__setitem__('logical_error_rate', False),
            'float-shots': lambda r: r['cases'][0].__setitem__('shots', 5000.0),
            'float-seed-plan': lambda r: r.__setitem__('shots_per_seed', 5000.0),
            'missing-pairs': lambda r: r['cases'][0]['paired'].pop('pymatching-envelope'),
            'bool-pair': lambda r: r['cases'][0]['paired']['pymatching-envelope'].__setitem__('native_only_wrong', False),
            'float-pooled-shots': lambda r: r['pooled'][0].__setitem__('shots', 15000.0),
        }
        with tempfile.TemporaryDirectory() as temp:
            for label, mutate in mutations.items():
                with self.subTest(attack=label):
                    payload = self.seeded.copy()
                    report = json.loads(payload['accuracy-seeds.json'])
                    mutate(report)
                    payload['accuracy-seeds.json'] = json.dumps(report).encode()
                    path = Path(temp)/(label+'.zip')
                    resealed(payload, path)
                    self.check_cli(path, 'rescore-seeds', accepted=False)


if __name__ == '__main__':
    unittest.main()
