from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import check_tracked_file_sizes as checker


class TrackedFileSizeCheckerTest(unittest.TestCase):
    def make_repo(self) -> tuple[tempfile.TemporaryDirectory[str], Path, str, str]:
        temporary = tempfile.TemporaryDirectory()
        repo = Path(temporary.name)
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.email", "test@example.com"], check=True
        )
        (repo / "small.txt").write_text("small\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(repo), "add", "small.txt"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "initial"], check=True)
        base = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        return temporary, repo, base, ""

    def test_new_oversized_file_is_rejected(self) -> None:
        temporary, repo, base, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        (repo / "large.bin").write_bytes(b"x" * 1025)
        subprocess.run(["git", "-C", str(repo), "add", "large.bin"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "large"], check=True)
        head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()

        self.assertEqual(checker.oversized_files(repo, base, head, 1024), [("large.bin", 1025)])

    def test_existing_oversized_file_is_not_rechecked_without_a_change(self) -> None:
        temporary, repo, base, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        (repo / "large.bin").write_bytes(b"x" * 1025)
        subprocess.run(["git", "-C", str(repo), "add", "large.bin"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "large"], check=True)
        large = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        (repo / "small.txt").write_text("changed\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(repo), "add", "small.txt"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "small change"], check=True)
        head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()

        self.assertEqual(checker.changed_paths(repo, large, head), ["small.txt"])
        self.assertEqual(checker.oversized_files(repo, large, head, 1024), [])

    def test_exact_limit_is_allowed(self) -> None:
        temporary, repo, base, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        (repo / "exact.bin").write_bytes(b"x" * 1024)
        subprocess.run(["git", "-C", str(repo), "add", "exact.bin"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "exact"], check=True)
        head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()

        self.assertEqual(checker.oversized_files(repo, base, head, 1024), [])

    def test_aggregate_limit_rejects_multiple_small_files(self) -> None:
        temporary, repo, base, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        for index in range(3):
            (repo / f"part-{index}.bin").write_bytes(b"x" * 512)
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "parts"], check=True)
        head = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()

        files = checker.changed_file_sizes(repo, base, head)
        self.assertEqual(sum(size for _, size in files), 1536)
        self.assertTrue(all(size <= 1024 for _, size in files))

    def test_forbidden_generated_result_is_rejected_even_when_small(self) -> None:
        temporary, repo, base, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        result = repo / "benchmarks/near_clifford/results/tiny.json"
        result.parent.mkdir(parents=True)
        result.write_text("{}\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "result"], check=True)
        head = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()

        paths = checker.changed_paths(repo, base, head)
        self.assertEqual(
            checker.forbidden_files(paths),
            ["benchmarks/near_clifford/results/tiny.json"],
        )

    def test_deleting_forbidden_generated_result_is_allowed(self) -> None:
        temporary, repo, _, _ = self.make_repo()
        self.addCleanup(temporary.cleanup)
        result = repo / "benchmarks/near_clifford/results/old.json"
        result.parent.mkdir(parents=True)
        result.write_text("{}\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "result"], check=True)
        base = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()
        result.unlink()
        subprocess.run(["git", "-C", str(repo), "add", "-u"], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "remove result"], check=True)
        head = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()

        self.assertEqual(checker.changed_paths(repo, base, head), [])


if __name__ == "__main__":
    unittest.main()
