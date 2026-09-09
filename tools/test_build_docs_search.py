import tempfile
import unittest
from pathlib import Path
from tools.build_docs_search import build


class DocsSearchTest(unittest.TestCase):
    def test_indexes_nested_content_without_navigation_or_executable_text(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'qp101/protocol').mkdir(parents=True)
            (root / 'qp101/protocol/index.html').write_text('<title>QP101 &amp; formats</title><nav>navigation-only</nav><main><h2>Noise</h2><p>Atom <code>loss</code> &amp; b8</p><script>secretScript</script></main><footer>footer-only</footer>')
            records = build(root)
            self.assertEqual(records, [{'title': 'QP101 & formats', 'path': 'qp101/protocol/', 'text': 'Noise Atom loss & b8'}])
            self.assertTrue((root / 'data/docs-search.json').is_file())
