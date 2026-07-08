from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]


def _run_checked(command: list[str], *, cwd: Path) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def _probe(command: list[str]) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "status": "failed",
            "exit_code": None,
            "version": None,
            "stderr": str(error),
        }
    version = completed.stdout.strip()
    status = "ok" if completed.returncode == 0 else "failed"
    return {
        "command": command,
        "status": status,
        "exit_code": completed.returncode,
        "version": version if status == "ok" else None,
        "stderr": completed.stderr.strip(),
    }


def _version_string(command: list[str]) -> str:
    result = _probe(command)
    if result["status"] == "ok" and result["version"]:
        return str(result["version"])
    stderr = str(result.get("stderr") or "")
    return f"failed: {stderr}" if stderr else "failed"


def build_rstim(profile: str, *, repo_root: Path = REPO_ROOT) -> Path:
    if profile == "release":
        command = ["cargo", "build", "--release", "-p", "rstim", "--bin", "rstim"]
        binary = repo_root / "target/release/rstim"
    elif profile == "debug":
        command = ["cargo", "build", "-p", "rstim", "--bin", "rstim"]
        binary = repo_root / "target/debug/rstim"
    else:
        raise ValueError(f"unsupported profile: {profile}")

    _run_checked(command, cwd=repo_root)
    if not binary.exists():
        raise FileNotFoundError(f"expected rstim binary not found: {binary}")
    return binary


def collect_environment(
    *,
    profile: str,
    case_label: str,
    warmup_rounds: int,
    measure_rounds: int,
    rstim_binary_path: Path,
) -> dict[str, Any]:
    stim = _probe(["stim", "--version"])
    rstim = _probe([str(rstim_binary_path)])
    return {
        "profile": profile,
        "case_label": case_label,
        "warmup_rounds": warmup_rounds,
        "measure_rounds": measure_rounds,
        "rustc_version": _version_string(["rustc", "--version"]),
        "cargo_version": _version_string(["cargo", "--version"]),
        "rstim_binary_path": str(rstim_binary_path.resolve()),
        "rstim_version": rstim.get("version"),
        "rstim_status": rstim["status"],
        "stim_cli": stim,
        "stim_cli_status": stim["status"],
        "stim_cli_version": stim.get("version"),
        "stim_cli_stderr": stim.get("stderr"),
    }


def write_environment(path: Path, environment: dict[str, Any]) -> None:
    path.write_text(json.dumps(environment, indent=2, sort_keys=True) + "\n")


def run_speed_case(args: argparse.Namespace, *, repo_root: Path = REPO_ROOT) -> None:
    rstim_binary = build_rstim(args.profile, repo_root=repo_root)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    raw_path = out_dir / "raw.jsonl"
    summary_path = out_dir / "summary.json"
    report_path = out_dir / "report.md"
    environment_path = out_dir / "environment.json"

    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "run",
            "--case",
            args.case,
            "--warmup-rounds",
            str(args.warmup_rounds),
            "--measure-rounds",
            str(args.measure_rounds),
            "--out",
            str(raw_path),
        ],
        cwd=repo_root,
    )
    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "summarize",
            "--in",
            str(raw_path),
            "--out",
            str(summary_path),
        ],
        cwd=repo_root,
    )
    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "report",
            "--in",
            str(summary_path),
            "--out",
            str(report_path),
        ],
        cwd=repo_root,
    )
    write_environment(
        environment_path,
        collect_environment(
            profile=args.profile,
            case_label=args.case,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
            rstim_binary_path=rstim_binary,
        ),
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run one rstim-vs-Stim speed case with a selected rstim build profile."
    )
    parser.add_argument("--profile", choices=["debug", "release"], required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--warmup-rounds", type=int, default=1)
    parser.add_argument("--measure-rounds", type=int, default=5)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_speed_case(args)
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
