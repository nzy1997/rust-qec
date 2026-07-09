from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator import run_speed_case


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]


def parse_case_labels(raw_cases: str) -> list[str]:
    labels = [label.strip() for label in raw_cases.split(",") if label.strip()]
    if not labels:
        raise ValueError("no benchmark cases requested")
    seen: set[str] = set()
    for label in labels:
        if label in seen:
            raise ValueError(f"duplicate benchmark case: {label}")
        seen.add(label)
    return labels


def _append_file(source: Path, destination: Path) -> None:
    with source.open("r", encoding="utf-8") as src, destination.open("a", encoding="utf-8") as dst:
        dst.write(src.read())


def _merge_case_summary(summary_path: Path, merged: dict[str, list[Any]]) -> None:
    summary = json.loads(summary_path.read_text())
    merged["cases"].extend(summary.get("cases", []))
    merged["issues"].extend(summary.get("issues", []))


def _validate_case_label(rstim_binary: Path, label: str, *, cwd: Path, temp_root: Path, index: int) -> None:
    validation_summary_path = temp_root / f"validate-{index}.json"
    try:
        completed = subprocess.run(
            [
                str(rstim_binary),
                "perf",
                "summarize",
                "--case",
                label,
                "--in",
                str(temp_root / "empty.jsonl"),
                "--out",
                str(validation_summary_path),
            ],
            cwd=cwd,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise ValueError(str(error)) from error
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        message = f"unknown benchmark case: {label}"
        if detail:
            message = f"{message}: {detail}"
        raise ValueError(message)
    run_speed_case._require_artifact(validation_summary_path)


def run_speed_suite(
    args: argparse.Namespace,
    repo_root: Path = REPO_ROOT,
    command_line: list[str] | None = None,
) -> None:
    case_labels = parse_case_labels(args.cases)
    rstim_binary = run_speed_case.build_rstim(args.profile, repo_root=repo_root)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    raw_path = out_dir / "raw.jsonl"
    raw_path.write_text("")
    summary_path = out_dir / "summary.json"
    report_path = out_dir / "report.md"
    environment_path = out_dir / "environment.json"

    with tempfile.TemporaryDirectory() as temp_dir:
        temp_root = Path(temp_dir)
        empty_raw_path = temp_root / "empty.jsonl"
        empty_raw_path.write_text("")

        for index, label in enumerate(case_labels):
            _validate_case_label(rstim_binary, label, cwd=repo_root, temp_root=temp_root, index=index)

        for index, label in enumerate(case_labels):
            case_raw_path = temp_root / f"raw-{index}.jsonl"
            run_speed_case._run_checked(
                [
                    str(rstim_binary),
                    "perf",
                    "run",
                    "--case",
                    label,
                    "--warmup-rounds",
                    str(args.warmup_rounds),
                    "--measure-rounds",
                    str(args.measure_rounds),
                    "--out",
                    str(case_raw_path),
                ],
                cwd=repo_root,
            )
            run_speed_case._require_artifact(case_raw_path)
            _append_file(case_raw_path, raw_path)

        merged_summary: dict[str, list[Any]] = {"cases": [], "issues": []}
        for index, label in enumerate(case_labels):
            case_summary_path = temp_root / f"summary-{index}.json"
            run_speed_case._run_checked(
                [
                    str(rstim_binary),
                    "perf",
                    "summarize",
                    "--case",
                    label,
                    "--in",
                    str(raw_path),
                    "--out",
                    str(case_summary_path),
                ],
                cwd=repo_root,
            )
            run_speed_case._require_artifact(case_summary_path)
            _merge_case_summary(case_summary_path, merged_summary)

    summary_path.write_text(json.dumps(merged_summary, indent=2, sort_keys=True) + "\n")
    run_speed_case._run_checked(
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
    run_speed_case._require_artifact(report_path)
    run_speed_case.write_environment(
        environment_path,
        run_speed_case.collect_suite_environment(
            profile=args.profile,
            case_labels=case_labels,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
            rstim_binary_path=rstim_binary,
            command_line=list(sys.argv if command_line is None else command_line),
        ),
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run multiple rstim-vs-Stim speed cases with a selected rstim build profile."
    )
    parser.add_argument("--profile", choices=["debug", "release"], required=True)
    parser.add_argument("--cases", required=True)
    parser.add_argument("--warmup-rounds", type=int, default=1)
    parser.add_argument("--measure-rounds", type=int, default=5)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_speed_suite(args, command_line=sys.argv if argv is None else [sys.argv[0], *argv])
    except (OSError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
