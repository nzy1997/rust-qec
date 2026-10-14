from __future__ import annotations

import io
import tarfile
import unittest

from tools import fetch_benchmark_artifacts as fetcher


class FetchBenchmarkArtifactsTest(unittest.TestCase):
    def test_manifest_keeps_only_managed_safe_files(self) -> None:
        digest = "a" * 64
        entries = fetcher._manifest(
            f"{digest}  benchmarks/near_clifford/results/run/results.json\n"
            f"{digest}  README.md\n"
        )
        self.assertEqual(
            entries,
            {"benchmarks/near_clifford/results/run/results.json": digest},
        )

    def test_manifest_rejects_path_traversal(self) -> None:
        digest = "a" * 64
        with self.assertRaisesRegex(RuntimeError, "unsafe path"):
            fetcher._manifest(
                f"{digest}  benchmarks/near_clifford/results/../../outside\n"
            )

    def test_selectors_accept_a_file_or_directory(self) -> None:
        entries = {
            "benchmarks/near_clifford/results/a/one.json": "1",
            "benchmarks/near_clifford/results/a/two.json": "2",
            "benchmarks/near_clifford/results/b.json": "3",
        }
        self.assertEqual(
            fetcher._selected_entries(
                entries, ["benchmarks/near_clifford/results/a"]
            ),
            dict(list(entries.items())[:2]),
        )
        self.assertEqual(
            fetcher._selected_entries(
                entries, ["benchmarks/near_clifford/results/b.json"]
            ),
            {"benchmarks/near_clifford/results/b.json": "3"},
        )

    def test_selector_cannot_escape_managed_prefix(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "outside"):
            fetcher._selected_entries({}, ["README.md"])

    def test_archive_rejects_duplicate_relative_paths(self) -> None:
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w") as archive:
            first = tarfile.TarInfo("root/path.txt")
            first.size = 1
            archive.addfile(first, io.BytesIO(b"a"))
            second = tarfile.TarInfo("root/path.txt")
            second.size = 1
            archive.addfile(second, io.BytesIO(b"b"))
        buffer.seek(0)
        with tarfile.open(fileobj=buffer, mode="r") as archive:
            with self.assertRaisesRegex(RuntimeError, "duplicate archive path"):
                fetcher._archive_members(archive)


if __name__ == "__main__":
    unittest.main()
