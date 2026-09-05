#!/usr/bin/env python3
"""Verify a completed native support workflow against local Cargo metadata."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path


CELLS = {
    "ubuntu-24.04-x86_64-msrv": {
        "runner_label": "ubuntu-24.04",
        "runner_os": "Linux",
        "runner_arch": "X64",
        "machine": "x86_64",
        "os_version": "24.04",
        "target": "x86_64-unknown-linux-gnu",
        "toolchain": "1.88.0",
    },
    "ubuntu-24.04-x86_64-stable": {
        "runner_label": "ubuntu-24.04",
        "runner_os": "Linux",
        "runner_arch": "X64",
        "machine": "x86_64",
        "os_version": "24.04",
        "target": "x86_64-unknown-linux-gnu",
        "toolchain": "stable",
    },
    "macos-15-aarch64-msrv": {
        "runner_label": "macos-15",
        "runner_os": "macOS",
        "runner_arch": "ARM64",
        "machine": "arm64",
        "os_version": "15.",
        "target": "aarch64-apple-darwin",
        "toolchain": "1.88.0",
    },
    "macos-15-aarch64-stable": {
        "runner_label": "macos-15",
        "runner_os": "macOS",
        "runner_arch": "ARM64",
        "machine": "arm64",
        "os_version": "15.",
        "target": "aarch64-apple-darwin",
        "toolchain": "stable",
    },
}

EXPECTED_CLI_OUTPUT = {
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


class MatrixError(RuntimeError):
    pass


def run_json(argv: list[str], *, cwd: Path | None = None) -> object:
    completed = subprocess.run(argv, cwd=cwd, check=True, text=True, capture_output=True)
    return json.loads(completed.stdout)


def version_tuple(value: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.(\d+))?(?:[-+].*)?", value)
    if not match:
        raise MatrixError(f"invalid Rust version {value!r}")
    return tuple(int(part or 0) for part in match.groups())


def workspace_packages(metadata: dict[str, object]) -> list[dict[str, str]]:
    member_ids = set(metadata.get("workspace_members", []))
    packages = [
        {
            "name": package["name"],
            "version": package["version"],
            "rust_version": package.get("rust_version"),
        }
        for package in metadata.get("packages", [])
        if package.get("id") in member_ids
    ]
    if not packages:
        raise MatrixError("Cargo metadata contains no workspace packages")
    missing = [package["name"] for package in packages if not package["rust_version"]]
    if missing:
        raise MatrixError(f"workspace packages missing rust-version: {', '.join(sorted(missing))}")
    return sorted(packages, key=lambda package: package["name"])


def validate_matrix(
    jobs: list[dict[str, object]],
    evidence_items: list[dict[str, object]],
    packages: list[dict[str, str]],
    run: dict[str, object],
    local_head: str,
) -> list[str]:
    if run.get("path") != ".github/workflows/native-support.yml":
        raise MatrixError(f"run {run.get('id')} is not the native-support workflow")
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise MatrixError(f"workflow run is not successful: status={run.get('status')} conclusion={run.get('conclusion')}")
    run_head = str(run.get("head_sha", ""))
    if not run_head or run_head != local_head:
        raise MatrixError(f"workflow source head {run_head!r} does not match checked-out HEAD {local_head!r}")

    declared_versions = {package["rust_version"] for package in packages}
    if len(declared_versions) != 1:
        raise MatrixError(f"workspace rust-version values differ: {sorted(declared_versions)}")
    declared_msrv = next(iter(declared_versions))

    jobs_by_name: dict[str, list[dict[str, object]]] = {}
    for job in jobs:
        jobs_by_name.setdefault(str(job.get("name", "")), []).append(job)
    evidence_by_cell: dict[str, list[dict[str, object]]] = {}
    for evidence in evidence_items:
        evidence_by_cell.setdefault(str(evidence.get("cell", "")), []).append(evidence)

    expected_package_map = {package["name"]: package for package in packages}
    summaries: list[str] = []
    for cell, expected in CELLS.items():
        job_name = f"Native support / {cell}"
        matching_jobs = jobs_by_name.get(job_name, [])
        if len(matching_jobs) != 1:
            raise MatrixError(f"expected exactly one GitHub job named {job_name!r}, found {len(matching_jobs)}")
        job = matching_jobs[0]
        if job.get("status") != "completed" or job.get("conclusion") != "success":
            raise MatrixError(
                f"GitHub job {job_name!r} was not successful: "
                f"status={job.get('status')} conclusion={job.get('conclusion')}"
            )
        if job.get("head_sha") != run_head:
            raise MatrixError(f"GitHub job {job_name!r} head {job.get('head_sha')!r} != workflow head {run_head!r}")

        matching_evidence = evidence_by_cell.get(cell, [])
        if len(matching_evidence) != 1:
            raise MatrixError(f"expected exactly one evidence artifact for {cell}, found {len(matching_evidence)}")
        evidence = matching_evidence[0]
        if evidence.get("schema_version") != "rustqec.native-support.v1" or evidence.get("status") != "pass":
            raise MatrixError(f"{cell} evidence is not a passing rustqec.native-support.v1 record")
        tested_sha = str(evidence.get("tested_sha", ""))
        source_head_sha = str(evidence.get("source_head_sha", ""))
        if not tested_sha:
            raise MatrixError(f"{cell} evidence does not record the tested checkout SHA")
        if source_head_sha != run_head:
            raise MatrixError(f"{cell} source head {source_head_sha!r} != workflow head {run_head!r}")

        requested = str(evidence.get("requested_toolchain", ""))
        if requested != expected["toolchain"]:
            raise MatrixError(f"{cell} requested toolchain {requested!r} != {expected['toolchain']!r}")
        target = str(evidence.get("target", ""))
        if target != expected["target"]:
            raise MatrixError(f"{cell} target {target!r} != {expected['target']!r}")

        compiler = evidence.get("compiler", {})
        release = str(compiler.get("release", ""))
        host = str(compiler.get("host", ""))
        if host != target:
            raise MatrixError(f"{cell} rustc host {host!r} != target {target!r}")
        if requested != "stable" and release != requested:
            raise MatrixError(f"{cell} rustc release {release!r} != requested {requested!r}")
        if version_tuple(release) < version_tuple(declared_msrv):
            raise MatrixError(f"{cell} rustc {release} is below declared package requirement {declared_msrv}")
        compiler_path = str(compiler.get("path", ""))
        rustup_which = str(compiler.get("rustup_which", ""))
        cargo_path = str(compiler.get("cargo_path", ""))
        expected_marker = f"/toolchains/{requested}-{target}/bin/"
        if compiler_path != rustup_which or expected_marker not in compiler_path:
            raise MatrixError(f"{cell} compiler path does not prove the requested rustup toolchain: {compiler_path!r}")
        if not cargo_path.startswith(compiler_path.rsplit("/", 1)[0] + "/"):
            raise MatrixError(f"{cell} cargo and rustc were not selected from the same toolchain directory")

        runner = evidence.get("runner", {})
        for key in ("label", "os", "arch", "machine"):
            expected_value = expected[f"runner_{key}"] if key in ("label", "os", "arch") else expected[key]
            if runner.get(key) != expected_value:
                raise MatrixError(f"{cell} runner {key} {runner.get(key)!r} != {expected_value!r}")
        if not str(runner.get("os_version", "")).startswith(str(expected["os_version"])):
            raise MatrixError(f"{cell} runner OS version {runner.get('os_version')!r} is outside the supported baseline")
        if not runner.get("image_os") or not runner.get("image_version"):
            raise MatrixError(f"{cell} runner image identity is incomplete")

        if evidence.get("cli_output") != EXPECTED_CLI_OUTPUT:
            raise MatrixError(f"{cell} one-qubit CLI output is invalid")

        recorded_packages = {package["name"]: package for package in evidence.get("packages", [])}
        if recorded_packages != expected_package_map:
            raise MatrixError(f"{cell} package metadata does not match the checked-out workspace")
        summaries.append(f"{cell}: rustc {release} host={host}")

    tested_shas = {str(evidence.get("tested_sha", "")) for evidence in evidence_items}
    if len(tested_shas) != 1:
        raise MatrixError(f"native support cells tested different checkout SHAs: {sorted(tested_shas)}")
    unexpected = sorted(set(evidence_by_cell) - set(CELLS))
    if unexpected:
        raise MatrixError(f"unexpected native support evidence cells: {', '.join(unexpected)}")
    return summaries


def infer_repository(repo_root: Path) -> str:
    remote = subprocess.run(
        ["git", "remote", "get-url", "origin"], cwd=repo_root, check=True, text=True, capture_output=True
    ).stdout.strip()
    match = re.search(r"github\.com[/:]([^/]+/[^/]+?)(?:\.git)?$", remote)
    if not match:
        raise MatrixError(f"cannot infer GitHub repository from origin {remote!r}")
    return match.group(1)


def download_run(
    repository: str, run_id: int, destination: Path
) -> tuple[dict[str, object], list[dict[str, object]], list[dict[str, object]]]:
    run_payload = run_json(["gh", "api", f"repos/{repository}/actions/runs/{run_id}"])
    jobs_payload = run_json(
        ["gh", "api", f"repos/{repository}/actions/runs/{run_id}/jobs?per_page=100"]
    )
    subprocess.run(
        [
            "gh",
            "run",
            "download",
            str(run_id),
            "--repo",
            repository,
            "--pattern",
            "native-support-*",
            "--dir",
            str(destination),
        ],
        check=True,
    )
    evidence_paths = sorted(destination.glob("**/evidence.json"))
    evidence = [json.loads(path.read_text()) for path in evidence_paths]
    return run_payload, jobs_payload.get("jobs", []), evidence


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--repository", help="owner/repository; inferred from origin by default")
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()

    try:
        metadata = run_json(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=repo_root
        )
        packages = workspace_packages(metadata)
        repository = args.repository or infer_repository(repo_root)
        local_head = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=repo_root, check=True, text=True, capture_output=True
        ).stdout.strip()
        with tempfile.TemporaryDirectory(prefix="rustqec-native-support-") as temporary:
            run, jobs, evidence = download_run(repository, args.run_id, Path(temporary))
        summaries = validate_matrix(jobs, evidence, packages, run, local_head)
    except (MatrixError, subprocess.CalledProcessError, json.JSONDecodeError, OSError) as error:
        print(f"FAIL native support matrix: {error}", file=sys.stderr)
        return 1

    for summary in summaries:
        print(summary)
    print(f"source HEAD: {local_head}")
    print(f"tested checkout SHA: {evidence[0]['tested_sha']}")
    print(f"PASS native support matrix cells={len(CELLS)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
