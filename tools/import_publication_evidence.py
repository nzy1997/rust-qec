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
import shutil
import subprocess
import sys
import tempfile
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
    extra_environment: dict | None = None,
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
    environment = {
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
    }
    if extra_environment:
        environment.update(extra_environment)
    pub.write_json(run_dir / "environment.json", environment)
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


RSMP_MATRIX_SHOTS = (256, 1024, 4096)
RSMP_MATRIX_SEEDS = (7, 11, 17, 23, 31)
RSMP_OLD_RUN_DIR = (
    RESULTS_ROOT / "rsmp-v1" / "hw01-apple-m4-macos" / "clean-regen-d11-r100"
)


def rsmp_row_records(row: dict, record_prefix: str, seed: int | None) -> list[dict]:
    """Emit the publication records for one rsmp-v1 bundle row.

    Every row contributes the core codec comparison:

    - ``b8``: the canonical measurement bytes (identity baseline);
    - ``fixed_codec``: a direct level-3 Zstandard frame over the same b8
      bytes. This is the issue #601 fixed-codec ablation: it isolates the
      contribution of RSMP v1 adaptive syndrome encoding, because the only
      difference from ``rsmp_v1_adaptive`` is the codec, not the input;
    - ``rsmp_v1_adaptive``: the RSMP v1 archive.

    Rows that carry Stim-format baselines (issue #600) additionally contribute
    ``r8``/``ptb64`` and their own direct-Zstandard frames so every required
    baseline can be compared against the archive on the same input basis.
    """
    case_id = row["case_id"]
    shots = str(row["dimensions"]["shots"])
    raw_b8 = row["measurement_input"]["raw_b8_bytes"]
    scale = {"case_id": case_id, "shots": shots}
    records = [
        {
            "record_id": f"{record_prefix}-b8",
            "kind": "bytes",
            "variant": "b8",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": raw_b8},
        },
        {
            "record_id": f"{record_prefix}-fixed-codec",
            "kind": "bytes",
            "variant": "fixed_codec",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": row["direct_zstd"]["bytes"]},
        },
        {
            "record_id": f"{record_prefix}-rsmp",
            "kind": "bytes",
            "variant": "rsmp_v1_adaptive",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": row["rsmp_archive"]["bytes"]},
        },
    ]
    baselines = row.get("stim_baselines") or {}
    for fmt in ("r8", "ptb64"):
        entry = baselines.get(fmt)
        if entry is None:
            continue
        records.append({
            "record_id": f"{record_prefix}-{fmt}",
            "kind": "bytes",
            "variant": fmt,
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": entry["artifact"]["bytes"]},
        })
        records.append({
            "record_id": f"{record_prefix}-{fmt}-fixed-codec",
            "kind": "bytes",
            "variant": f"{fmt}_fixed_codec",
            "scale": scale,
            "seed": seed,
            "values": {"input_bytes": raw_b8, "output_bytes": entry["direct_zstd"]["bytes"]},
        })
    return records


def run_rsmp_matrix(repo_root: Path) -> tuple[list[dict], list[list[str]]]:
    """Run the declared RSMP shots x seed matrix with the merged #600 runner.

    Each cell samples the pinned d11/r100 benchmark circuit with the declared
    (shots, seed) pair and builds its evidence row through the runner's own
    ``build_row`` pipeline, so matrix cells get exactly the same pack/unpack,
    level-3 Zstandard, and b8/r8/ptb64 Stim-baseline treatment as the pinned
    canonical bundle. The runner's command-line entry point stays pinned to
    (1024, 7); matrix cells reuse its implementation, not a loosened CLI.
    """
    if str(repo_root) not in sys.path:
        sys.path.insert(0, str(repo_root))
    from benchmarks.rstim_vs_stim_simulator import run_rsmp_compression as rsmp_runner
    from tools import check_rsmp_v1_compression_evidence as rsmp_checker

    catalog = rsmp_checker.load_catalog_cases_from_path(
        repo_root / "rstim/tests/fixtures/rsmp/catalog.json"
    )
    catalog_case = catalog["surface_d11_r100"]
    circuit_path = str(catalog_case["circuit_path"])
    circuit_bytes = (repo_root / circuit_path).read_bytes()
    circuit_sha256 = str(catalog_case["circuit_sha256"])
    committed_benchmark_sha = None
    with (repo_root / RSMP_DIR / "raw.jsonl").open(encoding="utf-8") as handle:
        for line in handle:
            row = json.loads(line)
            if row.get("is_benchmark"):
                committed_benchmark_sha = row["measurement_input"]["sha256"]
                break

    records: list[dict] = []
    commands: list[list[str]] = []
    with tempfile.TemporaryDirectory(prefix="rstim-publication-rsmp-") as tmp:
        for shots in RSMP_MATRIX_SHOTS:
            for seed in RSMP_MATRIX_SEEDS:
                sample_argv = [
                    "target/release/rstim", "sample",
                    "--shots", str(shots),
                    "--seed", str(seed),
                    "--out_format", "b8",
                    "--in", circuit_path,
                ]
                result = subprocess.run(
                    sample_argv, cwd=repo_root, check=True,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                )
                row = rsmp_runner.build_row(
                    rstim="target/release/rstim",
                    stim="stim",
                    work_dir=Path(tmp),
                    case_id="stim_surface_d11_r100",
                    semantic_role="surface_d11_r100",
                    catalog_case_id="surface_d11_r100",
                    circuit_path=circuit_path,
                    circuit_bytes=circuit_bytes,
                    canonical_circuit_sha256=circuit_sha256,
                    shots=shots,
                    measurements_b8=result.stdout,
                    measurement_argv=sample_argv,
                    generator="rstim_sample",
                    generator_sha256=None,
                )
                commands.append(sample_argv)
                if (shots, seed) == (1024, 7) and committed_benchmark_sha is not None:
                    produced = row["measurement_input"]["sha256"]
                    if produced != committed_benchmark_sha:
                        raise RuntimeError(
                            "matrix cell (1024, 7) disagrees with the committed canonical "
                            f"bundle: measurement sha256 {produced} != {committed_benchmark_sha}"
                        )
                records.extend(rsmp_row_records(row, f"rsmp-s{shots}-seed{seed}", seed))
    return records, commands


def import_rsmp(repo_root: Path, commit: str, dirty: bool) -> Path:
    environment = json.loads((repo_root / RSMP_DIR / "environment.json").read_text(encoding="utf-8"))
    records = []
    with (repo_root / RSMP_DIR / "raw.jsonl").open(encoding="utf-8") as handle:
        rows = [json.loads(line) for line in handle]
    # Semantic fixtures and the high-entropy control are seed-independent and
    # imported once from the committed canonical bundle; the declared
    # shots x seed matrix for the benchmark case is regenerated below.
    for index, row in enumerate(rows, start=1):
        if row.get("is_benchmark"):
            continue
        records.extend(rsmp_row_records(row, f"rsmp-fix-{index:04d}", None))
    matrix_records, matrix_commands = run_rsmp_matrix(repo_root)
    records.extend(matrix_records)
    production_clean = environment.get("git", {}).get("dirty") is False
    stale_run_dir = repo_root / RSMP_OLD_RUN_DIR
    if stale_run_dir.is_dir():
        shutil.rmtree(stale_run_dir)
    return write_run(
        repo_root, "rsmp-v1", "hw01-apple-m4-macos", "d11-r100-matrix",
        records, HW01_HARDWARE,
        {
            "rust_target": environment.get("platform", {}).get("target", "aarch64-apple-darwin"),
            "rustc": environment.get("platform", {}).get("rustc", "not recorded"),
            "build_profile": "release",
            "threads": "1 (zstd single_threaded contract)",
        },
        ["python3", "tools/import_publication_evidence.py", "--repo-root", "."],
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
            "note": "benchmark-case rows regenerated from a clean tree for the declared "
            "shots x seed matrix with the merged #600 runner (issue #601 RSMP provenance fix)"
            if production_clean else "legacy bundle recorded git.dirty=true; regenerate from a clean tree",
        },
        commit, dirty,
        extra_environment={
            "matrix": {
                "case": "stim_surface_d11_r100",
                "shots": [str(shots) for shots in RSMP_MATRIX_SHOTS],
                "seeds": list(RSMP_MATRIX_SEEDS),
            },
            "matrix_commands": matrix_commands,
            "variant_notes": {
                "fixed_codec": "direct level-3 Zstandard frame over the same b8 bytes; "
                "the fixed-codec ablation isolating RSMP v1 adaptive syndrome encoding",
                "r8_fixed_codec": "direct level-3 Zstandard frame over the Stim r8 baseline bytes",
                "ptb64_fixed_codec": "direct level-3 Zstandard frame over the Stim ptb64 baseline bytes",
            },
        },
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
