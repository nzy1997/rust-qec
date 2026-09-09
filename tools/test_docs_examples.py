"""Execute the exact downloadable, version-pinned documentation example."""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


class DecoderDocumentationTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which('cargo'), 'Cargo is required for the Rust documentation example')
    def test_published_decoder_predicts_and_rejects_invalid_rows(self):
        root = Path(__file__).resolve().parents[1]
        env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target' / 'docs-examples'))
        with tempfile.TemporaryDirectory(prefix='rustqec-docs-') as tmp:
            project = Path(tmp) / 'first-decode'
            shutil.copytree(root / 'site/static/examples/first-decode', project)
            def run():
                return subprocess.run(['cargo', 'run', '--locked', '--quiet'], cwd=project,
                                      env=env, capture_output=True, text=True, timeout=180)
            positive = run()
            self.assertEqual(positive.returncode, 0, positive.stderr)
            self.assertEqual(positive.stdout, 'row 1: predicted L0=1\nrow 2: predicted L0=0\n')
            (project / 'detectors.01').write_text('1x\n')
            negative = run()
            self.assertNotEqual(negative.returncode, 0)
            self.assertIn('row 1: expected two detector bits', negative.stderr)
            self.assertEqual(negative.stdout, '')
