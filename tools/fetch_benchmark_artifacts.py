#!/usr/bin/env python3
"""Fetch the generated benchmark artifacts pinned by this checkout."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tarfile
import tempfile
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path, PurePosixPath


REPO_ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_REPOSITORY = "nzy1997/rust-qec-benchmarks"
REVISION_PATH = Path(__file__).with_name("benchmark_artifacts_revision.txt")
MANAGED_PREFIX = "benchmarks/near_clifford/results/"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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
    revision = REVISION_PATH.read_text(encoding="utf-8").strip()
    if len(revision) != 40 or any(character not in "0123456789abcdef" for character in revision):
        raise RuntimeError(f"invalid artifact revision in {REVISION_PATH}")
    manifest_url = (
        f"https://raw.githubusercontent.com/{ARTIFACT_REPOSITORY}/"
        f"{revision}/ARTIFACTS.sha256"
    )
    expected = _selected_entries(_manifest(_read_url(manifest_url).decode("utf-8")), selectors)

    if check_only:
        for relative, digest in expected.items():
            destination = REPO_ROOT / relative
            if not destination.is_file() or _sha256(destination) != digest:
                raise RuntimeError(f"missing or stale benchmark artifact: {relative}")
    else:
        with tempfile.TemporaryDirectory(prefix="rust-qec-benchmarks-") as temporary:
            staging = Path(temporary)
            if selectors:
                _fetch_selected_files(revision, expected, staging)
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--path",
        action="append",
        default=[],
        help="fetch one managed file or directory; repeat for multiple selections",
    )
    parser.add_argument("--check", action="store_true", help="only verify already-fetched files")
    args = parser.parse_args()
    fetch_artifacts(selectors=args.path, check_only=args.check)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
