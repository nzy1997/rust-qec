#!/usr/bin/env python3
"""Import existing committed benchmark artifacts into the publication bundle.

This converter is the recorded provenance bridge for issue #601: every run it
writes names the source artifacts (path + sha256) it consumed, and runs whose
original production provenance was not captured are marked
``production_provenance.recorded = false`` so the readiness report lists them
as gaps instead of silently upgrading them to publication-grade evidence.

Re-run from a clean tree:

    python3 tools/import_publication_evidence.py --repo-root .
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_publication_benchmark_evidence as pub  # noqa: E402

RESULTS_ROOT = Path("benchmarks/publication_evidence/results")

SURFACE_CSV = Path("benchmarks/surface_decoder_compare/results/full/results.csv")
BB_CSV = Path("benchmarks/bb_circuit_bposd_compare/results/full/results.csv")
BB_CONTRACT = Path("benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json")
FAIR_CLI_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/fair-cli-release")
RSMP_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/rsmp-v1")

LEGACY_HARDWARE = {
    "cpu_model": "not recorded in source artifact",
    "cpu_class": "unidentified",
    "physical_cores": 0,
    "logical_cores": 0,
    "ram_gb": 0,
    "os": "not recorded in source artifact",
    "identified": False,
}

HW01_HARDWARE = {
    "cpu_model": "Apple M4",
    "cpu_class": "aarch64-apple-darwin",
    "physical_cores": 10,
    "logical_cores": 10,
    "ram_gb": 32,
    "os": "macOS (Darwin 25.6.0)",
    "identified": True,
}

SURFACE_VARIANT_MAP = {
    "pymatching": "pymatching",
    "ldpc": "ldpc_bposd",
    "ilpqec": "ilpqec",
    "rmatching": "rmatching",
    "rbposd": "rbposd",
    "rilpqec": "rilpqec",
}


def git_commit(repo_root: Path) -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=repo_root, text=True,
        stdout=subprocess.PIPE, check=True,
    ).stdout.strip()


def git_dirty_tracked(repo_root: Path) -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=no"],
        cwd=repo_root, text=True, stdout=subprocess.PIPE, check=True,
    )
    return bool(result.stdout.strip())


def write_run(
    repo_root: Path,
    family: str,
    hardware_id: str,
    run_id: str,
    records: list[dict],
    hardware: dict,
    toolchain: dict,
    argv: list[str],
    sources: list[Path],
    production_provenance: dict,
    commit: str,
    dirty: bool,
) -> Path:
    run_dir = repo_root / RESULTS_ROOT / family / hardware_id / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    with (run_dir / "raw.jsonl").open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True) + "\n")
    pub.write_json(run_dir / "summary.json", {
        "schema": "publication-summary-v1",
        "family": family,
        "hardware_id": hardware_id,
        "run_id": run_id,
        "estimates": pub.derive_estimates(records),
    })
    pub.write_json(run_dir / "environment.json", {
        "schema": "publication-environment-v1",
        "family": family,
        "hardware_id": hardware_id,
        "run_id": run_id,
        "git": {"commit": commit, "dirty": dirty},
        "hardware": hardware,
        "toolchain": toolchain,
        "argv": argv,
        "source_artifacts": [
            {"path": source.as_posix(), "sha256": pub.sha256_file(repo_root / source)}
            for source in sources
        ],
        "production_provenance": production_provenance,
    })
    pub.write_json(run_dir / "artifact-sha256.json", {
        name: pub.sha256_file(run_dir / name) for name in pub.RUN_FILES if name != "artifact-sha256.json"
    })
    return run_dir


def legacy_toolchain() -> dict:
    return {
        "rust_target": "not recorded in source artifact",
        "rustc": "not recorded in source artifact",
        "build_profile": "not recorded",
        "threads": "not recorded",
    }


def import_surface(repo_root: Path, commit: str, dirty: bool) -> Path:
    records = []
    with (repo_root / SURFACE_CSV).open(newline="", encoding="utf-8") as handle:
        for index, row in enumerate(csv.DictReader(handle), start=1):
            if row["status"] != "ok":
                continue
            records.append({
                "record_id": f"surface-full-{index:04d}",
                "kind": "logical_error",
                "variant": SURFACE_VARIANT_MAP.get(row["decoder"], row["decoder"]),
                "scale": {"distance": row["distance"], "p": row["p"]},
                "seed": int(row["seed"]),
                "protocol": "error_budget_stopped",
                "values": {
                    "shots": int(row["shots_used"]),
                    "logical_errors": int(row["logical_errors"]),
                },
            })
    return write_run(
        repo_root, "surface-decoder-compare", "hw-legacy-unidentified", "legacy-full-2025",
        records, LEGACY_HARDWARE, legacy_toolchain(),
        ["import://tools/import_publication_evidence.py", "--family", "surface-decoder-compare",
         "--source", SURFACE_CSV.as_posix()],
        [SURFACE_CSV],
        {
            "recorded": False,
            "note": "historical error-budget-stopped full comparison; original commit, hardware, "
                    "and declared-seed fixed-shot protocol were not recorded in the source artifact",
        },
        commit, dirty,
    )


def import_bb(repo_root: Path, commit: str, dirty: bool) -> Path:
    records = []
    with (repo_root / BB_CSV).open(newline="", encoding="utf-8") as handle:
        for index, row in enumerate(csv.DictReader(handle), start=1):
            if row["status"] != "ok":
                continue
            records.append({
                "record_id": f"bb-full-{index:04d}",
                "kind": "logical_error",
                "variant": row["decoder_impl"],
                "scale": {"code_id": row["code_id"], "p": row["p"]},
                "seed": int(row["seed"]),
                "protocol": "error_budget_stopped",
                "values": {
                    "shots": int(row["shots_used"]),
                    "logical_errors": int(row["logical_errors"]),
                },
            })
    return write_run(
        repo_root, "bb-circuit-bposd-compare", "hw-legacy-unidentified", "legacy-full-2025",
        records, LEGACY_HARDWARE, legacy_toolchain(),
        ["import://tools/import_publication_evidence.py", "--family", "bb-circuit-bposd-compare",
         "--source", BB_CSV.as_posix()],
        [BB_CSV, BB_CONTRACT],
        {
            "recorded": False,
            "note": "historical error-budget-stopped bb72/bb144 comparison; the Bravyi reference "
                    "curve and fixed-shot protocol rows are not part of the committed artifact",
        },
        commit, dirty,
    )


def import_simulator(repo_root: Path, commit: str, dirty: bool) -> Path:
    records = []
    with (repo_root / FAIR_CLI_DIR / "raw.jsonl").open(encoding="utf-8") as handle:
        for index, line in enumerate(handle, start=1):
            row = json.loads(line)
            variant = {"tool://stim": "stim_cli", "tool://rstim": "rstim_cli"}.get(row["argv"][0])
            if variant is None:
                continue
            records.append({
                "record_id": f"fair-cli-{index:04d}",
                "kind": "timing",
                "variant": variant,
                "scale": {"workload": row["case_id"]},
                "seed": row.get("seed"),
                "phase": row["phase"],
                "repetition": row["round_index"],
                "timer_scope": "cli_end_to_end",
                "values": {"elapsed_ns": row["elapsed_ns"]},
            })
    return write_run(
        repo_root, "rstim-vs-stim-simulator", "hw-legacy-unidentified", "legacy-fair-cli-release",
        records, LEGACY_HARDWARE, legacy_toolchain(),
        ["import://tools/import_publication_evidence.py", "--family", "rstim-vs-stim-simulator",
         "--source", (FAIR_CLI_DIR / "raw.jsonl").as_posix()],
        [FAIR_CLI_DIR / "raw.jsonl", FAIR_CLI_DIR / "environment.json"],
        {
            "recorded": False,
            "note": "portable fair-cli bundle pins executable hashes and argv but does not record "
                    "the source commit or an identified hardware profile; 2 warmup + 7 measured "
                    "rounds fall below the publication repetition protocol",
        },
        commit, dirty,
    )


def import_rsmp(repo_root: Path, commit: str, dirty: bool) -> Path:
    environment = json.loads((repo_root / RSMP_DIR / "environment.json").read_text(encoding="utf-8"))
    records = []
    with (repo_root / RSMP_DIR / "raw.jsonl").open(encoding="utf-8") as handle:
        rows = [json.loads(line) for line in handle]
    for index, row in enumerate(rows, start=1):
        case_id = row["case_id"]
        shots = str(row["dimensions"]["shots"])
        seed = 7 if row.get("is_benchmark") else None
        raw_b8 = row["measurement_input"]["raw_b8_bytes"]
        scale = {"case_id": case_id, "shots": shots}
        records.append({
            "record_id": f"rsmp-{index:04d}-b8",
            "kind": "bytes",
            "variant": "b8",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": raw_b8},
        })
        records.append({
            "record_id": f"rsmp-{index:04d}-zstd",
            "kind": "bytes",
            "variant": "direct_zstd_frame",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": row["direct_zstd"]["bytes"]},
        })
        records.append({
            "record_id": f"rsmp-{index:04d}-rsmp",
            "kind": "bytes",
            "variant": "rsmp_v1_adaptive",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": row["rsmp_archive"]["bytes"]},
        })
    production_clean = environment.get("git", {}).get("dirty") is False
    return write_run(
        repo_root, "rsmp-v1", "hw01-apple-m4-macos", "clean-regen-d11-r100",
        records, HW01_HARDWARE,
        {
            "rust_target": environment.get("platform", {}).get("target", "aarch64-apple-darwin"),
            "rustc": environment.get("platform", {}).get("rustc", "not recorded"),
            "build_profile": "release",
            "threads": "1 (zstd single_threaded contract)",
        },
        [
            "python3", "-m", "benchmarks.rstim_vs_stim_simulator.run_rsmp_compression",
            "--rstim", "target/release/rstim",
            "--catalog", "rstim/tests/fixtures/rsmp/catalog.json",
            "--case", "stim_surface_d11_r100",
            "--shots", "1024", "--seed", "7", "--zstd-level", "3",
            "--out-dir", "/tmp/rstim-rsmp-v1-evidence",
        ],
        [
            RSMP_DIR / "raw.jsonl",
            RSMP_DIR / "summary.json",
            RSMP_DIR / "report.md",
            RSMP_DIR / "environment.json",
            RSMP_DIR / "artifact-sha256.json",
        ],
        {
            "recorded": production_clean,
            "commit": environment.get("git", {}).get("commit"),
            "note": "regenerated from a clean tree with git.dirty=false (issue #601 RSMP provenance fix)"
            if production_clean else "legacy bundle recorded git.dirty=true; regenerate from a clean tree",
        },
        commit, dirty,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()
    commit = git_commit(repo_root)
    dirty = git_dirty_tracked(repo_root)
    if dirty:
        print("refusing to import: tracked working-tree changes present", file=sys.stderr)
        return 1
    for importer in (import_surface, import_bb, import_simulator, import_rsmp):
        run_dir = importer(repo_root, commit, dirty)
        print(f"wrote {run_dir.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
