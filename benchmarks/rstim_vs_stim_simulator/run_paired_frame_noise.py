from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
import json
import os
import platform
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
MODULE_NAME = "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise"
CANONICAL_FIXTURE_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
CANONICAL_FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
PINNED_BASELINE_REV = "f10d1ed024d3519318ed244c9095724074519595"
CASE_ID = "stim_surface_d11_r100"
MEASUREMENT_COUNT = 12_121
OUTPUT_FORMAT = "b8"
BYTES_PER_SHOT = 1_516
EXPECTED_OUTPUT_BYTES = 1_552_384
TIMER_SCOPE = "process_spawn_stdout_stderr_drain_exit"
BASELINE_VARIANT = "baseline-rstim-frame-noise-b8"
CANDIDATE_VARIANT = "candidate-rstim-frame-noise-b8"
VARIANT_LABELS = {
    BASELINE_VARIANT: "baseline",
    CANDIDATE_VARIANT: "candidate",
}
TOOL_ROLES = {
    BASELINE_VARIANT: "tool://rstim-baseline-frame-noise",
    CANDIDATE_VARIANT: "tool://rstim-candidate-frame-noise",
}
ARTIFACT_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")


@dataclass(frozen=True)
class CliResult:
    exit_code: int
    stdout: bytes
    stderr: bytes
    elapsed_ns: int


@dataclass(frozen=True)
class RevisionBuild:
    label: str
    requested_rev: str
    resolved_commit: str
    source_dir: Path
    target_dir: Path
    binary_path: Path


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8")


def _record_path(path: Path, *, repo_root: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(repo_root.resolve()).as_posix()
    except ValueError:
        return str(resolved)


def _probe_stdout(argv: list[str], *, cwd: Path) -> str:
    completed = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"{argv[0]} exited with code {completed.returncode}: {detail}")
    return completed.stdout.strip()


def _version_or_failed(argv: list[str], *, cwd: Path) -> str:
    try:
        return _probe_stdout(argv, cwd=cwd)
    except (OSError, RuntimeError) as error:
        return f"failed: {error}"


def _cpu_model() -> str:
    try:
        completed = subprocess.run(
            ["sysctl", "-n", "machdep.cpu.brand_string"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        completed = None
    if completed is not None and completed.returncode == 0 and completed.stdout.strip():
        return completed.stdout.strip()
    return platform.processor() or platform.machine() or "unknown"


def _git_stdout(argv: list[str], *, repo_root: Path) -> bytes:
    completed = subprocess.run(argv, cwd=repo_root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if completed.returncode != 0:
        detail = completed.stderr.decode(errors="replace").strip()
        raise RuntimeError(f"{' '.join(argv)} failed: {detail or completed.returncode}")
    return completed.stdout


def resolve_revision(revision: str, *, repo_root: Path) -> str:
    return _git_stdout(["git", "rev-parse", revision], repo_root=repo_root).decode("ascii").strip()


def ensure_distinct_revisions(baseline_commit: str, candidate_commit: str) -> None:
    if baseline_commit == candidate_commit:
        raise ValueError("baseline and candidate revisions must differ")


def materialize_revision(revision: str, *, repo_root: Path, temp_root: Path, label: str) -> RevisionBuild:
    resolved = resolve_revision(revision, repo_root=repo_root)
    source_dir = temp_root / f"{label}-source"
    target_dir = temp_root / f"{label}-target"
    source_dir.mkdir(parents=True, exist_ok=False)
    target_dir.mkdir(parents=True, exist_ok=False)
    archive = _git_stdout(["git", "archive", "--format=tar", resolved], repo_root=repo_root)
    with tarfile.open(fileobj=BytesIO(archive), mode="r:") as tar:
        tar.extractall(source_dir, filter="data")
    return RevisionBuild(
        label=label,
        requested_rev=revision,
        resolved_commit=resolved,
        source_dir=source_dir,
        target_dir=target_dir,
        binary_path=target_dir / "release" / "rstim",
    )


def build_revision(revision: RevisionBuild) -> Path:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(revision.target_dir)
    subprocess.run(
        ["cargo", "build", "--release", "-p", "rstim", "--bin", "rstim"],
        cwd=revision.source_dir,
        env=env,
        check=True,
    )
    if not revision.binary_path.is_file():
        raise FileNotFoundError(f"expected rstim binary not found: {revision.binary_path}")
    return revision.binary_path


def time_cli(argv: list[str], *, cwd: Path) -> CliResult:
    started_ns = time.perf_counter_ns()
    process = subprocess.Popen(argv, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout, stderr = process.communicate()
    return CliResult(process.returncode, stdout, stderr, time.perf_counter_ns() - started_ns)


def _canonical_argv(role: str, *, fixture: Path, shots: int, seed: int, repo_root: Path) -> list[str]:
    return [
        role,
        "sample",
        "--skip_reference_sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        _record_path(fixture, repo_root=repo_root),
    ]


def validate_canonical_command(argv: list[str], *, variant: str, fixture: Path, shots: int, seed: int) -> None:
    expected = _canonical_argv(argv[0], fixture=fixture, shots=shots, seed=seed, repo_root=REPO_ROOT)
    if argv != expected:
        if "--skip_reference_sample" not in argv:
            raise ValueError(f"{variant} command must include --skip_reference_sample")
        raise ValueError(f"{variant} command is not the canonical frame-noise command")


def _validate_fixture(fixture: Path, *, repo_root: Path) -> Path:
    canonical = repo_root / CANONICAL_FIXTURE_PATH
    if fixture.resolve() != canonical.resolve():
        raise ValueError(f"fixture must be {CANONICAL_FIXTURE_PATH}")
    if sha256_file(fixture) != CANONICAL_FIXTURE_SHA256:
        raise ValueError("canonical fixture SHA-256 does not match")
    return fixture.resolve()


def _record_result(
    *,
    variant: str,
    phase: str,
    round_index: int,
    seed: int,
    logical_argv: list[str],
    result: CliResult,
) -> dict[str, Any]:
    if result.exit_code != 0:
        detail = result.stderr.decode(errors="replace").strip()
        raise RuntimeError(f"{variant} exited with code {result.exit_code}: {detail}")
    actual_output_bytes = len(result.stdout)
    if actual_output_bytes != EXPECTED_OUTPUT_BYTES:
        raise RuntimeError(
            f"{variant} output bytes: expected {EXPECTED_OUTPUT_BYTES}, got {actual_output_bytes}"
        )
    return {
        "case_id": CASE_ID,
        "variant": variant,
        "phase": phase,
        "round_index": round_index,
        "seed": seed,
        "argv": logical_argv,
        "elapsed_ns": result.elapsed_ns,
        "timer_scope": TIMER_SCOPE,
        "exit_code": result.exit_code,
        "actual_output_bytes": actual_output_bytes,
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "stderr_bytes": len(result.stderr),
    }


def _summary(records: list[dict[str, Any]], *, baseline: RevisionBuild, candidate: RevisionBuild) -> dict[str, Any]:
    variants: list[dict[str, Any]] = []
    for variant in (BASELINE_VARIANT, CANDIDATE_VARIANT):
        elapsed_ns = [record["elapsed_ns"] for record in records if record["variant"] == variant and record["phase"] == "measured"]
        variants.append({
            "variant": variant,
            "measured_count": len(elapsed_ns),
            "median_elapsed_ns": statistics.median(elapsed_ns),
            "mean_elapsed_ns": statistics.mean(elapsed_ns),
            "min_elapsed_ns": min(elapsed_ns),
            "max_elapsed_ns": max(elapsed_ns),
        })
    return {
        "module": MODULE_NAME,
        "case_id": CASE_ID,
        "timer_scope": TIMER_SCOPE,
        "baseline_revision": baseline.resolved_commit,
        "candidate_revision": candidate.resolved_commit,
        "expected_output_bytes": EXPECTED_OUTPUT_BYTES,
        "measured_record_count": sum(variant["measured_count"] for variant in variants),
        "variants": variants,
    }


def _report(summary: dict[str, Any]) -> str:
    lines = [
        "# Paired Frame-Noise Benchmark",
        "",
        f"Case: `{CASE_ID}`",
        f"Timer scope: `{TIMER_SCOPE}`",
        f"Expected stdout bytes per process: `{EXPECTED_OUTPUT_BYTES}`",
        "",
        "| Variant | Measured runs | Median elapsed (ns) |",
        "| --- | ---: | ---: |",
    ]
    for variant in summary["variants"]:
        lines.append(f"| {variant['variant']} | {variant['measured_count']} | {variant['median_elapsed_ns']} |")
    return "\n".join(lines) + "\n"


def _environment(*, repo_root: Path, fixture: Path, baseline: RevisionBuild, candidate: RevisionBuild) -> dict[str, Any]:
    return {
        "module": MODULE_NAME,
        "case_id": CASE_ID,
        "fixture_path": _record_path(fixture, repo_root=repo_root),
        "fixture_sha256": sha256_file(fixture),
        "expected_output_bytes": EXPECTED_OUTPUT_BYTES,
        "measurement_count": MEASUREMENT_COUNT,
        "bytes_per_shot": BYTES_PER_SHOT,
        "output_format": OUTPUT_FORMAT,
        "timer_scope": TIMER_SCOPE,
        "python_version": sys.version,
        "platform": platform.platform(),
        "cpu_model": _cpu_model(),
        "baseline_revision": {
            "requested_rev": baseline.requested_rev,
            "resolved_commit": baseline.resolved_commit,
            "rstim_version": _version_or_failed([str(baseline.binary_path)], cwd=repo_root),
        },
        "candidate_revision": {
            "requested_rev": candidate.requested_rev,
            "resolved_commit": candidate.resolved_commit,
            "rstim_version": _version_or_failed([str(candidate.binary_path)], cwd=repo_root),
        },
    }


def _write_artifact_hashes(out_dir: Path) -> None:
    write_json(out_dir / "artifact-sha256.json", {name: sha256_file(out_dir / name) for name in ARTIFACT_FILES})


def run_paired_frame_noise(args: argparse.Namespace, *, repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    if args.baseline_rev != PINNED_BASELINE_REV:
        raise ValueError(f"baseline revision is pinned to {PINNED_BASELINE_REV}")
    fixture = _validate_fixture(Path(args.fixture), repo_root=repo_root)
    if args.shots != 1024:
        raise ValueError("paired frame-noise benchmark requires --shots 1024")
    if args.warmup_rounds < 0 or args.measure_rounds <= 0:
        raise ValueError("round counts must be nonnegative warmup and positive measured")
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="rstim-paired-frame-noise-") as temp_dir:
        temp_root = Path(temp_dir)
        baseline = materialize_revision(args.baseline_rev, repo_root=repo_root, temp_root=temp_root, label="baseline")
        candidate = materialize_revision(args.candidate_rev, repo_root=repo_root, temp_root=temp_root, label="candidate")
        ensure_distinct_revisions(baseline.resolved_commit, candidate.resolved_commit)
        binaries = {"baseline": build_revision(baseline), "candidate": build_revision(candidate)}

        records: list[dict[str, Any]] = []
        for phase, rounds in (("warmup", args.warmup_rounds), ("measured", args.measure_rounds)):
            for round_index in range(rounds):
                labels = ["baseline", "candidate"]
                if round_index % 2:
                    labels.reverse()
                seed = round_index
                for label in labels:
                    variant = BASELINE_VARIANT if label == "baseline" else CANDIDATE_VARIANT
                    logical_argv = _canonical_argv(TOOL_ROLES[variant], fixture=fixture, shots=args.shots, seed=seed, repo_root=repo_root)
                    validate_canonical_command(logical_argv, variant=variant, fixture=fixture, shots=args.shots, seed=seed)
                    actual_argv = [str(binaries[label]), *logical_argv[1:-1], str(fixture)]
                    result = time_cli(actual_argv, cwd=repo_root)
                    records.append(_record_result(
                        variant=variant,
                        phase=phase,
                        round_index=round_index,
                        seed=seed,
                        logical_argv=logical_argv,
                        result=result,
                    ))

        summary = _summary(records, baseline=baseline, candidate=candidate)
        environment = _environment(repo_root=repo_root, fixture=fixture, baseline=baseline, candidate=candidate)
        write_jsonl(out_dir / "raw.jsonl", records)
        write_json(out_dir / "summary.json", summary)
        (out_dir / "report.md").write_text(_report(summary), encoding="utf-8")
        write_json(out_dir / "environment.json", environment)
        _write_artifact_hashes(out_dir)
        return summary


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Compare paired rstim frame-noise CLI revisions.")
    parser.add_argument("--baseline-rev", default=PINNED_BASELINE_REV)
    parser.add_argument("--candidate-rev", default="HEAD")
    parser.add_argument("--fixture", type=Path, default=REPO_ROOT / CANONICAL_FIXTURE_PATH)
    parser.add_argument("--shots", type=int, default=1024)
    parser.add_argument("--warmup-rounds", type=int, default=2)
    parser.add_argument("--measure-rounds", type=int, default=7)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    summary = run_paired_frame_noise(args)
    print(
        "PASS paired frame-noise benchmark "
        f"variants=2 measured={summary['measured_record_count']} bytes={EXPECTED_OUTPUT_BYTES}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
