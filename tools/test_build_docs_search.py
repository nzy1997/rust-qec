import tempfile
import unittest
from pathlib import Path
from tools.build_docs_search import build


class DocsSearchTest(unittest.TestCase):
    def index(self, html):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'qp101/protocol').mkdir(parents=True)
            (root / 'qp101/protocol/index.html').write_text(html)
            records = build(root)
            self.assertTrue((root / 'data/docs-search.json').is_file())
            return records

    def test_sections_deep_link_without_navigation_or_executable_text(self):
        records = self.index('<title>QP101 &amp; formats</title><nav>navigation-only</nav><main><h2 id="noise">Noise</h2><p>Atom <code>loss</code> &amp; b8</p><script>secretScript</script><h2 id="format">Format</h2><p>JSON schema.</p></main><footer>footer-only</footer>')
        self.assertEqual(records, [
            {'title': 'Noise', 'page_title': 'QP101 & formats', 'path': 'qp101/protocol/#noise', 'text': 'Atom loss & b8', 'excerpt': 'Atom loss & b8'},
            {'title': 'Format', 'page_title': 'QP101 & formats', 'path': 'qp101/protocol/#format', 'text': 'JSON schema.', 'excerpt': 'JSON schema.'},
        ])

    def test_dense_content_remains_searchable_but_never_becomes_the_prose_preview(self):
        records = self.index('<title>Sampling</title><main><h2>Atom loss</h2><table><tr><td>16.775 ms</td><td>60,896 shots</td></tr></table><p>Preserve loss flags for the decoder.</p><pre>rustqec circuit detect</pre><p>Inspect the result.</p></main>')
        record = records[0]
        self.assertIn('16.775 ms', record['text'])
        self.assertIn('rustqec circuit detect', record['text'])
        self.assertEqual(record['excerpt'], 'Preserve loss flags for the decoder. Inspect the result.')
        self.assertEqual(record['path'], 'qp101/protocol/#atom-loss')

    def test_generated_fragment_ids_match_browser_collision_rules(self):
        records = self.index('<title>Reference</title><main><h2>Noise</h2><p>A</p><h3>Noise</h3><p>B</p><h2 id="noise">Existing</h2><p>C</p></main>')
        self.assertEqual([r['path'] for r in records], ['qp101/protocol/#noise-2', 'qp101/protocol/#noise-3', 'qp101/protocol/#noise'])

    def test_empty_document_does_not_create_false_search_results(self):
        self.assertEqual(self.index('<title>Empty</title><main><script>hidden</script></main>'), [])

    def test_transient_widget_headings_never_become_search_destinations(self):
        records = self.index('<title>Diagrams</title><main><h2 id="schema">Schema</h2><p>Read the format.</p><div data-toc-skip><div><h3>Loading schema</h3><img src="spinner.svg"><p>Temporary text.</p></div></div><h2 id="next">Next</h2><p>Continue reading.</p></main>')
        self.assertEqual([r['path'] for r in records], ['qp101/protocol/#schema', 'qp101/protocol/#next'])
        self.assertEqual(records[0]['text'], 'Read the format.')
        self.assertEqual(records[1]['text'], 'Continue reading.')

    def test_named_disclosures_have_separate_destinations_and_exclude_widget_navigation(self):
        records = self.index('<title>Diagrams</title><main><h2 id="gallery">Gallery</h2><p>Examples.</p><details><summary id="operations">Operation types</summary><p>Gates and noise.</p><aside>Loading navigation</aside></details></main>')
        self.assertEqual([r['path'] for r in records], ['qp101/protocol/#gallery', 'qp101/protocol/#operations'])
        self.assertEqual(records[0]['text'], 'Examples.')
        self.assertEqual(records[1]['text'], 'Gates and noise.')
