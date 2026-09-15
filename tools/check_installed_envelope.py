#!/usr/bin/env python3
"""Test an installed release-candidate archive against the envelope contract.

Executes the support matrix's small public-input known-answer and rejection
controls with the candidate binary from ``--bin-dir`` (never a PATH binary),
from a clean temporary working directory. ILP-capable and no-ILP artifacts are
tested as separate expectations: a missing expected decoder is a failure, not
a skip. The pinned previous dataset fixtures are compared semantically
(predictions and declared statistics fields); timings and cache counters are
never part of the comparison.

Usage:
    python3 tools/check_installed_envelope.py \
      --bin-dir drafts/envelope-candidate/bin \
      --matrix docs/envelope-support.json \
      --out drafts/envelope-readiness/installed-aarch64-apple-darwin.json \
      --target aarch64-apple-darwin \
      --archive drafts/envelope-candidate.tar.gz \
      --source-sha <candidate-revision> \
      --expect-ilp
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_envelope_support as support  # noqa: E402

SCHEMA_VERSION = "rustqec.installed-envelope-report.v1"
PASS_LINE = "PASS installed envelope contract"
DEFAULT_MATRIX = Path("docs/envelope-support.json")

TARGET_BY_HOST = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "amd64"): "x86_64-unknown-linux-gnu",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
    ("Darwin", "aarch64"): "aarch64-apple-darwin",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
}


class InstalledError(Exception):
    pass


def host_target() -> str:
    key = (platform.system(), platform.machine().lower())
    if key not in TARGET_BY_HOST:
        raise InstalledError(f"unrecognized host platform: {key}")
    return TARGET_BY_HOST[key]


def locate_binary(bin_dir: Path) -> Path:
    binary = bin_dir / "rustqec"
    if not binary.is_file():
        raise InstalledError(f"missing executable: {binary}")
    if not binary.stat().st_mode & stat.S_IXUSR:
        raise InstalledError(f"not executable: {binary}")
    resolved = binary.resolve()
    path_binary = shutil_which_rustqec()
    if path_binary is not None and resolved == path_binary:
        raise InstalledError(
            "the bin-dir binary resolves to the PATH rustqec; test the extracted archive, "
            "not a conveniently available PATH binary"
        )
    return resolved


def shutil_which_rustqec() -> Path | None:
    for entry in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(entry) / "rustqec" if entry else None
        if candidate is not None and candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate.resolve()
    return None


def run(binary: Path, *args: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *args], cwd=cwd, capture_output=True, text=True, check=False
    )


def reduced_matrix(matrix: dict[str, Any], available: list[str]) -> dict[str, Any]:
    """View of the matrix restricted to the decoders an artifact advertises.

    A default/no-ILP artifact is still held to every matching-decoder control;
    MLE-only controls do not apply to it. Declared controls that cover both
    decoders are narrowed to the available ones.
    """
    decoders = {
        name: decoder
        for name, decoder in matrix["decoders"].items()
        if name in available
    }
    if not decoders:
        raise InstalledError("the artifact advertises none of the matrix decoders")
    controls = []
    for control in matrix["controls"]:
        kept = [name for name in control["decoders"] if name in available]
        if kept:
            controls.append({**control, "decoders": kept})
    reduced = {**matrix, "decoders": decoders, "controls": controls}
    support.validate_matrix(reduced)
    return reduced


def check_no_ilp_contract(binary: Path, matrix: dict[str, Any], work: Path) -> list[str]:
    """A default/no-ILP artifact must lack MLE and reject MLE invocations."""
    problems = []
    advertised = support.advertised_decoders(binary, work)
    if "envelope-mle" in advertised:
        problems.append("no-ILP artifact unexpectedly advertises envelope-mle")
    rejected = run(binary, "decode", "--decoder", "envelope-mle", cwd=work)
    stderr = rejected.stderr
    if rejected.returncode == 0 or "envelope-mle" not in stderr:
        problems.append("no-ILP artifact did not reject an envelope-mle invocation")
    return problems


def execute(
    bin_dir: Path,
    matrix_path: Path,
    *,
    expect_ilp: bool,
    target: str | None,
    archive: Path | None,
    source_sha: str | None,
) -> dict[str, Any]:
    matrix = support.load_matrix(matrix_path)
    binary = locate_binary(bin_dir.resolve())
    observed_target = host_target()
    problems: list[str] = []
    if target is not None and target != observed_target:
        raise InstalledError(
            f"expected target {target} but this machine is {observed_target}; "
            "run this check in the matching platform job against that job's archive"
        )
    with tempfile.TemporaryDirectory(prefix="installed-envelope-") as temporary:
        work = Path(temporary)
        advertised = support.advertised_decoders(binary, work)
        ilp_available = "envelope-mle" in advertised
        if expect_ilp and not ilp_available:
            problems.append(
                "expected an ILP-capable archive but envelope-mle is missing; "
                "a missing expected decoder is a failure, not a skip"
            )
        if not expect_ilp:
            problems += check_no_ilp_contract(binary, matrix, work)
        checked_matrix = matrix if expect_ilp else reduced_matrix(matrix, advertised)
        report = support.run_checks(binary, checked_matrix)
        problems += report["problems"]
        version = run(binary, "--version", cwd=work).stdout.strip()
    result = {
        "schema_version": SCHEMA_VERSION,
        "target": observed_target,
        "bin_dir": str(bin_dir.resolve()),
        "binary": {
            "path": str(binary),
            "sha256": support.sha256_file(binary),
            "version": version,
            "advertised_decoders": advertised,
        },
        "archive": {
            "path": str(archive),
            "sha256": support.sha256_file(archive),
        }
        if archive is not None
        else None,
        "source_revision": source_sha,
        "ilp": {"expected": expect_ilp, "available": ilp_available},
        "matrix": {
            "path": str(matrix_path),
            "sha256": support.sha256_file(matrix_path),
            "schema_version": matrix["schema_version"],
        },
        "controls": report["results"],
        "previous_fixture_semantics": (
            "pinned fixture predictions and declared statistics fields are compared exactly; "
            "timing and cache-counter fields are excluded from the comparison"
        ),
        "status": "pass" if not problems and report["status"] == "pass" else "fail",
        "problems": problems,
    }
    return result


def self_test(real_binary: Path, matrix_path: Path) -> int:
    """The installed checker must reject defective candidate binaries."""
    observations: list[dict[str, Any]] = []

    def make_shim(directory: Path, body: str) -> Path:
        directory.mkdir(parents=True, exist_ok=True)
        shim = directory / "rustqec"
        shim.write_text(body, encoding="utf-8")
        shim.chmod(0o755)
        return shim

    # Defect 1: a candidate that returns a flipped prediction.
    with tempfile.TemporaryDirectory(prefix="installed-selftest-flip-") as temporary:
        work = Path(temporary)
        make_shim(
            work / "bin",
            f"""#!/bin/bash
set -e
"{real_binary}" "$@"
out=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--out" ]; then out="$arg"; fi
  prev="$arg"
done
for arg in "$@"; do
  if [ "$arg" = "decode" ] && [ -n "$out" ] && [ -f "$out" ]; then
    python3 -c 'import sys; p=sys.argv[1]; b=bytearray(open(p,"rb").read()); b[0]^=1; open(p,"wb").write(bytes(b))' "$out"
  fi
done
""",
        )
        try:
            report = execute(work / "bin", matrix_path, expect_ilp=True, target=None,
                             archive=None, source_sha=None)
            flipped_rejected = report["status"] == "fail"
            detail = report["problems"][:1]
        except InstalledError as error:
            flipped_rejected = True
            detail = [str(error)]
    observations.append({"defect": "flipped-prediction-binary",
                         "rejected": flipped_rejected, "detail": detail})

    # Defect 2: advertises MLE but cannot execute it.
    with tempfile.TemporaryDirectory(prefix="installed-selftest-mle-") as temporary:
        work = Path(temporary)
        make_shim(
            work / "bin",
            f"""#!/bin/bash
for arg in "$@"; do
  if [ "$arg" = "envelope-mle" ]; then
    echo '{{"schema_version":"rustqec.cli.v1","status":"error","command":"decode","error":{{"code":"feature_disabled","message":"shim: mle cannot execute"}}}}' >&2
    exit 2
  fi
done
exec "{real_binary}" "$@"
""",
        )
        try:
            report = execute(work / "bin", matrix_path, expect_ilp=True, target=None,
                             archive=None, source_sha=None)
            mle_rejected = report["status"] == "fail"
            detail = report["problems"][:1]
        except (InstalledError, support.MatrixError) as error:
            mle_rejected = True
            detail = [str(error)]
    observations.append({"defect": "advertises-mle-but-cannot-execute",
                         "rejected": mle_rejected, "detail": detail})

    passed = all(o["rejected"] for o in observations)
    print(json.dumps({"self_test_defective_binaries": observations}, indent=2))
    if passed:
        print("PASS installed envelope contract self-test")
        return 0
    print("FAIL installed envelope contract self-test", file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--target", help="expected target triple for this platform job")
    parser.add_argument("--archive", type=Path, help="candidate archive for hash binding")
    parser.add_argument("--source-sha", help="candidate source revision")
    parser.add_argument("--expect-ilp", action="store_true",
                        help="require an ILP-capable archive (official native archives)")
    parser.add_argument("--no-expect-ilp", action="store_true",
                        help="assert the default/no-ILP artifact contract instead")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--binary", type=Path, default=Path("target/release/rustqec"),
                        help="real binary used by --self-test shims")
    args = parser.parse_args()

    if args.self_test:
        binary = args.binary.resolve()
        if not binary.is_file():
            print(f"missing binary for self-test: {binary}", file=sys.stderr)
            return 2
        return self_test(binary, args.matrix)

    if args.bin_dir is None:
        print("--bin-dir is required", file=sys.stderr)
        return 2
    if args.expect_ilp and args.no_expect_ilp:
        print("--expect-ilp and --no-expect-ilp are mutually exclusive", file=sys.stderr)
        return 2
    expect_ilp = not args.no_expect_ilp
    try:
        report = execute(
            args.bin_dir,
            args.matrix,
            expect_ilp=expect_ilp,
            target=args.target,
            archive=args.archive.resolve() if args.archive else None,
            source_sha=args.source_sha,
        )
    except (InstalledError, support.MatrixError) as error:
        print(f"FAIL installed envelope contract: {error}", file=sys.stderr)
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(
                json.dumps(
                    {
                        "schema_version": SCHEMA_VERSION,
                        "status": "fail",
                        "problems": [str(error)],
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
        return 1
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if report["status"] == "pass":
        print(f"{PASS_LINE} target={report['target']} controls={len(report['controls'])}")
        return 0
    print("FAIL installed envelope contract", file=sys.stderr)
    for problem in report["problems"]:
        print(f"  - {problem}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
