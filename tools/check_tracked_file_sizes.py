#!/usr/bin/env python3
"""Reject oversized or misplaced files anywhere in the tracked tree."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MAX_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_TOTAL_BYTES = 5 * 1024 * 1024
FORBIDDEN_PREFIXES = ("benchmarks/near_clifford/results/",)
FORBIDDEN_PATHS = (
    "benchmarks/rstim_vs_stim_simulator/fixtures/"
    "stim_surface_code_rotated_memory_z_d11_r100.dem",
)
ZERO_SHA = "0" * 40


def git(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def changed_paths(repo: Path, base: str | None, head: str) -> list[str]:
    if base and base != ZERO_SHA:
        output = git(
            repo,
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            "--no-renames",
            "-z",
            base,
            head,
            "--",
        )
    else:
        output = git(repo, "ls-tree", "-r", "--name-only", "-z", head)
    return [os.fsdecode(path) for path in output.split(b"\0") if path]


def tracked_file_sizes(repo: Path, head: str) -> list[tuple[str, int]]:
    output = git(repo, "ls-tree", "-lr", "-z", head)
    files: list[tuple[str, int]] = []
    for entry in output.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        _, object_type, _, raw_size = metadata.split()
        if object_type != b"blob":
            continue
        files.append((os.fsdecode(raw_path), int(raw_size)))
    return files


def blob_size(repo: Path, head: str, path: str) -> int:
    return int(git(repo, "cat-file", "-s", f"{head}:{path}").strip())


def oversized_files(repo: Path, head: str, max_bytes: int) -> list[tuple[str, int]]:
    return [
        (path, size)
        for path, size in tracked_file_sizes(repo, head)
        if size > max_bytes
    ]


def changed_file_sizes(repo: Path, base: str | None, head: str) -> list[tuple[str, int]]:
    return [(path, blob_size(repo, head, path)) for path in changed_paths(repo, base, head)]


def forbidden_files(
    paths: list[str],
    prefixes: tuple[str, ...] = FORBIDDEN_PREFIXES,
    exact_paths: tuple[str, ...] = FORBIDDEN_PATHS,
) -> list[str]:
    return [
        path
        for path in paths
        if path in exact_paths or any(path.startswith(prefix) for prefix in prefixes)
    ]


def format_size(size: int) -> str:
    return f"{size / (1024 * 1024):.2f} MiB"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=REPO_ROOT)
    parser.add_argument("--base", help="base commit; omit to inspect the complete head tree")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    parser.add_argument(
        "--max-total-bytes", type=int, default=DEFAULT_MAX_TOTAL_BYTES
    )
    args = parser.parse_args(argv)
    if args.max_bytes <= 0:
        parser.error("--max-bytes must be positive")
    if args.max_total_bytes <= 0:
        parser.error("--max-total-bytes must be positive")

    repo = args.repo.resolve()
    tracked_files = tracked_file_sizes(repo, args.head)
    tracked_paths = [path for path, _ in tracked_files]
    changed_files = changed_file_sizes(repo, args.base, args.head)
    oversized = [
        (path, size) for path, size in tracked_files if size > args.max_bytes
    ]
    forbidden = forbidden_files(tracked_paths)
    total = sum(size for _, size in changed_files)
    if oversized or forbidden or total > args.max_total_bytes:
        print("ERROR tracked file policy check failed", file=sys.stderr)
        if oversized:
            print(
                f"Files above the {format_size(args.max_bytes)} per-file limit:",
                file=sys.stderr,
            )
        for path, size in oversized:
            print(f"- {path}: {format_size(size)}", file=sys.stderr)
        if total > args.max_total_bytes:
            print(
                f"Changed files total {format_size(total)}; "
                f"limit is {format_size(args.max_total_bytes)}",
                file=sys.stderr,
            )
        if forbidden:
            print("Generated benchmark results must stay out of this repository:", file=sys.stderr)
            for path in forbidden:
                print(f"- {path}", file=sys.stderr)
        return 1
    print(
        "PASS tracked file policy check: "
        f"{len(tracked_files)} tracked files scanned; "
        f"{len(changed_files)} changed files, {format_size(total)} changed total; "
        f"limits {format_size(args.max_bytes)} per file / "
        f"{format_size(args.max_total_bytes)} total"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
