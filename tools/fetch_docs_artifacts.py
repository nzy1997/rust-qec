#!/usr/bin/env python3
"""Fetch the generated documentation artifacts pinned by this checkout."""

from __future__ import annotations

import argparse
import hashlib
import io
import shutil
import tarfile
import tempfile
import urllib.request
from pathlib import Path, PurePosixPath


REPO_ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_REPOSITORY = "nzy1997/rust-qec-docs"
REVISION_PATH = Path(__file__).with_name("docs_artifacts_revision.txt")
MANAGED_PREFIXES = (
    "docs/test-reports/",
    "site/static/data/atom-loss/",
    "site/static/rsmp-v1-showcase/og.png",
)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _is_managed(path: str) -> bool:
    return any(path.startswith(prefix) for prefix in MANAGED_PREFIXES)


def _manifest(data: str) -> dict[str, str]:
    entries: dict[str, str] = {}
    for line in data.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        digest, path = line.split(None, 1)
        if not _is_managed(path):
            continue
        entries[path] = digest
    if not entries:
        raise RuntimeError("the artifact archive has no managed files")
    return entries


def _archive_members(archive: tarfile.TarFile) -> dict[str, tarfile.TarInfo]:
    members: dict[str, tarfile.TarInfo] = {}
    for member in archive.getmembers():
        parts = PurePosixPath(member.name).parts
        if len(parts) < 2:
            continue
        relative = "/".join(parts[1:])
        members[relative] = member
    return members


def fetch_artifacts(*, check_only: bool = False) -> None:
    revision = REVISION_PATH.read_text(encoding="utf-8").strip()
    if not revision:
        raise RuntimeError(f"empty artifact revision in {REVISION_PATH}")
    url = f"https://codeload.github.com/{ARTIFACT_REPOSITORY}/tar.gz/{revision}"

    with urllib.request.urlopen(url, timeout=60) as response:
        archive_bytes = response.read()

    with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as archive:
        members = _archive_members(archive)
        manifest_member = members.get("ARTIFACTS.sha256")
        if manifest_member is None:
            raise RuntimeError("artifact archive is missing ARTIFACTS.sha256")
        manifest_file = archive.extractfile(manifest_member)
        if manifest_file is None:
            raise RuntimeError("cannot read ARTIFACTS.sha256")
        expected = _manifest(manifest_file.read().decode("utf-8"))

        missing = [path for path in expected if path not in members]
        if missing:
            raise RuntimeError(f"artifact archive is missing files: {missing}")

        if check_only:
            for relative, digest in expected.items():
                destination = REPO_ROOT / relative
                if not destination.is_file() or _sha256(destination) != digest:
                    raise RuntimeError(f"missing or stale documentation artifact: {relative}")
            return

        with tempfile.TemporaryDirectory(prefix="rust-qec-docs-") as temporary:
            staging = Path(temporary)
            for relative, digest in expected.items():
                member_file = archive.extractfile(members[relative])
                if member_file is None:
                    raise RuntimeError(f"cannot read artifact: {relative}")
                staged = staging / relative
                staged.parent.mkdir(parents=True, exist_ok=True)
                with staged.open("wb") as output:
                    shutil.copyfileobj(member_file, output)
                if _sha256(staged) != digest:
                    raise RuntimeError(f"artifact checksum mismatch: {relative}")

            for relative in expected:
                source = staging / relative
                destination = REPO_ROOT / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)

    print(f"Fetched {len(expected)} documentation artifacts from {ARTIFACT_REPOSITORY}@{revision}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="only verify already-fetched files")
    args = parser.parse_args()
    fetch_artifacts(check_only=args.check)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
