"""Run the atom-loss page's exact commands against the workspace CLI."""
from html.parser import HTMLParser
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class GuideCommands(HTMLParser):
    def __init__(self):
        super().__init__()
        self.steps = {}
        self.current = None

    def handle_starttag(self, tag, attrs):
        if tag == 'pre':
            self.current = dict(attrs).get('data-atom-loss-step')
            if self.current:
                self.steps[self.current] = ''

    def handle_data(self, data):
        if self.current:
            self.steps[self.current] += data

    def handle_endtag(self, tag):
        if tag == 'pre':
            self.current = None


class AtomLossGuideTest(unittest.TestCase):
    def test_page_runs_one_dataset_through_sampling_decoding_and_scoring(self):
        subprocess.run(['cargo', 'build', '--quiet', '--locked', '-p', 'rustqec-cli'],
                       cwd=REPO, check=True, timeout=240)
        target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target'))
        if not target.is_absolute():
            target = REPO / target
        env = dict(os.environ, PATH=str(target / 'debug') + os.pathsep + os.environ['PATH'])
        parser = GuideCommands()
        parser.feed((REPO / 'site/templates/atom-loss.html').read_text())
        self.assertEqual(list(parser.steps), ['model', 'sample', 'decode', 'check'])
        with tempfile.TemporaryDirectory(prefix='rustqec-loss-guide-') as tmp:
            work = Path(tmp)
            def run(script):
                return subprocess.run(['sh', '-eu'], input=script, cwd=work, env=env,
                                      text=True, capture_output=True, timeout=60)
            for step, script in parser.steps.items():
                result = run(script)
                self.assertEqual(result.returncode, 0, f'{step}: {result.stdout}\n{result.stderr}')
            self.assertEqual(result.stdout, 'Decoded shots: 64\nLoss patterns: 10\nLogical errors: 0 / 64\n')
            # A truncated prediction file must not produce a plausible-looking score.
            predictions = work / 'predictions.b8'
            predictions.write_bytes(predictions.read_bytes()[:-1])
            negative = run(parser.steps['check'])
            self.assertNotEqual(negative.returncode, 0)
            self.assertIn('Incomplete rows', negative.stderr)
            self.assertEqual(negative.stdout, '')
            # The decoder must reject tampered public input instead of writing predictions.
            shots = work / 'data/public/shots.b8'
            shots.write_bytes(shots.read_bytes()[:-1])
            bad_decode = run('rustqec decode --decoder envelope-matching --dataset data/public --out rejected.b8 --stats-out rejected.json')
            self.assertNotEqual(bad_decode.returncode, 0)
            self.assertFalse((work / 'rejected.b8').exists())
            self.assertFalse((work / 'rejected.json').exists())


if __name__ == '__main__':
    unittest.main()
