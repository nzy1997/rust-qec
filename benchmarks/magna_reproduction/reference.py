#!/usr/bin/env python3
"""Verify pinned Magna inputs and capture isolated upstream baseline runs."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path, PurePosixPath


HERE = Path(__file__).resolve().parent
MANIFEST = HERE / "reference.json"


class ReferenceError(ValueError):
    """An input or completed run does not satisfy the reference contract."""


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative_file(root: Path, name: str) -> Path:
    parts = PurePosixPath(name)
    if not name or "\\" in name or parts.is_absolute() or any(
        p in ("", ".", "..") for p in name.split("/")
    ):
        raise ReferenceError(f"invalid relative reference path: {name!r}")
    path = root / name
    # Reject symlinks even when they currently resolve inside the input root.
    if any(p.is_symlink() for p in [path, *path.parents] if p != root.parent):
        raise ReferenceError(f"symlink in reference path: {name}")
    if not path.resolve().is_relative_to(root.resolve()):
        raise ReferenceError(f"reference path escapes root: {name}")
    return path


def verify_files(root: Path, files: dict[str, str]) -> int:
    if not root.is_dir():
        raise ReferenceError(f"reference directory does not exist: {root}")
    for name, expected in files.items():
        if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
            raise ReferenceError(f"invalid SHA-256 for {name}")
        path = relative_file(root, name)
        if not path.is_file():
            raise ReferenceError(f"missing reference file: {name}")
        if digest(path) != expected:
            raise ReferenceError(f"reference SHA-256 mismatch: {name}")
    return len(files)


def generated(path: Path) -> bool:
    return "__pycache__" in path.parts or path.suffix in (".pyc", ".pyo", ".so", ".pyd") or path.name in (
        "native-build.json", "native-reference-build.json",
    )


def verify_source(root: Path, manifest: dict, require_decoders: bool = False) -> dict:
    count = verify_files(root, manifest["upstream"]["files"])
    expected = set(manifest["upstream"]["files"])
    decoder_files = {}
    for name in manifest["tasks"]:
        provenance = root / "examples" / name / "decoder/author_source/provenance.json"
        if not provenance.is_file():
            continue
        values = json.loads(provenance.read_text(encoding="utf-8"))["files"]
        for filename, sha in values.items():
            # The upstream fetcher installs top-level author modules only.
            if "/" not in filename:
                relative = f"examples/{name}/decoder/author_source/{filename}"
                if require_decoders or (root / relative).exists():
                    decoder_files[relative] = sha
    verify_files(root, decoder_files)
    expected.update(decoder_files)
    for directory in ("core", "scripts", "examples", "tests"):
        for path in (root / directory).rglob("*"):
            relative = path.relative_to(root)
            if path.is_symlink():
                raise ReferenceError(f"symlink in reference source: {relative}")
            if path.is_file() and relative.as_posix() not in expected and not generated(relative):
                raise ReferenceError(f"unlisted reference source file: {relative}")
    actual_tasks = sorted(p.parent.name for p in (root / "examples").glob("*/run.json"))
    actual_tests = sorted(p.name for p in (root / "tests/core").glob("test_*.py"))
    if actual_tasks != manifest["tasks"] or actual_tests != manifest["test_modules"]:
        raise ReferenceError("task or upstream test-module coverage differs from the pinned inventory")
    return {"source_files": count, "decoder_files": len(decoder_files), "tasks": len(actual_tasks)}


def load_manifest() -> dict:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest.get("schema") != "rstim.magna-reference.v1":
        raise ReferenceError("unsupported Magna reference manifest schema")
    return manifest


def verify_capture(root: Path, manifest: dict, all_tasks: bool = False) -> dict:
    receipt = json.loads(relative_file(root, "receipt.json").read_text(encoding="utf-8"))
    if receipt.get("schema") != "rstim.magna-baseline.v1" or receipt.get("status") != "complete":
        raise ReferenceError("baseline receipt is incomplete or has an unsupported schema")
    if receipt.get("manifest_sha256") != digest(MANIFEST) or receipt.get("commit") != manifest["upstream"]["commit"]:
        raise ReferenceError("baseline receipt does not match the pinned reference")
    if receipt.get("worker_sha256") != digest(HERE / "worker.py"):
        raise ReferenceError("baseline worker differs from the current capture implementation")
    cases = receipt.get("cases", [])
    if not cases or cases != sorted(set(cases)) or set(cases) - set(manifest["tasks"]):
        raise ReferenceError("baseline task inventory is invalid")
    if all_tasks and cases != manifest["tasks"]:
        raise ReferenceError("baseline does not cover every packaged task")
    runs = receipt.get("runs", [])
    if [run["case"] for run in runs] != cases:
        raise ReferenceError("baseline task coverage is incomplete")
    binary_hashes = set()
    for run in runs:
        path = relative_file(root, f"{run['case']}.json")
        if digest(path) != run["sha256"]:
            raise ReferenceError(f"baseline result SHA-256 mismatch: {run['case']}")
        data = json.loads(path.read_text(encoding="utf-8"))
        check_case_result(data["result"], receipt["shots"])
        if data["native_sha256"] != run["native_sha256"]:
            raise ReferenceError("baseline native extension identity differs from receipt")
        binary_hashes.add(data["native_sha256"])
    if len(binary_hashes) != 1:
        raise ReferenceError("baseline uses different native extensions")
    return {"tasks": len(cases), "shots_per_task": receipt["shots"], "scope": "smoke"}


def check_case_result(result: dict, shots: int) -> None:
    if result.get("status") != "complete" or result.get("requested") != shots or result.get("completed") != shots:
        raise ReferenceError("baseline did not complete the requested draw count")
    if result.get("failed") != 0 or result.get("not_attempted") != 0 or result.get("failures"):
        raise ReferenceError("baseline reports failures or unattempted draws")
    estimates = result.get("estimates", {})
    for key in ("acceptance", "error"):
        value = estimates.get(key)
        if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
            raise ReferenceError(f"baseline has an invalid {key} estimate")


def capture(args: argparse.Namespace, manifest: dict) -> None:
    unknown = set(args.case or ()) - set(manifest["tasks"])
    if unknown:
        raise ReferenceError(f"unknown packaged tasks: {', '.join(sorted(unknown))}")
    cases = sorted(set(args.case or manifest["tasks"]))
    source = args.upstream.resolve()
    before = verify_source(source, manifest, require_decoders=any(c.startswith("tqd") for c in cases))
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    receipt = {"schema": "rstim.magna-baseline.v1", "status": "failed", "scope": "smoke",
               "manifest_sha256": digest(MANIFEST), "commit": manifest["upstream"]["commit"],
               "worker_sha256": digest(HERE / "worker.py"),
               "cases": cases, "shots": args.shots, "seed": args.seed, "before": before, "runs": []}
    failure = None
    try:
        with tempfile.TemporaryDirectory(prefix="magna-reference-") as cwd:
            for case in cases:
                # Resolve the location without following the venv interpreter symlink.
                command = [str(args.python.absolute()), "-I", str(HERE / "worker.py"),
                           "--upstream", str(source), "--case", case,
                           "--shots", str(args.shots), "--seed", str(args.seed)]
                print(f"reference task {case}: {args.shots} draws", flush=True)
                try:
                    run = subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=args.timeout)
                except subprocess.TimeoutExpired as error:
                    for suffix, value in [("json", error.stdout), ("stderr.txt", error.stderr)]:
                        text = value.decode(errors="replace") if isinstance(value, bytes) else value or ""
                        (output / f"{case}.{suffix}").write_text(text, encoding="utf-8")
                    raise ReferenceError(f"reference task {case} exceeded {args.timeout} seconds") from error
                (output / f"{case}.stderr.txt").write_text(run.stderr, encoding="utf-8")
                (output / f"{case}.json").write_text(run.stdout, encoding="utf-8")
                if run.returncode:
                    raise ReferenceError(f"reference task {case} failed with exit {run.returncode}; see stderr receipt")
                data = json.loads(run.stdout)
                check_case_result(data["result"], args.shots)
                if receipt["runs"] and data["native_sha256"] != receipt["runs"][0]["native_sha256"]:
                    raise ReferenceError("reference native extension changed between tasks")
                receipt["runs"].append({"case": case, "sha256": digest(output / f"{case}.json"),
                                        "native_sha256": data["native_sha256"]})
        receipt["status"] = "complete"
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        failure = error
        receipt["failure"] = str(error)
    finally:
        try:
            receipt["after"] = verify_source(source, manifest, require_decoders=any(c.startswith("tqd") for c in cases))
            if digest(HERE / "worker.py") != receipt["worker_sha256"]:
                raise ReferenceError("reference worker changed during capture")
        except (OSError, ValueError) as error:
            failure = error
            receipt["failure"] = str(error)
        if failure is not None:
            receipt["status"] = "failed"
        (output / "receipt.json").write_text(json.dumps(receipt, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    if failure is not None:
        raise ReferenceError(str(failure))


def positive(value: str) -> int:
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return number


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify", help="verify source and optionally all deposited paper files")
    verify.add_argument("--upstream", type=Path, required=True)
    verify.add_argument("--paper-data", type=Path)
    check = commands.add_parser("check", help="verify captured baseline JSON without simulator imports")
    check.add_argument("results", type=Path)
    check.add_argument("--all-tasks", action="store_true")
    run = commands.add_parser("run", help="capture packaged task smoke results in isolated child processes")
    run.add_argument("--upstream", type=Path, required=True)
    run.add_argument("--python", type=Path, required=True)
    run.add_argument("--out", type=Path, required=True)
    run.add_argument("--case", action="append")
    run.add_argument("--shots", type=positive, default=1024)
    run.add_argument("--seed", type=int, default=20261008)
    run.add_argument("--timeout", type=positive, default=300)
    args = parser.parse_args()
    try:
        manifest = load_manifest()
        if args.command == "verify":
            result = verify_source(args.upstream.resolve(), manifest)
            if args.paper_data:
                result["paper_files"] = verify_files(args.paper_data.resolve(), manifest["paper"]["files"])
            print(json.dumps(result, sort_keys=True))
        elif args.command == "check":
            print(json.dumps(verify_capture(args.results.resolve(), manifest, args.all_tasks), sort_keys=True))
        else:
            capture(args, manifest)
    except (OSError, ValueError) as error:
        print(f"FAIL Magna reference: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
