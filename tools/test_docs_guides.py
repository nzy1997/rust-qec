"""Run guide commands and check their displayed outputs against the real CLIs."""
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

REPO = Path(__file__).resolve().parents[1]


class CodeBlocks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.blocks = []
        self.current = None

    def handle_starttag(self, tag, attrs):
        if tag == 'pre':
            attributes = dict(attrs)
            self.current = {'text': '', 'output': attributes.get('data-output') == 'true',
                            'language': attributes.get('data-language', 'Shell')}

    def handle_data(self, data):
        if self.current is not None:
            self.current['text'] += data

    def handle_endtag(self, tag):
        if tag == 'pre' and self.current is not None:
            self.blocks.append(self.current)
            self.current = None


class GuideExampleTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(['cargo', 'build', '--quiet', '--locked', '-p', 'rstim',
                        '-p', 'qec-code', '-p', 'rsinter', '--features',
                        'qec-code/cli,rsinter/rbposd-runner,rsinter/rmatching-runner'],
                       cwd=REPO, check=True, timeout=240)
        target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target'))
        if not target.is_absolute():
            target = REPO / target
        cls.env = dict(os.environ, PATH=str(target / 'debug') + os.pathsep + os.environ['PATH'])

    def test_css_and_frozen_decoder_comparison_match_displayed_outputs(self):
        for name, expected in (('css-codes', 2), ('decoding', 1)):
            with self.subTest(page=name), tempfile.TemporaryDirectory(prefix='qec-guide-') as tmp:
                work = Path(tmp)
                (work / 'circuit.stim').write_text(
                    (REPO / 'site/static/examples/getting-started.stim').read_text())
                parser = CodeBlocks()
                parser.feed((REPO / f'site/templates/{name}.html').read_text())
                previous = None
                checked = 0
                for block in parser.blocks:
                    if block['output']:
                        self.assertIsNotNone(previous, 'Output must follow a command')
                        self.assertEqual(previous.stdout.strip(), block['text'].strip(), name)
                        checked += 1
                    elif block['language'].startswith('Shell'):
                        if block['text'].startswith('cargo install'):
                            continue  # Executables were built above, without changing user PATH.
                        previous = subprocess.run(['sh', '-eu'], input=block['text'], cwd=work,
                                                  env=self.env, text=True, capture_output=True, timeout=60)
                        self.assertEqual(previous.returncode, 0, previous.stderr)
                self.assertEqual(checked, expected, name)
                if name == 'css-codes':
                    bound = json.loads(subprocess.check_output([
                        'qec-code', 'code', 'css-distance', 'randomized-upper-bound',
                        '--hx', 'hx.json', '--hz', 'hz.json', '--iterations', '100',
                        '--seed', '7', '--json'], cwd=work, env=self.env, text=True))
                    self.assertEqual(bound['bound_type'], 'upper')
                    self.assertEqual(bound['upper_bound'], 3)
                    self.assertEqual(bound['witness']['weight'], 3)
                    (work / 'hz.json').write_text('{"format":"sparse_rows","num_cols":7,"rows":[[0]]}')
                    rejected = subprocess.run(['rstim', 'gen', '--code', 'css', '--task', 'memory',
                        '--hx', 'hx.json', '--hz', 'hz.json', '--basis', 'z', '--rounds', '3',
                        '--out', 'invalid.stim'], cwd=work, env=self.env, capture_output=True)
                    self.assertNotEqual(rejected.returncode, 0, 'Noncommuting checks must fail')
                else:
                    (work / 'matching.b8').write_bytes((work / 'matching.b8').read_bytes()[:-1])
                    score = next(block['text'] for block in parser.blocks if block['text'].startswith("python3 - <<'PYTHON'"))
                    failed_score = subprocess.run(['sh', '-eu'], input=score, cwd=work,
                        env=self.env, text=True, capture_output=True)
                    self.assertNotEqual(failed_score.returncode, 0, 'Truncated predictions must not score')
                    self.assertNotIn('logical errors', failed_score.stdout)
                    (work / 'invalid.b8').write_bytes(bytes([128]))
                    failed_replay = subprocess.run(['rsinter', 'replay', '--dem', 'model.dem',
                        '--dets', 'invalid.b8', '--decoder', 'rmatching', '--predictions-out',
                        'invalid-predictions.b8', '--stats-out', 'invalid-stats.json'],
                        cwd=work, env=self.env, capture_output=True)
                    self.assertNotEqual(failed_replay.returncode, 0)
                    self.assertFalse((work / 'invalid-predictions.b8').exists())

    def test_loss_model_reports_flag_placeholder_and_reset(self):
        parser = CodeBlocks()
        parser.feed((REPO / 'site/templates/atom-loss.html').read_text())
        command = next(block['text'] for block in parser.blocks
                       if 'cat > loss-record.stim' in block['text'])
        with tempfile.TemporaryDirectory(prefix='qec-loss-model-') as tmp:
            result = subprocess.run(['sh', '-eu'], input=command, cwd=tmp,
                env=self.env, text=True, capture_output=True, check=True)
            self.assertEqual(result.stdout.strip(), '1100')

    def test_downloaded_rust_experiment_is_complete_and_runs_locked(self):
        source = REPO / 'site/static/examples/rust-experiment'
        with tempfile.TemporaryDirectory(prefix='qec-rust-api-') as tmp:
            with zipfile.ZipFile(source / 'tutorial.zip') as archive:
                self.assertEqual(set(archive.namelist()), {'Cargo.toml', 'Cargo.lock', 'src/main.rs'})
                for name in archive.namelist():
                    self.assertEqual(archive.read(name), (source / name).read_bytes())
                archive.extractall(tmp)
            env = dict(self.env, CARGO_TARGET_DIR=str(REPO / 'target/docs-consumer'))
            result = subprocess.run(['cargo', 'run', '--quiet', '--locked'], cwd=tmp,
                env=env, capture_output=True, text=True, timeout=240, check=True)
            self.assertEqual(result.stdout.strip(), 'validated Bell parity and 128/128 observable predictions')

    def test_archive_roundtrip_and_diagram_exports(self):
        for name in ('rsmp-v1-showcase', 'qp101'):
            with self.subTest(page=name), tempfile.TemporaryDirectory(prefix='qec-guide-') as tmp:
                work = Path(tmp)
                (work / 'circuit.stim').write_text(
                    (REPO / 'site/static/examples/getting-started.stim').read_text())
                parser = CodeBlocks()
                parser.feed((REPO / f'site/templates/{name}.html').read_text())
                for block in parser.blocks:
                    if block['language'].startswith('Shell') and not block['output']:
                        result = subprocess.run(['sh', '-eu'], input=block['text'], cwd=work,
                                                env=self.env, text=True, capture_output=True, timeout=60)
                        self.assertEqual(result.returncode, 0, result.stderr)
                if name == 'rsmp-v1-showcase':
                    self.assertEqual((work / 'samples.b8').read_bytes(),
                                     (work / 'recovered.b8').read_bytes())
                    self.assertEqual((work / 'samples.b8').stat().st_size, 4)
                else:
                    self.assertIn('<svg', (work / 'circuit.svg').read_text())
                    self.assertIsInstance(json.loads((work / 'circuit.qp101.json').read_text()), dict)


if __name__ == '__main__':
    unittest.main()
