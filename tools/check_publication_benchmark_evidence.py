#!/usr/bin/env python3
"""Validate the publication benchmark evidence bundle (issue #601).

The checker validates structural integrity of every committed run under
``benchmarks/publication_evidence/results/`` (hashes, clean provenance,
portable paths, summary estimates recomputed from raw records) and then
evaluates the publication readiness gates from
``benchmarks/publication_evidence/manifest.toml``.

Exit codes:

- 1: a structural defect was found, or ``--require-ready`` was given and one
  or more readiness gates report a gap.
- 0: all committed evidence is self-consistent. The final line is the full
  contract line when every readiness gate is ready, otherwise a ``PARTIAL``
  line listing the open gap count.

``--self-test`` builds a synthetic fully ready bundle plus the seven
calibrated negative-control fixtures in a temporary directory and checks
that the positive fixture passes and every negative fixture is rejected
with its specific reason.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import shutil
import statistics
import sys
import tempfile
import tomllib
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any

SCHEMA_VERSION = 1
CONFIDENCE = 0.95
DECLARED_SEEDS = (7, 11, 17, 23, 31)
MIN_SCALE_LEVELS = 3
MIN_HARDWARE_PROFILES = 2
MIN_WARMUPS = 3
MIN_TIMING_REPETITIONS = 10
REQUIRED_CPU_CLASSES = ("x86_64-linux", "aarch64")
FAMILY_IDS = (
    "surface-decoder-compare",
    "bb-circuit-bposd-compare",
    "rstim-vs-stim-simulator",
    "rsmp-v1",
    "qec-code-random-window",
)
RUN_FILES = ("raw.jsonl", "environment.json", "summary.json", "artifact-sha256.json")
BOOTSTRAP_RESAMPLES = 2000
BOOTSTRAP_SEED = 0x601
PASS_LINE = (
    "PASS publication benchmark evidence families=5 stochastic_seeds=5 "
    "performance_hardware_profiles>=2 min_scale_levels=3 confidence=0.95 "
    "clean_provenance=1"
)
SELF_TEST_PASS_LINE = "PASS publication benchmark checker positive=1 negative=7"
SELF_TEST_DIRTY_FAIL_LINE = "FAIL publication benchmark evidence: dirty provenance in rsmp-v1"

_FULL_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_POSIX_ABS_RE = re.compile(r"^/(?!/)")
_WINDOWS_ABS_RE = re.compile(r"^([A-Za-z]:[\\/]|\\\\)")


class CheckFailure(Exception):
    """A structural defect or (with require_ready) a readiness gap."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path: Path, payload: Any) -> None:
    path.write_text(json.dumps(payload, indent=1, sort_keys=True) + "\n", encoding="utf-8")


def round_float(value: float) -> float:
    return round(float(value), 12)


def canonical_scale_value(value: Any) -> Any:
    """Normalize scale levels so "0.01" and "0.010" compare equal."""
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return value
    raise CheckFailure(f"scale level {value!r} is not a string or number")


# ---------------------------------------------------------------------------
# Uncertainty math (recomputed by the checker from raw records)
# ---------------------------------------------------------------------------


def wilson_interval(errors: int, shots: int) -> tuple[float, float, float]:
    """Wilson score interval for a binomial rate at 95% confidence.

    The zero-error case yields the lower bound 0 and the Wilson upper bound,
    which is a valid binomial zero-error upper bound.
    """
    if shots <= 0:
        raise CheckFailure("logical-error record has non-positive shots")
    if errors < 0 or errors > shots:
        raise CheckFailure("logical-error record has errors outside [0, shots]")
    z = 1.959963984540054  # 97.5 percentile of the normal distribution
    p_hat = errors / shots
    denom = 1.0 + z * z / shots
    centre = p_hat + z * z / (2.0 * shots)
    margin = z * ((p_hat * (1.0 - p_hat) + z * z / (4.0 * shots)) / shots) ** 0.5
    low = max(0.0, (centre - margin) / denom)
    high = min(1.0, (centre + margin) / denom)
    return round_float(p_hat), round_float(low), round_float(high)


def bootstrap_median_interval(values: list[float]) -> tuple[float, float, float]:
    """Deterministic percentile bootstrap over independent repetitions."""
    if not values:
        raise CheckFailure("timing cell has no measured repetitions")
    ordered = sorted(float(v) for v in values)
    median = statistics.median(ordered)
    if len(ordered) == 1:
        return round_float(median), round_float(median), round_float(median)
    rng = random.Random(BOOTSTRAP_SEED)
    n = len(ordered)
    medians = []
    for _ in range(BOOTSTRAP_RESAMPLES):
        sample = [ordered[rng.randrange(n)] for _ in range(n)]
        medians.append(statistics.median(sample))
    medians.sort()
    low_index = max(0, int(0.025 * BOOTSTRAP_RESAMPLES))
    high_index = min(BOOTSTRAP_RESAMPLES - 1, int(0.975 * BOOTSTRAP_RESAMPLES))
    return round_float(median), round_float(medians[low_index]), round_float(medians[high_index])


# ---------------------------------------------------------------------------
# Loading and structural validation
# ---------------------------------------------------------------------------


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise CheckFailure("manifest root must be a TOML table")
    return manifest


def validate_manifest(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if manifest.get("schema") != SCHEMA_VERSION:
        raise CheckFailure(f"manifest schema must be {SCHEMA_VERSION}")
    contract = manifest.get("contract")
    if not isinstance(contract, dict):
        raise CheckFailure('manifest must define a "contract" table')
    if tuple(contract.get("declared_seeds", ())) != DECLARED_SEEDS:
        raise CheckFailure("manifest declared_seeds must be exactly 7, 11, 17, 23, 31")
    if float(contract.get("confidence", 0)) != CONFIDENCE:
        raise CheckFailure("manifest confidence must be 0.95")
    if int(contract.get("min_scale_levels", 0)) != MIN_SCALE_LEVELS:
        raise CheckFailure(f"manifest min_scale_levels must be {MIN_SCALE_LEVELS}")
    if int(contract.get("min_hardware_profiles", 0)) != MIN_HARDWARE_PROFILES:
        raise CheckFailure(f"manifest min_hardware_profiles must be {MIN_HARDWARE_PROFILES}")

    families: dict[str, dict[str, Any]] = {}
    for entry in manifest.get("families", []):
        if not isinstance(entry, dict) or not isinstance(entry.get("id"), str):
            raise CheckFailure("every [[families]] entry needs a string id")
        family_id = entry["id"]
        seed_policy = entry.get("seed_policy")
        if seed_policy not in ("declared", "not_applicable"):
            raise CheckFailure(f"family {family_id} seed_policy must be declared or not_applicable")
        axes = entry.get("scale_axes", [])
        for axis in axes:
            levels = axis.get("required_levels", [])
            if len(levels) < MIN_SCALE_LEVELS:
                raise CheckFailure(
                    f"family {family_id} scale axis {axis.get('name')!r} declares "
                    f"fewer than {MIN_SCALE_LEVELS} required levels"
                )
        families[family_id] = entry
    if tuple(families) != FAMILY_IDS:
        raise CheckFailure(f"manifest families must be exactly: {', '.join(FAMILY_IDS)}")
    return families


def is_portable_relative(value: str) -> bool:
    if not value or _POSIX_ABS_RE.search(value) or _WINDOWS_ABS_RE.search(value):
        return False
    posix = PurePosixPath(value)
    if posix.is_absolute() or PureWindowsPath(value).drive:
        return False
    return all(part not in ("", ".", "..") for part in value.split("/"))


def check_no_repo_absolute_strings(payload: Any, repo_root: Path, label: str) -> None:
    """Reject recorded paths that bake in the absolute worktree location."""
    root_text = str(repo_root)
    if isinstance(payload, str):
        if root_text in payload:
            raise CheckFailure(f"{label} records absolute worktree path: {payload[:80]}")
    elif isinstance(payload, list):
        for item in payload:
            check_no_repo_absolute_strings(item, repo_root, label)
    elif isinstance(payload, dict):
        for key, item in payload.items():
            check_no_repo_absolute_strings(item, repo_root, f"{label}.{key}")


def load_raw_records(path: Path, label: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CheckFailure(f"{label} raw.jsonl line {line_number} is not JSON: {exc}") from exc
            if not isinstance(record, dict):
                raise CheckFailure(f"{label} raw.jsonl line {line_number} must be an object")
            records.append(record)
    if not records:
        raise CheckFailure(f"{label} raw.jsonl has no records")
    return records


def validate_record(record: dict[str, Any], label: str) -> None:
    record_id = record.get("record_id")
    if not isinstance(record_id, str) or not record_id:
        raise CheckFailure(f"{label} record is missing a record_id")
    kind = record.get("kind")
    if kind not in ("logical_error", "timing", "bytes", "scalar"):
        raise CheckFailure(f"{label} record {record_id} has unknown kind {kind!r}")
    if not isinstance(record.get("variant"), str) or not record["variant"]:
        raise CheckFailure(f"{label} record {record_id} is missing a variant")
    scale = record.get("scale")
    if not isinstance(scale, dict) or not scale:
        raise CheckFailure(f"{label} record {record_id} is missing a scale object")
    for axis, level in scale.items():
        canonical_scale_value(level)
    seed = record.get("seed")
    if seed is not None and not isinstance(seed, int):
        raise CheckFailure(f"{label} record {record_id} seed must be an integer or null")
    values = record.get("values")
    if not isinstance(values, dict):
        raise CheckFailure(f"{label} record {record_id} is missing a values object")
    if kind == "logical_error":
        shots = values.get("shots")
        errors = values.get("logical_errors")
        if not isinstance(shots, int) or not isinstance(errors, int):
            raise CheckFailure(f"{label} record {record_id} needs integer shots and logical_errors")
    elif kind == "timing":
        elapsed = values.get("elapsed_ns")
        if not isinstance(elapsed, (int, float)) or elapsed <= 0:
            raise CheckFailure(f"{label} record {record_id} needs a positive elapsed_ns")
        if record.get("phase") not in ("warmup", "measured"):
            raise CheckFailure(f"{label} record {record_id} timing phase must be warmup or measured")
    elif kind == "bytes":
        if not isinstance(values.get("input_bytes"), int) or not isinstance(values.get("output_bytes"), int):
            raise CheckFailure(f"{label} record {record_id} needs integer input_bytes and output_bytes")


def group_key(record: dict[str, Any]) -> str:
    scale = record["scale"]
    axes = ",".join(f"{axis}={scale[axis]}" for axis in sorted(scale))
    return f'{record["kind"]}|{record["variant"]}|{axes}'


def derive_estimates(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Derive summary estimates from raw records (checked against summary.json)."""
    groups: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        groups.setdefault(group_key(record), []).append(record)
    estimates: list[dict[str, Any]] = []
    for key in sorted(groups):
        group = groups[key]
        kind = group[0]["kind"]
        variant = group[0]["variant"]
        scale = {axis: group[0]["scale"][axis] for axis in sorted(group[0]["scale"])}
        ids = sorted(record["record_id"] for record in group)
        if kind == "logical_error":
            shots = sum(record["values"]["shots"] for record in group)
            errors = sum(record["values"]["logical_errors"] for record in group)
            point, low, high = wilson_interval(errors, shots)
            estimates.append({
                "group": key, "kind": kind, "variant": variant, "scale": scale,
                "point": point, "ci_low": low, "ci_high": high, "n": shots,
                "method": "wilson_score_95", "raw_records": ids,
            })
        elif kind == "timing":
            measured = [record for record in group if record["phase"] == "measured"]
            warmups = [record for record in group if record["phase"] == "warmup"]
            if measured:
                point, low, high = bootstrap_median_interval(
                    [record["values"]["elapsed_ns"] for record in measured]
                )
                estimates.append({
                    "group": key, "kind": kind, "variant": variant, "scale": scale,
                    "point": point, "ci_low": low, "ci_high": high, "n": len(measured),
                    "warmups": len(warmups),
                    "method": "bootstrap_percentile_95",
                    "raw_records": sorted(record["record_id"] for record in measured),
                })
        elif kind == "bytes":
            input_bytes = sum(record["values"]["input_bytes"] for record in group)
            output_bytes = sum(record["values"]["output_bytes"] for record in group)
            ratio = round_float(output_bytes / input_bytes) if input_bytes else 0.0
            estimates.append({
                "group": key, "kind": kind, "variant": variant, "scale": scale,
                "point": ratio, "ci_low": ratio, "ci_high": ratio, "n": len(group),
                "method": "deterministic_ratio", "raw_records": ids,
            })
        else:  # scalar
            point, low, high = bootstrap_median_interval(
                [float(record["values"]["value"]) for record in group]
            )
            estimates.append({
                "group": key, "kind": kind, "variant": variant, "scale": scale,
                "point": point, "ci_low": low, "ci_high": high, "n": len(group),
                "method": "bootstrap_percentile_95", "raw_records": ids,
            })
    return estimates


def validate_run(run_dir: Path, family_id: str, hardware_id: str, run_id: str, repo_root: Path) -> dict[str, Any]:
    label = f"{family_id}/{hardware_id}/{run_id}"
    for name in RUN_FILES:
        if not (run_dir / name).is_file():
            raise CheckFailure(f"{label} is missing {name}")

    hashes = load_json(run_dir / "artifact-sha256.json")
    if not isinstance(hashes, dict):
        raise CheckFailure(f"{label} artifact-sha256.json must be an object")
    expected = {name: sha256_file(run_dir / name) for name in RUN_FILES if name != "artifact-sha256.json"}
    if set(hashes) != set(expected):
        raise CheckFailure(f"{label} artifact-sha256.json must hash exactly {sorted(expected)}")
    for name, digest in expected.items():
        if hashes[name] != digest:
            raise CheckFailure(f"{label} artifact hash mismatch for {name}")

    environment = load_json(run_dir / "environment.json")
    if environment.get("schema") != "publication-environment-v1":
        raise CheckFailure(f"{label} environment.json schema must be publication-environment-v1")
    if environment.get("family") != family_id:
        raise CheckFailure(f"{label} environment family must be {family_id}")
    if environment.get("hardware_id") != hardware_id or environment.get("run_id") != run_id:
        raise CheckFailure(f"{label} environment hardware_id/run_id must match the directory layout")
    git = environment.get("git")
    if not isinstance(git, dict) or not _FULL_SHA_RE.match(str(git.get("commit", ""))):
        raise CheckFailure(f"{label} environment must record a full 40-hex source commit")
    if git.get("dirty") is not False:
        raise CheckFailure(f"dirty provenance in {family_id}")
    hardware = environment.get("hardware")
    if not isinstance(hardware, dict):
        raise CheckFailure(f"{label} environment must record a hardware object")
    for field in ("cpu_model", "cpu_class", "os"):
        if not isinstance(hardware.get(field), str) or not hardware[field]:
            raise CheckFailure(f"{label} environment hardware is missing {field}")
    toolchain = environment.get("toolchain")
    if not isinstance(toolchain, dict) or not isinstance(toolchain.get("build_profile"), str):
        raise CheckFailure(f"{label} environment must record toolchain with build_profile")
    argv = environment.get("argv")
    if not isinstance(argv, list) or not argv or any(not isinstance(part, str) for part in argv):
        raise CheckFailure(f"{label} environment must record the complete argv")
    check_no_repo_absolute_strings(argv, repo_root, f"{label} argv")
    sources = environment.get("source_artifacts")
    if not isinstance(sources, list) or not sources:
        raise CheckFailure(f"{label} environment must record source_artifacts")
    for entry in sources:
        if not isinstance(entry, dict):
            raise CheckFailure(f"{label} source_artifacts entries must be objects")
        path_value = entry.get("path")
        digest = entry.get("sha256")
        if not isinstance(path_value, str) or not is_portable_relative(path_value):
            raise CheckFailure(f"{label} source artifact path must be a portable relative path: {path_value!r}")
        if not isinstance(digest, str) or not _SHA256_RE.match(digest):
            raise CheckFailure(f"{label} source artifact {path_value} needs a sha256")
        source_path = repo_root / path_value
        if not source_path.is_file():
            raise CheckFailure(f"{label} source artifact {path_value} is missing from the repository")
        if sha256_file(source_path) != digest:
            raise CheckFailure(f"{label} source artifact hash mismatch for {path_value}")

    records = load_raw_records(run_dir / "raw.jsonl", label)
    seen_ids: set[str] = set()
    for record in records:
        validate_record(record, label)
        if record["record_id"] in seen_ids:
            raise CheckFailure(f"{label} duplicate record_id {record['record_id']}")
        seen_ids.add(record["record_id"])

    summary = load_json(run_dir / "summary.json")
    if summary.get("schema") != "publication-summary-v1":
        raise CheckFailure(f"{label} summary.json schema must be publication-summary-v1")
    if summary.get("family") != family_id or summary.get("hardware_id") != hardware_id or summary.get("run_id") != run_id:
        raise CheckFailure(f"{label} summary identity fields must match the directory layout")
    derived = derive_estimates(records)
    recorded = summary.get("estimates")
    if recorded != derived:
        raise CheckFailure(f"{label} summary estimates are not derivable from raw.jsonl")
    return {
        "family": family_id,
        "hardware_id": hardware_id,
        "run_id": run_id,
        "records": records,
        "estimates": derived,
        "environment": environment,
    }


def discover_runs(results_dir: Path) -> list[tuple[str, str, str, Path]]:
    runs = []
    if not results_dir.is_dir():
        return runs
    for family_dir in sorted(results_dir.iterdir()):
        if not family_dir.is_dir():
            continue
        for hardware_dir in sorted(family_dir.iterdir()):
            if not hardware_dir.is_dir():
                continue
            for run_dir in sorted(hardware_dir.iterdir()):
                if run_dir.is_dir():
                    runs.append((family_dir.name, hardware_dir.name, run_dir.name, run_dir))
    return runs


# ---------------------------------------------------------------------------
# Readiness gates
# ---------------------------------------------------------------------------


def evaluate_readiness(
    manifest: dict[str, Any],
    families: dict[str, dict[str, Any]],
    runs: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    gates: list[dict[str, Any]] = []

    def gate(gate_id: str, description: str, ok: bool, detail: str) -> None:
        gates.append({
            "id": gate_id,
            "description": description,
            "status": "ready" if ok else "gap",
            "detail": detail,
        })

    by_family: dict[str, list[dict[str, Any]]] = {family_id: [] for family_id in FAMILY_IDS}
    for run in runs:
        by_family.setdefault(run["family"], []).append(run)

    missing = [family_id for family_id in FAMILY_IDS if not by_family.get(family_id)]
    gate(
        "families",
        "all five benchmark families contribute at least one checked run",
        not missing,
        "all five families have runs" if not missing else f"families without runs: {', '.join(missing)}",
    )

    for family_id in FAMILY_IDS:
        family = families[family_id]
        family_runs = by_family.get(family_id, [])
        records = [record for run in family_runs for record in run["records"]]
        variants = {record["variant"] for record in records}

        if family.get("seed_policy") == "declared":
            present = {record["seed"] for record in records if record.get("seed") is not None}
            missing_seeds = [seed for seed in DECLARED_SEEDS if seed not in present]
            gate(
                f"seeds:{family_id}",
                f"family {family_id} covers the declared seed set {list(DECLARED_SEEDS)}",
                not missing_seeds,
                "all declared seeds present" if not missing_seeds else f"missing declared seeds: {missing_seeds}",
            )

        for axis in family.get("scale_axes", []):
            name = axis["name"]
            required = {canonical_scale_value(level) for level in axis["required_levels"]}
            measured = {
                canonical_scale_value(record["scale"][name])
                for record in records
                if name in record["scale"]
            }
            missing_levels = sorted(required - measured, key=str)
            gate(
                f"scale:{family_id}:{name}",
                f"family {family_id} measures required levels of scale axis {name}",
                not missing_levels,
                f"measured levels cover the contract" if not missing_levels else f"missing scale levels: {missing_levels}",
            )

        required_baselines = set(family.get("required_baselines", []))
        missing_baselines = sorted(required_baselines - variants)
        gate(
            f"baselines:{family_id}",
            f"family {family_id} includes all required baselines",
            not missing_baselines,
            "all required baselines present" if not missing_baselines else f"missing required baselines: {missing_baselines}",
        )

        required_ablations = set(family.get("required_ablations", []))
        missing_ablations = sorted(required_ablations - variants)
        gate(
            f"ablations:{family_id}",
            f"family {family_id} includes all required ablations",
            not missing_ablations,
            "all required ablations present" if not missing_ablations else f"missing required ablations: {missing_ablations}",
        )

        if family_runs:
            unrecorded = [
                f"{run['hardware_id']}/{run['run_id']}"
                for run in family_runs
                if run["environment"].get("production_provenance", {}).get("recorded") is not True
            ]
            gate(
                f"provenance:{family_id}",
                f"family {family_id} runs record complete original production provenance",
                not unrecorded,
                "all runs record original production commit, hardware, and argv"
                if not unrecorded
                else f"runs without original production provenance (legacy imports): {', '.join(unrecorded)}",
            )

        if family.get("timing"):
            timing_runs = [run for run in family_runs if any(r["kind"] == "timing" for r in run["records"])]
            identified_hosts = {
                run["hardware_id"]
                for run in timing_runs
                if run["environment"].get("hardware", {}).get("identified") is True
            }
            cpu_classes = {
                run["environment"]["hardware"]["cpu_class"]
                for run in timing_runs
                if run["environment"].get("hardware", {}).get("identified") is True
            }
            has_x86 = any("x86_64" in cpu_class and "linux" in cpu_class for cpu_class in cpu_classes)
            has_arm = any("aarch64" in cpu_class for cpu_class in cpu_classes)
            ok = len(identified_hosts) >= MIN_HARDWARE_PROFILES and has_x86 and has_arm
            detail = (
                f"{len(identified_hosts)} identified timing hosts covering x86_64-linux and aarch64"
                if ok
                else f"only {len(identified_hosts)} identified timing host(s); need >= {MIN_HARDWARE_PROFILES} physical hosts spanning x86_64-linux and aarch64"
            )
            gate(
                f"hardware:{family_id}",
                f"family {family_id} wall-clock claims span two identified physical hosts",
                ok,
                detail,
            )

            bad_cells = []
            for run in timing_runs:
                for estimate in run["estimates"]:
                    if estimate["kind"] != "timing":
                        continue
                    warmups = estimate.get("warmups", 0)
                    if warmups < MIN_WARMUPS or estimate["n"] < MIN_TIMING_REPETITIONS:
                        bad_cells.append(
                            f"{run['run_id']}:{estimate['group']} (warmups={warmups}, measured={estimate['n']})"
                        )
            gate(
                f"timing-protocol:{family_id}",
                f"family {family_id} timing cells use >= {MIN_WARMUPS} warmups and >= {MIN_TIMING_REPETITIONS} measured repetitions",
                not bad_cells,
                "all timing cells meet the repetition protocol" if not bad_cells else f"cells below protocol: {'; '.join(bad_cells)}",
            )

    return gates


# ---------------------------------------------------------------------------
# Report rendering
# ---------------------------------------------------------------------------


def render_readiness(manifest: dict[str, Any], gates: list[dict[str, Any]], runs: list[dict[str, Any]]) -> dict[str, Any]:
    gaps = [gate for gate in gates if gate["status"] == "gap"]
    return {
        "schema": "publication-readiness-v1",
        "bundle": manifest.get("bundle"),
        "publication_ready": not gaps,
        "gate_count": len(gates),
        "gap_count": len(gaps),
        "gates": gates,
        "runs": [
            {
                "family": run["family"],
                "hardware_id": run["hardware_id"],
                "run_id": run["run_id"],
                "record_count": len(run["records"]),
                "estimate_count": len(run["estimates"]),
            }
            for run in runs
        ],
        "external_dependencies": manifest.get("external_dependencies", []),
    }


def render_report(manifest: dict[str, Any], gates: list[dict[str, Any]], runs: list[dict[str, Any]]) -> str:
    lines = [
        "# Publication benchmark evidence report",
        "",
        f"Bundle: `{manifest.get('bundle')}` (schema {SCHEMA_VERSION}, confidence {CONFIDENCE}).",
        "",
        "This report is generated by `tools/check_publication_benchmark_evidence.py` from the",
        "committed raw records under `benchmarks/publication_evidence/results/`. Do not hand-edit;",
        "the checker hashes and regenerates this file.",
        "",
        "## Readiness",
        "",
    ]
    gaps = [gate for gate in gates if gate["status"] == "gap"]
    if gaps:
        lines.append(f"Publication readiness: **not ready** ({len(gaps)} of {len(gates)} gates report gaps).")
    else:
        lines.append(f"Publication readiness: **ready** (all {len(gates)} gates satisfied).")
    lines += ["", "| Gate | Status | Detail |", "|---|---|---|"]
    for gate in gates:
        lines.append(f"| {gate['id']} | {gate['status']} | {gate['detail']} |")
    lines += ["", "## Runs", ""]
    if not runs:
        lines.append("No runs are committed yet.")
    for run in runs:
        lines.append(
            f"- `{run['family']}/{run['hardware_id']}/{run['run_id']}`: "
            f"{len(run['records'])} raw records, {len(run['estimates'])} derived estimates."
        )
    lines += ["", "## Estimates", ""]
    for run in runs:
        lines.append(f"### {run['family']} / {run['hardware_id']} / {run['run_id']}")
        lines.append("")
        lines.append("| Group | Point | 95% CI | n | Method |")
        lines.append("|---|---|---|---|---|")
        for estimate in run["estimates"]:
            lines.append(
                f"| {estimate['group']} | {estimate['point']} | "
                f"[{estimate['ci_low']}, {estimate['ci_high']}] | {estimate['n']} | {estimate['method']} |"
            )
        lines.append("")
    lines += [
        "## Claim limits",
        "",
    ]
    for note in manifest.get("claim_limits", []):
        lines.append(f"- {note}")
    lines += ["", "## External dependencies", ""]
    for note in manifest.get("external_dependencies", []):
        lines.append(f"- {note}")
    lines.append("")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Top-level validation
# ---------------------------------------------------------------------------


def validate_bundle(manifest_path: Path, results_dir: Path, require_ready: bool) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    manifest = load_manifest(manifest_path)
    families = validate_manifest(manifest)
    repo_root = manifest_path.resolve().parents[2]
    runs = []
    for family_id, hardware_id, run_id, run_dir in discover_runs(results_dir):
        if family_id not in families:
            raise CheckFailure(f"results directory contains unknown family {family_id!r}")
        runs.append(validate_run(run_dir, family_id, hardware_id, run_id, repo_root))
    gates = evaluate_readiness(manifest, families, runs)
    if require_ready:
        gaps = [gate for gate in gates if gate["status"] == "gap"]
        if gaps:
            first = gaps[0]
            raise CheckFailure(f"readiness gate {first['id']} not satisfied: {first['detail']}")
    return manifest, gates, runs


def run_checker(args: argparse.Namespace) -> int:
    manifest_path = Path(args.manifest)
    results_dir = Path(args.results_dir)
    try:
        manifest, gates, runs = validate_bundle(manifest_path, results_dir, args.require_ready)
    except CheckFailure as exc:
        print(f"FAIL publication benchmark evidence: {exc}")
        return 1

    readiness = render_readiness(manifest, gates, runs)
    report = render_report(manifest, gates, runs)
    if args.report_out:
        Path(args.report_out).write_text(report, encoding="utf-8")
    if args.readiness_out:
        Path(args.readiness_out).write_text(json.dumps(readiness, indent=1, sort_keys=True) + "\n", encoding="utf-8")

    gaps = [gate for gate in gates if gate["status"] == "gap"]
    if gaps:
        print(
            "PARTIAL publication benchmark evidence families=5 structural=clean "
            f"publication_ready=0 gaps={len(gaps)}"
        )
    else:
        print(PASS_LINE)
    return 0


# ---------------------------------------------------------------------------
# Self-test fixtures
# ---------------------------------------------------------------------------

_SELFTEST_COMMIT = "b0869db93ca81254e680743230418b6a1089f0c0"


def _selftest_hardware(cpu_class: str, identified: bool = True) -> dict[str, Any]:
    if "x86_64" in cpu_class:
        return {
            "cpu_model": "Synthetic x86_64 Linux CPU",
            "cpu_class": "x86_64-linux",
            "physical_cores": 16,
            "logical_cores": 32,
            "ram_gb": 64,
            "os": "Linux 6.8.0",
            "identified": identified,
        }
    return {
        "cpu_model": "Synthetic aarch64 Darwin CPU",
        "cpu_class": "aarch64-apple-darwin",
        "physical_cores": 10,
        "logical_cores": 10,
        "ram_gb": 32,
        "os": "Darwin 25.0.0",
        "identified": identified,
    }


def _selftest_manifest(root: Path, hardware_profiles: list[dict[str, Any]]) -> Path:
    families_toml = []
    specs = {
        "surface-decoder-compare": {
            "axis": ("distance", ["3", "5", "7"]),
            "baselines": ["pymatching", "ldpc_bposd", "ilpqec"],
            "ablations": ["rbposd_lsd_order1", "rbposd_product_sum_serial"],
            "timing": True,
        },
        "bb-circuit-bposd-compare": {
            "axis": ("code_id", ["bb72", "bb90", "bb144"]),
            "baselines": ["ldpc_bposd", "bravyi_reference"],
            "ablations": ["fixed_shot_reference"],
            "timing": False,
        },
        "rstim-vs-stim-simulator": {
            "axis": ("workload", ["d3_r3", "d7_r7", "d11_r100"]),
            "baselines": ["stim_cli"],
            "ablations": ["frame_noise_baseline"],
            "timing": True,
        },
        "rsmp-v1": {
            "axis": ("shots", ["256", "1024", "4096"]),
            "baselines": ["b8", "r8", "ptb64"],
            "ablations": ["fixed_codec"],
            "timing": False,
        },
        "qec-code-random-window": {
            "axis": ("case_id", ["case_a", "case_b", "case_c"]),
            "baselines": ["codedistance_pypi"],
            "ablations": ["no_pruning"],
            "timing": False,
        },
    }
    for family_id, spec in specs.items():
        axis_name, levels = spec["axis"]
        level_text = ", ".join(json.dumps(level) for level in levels)
        baselines = ", ".join(json.dumps(b) for b in spec["baselines"])
        ablations = ", ".join(json.dumps(a) for a in spec["ablations"])
        families_toml.append(
            f"""
[[families]]
id = "{family_id}"
seed_policy = "declared"
timing = {str(spec["timing"]).lower()}
required_baselines = [{baselines}]
required_ablations = [{ablations}]

[[families.scale_axes]]
name = "{axis_name}"
required_levels = [{level_text}]
"""
        )
    manifest_text = f"""schema = 1
bundle = "selftest-publication-evidence"

[contract]
declared_seeds = [7, 11, 17, 23, 31]
confidence = 0.95
min_scale_levels = 3
min_hardware_profiles = 2
min_warmups = 3
min_timing_repetitions = 10
{''.join(families_toml)}"""
    path = root / "manifest.toml"
    path.write_text(manifest_text, encoding="utf-8")
    return path


def _selftest_records(family_id: str, axis_name: str, levels: list[str], spec_timing: bool) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    variants = ["candidate"] + list(_SELFTEST_VARIANT_EXTRAS[family_id])
    counter = 0

    def next_id() -> str:
        nonlocal counter
        counter += 1
        return f"{family_id}-r{counter:04d}"

    for level in levels:
        for seed in DECLARED_SEEDS:
            for variant in variants:
                records.append({
                    "record_id": next_id(),
                    "kind": "logical_error",
                    "variant": variant,
                    "scale": {axis_name: level},
                    "seed": seed,
                    "values": {"shots": 1000, "logical_errors": 25},
                })
        if spec_timing:
            for variant in variants:
                for index in range(MIN_WARMUPS):
                    records.append({
                        "record_id": next_id(),
                        "kind": "timing",
                        "variant": variant,
                        "scale": {axis_name: level},
                        "seed": None,
                        "phase": "warmup",
                        "repetition": index,
                        "values": {"elapsed_ns": 1_000_000 + index},
                    })
                for index in range(MIN_TIMING_REPETITIONS):
                    records.append({
                        "record_id": next_id(),
                        "kind": "timing",
                        "variant": variant,
                        "scale": {axis_name: level},
                        "seed": None,
                        "phase": "measured",
                        "repetition": index,
                        "values": {"elapsed_ns": 2_000_000 + 1000 * index},
                    })
    return records


_SELFTEST_VARIANT_EXTRAS = {
    "surface-decoder-compare": ["pymatching", "ldpc_bposd", "ilpqec", "rbposd_lsd_order1", "rbposd_product_sum_serial"],
    "bb-circuit-bposd-compare": ["ldpc_bposd", "bravyi_reference", "fixed_shot_reference"],
    "rstim-vs-stim-simulator": ["stim_cli", "frame_noise_baseline"],
    "rsmp-v1": ["b8", "r8", "ptb64", "fixed_codec"],
    "qec-code-random-window": ["codedistance_pypi", "no_pruning"],
}

_SELFTEST_AXES = {
    "surface-decoder-compare": ("distance", ["3", "5", "7"], True),
    "bb-circuit-bposd-compare": ("code_id", ["bb72", "bb90", "bb144"], False),
    "rstim-vs-stim-simulator": ("workload", ["d3_r3", "d7_r7", "d11_r100"], True),
    "rsmp-v1": ("shots", ["256", "1024", "4096"], False),
    "qec-code-random-window": ("case_id", ["case_a", "case_b", "case_c"], False),
}


def _selftest_write_run(
    root: Path,
    repo_source: Path,
    family_id: str,
    hardware_id: str,
    cpu_class: str,
    records: list[dict[str, Any]],
    dirty: bool = False,
) -> Path:
    run_dir = root / "benchmarks" / "publication_evidence" / "results" / family_id / hardware_id / "run-0001"
    run_dir.mkdir(parents=True, exist_ok=True)
    full_records = list(records)
    with (run_dir / "raw.jsonl").open("w", encoding="utf-8") as handle:
        for record in full_records:
            handle.write(json.dumps(record, sort_keys=True) + "\n")
    estimates = derive_estimates(full_records)
    write_json(run_dir / "summary.json", {
        "schema": "publication-summary-v1",
        "family": family_id,
        "hardware_id": hardware_id,
        "run_id": "run-0001",
        "estimates": estimates,
    })
    write_json(run_dir / "environment.json", {
        "schema": "publication-environment-v1",
        "family": family_id,
        "hardware_id": hardware_id,
        "run_id": "run-0001",
        "git": {"commit": _SELFTEST_COMMIT, "dirty": dirty},
        "hardware": _selftest_hardware(cpu_class),
        "toolchain": {
            "rust_target": cpu_class,
            "rustc": "rustc 1.93.1 (selftest)",
            "build_profile": "release",
            "threads": 1,
        },
        "argv": ["selftest://run", "--family", family_id],
        "production_provenance": {"recorded": True, "note": "synthetic selftest run"},
        "source_artifacts": [{
            "path": repo_source.name,
            "sha256": sha256_file(repo_source),
        }],
    })
    write_json(run_dir / "artifact-sha256.json", {
        name: sha256_file(run_dir / name) for name in RUN_FILES if name != "artifact-sha256.json"
    })
    return run_dir


def _selftest_build(root: Path, mutation: str | None) -> Path:
    """Build a synthetic fully ready bundle; apply a negative-control mutation."""
    bundle_dir = root / "benchmarks" / "publication_evidence"
    bundle_dir.mkdir(parents=True, exist_ok=True)
    repo_source = root / "source-artifact.json"
    repo_source.write_text(json.dumps({"selftest": True}), encoding="utf-8")
    hardware_profiles = [
        ("hw-x86-linux", "x86_64-linux"),
        ("hw-arm-darwin", "aarch64-apple-darwin"),
    ]
    if mutation == "one-hardware-profile":
        hardware_profiles = hardware_profiles[:1]
    manifest_path = _selftest_manifest(bundle_dir, hardware_profiles)

    for family_id, (axis_name, levels, spec_timing) in _SELFTEST_AXES.items():
        family_levels = list(levels)
        if mutation == "fewer-scale-levels" and family_id == "bb-circuit-bposd-compare":
            family_levels = levels[:2]
        if mutation == "missing-baseline" and family_id == "bb-circuit-bposd-compare":
            extras = _SELFTEST_VARIANT_EXTRAS[family_id]
            _SELFTEST_VARIANT_EXTRAS[family_id] = [v for v in extras if v != "ldpc_bposd"]
        if mutation == "missing-ablation" and family_id == "surface-decoder-compare":
            extras = _SELFTEST_VARIANT_EXTRAS[family_id]
            _SELFTEST_VARIANT_EXTRAS[family_id] = [v for v in extras if v != "rbposd_lsd_order1"]
        records = _selftest_records(family_id, axis_name, family_levels, spec_timing)
        if mutation == "missing-seed" and family_id == "surface-decoder-compare":
            records = [r for r in records if r.get("seed") != 31]
        dirty = mutation == "dirty-provenance" and family_id == "rsmp-v1"
        hosts = hardware_profiles if spec_timing else hardware_profiles[:1]
        for hardware_id, cpu_class in hosts:
            _selftest_write_run(root, repo_source, family_id, hardware_id, cpu_class, records, dirty=dirty)

    if mutation == "tampered-summary":
        summary_path = root / "benchmarks" / "publication_evidence" / "results" / "rsmp-v1" / "hw-x86-linux" / "run-0001" / "summary.json"
        summary = load_json(summary_path)
        summary["estimates"][0]["ci_high"] = round_float(summary["estimates"][0]["ci_high"] + 0.5)
        write_json(summary_path, summary)
        write_json(summary_path.parent / "artifact-sha256.json", {
            name: sha256_file(summary_path.parent / name) for name in RUN_FILES if name != "artifact-sha256.json"
        })
    return manifest_path


def run_self_test() -> int:
    mutations = [
        ("dirty-provenance", "dirty provenance in rsmp-v1"),
        ("missing-seed", "missing declared seeds"),
        ("one-hardware-profile", "identified timing host"),
        ("fewer-scale-levels", "missing scale levels"),
        ("missing-baseline", "missing required baselines"),
        ("missing-ablation", "missing required ablations"),
        ("tampered-summary", "not derivable from raw.jsonl"),
    ]
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="publication-evidence-selftest-") as temp:
        base = Path(temp)

        # Positive control: fully ready synthetic bundle must pass cleanly.
        positive_root = base / "positive"
        positive_root.mkdir()
        manifest_path = _selftest_build(positive_root, None)
        try:
            manifest, gates, runs = validate_bundle(
                manifest_path,
                positive_root / "benchmarks" / "publication_evidence" / "results",
                require_ready=True,
            )
            gaps = [gate for gate in gates if gate["status"] == "gap"]
            if gaps:
                failures.append(f"positive fixture reported gaps: {[g['id'] for g in gaps]}")
        except CheckFailure as exc:
            failures.append(f"positive fixture rejected: {exc}")

        # Negative controls: every mutation must be rejected with its reason.
        for name, reason in mutations:
            mutation_root = base / name
            mutation_root.mkdir()
            manifest_path = _selftest_build(mutation_root, name)
            try:
                validate_bundle(
                    manifest_path,
                    mutation_root / "benchmarks" / "publication_evidence" / "results",
                    require_ready=True,
                )
                failures.append(f"negative fixture {name} was not rejected")
            except CheckFailure as exc:
                if reason not in str(exc):
                    failures.append(f"negative fixture {name} rejected with wrong reason: {exc}")

    if failures:
        for failure in failures:
            print(f"FAIL publication benchmark checker self-test: {failure}")
        return 1
    print(SELF_TEST_PASS_LINE)
    return 0


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default="benchmarks/publication_evidence/manifest.toml")
    parser.add_argument("--results-dir", default="benchmarks/publication_evidence/results")
    parser.add_argument("--report-out")
    parser.add_argument("--readiness-out")
    parser.add_argument("--require-ready", action="store_true",
                        help="exit 1 when any publication readiness gate reports a gap")
    parser.add_argument("--self-test", action="store_true",
                        help="run the calibrated positive/negative fixture self-test")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        return run_self_test()
    return run_checker(args)


if __name__ == "__main__":
    sys.exit(main())
