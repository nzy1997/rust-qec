#!/usr/bin/env python3
"""Verify and execute one downloaded RustQEC native release archive."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath


SCHEMA = "rustqec.release-manifest.v1"
FIXTURE = "M 0\nDETECTOR rec[-1]\n"
RUSTQEC_OUTPUT = {
    "schema_version": "rustqec.cli.v1",
    "status": "ok",
    "command": "circuit.stats",
    "result": {
        "instruction_count": 2,
        "repeat_blocks": 0,
        "max_repeat_depth": 0,
        "num_qubits": 1,
        "num_measurements": 1,
        "num_detectors": 1,
        "num_observables": 0,
        "num_ticks": 0,
        "num_sweep_bits": 0,
    },
    "warnings": [],
    "artifacts": [],
}
RSTIM_OUTPUT = {
    "instruction_count": 2,
    "repeat_blocks": 0,
    "max_repeat_depth": 0,
    "num_qubits": 1,
    "num_measurements": 1,
    "num_detectors": 1,
    "num_observables": 0,
    "num_ticks": 0,
    "num_sweep_bits": 0,
}
MAX_EXTRACTED_BYTES = 512 * 1024 * 1024


class VerificationError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def current_target() -> str:
    system = platform.system()
    machine = platform.machine()
    if system == "Linux" and machine == "x86_64":
        return "x86_64-unknown-linux-gnu"
    if system == "Darwin" and machine == "arm64":
        return "aarch64-apple-darwin"
    raise VerificationError(f"unsupported verification host: {system} {machine}")


def parse_checksums(path: Path) -> dict[str, str]:
    checksums = {}
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._-]*)", line)
        if not match:
            raise VerificationError(f"invalid SHA256SUMS line {line_number}")
        digest, filename = match.groups()
        if filename in checksums:
            raise VerificationError(f"duplicate SHA256SUMS entry: {filename}")
        checksums[filename] = digest
    return checksums


def validate_manifest(manifest: dict[str, object], expected_tag: str) -> None:
    if not isinstance(manifest, dict):
        raise VerificationError("release manifest root must be an object")
    if manifest.get("schema_version") != SCHEMA:
        raise VerificationError("invalid release manifest schema")
    if manifest.get("tag") != expected_tag:
        raise VerificationError(
            f"release tag mismatch: manifest={manifest.get('tag')!r} expected={expected_tag!r}"
        )
    if not re.fullmatch(r"[0-9a-f]{40}", str(manifest.get("source_sha", ""))):
        raise VerificationError("release manifest source SHA is invalid")
    packages = manifest.get("packages")
    if not isinstance(packages, list) or not packages:
        raise VerificationError("release manifest package versions are missing")
    package_names = [package.get("name") for package in packages if isinstance(package, dict)]
    if len(package_names) != len(packages) or len(package_names) != len(set(package_names)):
        raise VerificationError("release manifest package identities are invalid")
    if not {"rustqec-cli", "rstim"}.issubset(package_names):
        raise VerificationError("release manifest omits a packaged CLI version")
    for package in packages:
        if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?", str(package.get("version", ""))):
            raise VerificationError(f"invalid package version for {package.get('name')}")

    shot_lab = manifest.get("shot_lab_assets")
    if not isinstance(shot_lab, dict) or shot_lab.get("rebuilt_from_tag") is not True:
        raise VerificationError("release manifest does not prove Shot Lab asset regeneration")
    if not re.fullmatch(r"[0-9a-f]{64}", str(shot_lab.get("manifest_sha256", ""))):
        raise VerificationError("release manifest Shot Lab asset digest is invalid")
    shot_manifest = shot_lab.get("manifest", {})
    if not isinstance(shot_manifest, dict) or shot_manifest.get("format_version") != "rstim-shot-assets-v1":
        raise VerificationError("release manifest Shot Lab asset metadata is invalid")

    archives = manifest.get("archives")
    expected_names = {f"rustqec-{expected_tag}-{target}.tar.gz" for target in (
        "x86_64-unknown-linux-gnu", "aarch64-apple-darwin"
    )}
    if not isinstance(archives, dict) or set(archives) != expected_names:
        raise VerificationError("release manifest does not contain exactly the two supported archives")
    for filename, identity in archives.items():
        if not isinstance(identity, dict):
            raise VerificationError(f"invalid archive identity: {filename}")
        target = identity.get("target")
        if filename != f"rustqec-{expected_tag}-{target}.tar.gz":
            raise VerificationError(f"release archive filename/target mismatch: {filename}")
        if identity.get("filename") != filename:
            raise VerificationError(f"release archive identity filename mismatch: {filename}")
        if not re.fullmatch(r"[0-9a-f]{64}", str(identity.get("sha256", ""))):
            raise VerificationError(f"invalid archive digest: {filename}")
        if not isinstance(identity.get("size"), int) or identity["size"] <= 0:
            raise VerificationError(f"invalid archive size: {filename}")
        compiler = identity.get("compiler", {})
        if (not isinstance(compiler, dict) or compiler.get("host") != target
                or not compiler.get("release") or not compiler.get("commit_hash")):
            raise VerificationError(f"incomplete compiler identity: {filename}")
        runtime = identity.get("runtime", {})
        if not isinstance(runtime, dict) or not runtime.get("baseline") or not runtime.get("linkage"):
            raise VerificationError(f"incomplete runtime identity: {filename}")


def validate_member(member: tarfile.TarInfo, root: str) -> None:
    path = PurePosixPath(member.name)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != root:
        raise VerificationError(f"unsafe archive member path: {member.name}")
    if member.issym() or member.islnk() or member.isdev() or member.isfifo():
        raise VerificationError(f"unsupported archive member type: {member.name}")
    if not member.isfile() and not member.isdir():
        raise VerificationError(f"unsupported archive member type: {member.name}")


def safe_extract(archive_path: Path, destination: Path, root: str) -> Path:
    expected = {
        f"{root}/bin/rustqec",
        f"{root}/bin/rstim",
        f"{root}/LICENSE",
        f"{root}/RUNTIME.md",
    }
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        for member in members:
            validate_member(member, root)
        names = [member.name.rstrip("/") for member in members if member.isfile()]
        if len(names) != len(set(names)):
            raise VerificationError("archive contains duplicate file members")
        if set(names) != expected:
            raise VerificationError(f"archive members do not match contract: {sorted(names)}")
        total_size = 0
        for member in members:
            total_size += member.size
            if total_size > MAX_EXTRACTED_BYTES:
                raise VerificationError("archive expands beyond the release size limit")
        for member in members:
            relative = PurePosixPath(member.name)
            output = destination.joinpath(*relative.parts)
            if member.isdir():
                output.mkdir(parents=True, exist_ok=True)
                continue
            output.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise VerificationError(f"cannot read archive member: {member.name}")
            with output.open("wb") as handle:
                while chunk := source.read(1024 * 1024):
                    handle.write(chunk)
            output.chmod(0o755 if member.name.endswith(("/rustqec", "/rstim")) else 0o644)
    extracted = (destination / root).resolve()
    if destination.resolve() not in extracted.parents:
        raise VerificationError("archive extraction escaped the temporary directory")
    return extracted


def run_binary(path: Path, arguments: list[str], *, stdin: str = "") -> subprocess.CompletedProcess[str]:
    environment = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(path.parent),
        "LANG": "C",
        "LC_ALL": "C",
    }
    return subprocess.run(
        [str(path.resolve()), *arguments], input=stdin, text=True, capture_output=True,
        timeout=30, check=False, env=environment,
    )


def check_linkage(binary: Path, target: str) -> None:
    if target == "x86_64-unknown-linux-gnu":
        result = subprocess.run(["/usr/bin/ldd", str(binary)], text=True, capture_output=True, check=False)
        if result.returncode != 0 or "not found" in result.stdout:
            raise VerificationError(f"unresolved Linux runtime linkage for {binary.name}: {result.stdout}{result.stderr}")
        for line in result.stdout.splitlines():
            match = re.search(r"=>\s+(/[^ ]+)", line)
            if match and not match.group(1).startswith(("/lib/", "/usr/lib/")):
                raise VerificationError(f"non-system Linux runtime dependency for {binary.name}: {line.strip()}")
    else:
        result = subprocess.run(["/usr/bin/otool", "-L", str(binary)], text=True, capture_output=True, check=False)
        if result.returncode != 0:
            raise VerificationError(f"cannot inspect macOS runtime linkage for {binary.name}: {result.stderr}")
        for line in result.stdout.splitlines()[1:]:
            dependency = line.strip().split(" ", 1)[0]
            if dependency and not dependency.startswith(("/usr/lib/", "/System/Library/")):
                raise VerificationError(f"non-system macOS runtime dependency for {binary.name}: {dependency}")
    print(f"runtime linkage: {binary.name} system-only")


def verify(args: argparse.Namespace) -> tuple[str, str]:
    manifest = json.loads(args.manifest.read_text())
    validate_manifest(manifest, args.expected_tag)

    target = current_target()
    archive_name = args.archive.name
    archive_identity = manifest.get("archives", {}).get(archive_name)
    if not isinstance(archive_identity, dict):
        raise VerificationError(f"archive is not listed in release manifest: {archive_name}")
    if archive_identity.get("target") != target:
        raise VerificationError(
            f"release target mismatch: archive={archive_identity.get('target')!r} host={target!r}"
        )

    checksums = parse_checksums(args.checksums)
    if set(checksums) != set(manifest["archives"]):
        raise VerificationError("SHA256SUMS does not contain exactly the manifested archives")
    recorded_digest = archive_identity.get("sha256")
    checksum_digest = checksums.get(archive_name)
    actual_digest = sha256(args.archive)
    if recorded_digest != checksum_digest or actual_digest != recorded_digest:
        raise VerificationError(
            f"archive SHA-256 mismatch: actual={actual_digest} manifest={recorded_digest} checksums={checksum_digest}"
        )
    if args.archive.stat().st_size != archive_identity.get("size"):
        raise VerificationError("archive size does not match release manifest")

    root = archive_identity.get("root_directory")
    if root != archive_name.removesuffix(".tar.gz"):
        raise VerificationError("archive root directory does not match its filename")
    with tempfile.TemporaryDirectory(prefix="rustqec-release-") as temporary:
        extracted = safe_extract(args.archive, Path(temporary), root)
        runtime_lines = (extracted / "RUNTIME.md").read_text().splitlines()
        expected_identity = [
            f"Tag: {args.expected_tag}",
            f"Source commit: {manifest['source_sha']}",
            f"Target: {target}",
        ]
        if runtime_lines[2:5] != expected_identity:
            raise VerificationError("RUNTIME.md identity does not match the release manifest")
        rustqec = extracted / "bin/rustqec"
        rstim = extracted / "bin/rstim"
        check_linkage(rustqec, target)
        check_linkage(rstim, target)

        rustqec_result = run_binary(rustqec, ["circuit", "stats", "--format", "json"], stdin=FIXTURE)
        if rustqec_result.returncode != 0 or rustqec_result.stderr or json.loads(rustqec_result.stdout) != RUSTQEC_OUTPUT:
            raise VerificationError(
                f"rustqec archive workflow failed: code={rustqec_result.returncode} "
                f"stdout={rustqec_result.stdout!r} stderr={rustqec_result.stderr!r}"
            )
        rstim_result = run_binary(rstim, ["stats", "--json"], stdin=FIXTURE)
        if rstim_result.returncode != 0 or rstim_result.stderr or json.loads(rstim_result.stdout) != RSTIM_OUTPUT:
            raise VerificationError(
                f"rstim archive workflow failed: code={rstim_result.returncode} "
                f"stdout={rstim_result.stdout!r} stderr={rstim_result.stderr!r}"
            )
    return target, args.expected_tag


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--checksums", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--expected-tag", required=True)
    args = parser.parse_args()
    try:
        target, tag = verify(args)
    except (VerificationError, OSError, tarfile.TarError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(f"FAIL release archive: {error}", file=sys.stderr)
        return 1
    print(f"PASS release archive {target} {tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
