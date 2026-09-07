"""Canonical contracts must refresh on every build, without modifying their source."""
from pathlib import Path
import tempfile
import unittest
from tools.prepare_site_docs import prepare


class PrepareSiteDocsTests(unittest.TestCase):
    def test_stages_and_refreshes_both_contracts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pairs = [('rstim/doc/QP101-ZY.md', 'qp101-protocol.md'),
                     ('docs/support-compatibility.md', 'support-compatibility.md')]
            for source, _ in pairs:
                path = root / source
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('# Original\nUnicode: 量子\n')
            prepare(root)
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
