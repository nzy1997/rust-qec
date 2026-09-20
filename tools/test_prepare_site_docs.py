"""Canonical contracts must refresh on every build, without modifying their source."""
from pathlib import Path
import json
import tempfile
import unittest
from tools.prepare_site_docs import prepare


class PrepareSiteDocsTests(unittest.TestCase):
    def test_stages_and_refreshes_both_contracts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'site').mkdir()
            (root / 'site/versions.json').write_text(json.dumps({
                'default': 'master', 'versions': [
                    {'id': 'master', 'label': 'RustQEC 0.3 · Development', 'ref': 'master', 'path': '',
                     'channel': 'development', 'release_line': '0.3', 'native_release': '0.3.0'}
                ]
            }))
            members = []
            for name in ('qec-ilp-core', 'qec-code', 'rbposd', 'rilpqec', 'rmatching',
                         'rsinter', 'rstim', 'rustqec-cli'):
                members.append(name)
                manifest = root / name / 'Cargo.toml'
                manifest.parent.mkdir(parents=True)
                manifest.write_text(f'[package]\nname = "{name}"\nversion = "0.3.1"\n')
            (root / 'Cargo.toml').write_text('[workspace]\nmembers = [' +
                                             ','.join(f'"{name}"' for name in members) + ']\n')
            pairs = [('rstim/doc/QP101-ZY.md', 'qp101-protocol.md'),
                     ('docs/support-compatibility.md', 'support-compatibility.md'),
                     ('docs/maintainer-reference.md', 'maintainer-reference.md')]
            for source, _ in pairs:
                path = root / source
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('# Original\nUnicode: 量子\n')
            prepare(root)
            version = json.loads((root / 'site/generated/docs-version.json').read_text())
            self.assertEqual(version['id'], 'master')
            self.assertEqual(version['release_line'], '0.3')
            self.assertEqual(version['packages']['rustqec_cli']['version'], '0.3.1')
            for source, target in pairs:
                self.assertEqual((root / source).read_bytes(), (root / 'site/generated' / target).read_bytes())
                (root / source).write_text('# Revised\n')
            prepare(root)
            for source, target in pairs:
                self.assertEqual((root / 'site/generated' / target).read_text(), '# Revised\n')
                self.assertEqual((root / source).read_text(), '# Revised\n')

    def test_missing_contract_fails_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(FileNotFoundError):
                prepare(Path(tmp))
