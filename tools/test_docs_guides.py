"""Run guide commands and check their displayed outputs against the real CLIs."""
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

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
                        '-p', 'qec-code', '--features', 'qec-code/cli'],
                       cwd=REPO, check=True, timeout=240)
        target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target'))
        if not target.is_absolute():
            target = REPO / target
        cls.env = dict(os.environ, PATH=str(target / 'debug') + os.pathsep + os.environ['PATH'])

    def test_simulation_dem_and_css_outputs_match_commands(self):
        for name in ('simulator', 'detector-models', 'css-codes'):
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
                self.assertEqual(checked, 2, name)

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
