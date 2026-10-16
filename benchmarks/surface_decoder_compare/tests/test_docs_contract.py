import unittest
from pathlib import Path


class DocsContractTest(unittest.TestCase):
    def test_readme_and_makefile_document_both_tiers(self) -> None:
        readme = Path("benchmarks/surface_decoder_compare/README.md").read_text()
        makefile = Path("Makefile").read_text()

        self.assertIn("make surface-decoder-compare-smoke", readme)
        self.assertIn("make surface-decoder-compare-full", readme)
        self.assertIn("surface-decoder-compare-smoke:", makefile)
        self.assertIn("surface-decoder-compare-full:", makefile)

    def test_readme_and_makefile_document_rsinter_surface_benchmark_flow(self) -> None:
        readme = Path("benchmarks/surface_decoder_compare/README.md").read_text()
        makefile = Path("Makefile").read_text()

        self.assertIn("make bench-surface-smoke", readme)
        self.assertIn("make bench-surface-full", readme)
        self.assertIn("bench-surface-smoke:", makefile)
        self.assertIn("bench-surface-full:", makefile)


if __name__ == "__main__":
    unittest.main()
