from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_speed_case


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
EXPECTED_STIM_VERSION = "1.15.0"
KNOWN_ANSWER_CIRCUIT = "X 0\nM 0\n"
KNOWN_ANSWER_OUTPUT = b"\x01"

build_rstim = run_speed_case.build_rstim


@dataclass(frozen=True)
class CliResult:
    exit_code: int
    stdout: bytes
    stderr: bytes
    elapsed_ns: int


def time_cli(argv: list[str], *, cwd: Path) -> CliResult:
    started_ns = time.perf_counter_ns()
    process = subprocess.Popen(
        argv,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    stdout, stderr = process.communicate()
    elapsed_ns = time.perf_counter_ns() - started_ns
    return CliResult(
        exit_code=process.returncode,
        stdout=stdout,
        stderr=stderr,
        elapsed_ns=elapsed_ns,
    )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _resolve_repo_path(path: Path, *, repo_root: Path) -> Path:
    return path if path.is_absolute() else (repo_root / path).resolve()


def _resolve_executable(raw: str, *, repo_root: Path) -> Path:
    path = Path(raw)
    if path.is_absolute() or len(path.parts) > 1:
        candidate = path if path.is_absolute() else repo_root / path
        if candidate.is_file():
            return candidate.resolve()
        raise FileNotFoundError(f"executable not found: {raw}")
    resolved = shutil.which(raw)
    if resolved is not None:
        return Path(resolved).resolve()
    repo_candidate = repo_root / raw
    if repo_candidate.is_file():
        return repo_candidate.resolve()
    raise FileNotFoundError(f"executable not found on PATH: {raw}")


def _probe_stdout(argv: list[str], *, cwd: Path) -> str:
    completed = subprocess.run(
        argv,
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"{argv[0]} exited with code {completed.returncode}: {detail}")
    return completed.stdout.strip()


def _extract_semver(text: str) -> str | None:
    match = re.search(r"\b(\d+\.\d+\.\d+)\b", text)
    return match.group(1) if match is not None else None


def _stim_version(stim_binary: Path, *, repo_root: Path) -> str:
    completed = subprocess.run(
        [str(stim_binary), "--version"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    probe_text = "\n".join(part for part in (completed.stdout.strip(), completed.stderr.strip()) if part)
    if completed.returncode != 0:
        raise RuntimeError(
            f"Stim CLI version probe exited with code {completed.returncode}: "
            f"{probe_text or 'no output'}"
        )
    version = _extract_semver(probe_text)
    if version is None:
        module_version = _probe_stdout(
            ["python3", "-c", "import stim; print(stim.__version__)"],
            cwd=repo_root,
        )
        version = _extract_semver(module_version)
    if version != EXPECTED_STIM_VERSION:
        raise RuntimeError(
            f"Stim CLI must be version {EXPECTED_STIM_VERSION}; got {probe_text or 'unknown'!r}"
        )
    return version


def _rstim_version(rstim_binary: Path, *, repo_root: Path) -> str:
    return _probe_stdout([str(rstim_binary)], cwd=repo_root)


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


def _load_validated_case(
    args: argparse.Namespace,
    *,
    manifest_path: Path,
    repo_root: Path,
) -> dict[str, Any]:
    manifest = fair_cli_contract.load_manifest(manifest_path)
    case = fair_cli_contract.find_case(manifest, args.case)
    errors = fair_cli_contract.validate_case(
        case,
        manifest_path=manifest_path,
        repo_root=repo_root,
    )
    if errors:
        raise RuntimeError("fair CLI manifest validation failed: " + "; ".join(errors))
    return case


def _case_for_input(case: dict[str, Any], input_path: Path, *, shots: int) -> dict[str, Any]:
    expanded_case = dict(case)
    expanded_case["canonical_input_path"] = str(input_path)
    expanded_case["shots"] = shots
    return expanded_case


def _expand_argv(
    template: list[str],
    *,
    variant: str,
    case: dict[str, Any],
    input_path: Path,
    shots: int,
    seed: int,
    stim_binary: Path,
    rstim_binary: Path,
) -> list[str]:
    argv = fair_cli_contract.expand_argv(
        template,
        _case_for_input(case, input_path, shots=shots),
        seed=seed,
        rstim_binary=str(rstim_binary),
    )
    if variant == "stim-cli-b8":
        argv[0] = str(stim_binary)
    elif variant == "rstim-cli-b8":
        argv[0] = str(rstim_binary)
    else:
        argv[0] = str(_resolve_executable(argv[0], repo_root=REPO_ROOT))
    return argv


def _variant_templates(case: dict[str, Any]) -> list[tuple[str, list[str]]]:
    argv = case.get("argv")
    if not isinstance(argv, dict):
        raise RuntimeError("fair CLI case has no argv table")
    return [(name, list(template)) for name, template in argv.items()]


def _run_known_answer_preflight(
    *,
    case: dict[str, Any],
    templates: list[tuple[str, list[str]]],
    stim_binary: Path,
    rstim_binary: Path,
    repo_root: Path,
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory() as temp_dir:
        preflight_path = Path(temp_dir) / "known_answer.stim"
        preflight_path.write_text(KNOWN_ANSWER_CIRCUIT, encoding="utf-8")
        for variant, template in templates:
            argv = _expand_argv(
                template,
                variant=variant,
                case=case,
                input_path=preflight_path,
                shots=1,
                seed=0,
                stim_binary=stim_binary,
                rstim_binary=rstim_binary,
            )
            result = time_cli(argv, cwd=repo_root)
            if result.exit_code != 0:
                stderr = result.stderr.decode(errors="replace").strip()
                raise RuntimeError(
                    f"{variant} known-answer preflight exited with code {result.exit_code}: {stderr}"
                )
            if result.stdout != KNOWN_ANSWER_OUTPUT:
                raise RuntimeError(
                    f"{variant} known-answer preflight expected stdout 0x01, "
                    f"got {result.stdout.hex() or '<empty>'}"
                )
            results.append(
                {
                    "variant": variant,
                    "argv": argv,
                    "exit_code": result.exit_code,
                    "stdout_hex": result.stdout.hex(),
                    "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                    "elapsed_ns": result.elapsed_ns,
                }
            )
    return results


def _raw_record(
    *,
    case: dict[str, Any],
    variant: str,
    phase: str,
    round_index: int,
    seed: int,
    argv: list[str],
    result: CliResult,
) -> dict[str, Any]:
    return {
        "case_id": case["case_id"],
        "variant": variant,
        "phase": phase,
        "round_index": round_index,
        "seed": seed,
        "argv": argv,
        "shots": case["shots"],
        "measurement_count": case["measurement_count"],
        "output_format": case["output_format"],
        "timer_scope": case["timer_scope"],
        "elapsed_ns": result.elapsed_ns,
        "actual_output_bytes": len(result.stdout),
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "exit_code": result.exit_code,
    }


def _run_rounds(
    *,
    args: argparse.Namespace,
    case: dict[str, Any],
    templates: list[tuple[str, list[str]]],
    input_path: Path,
    stim_binary: Path,
    rstim_binary: Path,
    repo_root: Path,
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for variant, template in templates:
        round_seed = 0
        for phase, count in (("warmup", args.warmup_rounds), ("measured", args.measure_rounds)):
            for round_index in range(count):
                argv = _expand_argv(
                    template,
                    variant=variant,
                    case=case,
                    input_path=input_path,
                    shots=case["shots"],
                    seed=round_seed,
                    stim_binary=stim_binary,
                    rstim_binary=rstim_binary,
                )
                result = time_cli(argv, cwd=repo_root)
                if result.exit_code != 0:
                    stderr = result.stderr.decode(errors="replace").strip()
                    raise RuntimeError(
                        f"{variant} {phase} round {round_index} failed with exit code "
                        f"{result.exit_code}: {stderr}"
                    )
                if len(result.stdout) != case["expected_output_bytes"]:
                    raise RuntimeError(
                        f"{variant} {phase} round {round_index} produced "
                        f"{len(result.stdout)} output bytes; expected {case['expected_output_bytes']}"
                    )
                records.append(
                    _raw_record(
                        case=case,
                        variant=variant,
                        phase=phase,
                        round_index=round_index,
                        seed=round_seed,
                        argv=argv,
                        result=result,
                    )
                )
                round_seed += 1
    return records


def _elapsed_summary(samples: list[int]) -> dict[str, Any]:
    return {
        "samples": samples,
        "sample_count": len(samples),
        "min": min(samples),
        "max": max(samples),
        "mean": statistics.mean(samples),
        "median": statistics.median(samples),
    }


def _summary(records: list[dict[str, Any]], *, case: dict[str, Any]) -> dict[str, Any]:
    measured = [record for record in records if record["phase"] == "measured"]
    variants: list[dict[str, Any]] = []
    for variant in dict.fromkeys(record["variant"] for record in records):
        variant_records = [record for record in measured if record["variant"] == variant]
        samples = [int(record["elapsed_ns"]) for record in variant_records]
        variants.append(
            {
                "variant": variant,
                "sample_count": len(samples),
                "elapsed_ns": _elapsed_summary(samples),
                "total_output_bytes": sum(int(record["actual_output_bytes"]) for record in variant_records),
                "stdout_sha256": [record["stdout_sha256"] for record in variant_records],
            }
        )
    return {
        "case_id": case["case_id"],
        "shots": case["shots"],
        "measurement_count": case["measurement_count"],
        "output_format": case["output_format"],
        "timer_scope": case["timer_scope"],
        "measured_record_count": len(measured),
        "variants": variants,
    }


def _render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Fair CLI sampling benchmark",
        "",
        f"Case: {summary['case_id']}",
        f"Measured records: {summary['measured_record_count']}",
        "",
        "| variant | sample_count | median_elapsed_ns | min_elapsed_ns | max_elapsed_ns |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for variant in summary["variants"]:
        elapsed = variant["elapsed_ns"]
        lines.append(
            f"| {variant['variant']} | {variant['sample_count']} | {elapsed['median']} | "
            f"{elapsed['min']} | {elapsed['max']} |"
        )
    lines.append("")
    return "\n".join(lines)


def _collect_environment(
    *,
    args: argparse.Namespace,
    case: dict[str, Any],
    manifest_path: Path,
    input_path: Path,
    stim_binary: Path,
    rstim_binary: Path,
    stim_version: str,
    rstim_version: str,
    records: list[dict[str, Any]],
    preflight_results: list[dict[str, Any]],
    repo_root: Path,
) -> dict[str, Any]:
    source_manifest = str(case["source_manifest_path"])
    source_manifest_path = (repo_root / source_manifest).resolve()
    fixture = str(case["canonical_input_path"])
    return {
        "git_commit": _version_or_failed(["git", "rev-parse", "HEAD"], cwd=repo_root),
        "os": platform.platform(),
        "cpu_model": _cpu_model(),
        "profile": args.profile,
        "timer_scope": case["timer_scope"],
        "seed_policy": case["seed_policy"],
        "stim_version": stim_version,
        "rstim_version": rstim_version,
        "rustc_version": _version_or_failed(["rustc", "--version"], cwd=repo_root),
        "manifest": str(args.manifest),
        "manifest_sha256": _sha256_file(manifest_path),
        "fair_manifest_path": str(args.manifest),
        "fair_manifest_sha256": _sha256_file(manifest_path),
        "source_manifest": source_manifest,
        "source_manifest_sha256": _sha256_file(source_manifest_path),
        "source_manifest_path": source_manifest,
        "fixture": fixture,
        "fixture_sha256": _sha256_file(input_path),
        "fixture_path": fixture,
        "stim_binary": str(stim_binary),
        "stim_binary_sha256": _sha256_file(stim_binary),
        "rstim_binary": str(rstim_binary),
        "rstim_binary_sha256": _sha256_file(rstim_binary),
        "round_argv": [
            {
                "variant": record["variant"],
                "phase": record["phase"],
                "round_index": record["round_index"],
                "seed": record["seed"],
                "argv": record["argv"],
            }
            for record in records
        ],
        "warmup_rounds": args.warmup_rounds,
        "measure_rounds": args.measure_rounds,
        "known_answer_preflight": "passed",
        "known_answer_preflight_details": preflight_results,
    }


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run_fair_cli(args: argparse.Namespace, *, repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    if args.warmup_rounds < 0 or args.measure_rounds < 1:
        raise ValueError("warmup rounds must be nonnegative and measure rounds must be positive")

    manifest_path = _resolve_repo_path(args.manifest, repo_root=repo_root)
    case = _load_validated_case(args, manifest_path=manifest_path, repo_root=repo_root)
    templates = _variant_templates(case)
    rstim_binary = Path(build_rstim(args.profile, repo_root=repo_root)).resolve()
    stim_binary = _resolve_executable(templates[0][1][0], repo_root=repo_root)
    stim_version = _stim_version(stim_binary, repo_root=repo_root)
    rstim_version = _rstim_version(rstim_binary, repo_root=repo_root)

    input_path = (repo_root / case["canonical_input_path"]).resolve()
    preflight_results = _run_known_answer_preflight(
        case=case,
        templates=templates,
        stim_binary=stim_binary,
        rstim_binary=rstim_binary,
        repo_root=repo_root,
    )
    records = _run_rounds(
        args=args,
        case=case,
        templates=templates,
        input_path=input_path,
        stim_binary=stim_binary,
        rstim_binary=rstim_binary,
        repo_root=repo_root,
    )

    summary = _summary(records, case=case)
    environment = _collect_environment(
        args=args,
        case=case,
        manifest_path=manifest_path,
        input_path=input_path,
        stim_binary=stim_binary,
        rstim_binary=rstim_binary,
        stim_version=stim_version,
        rstim_version=rstim_version,
        records=records,
        preflight_results=preflight_results,
        repo_root=repo_root,
    )

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    _write_json(out_dir / "summary.json", summary)
    _write_json(out_dir / "environment.json", environment)
    (out_dir / "report.md").write_text(_render_report(summary), encoding="utf-8")
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the symmetric fair CLI sampling benchmark.")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--profile", choices=["release", "debug"], required=True)
    parser.add_argument("--warmup-rounds", type=int, required=True)
    parser.add_argument("--measure-rounds", type=int, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        summary = run_fair_cli(args)
    except (OSError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    measured = int(summary["measured_record_count"])
    variant_count = len(summary["variants"])
    bytes_per_run = fair_cli_contract.EXPECTED_CASE["expected_output_bytes"]
    print(
        "PASS symmetric fair CLI runner "
        f"variants={variant_count} warmups={variant_count * args.warmup_rounds} "
        f"measured={measured} bytes_per_run={bytes_per_run}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
