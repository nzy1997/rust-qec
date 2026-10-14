#!/usr/bin/env python3
"""Fetch the generated benchmark artifacts pinned by this checkout."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tarfile
import tempfile
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path, PurePosixPath


REPO_ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_REPOSITORY = "nzy1997/rust-qec-benchmarks"
REVISION_PATH = Path(__file__).with_name("benchmark_artifacts_revision.txt")
HISTORY_MAP_PATH = Path(__file__).with_name("history_commit_map.txt")
HISTORY_PACK_PATH = "history/history-compat.pack"
MANAGED_PREFIX = "benchmarks/near_clifford/results/"
ZERO_SHA = "0" * 40


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _history_replacements(data: str) -> list[tuple[str, str]]:
    replacements: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number, line in enumerate(data.splitlines(), start=1):
        line = line.strip()
        if not line or line.split() == ["old", "new"]:
            continue
        parts = line.split()
        if len(parts) != 2:
            raise RuntimeError(f"malformed history map line {line_number}")
        old, new = parts
        if any(
            len(commit) != 40
            or any(character not in "0123456789abcdef" for character in commit)
            for commit in (old, new)
        ):
            raise RuntimeError(f"invalid commit ID on history map line {line_number}")
        if old in seen:
            raise RuntimeError(f"duplicate old commit ID in history map: {old}")
        seen.add(old)
        if old != new and new != ZERO_SHA:
            replacements.append((old, new))
    return replacements


def _install_history_replacements(
    repo: Path = REPO_ROOT, map_path: Path = HISTORY_MAP_PATH
) -> int:
    replacements = _history_replacements(map_path.read_text(encoding="utf-8"))
    available: list[tuple[str, str]] = []
    for old, new in replacements:
        present = subprocess.run(
            ["git", "-C", str(repo), "cat-file", "-e", f"{new}^{{commit}}"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if present.returncode == 0:
            available.append((old, new))
    if available:
        commands = "".join(
            f"update refs/replace/{old} {new}\n" for old, new in available
        )
        subprocess.run(
            ["git", "-C", str(repo), "update-ref", "--stdin"],
            input=commands,
            text=True,
            check=True,
        )
    return len(available)


def _artifact_revision() -> str:
    revision = REVISION_PATH.read_text(encoding="utf-8").strip()
    if len(revision) != 40 or any(character not in "0123456789abcdef" for character in revision):
        raise RuntimeError(f"invalid artifact revision in {REVISION_PATH}")
    return revision


def _history_pack_digest(data: str) -> str:
    matches = []
    for line in data.splitlines():
        parts = line.split(None, 1)
        if len(parts) == 2 and parts[1] == HISTORY_PACK_PATH:
            matches.append(parts[0])
    if len(matches) != 1:
        raise RuntimeError(f"artifact manifest must name {HISTORY_PACK_PATH} exactly once")
    digest = matches[0]
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise RuntimeError(f"invalid SHA-256 for {HISTORY_PACK_PATH}")
    return digest


def _install_history_objects(revision: str, repo: Path = REPO_ROOT) -> int:
    old_commits = [old for old, _ in _history_replacements(HISTORY_MAP_PATH.read_text(encoding="utf-8"))]
    missing = [
        commit
        for commit in old_commits
        if subprocess.run(
            ["git", "-C", str(repo), "cat-file", "-e", f"{commit}^{{commit}}"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        != 0
    ]
    if not missing:
        return 0
    base_url = f"https://raw.githubusercontent.com/{ARTIFACT_REPOSITORY}/{revision}"
    manifest = _read_url(f"{base_url}/ARTIFACTS.sha256").decode("utf-8")
    expected = _history_pack_digest(manifest)
    pack = _read_url(f"{base_url}/{HISTORY_PACK_PATH}")
    if hashlib.sha256(pack).hexdigest() != expected:
        raise RuntimeError("history compatibility pack checksum mismatch")
    subprocess.run(
        ["git", "-C", str(repo), "index-pack", "--stdin", "--fix-thin"],
        input=pack,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    still_missing = [
        commit
        for commit in missing
        if subprocess.run(
            ["git", "-C", str(repo), "cat-file", "-e", f"{commit}^{{commit}}"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        != 0
    ]
    if still_missing:
        raise RuntimeError(
            f"history compatibility pack is missing {len(still_missing)} mapped commits"
        )
    return len(missing)


def _safe_path(path: str) -> bool:
    pure = PurePosixPath(path)
    return not pure.is_absolute() and ".." not in pure.parts and path == pure.as_posix()


def _manifest(data: str) -> dict[str, str]:
    entries: dict[str, str] = {}
    for line_number, line in enumerate(data.splitlines(), start=1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            raise RuntimeError(f"malformed manifest line {line_number}")
        digest, path = parts
        if not path.startswith(MANAGED_PREFIX):
            continue
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            raise RuntimeError(f"invalid SHA-256 on manifest line {line_number}")
        if not _safe_path(path):
            raise RuntimeError(f"unsafe path on manifest line {line_number}: {path}")
        if path in entries:
            raise RuntimeError(f"duplicate artifact path: {path}")
        entries[path] = digest
    if not entries:
        raise RuntimeError("the artifact manifest has no managed files")
    return entries


def _normalize_selector(selector: str) -> str:
    normalized = selector.rstrip("/")
    if not normalized.startswith(MANAGED_PREFIX) or not _safe_path(normalized):
        raise RuntimeError(f"artifact selection is outside {MANAGED_PREFIX}: {selector}")
    return normalized


def _selected_entries(entries: dict[str, str], selectors: list[str]) -> dict[str, str]:
    if not selectors:
        return entries
    selected: dict[str, str] = {}
    for raw_selector in selectors:
        selector = _normalize_selector(raw_selector)
        matches = {
            path: digest
            for path, digest in entries.items()
            if path == selector or path.startswith(f"{selector}/")
        }
        if not matches:
            raise RuntimeError(f"artifact selection matched no files: {raw_selector}")
        selected.update(matches)
    return selected


def _source_inventory_dependencies(
    root: Path, selected: dict[str, str], entries: dict[str, str]
) -> dict[str, str]:
    dependencies: dict[str, str] = {}
    for relative in selected:
        if Path(relative).name != "header.json":
            continue
        header_path = root / relative
        if not header_path.is_file():
            continue
        payload = json.loads(header_path.read_text(encoding="utf-8"))
        contract = payload.get("production_contract")
        if not isinstance(contract, dict) or not isinstance(contract.get("sources"), dict):
            continue
        for source_path in contract["sources"]:
            if not source_path.startswith(MANAGED_PREFIX):
                continue
            digest = entries.get(source_path)
            if digest is None:
                raise RuntimeError(
                    f"externalized source inventory file is absent from the manifest: {source_path}"
                )
            dependencies[source_path] = digest
    return dependencies


def _read_url(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def _fetch_selected_files(
    revision: str, expected: dict[str, str], staging: Path
) -> None:
    def fetch_one(item: tuple[str, str]) -> None:
        relative, digest = item
        quoted = urllib.parse.quote(relative, safe="/")
        url = (
            f"https://raw.githubusercontent.com/{ARTIFACT_REPOSITORY}/"
            f"{revision}/{quoted}"
        )
        destination = staging / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(_read_url(url))
        if _sha256(destination) != digest:
            raise RuntimeError(f"artifact checksum mismatch: {relative}")

    with ThreadPoolExecutor(max_workers=8) as executor:
        list(executor.map(fetch_one, expected.items()))


def _archive_members(archive: tarfile.TarFile) -> dict[str, tarfile.TarInfo]:
    members: dict[str, tarfile.TarInfo] = {}
    root: str | None = None
    for member in archive.getmembers():
        parts = PurePosixPath(member.name).parts
        if not parts:
            continue
        if root is None:
            root = parts[0]
        if parts[0] != root or len(parts) < 2:
            continue
        relative = "/".join(parts[1:])
        if not _safe_path(relative):
            raise RuntimeError(f"unsafe archive path: {member.name}")
        if relative in members:
            raise RuntimeError(f"duplicate archive path: {relative}")
        members[relative] = member
    return members


def _fetch_archive(revision: str, expected: dict[str, str], staging: Path) -> None:
    url = f"https://codeload.github.com/{ARTIFACT_REPOSITORY}/tar.gz/{revision}"
    archive_path = staging / "artifacts.tar.gz"
    with urllib.request.urlopen(url, timeout=120) as response, archive_path.open("wb") as output:
        shutil.copyfileobj(response, output)
    with tarfile.open(archive_path, mode="r:gz") as archive:
        members = _archive_members(archive)
        for relative, digest in expected.items():
            member = members.get(relative)
            if member is None or not member.isfile():
                raise RuntimeError(f"artifact archive is missing a regular file: {relative}")
            source = archive.extractfile(member)
            if source is None:
                raise RuntimeError(f"cannot read artifact: {relative}")
            destination = staging / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("wb") as output:
                shutil.copyfileobj(source, output)
            if _sha256(destination) != digest:
                raise RuntimeError(f"artifact checksum mismatch: {relative}")
    archive_path.unlink()


def fetch_artifacts(*, selectors: list[str], check_only: bool = False) -> None:
    revision = _artifact_revision()
    object_count = _install_history_objects(revision)
    replacement_count = _install_history_replacements()
    manifest_url = (
        f"https://raw.githubusercontent.com/{ARTIFACT_REPOSITORY}/"
        f"{revision}/ARTIFACTS.sha256"
    )
    entries = _manifest(_read_url(manifest_url).decode("utf-8"))
    expected = _selected_entries(entries, selectors)

    if check_only:
        for relative, digest in expected.items():
            destination = REPO_ROOT / relative
            if not destination.is_file() or _sha256(destination) != digest:
                raise RuntimeError(f"missing or stale benchmark artifact: {relative}")
        expected.update(_source_inventory_dependencies(REPO_ROOT, expected, entries))
        for relative, digest in expected.items():
            destination = REPO_ROOT / relative
            if not destination.is_file() or _sha256(destination) != digest:
                raise RuntimeError(f"missing or stale benchmark artifact: {relative}")
    else:
        with tempfile.TemporaryDirectory(prefix="rust-qec-benchmarks-") as temporary:
            staging = Path(temporary)
            if selectors:
                _fetch_selected_files(revision, expected, staging)
                dependencies = _source_inventory_dependencies(staging, expected, entries)
                missing_dependencies = {
                    path: digest
                    for path, digest in dependencies.items()
                    if path not in expected
                }
                if missing_dependencies:
                    _fetch_selected_files(revision, missing_dependencies, staging)
                expected.update(dependencies)
            else:
                _fetch_archive(revision, expected, staging)
            for relative in expected:
                source = staging / relative
                destination = REPO_ROOT / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)

    action = "Verified" if check_only else "Fetched"
    print(
        f"{action} {len(expected)} benchmark artifacts from "
        f"{ARTIFACT_REPOSITORY}@{revision}"
    )
    print(f"Installed {replacement_count} rewritten-history compatibility refs")
    print(f"Installed {object_count} missing pre-cleanup commit objects")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--path",
        action="append",
        default=[],
        help="fetch one managed file or directory; repeat for multiple selections",
    )
    parser.add_argument("--check", action="store_true", help="only verify already-fetched files")
    parser.add_argument(
        "--history-only",
        action="store_true",
        help="install rewritten-history compatibility refs without fetching artifacts",
    )
    args = parser.parse_args()
    if args.history_only:
        if args.path or args.check:
            parser.error("--history-only cannot be combined with --path or --check")
        revision = _artifact_revision()
        object_count = _install_history_objects(revision)
        count = _install_history_replacements()
        print(f"Installed {count} rewritten-history compatibility refs")
        print(f"Installed {object_count} missing pre-cleanup commit objects")
        return 0
    fetch_artifacts(selectors=args.path, check_only=args.check)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
