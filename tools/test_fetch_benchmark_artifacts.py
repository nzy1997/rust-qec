from __future__ import annotations

import io
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

from tools import fetch_benchmark_artifacts as fetcher


class FetchBenchmarkArtifactsTest(unittest.TestCase):
    def test_history_pack_digest_requires_one_valid_manifest_entry(self) -> None:
        digest = "a" * 64
        self.assertEqual(
            fetcher._history_pack_digest(
                f"{'b' * 64}  benchmarks/near_clifford/results/run.json\n"
                f"{digest}  {fetcher.HISTORY_PACK_PATH}\n"
            ),
            digest,
        )
        with self.assertRaisesRegex(RuntimeError, "exactly once"):
            fetcher._history_pack_digest("")
        with self.assertRaisesRegex(RuntimeError, "exactly once"):
            fetcher._history_pack_digest(
                f"{digest}  {fetcher.HISTORY_PACK_PATH}\n"
                f"{digest}  {fetcher.HISTORY_PACK_PATH}\n"
            )

    def test_history_map_keeps_only_changed_nonempty_commits(self) -> None:
        old = "1" * 40
        new = "2" * 40
        unchanged = "3" * 40
        removed = "4" * 40
        self.assertEqual(
            fetcher._history_replacements(
                "old                                      new\n"
                f"{old} {new}\n"
                f"{unchanged} {unchanged}\n"
                f"{removed} {fetcher.ZERO_SHA}\n"
            ),
            [(old, new)],
        )

    def test_history_replacement_resolves_a_missing_old_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary) / "repo"
            repo.mkdir()
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True)
            subprocess.run(
                ["git", "-C", str(repo), "config", "user.email", "test@example.com"],
                check=True,
            )
            (repo / "source.txt").write_text("source\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "source.txt"], check=True)
            subprocess.run(["git", "-C", str(repo), "commit", "-qm", "source"], check=True)
            new = subprocess.check_output(
                ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
            ).strip()
            old = "1" * 40
            map_path = Path(temporary) / "commit-map"
            map_path.write_text(f"old new\n{old} {new}\n", encoding="utf-8")

            self.assertEqual(fetcher._install_history_replacements(repo, map_path), 1)
            content = subprocess.check_output(
                ["git", "-C", str(repo), "show", f"{old}:source.txt"], text=True
            )
            self.assertEqual(content, "source\n")

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

    def test_selected_header_adds_externalized_source_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            header = root / "benchmarks/near_clifford/results/run/header.json"
            header.parent.mkdir(parents=True)
            source = "benchmarks/near_clifford/results/probe/main.rs"
            header.write_text(
                '{"production_contract":{"sources":{"' + source + '":"digest"}}}',
                encoding="utf-8",
            )
            selected = {
                "benchmarks/near_clifford/results/run/header.json": "header-digest"
            }
            entries = {**selected, source: "source-digest"}

            self.assertEqual(
                fetcher._source_inventory_dependencies(root, selected, entries),
                {source: "source-digest"},
            )

    def test_externalized_source_inventory_must_be_manifested(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            header = root / "benchmarks/near_clifford/results/run/header.json"
            header.parent.mkdir(parents=True)
            source = "benchmarks/near_clifford/results/probe/main.rs"
            header.write_text(
                '{"production_contract":{"sources":{"' + source + '":"digest"}}}',
                encoding="utf-8",
            )
            selected = {
                "benchmarks/near_clifford/results/run/header.json": "header-digest"
            }

            with self.assertRaisesRegex(RuntimeError, "absent from the manifest"):
                fetcher._source_inventory_dependencies(root, selected, selected)

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
