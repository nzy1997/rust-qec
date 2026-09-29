"""Run the P0/P1 near-Clifford matrix and retain raw per-call timings."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
P0 = ["terminal_20q", "repeated_20q", "mid_feedback", "qec_rounds"]
P1 = [
    "records_64", "records_65", "records_128", "records_256",
    "random_64", "random_65", "random_66", "random_72", "random_73",
    "random_74", "random_75", "random_76", "random_77", "random_78",
    "random_79", "random_80",
    "rank_8", "rank_9", "rank_10", "rank_11",
]


def command_output(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def ms(value: dict) -> float:
    return value["median_ns"] / 1_000_000


def report(result: dict) -> str:
    lines = [
        "# Near-Clifford P0/P1 benchmark",
        "",
        f"Source: `{result['git_sha']}`; {result['platform']}; Rust `{result['rustc']}`.",
        f"Run: {result['created_utc']}; quick={result['quick']}. All values are median milliseconds.",
        "Each fixture ran in a separate process. Compile and prepare are timed separately.",
        "Cold includes prepare; first excludes prepare but has no cache warmup; warm uses",
        "one sampler after a 64-shot cache warmup.",
        "The output allocation is timed; result destruction is outside the timed interval.",
        "The RSS figure is the fixture process peak, including the benchmark harness.",
        "",
        "| Group | Fixture | Compile | Prepare | Peak RSS MiB |",
        "| --- | --- | ---: | ---: | ---: |",
    ]
    for row in result["cases"]:
        group = "P0" if row["fixture"] in P0 else "P1"
        rss = row["peak_rss_bytes"]
        lines.append(
            f"| {group} | {row['fixture']} | {ms(row['compile']):.4f} | "
            f"{ms(row['prepare']):.4f} | {rss / 1048576:.1f} |"
        )
    lines += [
        "", "| Fixture | Shots | API sample | Cold structured | First structured | First flat | Warm structured | Warm flat |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in result["cases"]:
        for measurement in row["measurements"]:
            lines.append(
                f"| {row['fixture']} | {measurement['shots']} | "
                f"{ms(measurement['unprepared']):.4f} | "
                f"{ms(measurement['cold_prepared_structured']):.4f} | "
                f"{ms(measurement['first_prepared_structured']):.4f} | "
                f"{ms(measurement['first_prepared_flat']):.4f} | "
                f"{ms(measurement['warm_prepared_structured']):.4f} | "
                f"{ms(measurement['warm_prepared_flat']):.4f} |"
            )
    lines += [
        "", "Measurements within the same fixture share the circuit and shot count; ratios across",
        "different fixtures also reflect circuit complexity and output width. This benchmark does",
        "not expose internal cache-hit or active-rank counters. The rank fixtures request independent",
        "T axes, but the reported rank is a construction label, not a traced runtime peak.",
        "", "The JSON companion contains each circuit, all raw repetitions, and host metadata.",
    ]
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--group", choices=["p0", "p1", "all"], default="all")
    parser.add_argument("--quick", action="store_true", help="three repetitions and fewer shot counts")
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "drafts/near-clifford-matrix/results.json")
    args = parser.parse_args()

    if not args.no_build:
        subprocess.run(
            ["cargo", "build", "--release", "--locked", "-p", "rstim", "--example", "near_clifford_matrix_bench"],
            cwd=ROOT, check=True,
        )
    binary = ROOT / "target/release/examples/near_clifford_matrix_bench"
    if platform.system() == "Windows":
        binary = binary.with_suffix(".exe")
    names = P0 if args.group == "p0" else P1 if args.group == "p1" else P0 + P1
    result = {
        "schema": 1,
        "git_sha": command_output("git", "rev-parse", "HEAD"),
        "git_dirty": bool(command_output("git", "status", "--porcelain")),
        "rustc": command_output("rustc", "--version"),
        "platform": platform.platform(),
        "python": sys.version.split()[0],
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "quick": args.quick,
        "group": args.group,
        "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for name in names:
        print(f"Running {name}...", flush=True)
        invocation = [str(binary), name]
        if args.quick:
            invocation.append("--quick")
        row = json.loads(command_output(*invocation))
        result["cases"].append(row)
        args.output.write_text(json.dumps(result, indent=2) + "\n")
        args.output.with_suffix(".md").write_text(report(result))
    print(f"Raw data: {args.output}")
    print(f"Report: {args.output.with_suffix('.md')}")


if __name__ == "__main__":
    main()
