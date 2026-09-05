#!/usr/bin/env python3
"""Package native RustQEC binaries and assemble their release metadata."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import re
import subprocess
import tarfile
from pathlib import Path


TARGETS = {
    "x86_64-unknown-linux-gnu": "Ubuntu 24.04 x86_64 (glibc and standard system libraries)",
    "aarch64-apple-darwin": "macOS 15 on Apple silicon (system libraries supplied by macOS)",
}
ARCHIVE_SCHEMA = "rustqec.release-archive-fragment.v1"
MANIFEST_SCHEMA = "rustqec.release-manifest.v1"
SHOT_ASSET_PATHS = {
    "site/static/interactive/app.js",
    "site/static/interactive/pkg/rstim_shot_web.d.ts",
    "site/static/interactive/pkg/rstim_shot_web.js",
    "site/static/interactive/pkg/rstim_shot_web_bg.wasm",
    "site/static/interactive/pkg/rstim_shot_web_bg.wasm.d.ts",
    "site/static/interactive/shot-viewer.css",
    "rstim/assets/shot-viewer/app.js",
    "rstim/assets/shot-viewer/asset-manifest.json",
    "rstim/assets/shot-viewer/pkg/rstim_shot_web_bg.wasm",
    "rstim/assets/shot-viewer/shot-viewer.css",
}


class BuildError(RuntimeError):
    pass


def command(*argv: str, cwd: Path | None = None) -> str:
    return subprocess.run(argv, cwd=cwd, check=True, text=True, capture_output=True).stdout.strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_name(value: str, label: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", value):
        raise BuildError(f"invalid {label}: {value!r}")
    return value


def rustc_metadata(rustc: Path, target: str) -> dict[str, str]:
    values = {}
    for line in command(str(rustc), "--version", "--verbose").splitlines():
        if ": " in line:
            key, value = line.split(": ", 1)
            values[key] = value
    if values.get("host") != target:
        raise BuildError(f"rustc host {values.get('host')!r} does not match target {target!r}")
    return {
        "release": values.get("release", ""),
        "host": values.get("host", ""),
        "commit_hash": values.get("commit-hash", ""),
        "path": str(rustc.resolve()),
    }


def package_versions(cargo: Path, repo_root: Path) -> list[dict[str, str | None]]:
    metadata = json.loads(
        command(
            str(cargo), "metadata", "--locked", "--no-deps", "--format-version", "1",
            "--manifest-path", str(repo_root / "Cargo.toml"),
        )
    )
    members = set(metadata["workspace_members"])
    return sorted(
        (
            {
                "name": package["name"],
                "version": package["version"],
                "rust_version": package.get("rust_version"),
            }
            for package in metadata["packages"]
            if package["id"] in members
        ),
        key=lambda package: package["name"],
    )


def verify_source(repo_root: Path, tag: str, source_sha: str) -> None:
    head = command("git", "rev-parse", "HEAD", cwd=repo_root)
    tagged = command("git", "rev-parse", f"refs/tags/{tag}^{{commit}}", cwd=repo_root)
    if head != source_sha or tagged != source_sha:
        raise BuildError(f"tag/checkout mismatch: tag={tagged} HEAD={head} expected={source_sha}")
    changed = set(filter(None, command("git", "diff", "--name-only", "HEAD", cwd=repo_root).splitlines()))
    unexpected = sorted(changed - SHOT_ASSET_PATHS)
    if unexpected:
        raise BuildError(f"tagged source has unexpected tracked changes: {', '.join(unexpected)}")


def shot_assets(repo_root: Path) -> dict[str, object]:
    manifest_path = repo_root / "rstim/assets/shot-viewer/asset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    files = manifest.get("files", {})
    for relative, identity in files.items():
        path = repo_root / "rstim/assets/shot-viewer" / relative
        if not path.is_file() or sha256(path) != identity.get("sha256"):
            raise BuildError(f"rebuilt Shot Lab asset does not match manifest: {relative}")
    return {
        "rebuilt_from_tag": True,
        "manifest_sha256": sha256(manifest_path),
        "manifest": manifest,
    }


def add_bytes(archive: tarfile.TarFile, name: str, payload: bytes, mode: int) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(payload)
    info.mode = mode
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    archive.addfile(info, io.BytesIO(payload))


def write_archive(path: Path, root: str, files: list[tuple[str, bytes, int]]) -> None:
    with path.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for relative, payload, mode in files:
                    add_bytes(archive, f"{root}/{relative}", payload, mode)


def package(args: argparse.Namespace) -> None:
    repo_root = args.repo_root.resolve()
    target = safe_name(args.target, "target")
    tag = safe_name(args.tag, "tag")
    if target not in TARGETS:
        raise BuildError(f"unsupported native release target: {target}")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_sha):
        raise BuildError("source SHA must be a full lowercase Git commit ID")
    verify_source(repo_root, tag, args.source_sha)

    for binary in (args.rustqec, args.rstim):
        if not binary.is_file() or binary.is_symlink():
            raise BuildError(f"missing regular release binary: {binary}")
    if not args.runtime_linkage.is_file():
        raise BuildError(f"missing runtime linkage report: {args.runtime_linkage}")

    archive_root = f"rustqec-{tag}-{target}"
    archive_name = f"{archive_root}.tar.gz"
    runtime = (
        f"RustQEC native command-line archive\n\n"
        f"Tag: {tag}\nSource commit: {args.source_sha}\nTarget: {target}\n"
        f"Runtime baseline: {TARGETS[target]}\n\n"
        "Binaries:\n  bin/rustqec\n  bin/rstim\n\n"
        "Verify this archive with release-manifest.json, SHA256SUMS, and "
        "verify_release_archive.py before running it.\n\n"
        "Recorded dynamic linkage from the build host:\n"
        + args.runtime_linkage.read_text()
    ).encode()
    files = [
        ("bin/rustqec", args.rustqec.read_bytes(), 0o755),
        ("bin/rstim", args.rstim.read_bytes(), 0o755),
        ("LICENSE", (repo_root / "LICENSE").read_bytes(), 0o644),
        ("RUNTIME.md", runtime, 0o644),
    ]
    args.out_dir.mkdir(parents=True, exist_ok=True)
    archive_path = args.out_dir / archive_name
    write_archive(archive_path, archive_root, files)

    fragment = {
        "schema_version": ARCHIVE_SCHEMA,
        "tag": tag,
        "source_sha": args.source_sha,
        "target": target,
        "archive": {
            "filename": archive_name,
            "root_directory": archive_root,
            "sha256": sha256(archive_path),
            "size": archive_path.stat().st_size,
        },
        "packages": package_versions(args.cargo.resolve(), repo_root),
        "compiler": rustc_metadata(args.rustc.resolve(), target),
        "runtime": {
            "baseline": TARGETS[target],
            "linkage": args.runtime_linkage.read_text().splitlines(),
        },
        "shot_lab_assets": shot_assets(repo_root),
    }
    fragment_path = args.out_dir / f"release-fragment-{target}.json"
    fragment_path.write_text(json.dumps(fragment, indent=2, sort_keys=True) + "\n")
    print(f"built {archive_name} sha256={fragment['archive']['sha256']}")


def assemble(args: argparse.Namespace) -> None:
    fragments = [json.loads(path.read_text()) for path in sorted(args.fragments.glob("release-fragment-*.json"))]
    if {fragment.get("target") for fragment in fragments} != set(TARGETS):
        raise BuildError("release bundle must contain exactly one fragment for each supported target")
    tag = safe_name(args.tag, "tag")
    source_sha = args.source_sha
    first = fragments[0]
    for fragment in fragments:
        if fragment.get("schema_version") != ARCHIVE_SCHEMA:
            raise BuildError("invalid release archive fragment schema")
        if fragment.get("tag") != tag or fragment.get("source_sha") != source_sha:
            raise BuildError("release archive fragments disagree on tag or source SHA")
        if fragment.get("packages") != first.get("packages"):
            raise BuildError("release archive fragments disagree on workspace package versions")
        if fragment.get("shot_lab_assets") != first.get("shot_lab_assets"):
            raise BuildError("release archive fragments disagree on rebuilt Shot Lab assets")

    args.out_dir.mkdir(parents=True, exist_ok=True)
    archives = {}
    checksum_lines = []
    for fragment in sorted(fragments, key=lambda item: item["target"]):
        identity = fragment["archive"]
        archive_path = args.fragments / identity["filename"]
        if not archive_path.is_file() or sha256(archive_path) != identity["sha256"]:
            raise BuildError(f"archive does not match fragment: {identity['filename']}")
        destination = args.out_dir / archive_path.name
        destination.write_bytes(archive_path.read_bytes())
        archives[archive_path.name] = {
            **identity,
            "target": fragment["target"],
            "compiler": fragment["compiler"],
            "runtime": fragment["runtime"],
        }
        checksum_lines.append(f"{identity['sha256']}  {identity['filename']}")

    manifest = {
        "schema_version": MANIFEST_SCHEMA,
        "tag": tag,
        "source_sha": source_sha,
        "packages": first["packages"],
        "shot_lab_assets": first["shot_lab_assets"],
        "archives": archives,
    }
    (args.out_dir / "release-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    )
    (args.out_dir / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n")
    print(f"assembled release bundle targets={len(archives)} tag={tag}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    package_parser = subparsers.add_parser("package")
    package_parser.add_argument("--repo-root", type=Path, required=True)
    package_parser.add_argument("--out-dir", type=Path, required=True)
    package_parser.add_argument("--tag", required=True)
    package_parser.add_argument("--source-sha", required=True)
    package_parser.add_argument("--target", required=True)
    package_parser.add_argument("--cargo", type=Path, required=True)
    package_parser.add_argument("--rustc", type=Path, required=True)
    package_parser.add_argument("--rustqec", type=Path, required=True)
    package_parser.add_argument("--rstim", type=Path, required=True)
    package_parser.add_argument("--runtime-linkage", type=Path, required=True)
    package_parser.set_defaults(handler=package)
    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--fragments", type=Path, required=True)
    assemble_parser.add_argument("--out-dir", type=Path, required=True)
    assemble_parser.add_argument("--tag", required=True)
    assemble_parser.add_argument("--source-sha", required=True)
    assemble_parser.set_defaults(handler=assemble)
    args = parser.parse_args()
    try:
        args.handler(args)
    except (BuildError, OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
