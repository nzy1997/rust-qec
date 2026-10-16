#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import tools.check_site_manifest as check_site_manifest
import tools.copy_site_benchmark_data as copy_site_benchmark_data

PROVENANCE_NOT_RECORDED_REASON = "historical fixture predates canonical provenance capture"
SURFACE_RESULTS_PATH = "benchmarks/surface_decoder_compare/results/full/results.csv"
SURFACE_IMAGE_PATH = "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png"
SURFACE_RESULTS_SHA256 = "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
SURFACE_IMAGE_SHA256 = "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"
RSTIM_SPEED_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
RSTIM_SPEED_REPORT_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md"
RSTIM_CORRECTNESS_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json"
RSTIM_DISTRIBUTIONS_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json"
RSTIM_DISTRIBUTIONS_EXPANDED_CORRECTNESS_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json"
)
RSTIM_DISTRIBUTIONS_REPORT_PATH = "benchmarks/rstim_vs_stim_simulator/results/distributions/report.md"
RSTIM_RELEASE_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/release/summary.json"
RSTIM_RELEASE_REPORT_PATH = "benchmarks/rstim_vs_stim_simulator/results/release/report.md"
RSTIM_RELEASE_ENVIRONMENT_PATH = "benchmarks/rstim_vs_stim_simulator/results/release/environment.json"
RSTIM_REPETITION_SUMMARY_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json"
)
RSTIM_REPETITION_REPORT_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md"
)
RSTIM_REPETITION_ENVIRONMENT_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/environment.json"
)
RSTIM_SURFACE_DETECT_SUMMARY_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/summary.json"
)
RSTIM_SURFACE_DETECT_REPORT_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/report.md"
)
RSTIM_SURFACE_DETECT_ENVIRONMENT_PATH = (
    "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/environment.json"
)
RSTIM_DEM_RAW_PATH = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/raw.jsonl"
RSTIM_DEM_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/summary.json"
RSTIM_DEM_REPORT_PATH = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/report.md"
RSTIM_DEM_ENVIRONMENT_PATH = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/environment.json"
RSTIM_CASES_FULL_PATH = "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
RSTIM_CANONICAL_STIM_PATH = (
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
)
RSTIM_SHOWCASE_PATH = "docs/showcases/rstim-vs-stim-simulator.md"
RSTIM_SPEED_SUMMARY_SHA256 = "068c6cda6256254832b1f07979a475a1d747288cbdfaae6291e03697c2b3261d"
RSTIM_SPEED_REPORT_SHA256 = "ad2ce5a1a049d02dc3ef15ec90609362b12e580c172fe8a13f6c16071c73a2f4"
RSTIM_CORRECTNESS_SUMMARY_SHA256 = "423b0a945a73ecb5ab748c7c796af9328a4639bf6921af982e71ad00924f46e9"
RSTIM_CASES_FULL_SHA256 = "f86f77dff5135b7273d64aa8fd01a8d55901e2222a175ae97922db423cabccd6"
RSTIM_CANONICAL_STIM_SHA256 = "efb8217cc5ffbb305255ac47281b17964df5cf6cb2268e63450f06ce0e001fdb"
RSTIM_SHOWCASE_SHA256 = "382c8ba936ac311bfbf2b2d3da55618cd551f2840a4f284f12980986f992a72b"
RSTIM_EXPANDED_FIXTURE_CONTENT = '{"status":"pass"}\n'


def fixture_sha256(content: str) -> str:
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


RSTIM_EXPANDED_FIXTURE_ARTIFACTS = {
    RSTIM_DISTRIBUTIONS_SUMMARY_PATH: "correctness-summary",
    RSTIM_DISTRIBUTIONS_EXPANDED_CORRECTNESS_PATH: "correctness-summary",
    RSTIM_DISTRIBUTIONS_REPORT_PATH: "correctness-report",
    RSTIM_RELEASE_SUMMARY_PATH: "speed-summary",
    RSTIM_RELEASE_REPORT_PATH: "speed-report",
    RSTIM_RELEASE_ENVIRONMENT_PATH: "environment",
    RSTIM_REPETITION_SUMMARY_PATH: "speed-summary",
    RSTIM_REPETITION_REPORT_PATH: "speed-report",
    RSTIM_REPETITION_ENVIRONMENT_PATH: "environment",
    RSTIM_SURFACE_DETECT_SUMMARY_PATH: "speed-summary",
    RSTIM_SURFACE_DETECT_REPORT_PATH: "speed-report",
    RSTIM_SURFACE_DETECT_ENVIRONMENT_PATH: "environment",
    RSTIM_DEM_RAW_PATH: "speed-raw",
    RSTIM_DEM_SUMMARY_PATH: "speed-summary",
    RSTIM_DEM_REPORT_PATH: "speed-report",
    RSTIM_DEM_ENVIRONMENT_PATH: "environment",
}

SURFACE_FIXTURE_ARTIFACT_HASHES = {
    SURFACE_RESULTS_PATH: {"sha256": SURFACE_RESULTS_SHA256},
    SURFACE_IMAGE_PATH: {"sha256": SURFACE_IMAGE_SHA256},
}

RSTIM_FIXTURE_ARTIFACT_HASHES = {
    RSTIM_SPEED_SUMMARY_PATH: {"sha256": RSTIM_SPEED_SUMMARY_SHA256},
    RSTIM_SPEED_REPORT_PATH: {"sha256": RSTIM_SPEED_REPORT_SHA256},
    RSTIM_CORRECTNESS_SUMMARY_PATH: {"sha256": RSTIM_CORRECTNESS_SUMMARY_SHA256},
    RSTIM_CASES_FULL_PATH: {"sha256": RSTIM_CASES_FULL_SHA256},
    RSTIM_CANONICAL_STIM_PATH: {"sha256": RSTIM_CANONICAL_STIM_SHA256},
    RSTIM_SHOWCASE_PATH: {"sha256": RSTIM_SHOWCASE_SHA256},
    **{
        artifact_path: {"sha256": fixture_sha256(RSTIM_EXPANDED_FIXTURE_CONTENT)}
        for artifact_path in RSTIM_EXPANDED_FIXTURE_ARTIFACTS
    },
}
RSTIM_REQUIRED_PROVENANCE_REQUIREMENTS = [
    "OS",
    "CPU model",
    "Rust version",
    "Python version",
    "dependency versions",
    "Stim version",
    "external repository commits",
    "command line",
    "seeds",
    "build profile",
    "shot counts",
    "date",
]


def fixture_provenance(commands: list[str], artifact_hashes: dict[str, dict[str, str]]) -> dict[str, object]:
    return {
        "schema_version": 1,
        "artifact_date": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "source_commit": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "commands": {"status": "recorded", "value": commands},
        "os": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "cpu_model": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "rust_version": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "python_version": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "dependency_versions": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "external_repository_commits": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "seed_policy": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "build_profile": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "shots_or_error_budget": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "artifact_hashes": {"status": "recorded", "value": artifact_hashes},
    }


def rstim_fixture_provenance() -> dict[str, object]:
    provenance = fixture_provenance(
        [
            "python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.full.toml",
            "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness --cases benchmarks/rstim_vs_stim_simulator/cases.full.toml --shots 1024 --out /tmp/rstim-vs-stim-correctness.json",
            "cargo run -p rstim --bin rstim -- perf ci --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-vs-stim-perf-ci",
            "cp /tmp/rstim-vs-stim-perf-ci/summary.json benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
            "cp /tmp/rstim-vs-stim-perf-ci/report.md benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
            "cp /tmp/rstim-vs-stim-correctness.json benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
        ],
        RSTIM_FIXTURE_ARTIFACT_HASHES,
    )
    provenance["seed_policy"] = {
        "status": "recorded",
        "value": {
            "correctness_seeds": [12345],
            "speed_rstim_variants_seed": 1234,
            "speed_stim_cli_seed_policy": "Stim CLI speed variant is timed through the recorded perf runner command without a seed-bearing sampler output.",
        },
    }
    return provenance


VALID_MANIFEST = {
    "schema_version": 1,
    "families": [
        {
            "id": "surface-decoder-comparison",
            "title": "Surface Decoder Comparison",
            "status": "existing",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "Checked full artifacts are committed-run evidence, not a general decoder ordering claim.",
            "evidence_items": [
                {
                    "id": "surface-decoder-full",
                    "title": "Checked surface-decoder full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [
                        {
                            "path": SURFACE_RESULTS_PATH,
                            "kind": "csv",
                            "checked": True,
                        },
                        {
                            "path": SURFACE_IMAGE_PATH,
                            "kind": "image",
                            "checked": True,
                        },
                    ],
                    "commands": ["make surface-decoder-compare-full"],
                    "provenance": fixture_provenance(["make surface-decoder-compare-full"], SURFACE_FIXTURE_ARTIFACT_HASHES),
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "bb-circuit-bposd-comparison",
            "title": "BB Circuit BP-OSD Comparison",
            "status": "partial",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "BB72/BB144 only.",
            "evidence_items": [
                {
                    "id": "bb-circuit-full",
                    "title": "Checked BB full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [],
                    "commands": ["make bb-circuit-bposd-compare-full"],
                    "provenance": fixture_provenance(["make bb-circuit-bposd-compare-full"], {}),
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "qec-code-random-window",
            "title": "qec-code Random Window",
            "status": "local-only",
            "source_docs": ["benchmarks/qec_code_random_window/README.md"],
            "claims_limit": "Generated outputs are ignored local evidence.",
            "evidence_items": [
                {
                    "id": "qec-code-smoke",
                    "title": "Local smoke command",
                    "status": "local-only",
                    "tier": "smoke",
                    "artifacts": [],
                    "commands": ["make qec-code-random-window-bench-smoke"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["benchmarks/qec_code_random_window/README.md"],
                    "claims_limit": "Local wiring check only.",
                }
            ],
        },
        {
            "id": "rstim-vs-stim-simulator",
            "title": "rstim versus Stim Simulator",
            "status": "partial",
            "source_docs": [
                RSTIM_SHOWCASE_PATH,
                "benchmarks/rstim_vs_stim_simulator/README.md",
            ],
            "claims_limit": "Checked artifacts cover the recorded d11/r100 selected-case speed and full-manifest correctness evidence only; this family does not claim broad rstim-versus-Stim parity.",
            "evidence_items": [
                {
                    "id": "rstim-vs-stim-full",
                    "title": "Checked rstim versus Stim simulator artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [
                        {"path": RSTIM_SPEED_SUMMARY_PATH, "kind": "speed-summary", "checked": True},
                        {"path": RSTIM_SPEED_REPORT_PATH, "kind": "speed-report", "checked": True},
                        {"path": RSTIM_CORRECTNESS_SUMMARY_PATH, "kind": "correctness-summary", "checked": True},
                        {"path": RSTIM_CASES_FULL_PATH, "kind": "fixture-manifest", "checked": True},
                        {"path": RSTIM_CANONICAL_STIM_PATH, "kind": "stim-fixture", "checked": True},
                        {"path": RSTIM_SHOWCASE_PATH, "kind": "showcase", "checked": True},
                        *[
                            {"path": artifact_path, "kind": artifact_kind, "checked": True}
                            for artifact_path, artifact_kind in RSTIM_EXPANDED_FIXTURE_ARTIFACTS.items()
                        ],
                    ],
                    "commands": [
                        "python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                        "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness --cases benchmarks/rstim_vs_stim_simulator/cases.full.toml --shots 1024 --out /tmp/rstim-vs-stim-correctness.json",
                        "cargo run -p rstim --bin rstim -- perf ci --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-vs-stim-perf-ci",
                        "cp /tmp/rstim-vs-stim-perf-ci/summary.json benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
                        "cp /tmp/rstim-vs-stim-perf-ci/report.md benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
                        "cp /tmp/rstim-vs-stim-correctness.json benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                    ],
                    "provenance": rstim_fixture_provenance(),
                    "provenance_requirements": list(RSTIM_REQUIRED_PROVENANCE_REQUIREMENTS),
                    "provenance_sources": [
                        RSTIM_SHOWCASE_PATH,
                        "benchmarks/rstim_vs_stim_simulator/README.md",
                    ],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "internal-regression-evidence",
            "title": "Internal Regression Evidence",
            "status": "partial",
            "source_docs": [".github/workflows/ci.yml"],
            "claims_limit": "Regression gate evidence only.",
            "evidence_items": [
                {
                    "id": "rstim-perf-ci",
                    "title": "rstim perf CI",
                    "status": "partial",
                    "tier": "regression-gate",
                    "artifacts": [],
                    "commands": ["cargo run -p rstim --bin rstim -- perf ci --out-dir perf-artifacts"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": [".github/workflows/ci.yml"],
                    "claims_limit": "Regression gate evidence only.",
                }
            ],
        },
    ],
}


class SiteManifestTest(unittest.TestCase):
    def write_fixture_manifest(self, remove_family: str | None = None, mutation: str | None = None):
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        root = Path(tmpdir.name)

        (root / ".gitignore").write_text("/benchmarks/out/\n", encoding="utf-8")
        (root / "docs/showcases").mkdir(parents=True)
        (root / "benchmarks/surface_decoder_compare/results/full").mkdir(parents=True)
        (root / "benchmarks/qec_code_random_window").mkdir(parents=True)
        (root / "benchmarks/rstim_vs_stim_simulator/results/full").mkdir(parents=True)
        (root / "benchmarks/rstim_vs_stim_simulator/fixtures").mkdir(parents=True)
        (root / ".github/workflows").mkdir(parents=True)
        (root / "site").mkdir(parents=True)
        (root / "_site/data").mkdir(parents=True)
        (root / "benchmarks/out").mkdir(parents=True)

        (root / "docs/showcases/benchmark-evidence.md").write_text("# Benchmark Evidence\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/results.csv").write_text("distance,shots\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").write_text("png\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/unchecked.csv").write_text("unchecked\n", encoding="utf-8")
        (root / "benchmarks/qec_code_random_window/README.md").write_text("# Random Window\n", encoding="utf-8")
        (root / RSTIM_SHOWCASE_PATH).write_text("# rstim vs Stim\n", encoding="utf-8")
        (root / "benchmarks/rstim_vs_stim_simulator/README.md").write_text("# rstim fixtures\n", encoding="utf-8")
        (root / RSTIM_SPEED_SUMMARY_PATH).write_text('{"case":"d11"}\n', encoding="utf-8")
        (root / RSTIM_SPEED_REPORT_PATH).write_text("# speed report\n", encoding="utf-8")
        (root / RSTIM_CORRECTNESS_SUMMARY_PATH).write_text('{"status":"PASS"}\n', encoding="utf-8")
        (root / RSTIM_CASES_FULL_PATH).write_text("[[cases]]\nid = 'd11'\n", encoding="utf-8")
        (root / RSTIM_CANONICAL_STIM_PATH).write_text("M 0\n", encoding="utf-8")
        for artifact_path in RSTIM_EXPANDED_FIXTURE_ARTIFACTS:
            path = root / artifact_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(RSTIM_EXPANDED_FIXTURE_CONTENT, encoding="utf-8")
        (root / ".github/workflows/ci.yml").write_text("name: ci\n", encoding="utf-8")
        (root / "_site/index.html").write_text(
            '<section id="benchmarks">Benchmark Methodology Claims Policy<div id="benchmark-manifest"></div></section>\n'
            '<section id="checked-benchmark-results"><div id="checked-benchmark-result-cards" '
            'data-checked-items="surface-decoder-full bb-circuit-full"></div></section>\n',
            encoding="utf-8",
        )
        (root / "_site/app.js").write_text(
            'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
            'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
            'family.status; family.claims_limit; item.status; item.claims_limit; '
            'item.artifacts; item.commands; item.caveats; item.provenance; renderProvenance; '
            'renderProvenance(item.provenance); artifact.checked; artifact.kind === "image";\n',
            encoding="utf-8",
        )
        (root / "benchmarks/out/ignored.csv").write_text("ignored\n", encoding="utf-8")
        (root / "benchmarks/out/local-only.csv").write_text("local\n", encoding="utf-8")

        manifest = json.loads(json.dumps(VALID_MANIFEST))
        if remove_family is not None:
            manifest["families"] = [family for family in manifest["families"] if family["id"] != remove_family]
        if mutation == "missing_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/missing/results.csv"
        elif mutation == "missing_claims_limit":
            del manifest["families"][0]["evidence_items"][0]["claims_limit"]
        elif mutation == "ignored_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/out/ignored.csv"
        elif mutation == "force_tracked_ignored_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/out/ignored.csv"
        elif mutation == "unchecked_tracked_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"].append(
                {
                    "path": "benchmarks/surface_decoder_compare/results/full/unchecked.csv",
                    "kind": "csv",
                    "checked": False,
                }
            )
        elif mutation == "bad_artifact_path_type":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = 42
        elif mutation == "bad_artifact_kind_type":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["kind"] = []
        elif mutation == "bad_commands_type":
            manifest["families"][0]["evidence_items"][0]["commands"] = "make surface-decoder-compare-full"
        elif mutation == "bad_provenance_requirements_type":
            manifest["families"][0]["evidence_items"][0]["provenance_requirements"] = ["command line", 123]
        elif mutation == "duplicate_item_id":
            duplicate = json.loads(json.dumps(manifest["families"][0]["evidence_items"][0]))
            manifest["families"][0]["evidence_items"].append(duplicate)
        elif mutation == "cross_family_duplicate_item_id":
            manifest["families"][1]["evidence_items"][0]["id"] = "surface-decoder-full"
        elif mutation == "empty_source_docs":
            manifest["families"][0]["source_docs"] = []
        elif mutation == "empty_provenance_sources":
            manifest["families"][0]["evidence_items"][0]["provenance_sources"] = []
        elif mutation == "missing_provenance":
            del manifest["families"][0]["evidence_items"][0]["provenance"]
        elif mutation == "missing_provenance_cpu_model":
            del manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]
        elif mutation == "provenance_cpu_model_missing_reason":
            manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"] = {"status": "not_recorded"}
        elif mutation == "bad_provenance_schema_version":
            manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = 2
        elif mutation == "bad_provenance_schema_version_type":
            manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = "1"
        elif mutation == "bad_provenance_schema_version_bool":
            manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = True
        elif mutation == "bad_artifact_hash":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = "0" * 64
        elif mutation == "missing_artifact_hash":
            del manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH]
        elif mutation == "artifact_hashes_not_recorded":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"] = {
                "status": "not_recorded",
                "reason": PROVENANCE_NOT_RECORDED_REASON,
            }
        elif mutation == "missing_artifact_hashes":
            del manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]
        elif mutation == "artifact_hash_entry_not_object":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH] = (
                SURFACE_RESULTS_SHA256
            )
        elif mutation == "artifact_hash_missing_sha256":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH] = {}
        elif mutation == "artifact_hash_sha256_not_string":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = 42
        elif mutation == "artifact_hash_sha256_invalid_hex":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = "g" * 64
        elif mutation == "artifact_hash_extra_algorithm":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "md5"
            ] = "unsupported"
        elif mutation == "artifact_hash_extra_path":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                "benchmarks/surface_decoder_compare/results/full/unchecked.csv"
            ] = {
                "sha256": "a" * 64
            }
        elif mutation == "rstim_family_future_status":
            manifest["families"][3]["status"] = "future"
        elif mutation == "rstim_partial_without_checked_artifacts":
            manifest["families"][3]["evidence_items"][0]["artifacts"] = []
            manifest["families"][3]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"] = {}
        elif mutation == "rstim_missing_required_artifact":
            manifest["families"][3]["evidence_items"][0]["artifacts"] = [
                artifact
                for artifact in manifest["families"][3]["evidence_items"][0]["artifacts"]
                if artifact["path"] != RSTIM_CORRECTNESS_SUMMARY_PATH
            ]
            del manifest["families"][3]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                RSTIM_CORRECTNESS_SUMMARY_PATH
            ]
        elif mutation == "rstim_wrong_artifact_kind":
            manifest["families"][3]["evidence_items"][0]["artifacts"][0]["kind"] = "json"
        elif mutation == "rstim_missing_stim_provenance_requirement":
            manifest["families"][3]["evidence_items"][0]["provenance_requirements"] = [
                requirement
                for requirement in manifest["families"][3]["evidence_items"][0]["provenance_requirements"]
                if requirement != "Stim version"
            ]
        elif mutation == "rstim_unrecorded_speed_seed":
            manifest["families"][3]["evidence_items"][0]["provenance"]["seed_policy"] = {
                "status": "recorded",
                "value": {
                    "correctness_seeds": [12345],
                    "speed_run_seed": "not recorded in the checked speed summary/report artifacts",
                },
            }

        manifest_path = root / "site/benchmark-site.json"
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        built_manifest_path = root / "_site/data/benchmark-site.json"
        built_manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(
            [
                "git",
                "add",
                ".gitignore",
                "docs/showcases/benchmark-evidence.md",
                "benchmarks/surface_decoder_compare/results/full/results.csv",
                "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                "benchmarks/surface_decoder_compare/results/full/unchecked.csv",
                "benchmarks/qec_code_random_window/README.md",
                RSTIM_SHOWCASE_PATH,
                "benchmarks/rstim_vs_stim_simulator/README.md",
                RSTIM_SPEED_SUMMARY_PATH,
                RSTIM_SPEED_REPORT_PATH,
                RSTIM_CORRECTNESS_SUMMARY_PATH,
                RSTIM_CASES_FULL_PATH,
                RSTIM_CANONICAL_STIM_PATH,
                *RSTIM_EXPANDED_FIXTURE_ARTIFACTS,
                ".github/workflows/ci.yml",
                "site/benchmark-site.json",
            ],
            cwd=root,
            check=True,
        )
        if mutation == "force_tracked_ignored_artifact":
            subprocess.run(["git", "add", "-f", "benchmarks/out/ignored.csv"], cwd=root, check=True)
        return root, manifest_path, built_manifest_path

    def copy_checked_artifacts_to_site(self, repo: Path, site_root: Path) -> None:
        for artifact_path in (SURFACE_RESULTS_PATH, SURFACE_IMAGE_PATH):
            source = repo / artifact_path
            destination = site_root / artifact_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())

    def test_accepts_valid_fixture_and_reports_families(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest()
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertEqual(errors, [])

    def test_rejects_checked_rstim_artifact_when_allow_policy_is_stale(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest()
        allowed = dict(check_site_manifest.RSTIM_VS_STIM_REQUIRED_ARTIFACTS)
        del allowed[RSTIM_DEM_SUMMARY_PATH]

        with mock.patch.object(check_site_manifest, "RSTIM_VS_STIM_REQUIRED_ARTIFACTS", allowed):
            errors = check_site_manifest.validate_manifest(repo, manifest_path)

        self.assertTrue(
            any(RSTIM_DEM_SUMMARY_PATH in error and "not accepted" in error for error in errors),
            errors,
        )

    def test_rejects_broad_rstim_vs_stim_claims(self) -> None:
        for phrase in ("rstim is faster than Stim", "rstim beats Stim", "full Stim parity"):
            with self.subTest(phrase=phrase):
                repo, manifest_path, _ = self.write_fixture_manifest()
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                manifest["families"][3]["claims_limit"] = phrase
                manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
                errors = check_site_manifest.validate_manifest(repo, manifest_path)
                self.assertTrue(
                    any("broad rstim-vs-Stim claim is not allowed" in error for error in errors),
                    errors,
                )

    def test_rejects_missing_required_family(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(remove_family="rstim-vs-stim-simulator")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("manifest" in error and "rstim-vs-stim-simulator" in error for error in errors),
            errors,
        )

    def test_rejects_rstim_partial_family_without_checked_artifacts(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_partial_without_checked_artifacts")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-simulator" in error
                and "partial" in error
                and "checked artifact" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_rstim_family_future_regression(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_family_future_status")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-simulator" in error
                and "partial" in error
                and "future" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_rstim_missing_required_artifact(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_missing_required_artifact")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-simulator" in error
                and RSTIM_CORRECTNESS_SUMMARY_PATH in error
                and "required checked artifact" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_rstim_wrong_artifact_kind(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_wrong_artifact_kind")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-full" in error
                and RSTIM_SPEED_SUMMARY_PATH in error
                and "kind" in error
                and "speed-summary" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_rstim_missing_stim_provenance_requirement(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_missing_stim_provenance_requirement")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-full" in error
                and "Stim version" in error
                and "provenance_requirements" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_rstim_recorded_seed_policy_with_unrecorded_speed_seed(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_unrecorded_speed_seed")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "rstim-vs-stim-full" in error
                and "seed_policy" in error
                and "speed_rstim_variants_seed" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_negative_control_mutations(self) -> None:
        for mutation, entry_id, rule in [
            ("missing_artifact", "surface-decoder-full", "does not exist"),
            ("missing_claims_limit", "surface-decoder-full", "claims_limit"),
            ("ignored_artifact", "surface-decoder-full", "ignored"),
        ]:
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_self_test_exercises_negative_controls(self) -> None:
        self.assertEqual(check_site_manifest.run_self_test(), [])

    def test_rejects_force_tracked_ignored_artifact(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="force_tracked_ignored_artifact")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "ignored" in error for error in errors),
            errors,
        )

    def test_rejects_malformed_metadata_without_crashing(self) -> None:
        for mutation, entry_id, rule in [
            ("bad_artifact_path_type", "surface-decoder-full", "artifact path must be a non-empty string"),
            ("bad_artifact_kind_type", "surface-decoder-full", "artifact kind must be a non-empty string"),
            ("bad_commands_type", "surface-decoder-full", "commands must be a list"),
            ("bad_provenance_requirements_type", "surface-decoder-full", "provenance_requirements entries must be strings"),
        ]:
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_rejects_duplicate_evidence_item_ids(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="duplicate_item_id")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("family surface-decoder-comparison" in error and "duplicate evidence item id surface-decoder-full" in error for error in errors),
            errors,
        )

    def test_rejects_cross_family_duplicate_evidence_item_ids(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="cross_family_duplicate_item_id")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("manifest" in error and "duplicate evidence item id surface-decoder-full" in error for error in errors),
            errors,
        )

    def test_rejects_empty_source_and_provenance_sources(self) -> None:
        for mutation, entry_id, rule in [
            ("empty_source_docs", "family surface-decoder-comparison", "source_docs must not be empty"),
            ("empty_provenance_sources", "surface-decoder-full", "provenance_sources must not be empty"),
        ]:
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_rejects_checked_item_without_canonical_provenance(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_provenance")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "provenance" in error for error in errors),
            errors,
        )

    def test_rejects_checked_item_missing_provenance_key(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_provenance_cpu_model")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "cpu_model" in error for error in errors),
            errors,
        )

    def test_rejects_not_recorded_provenance_without_reason(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="provenance_cpu_model_missing_reason")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "cpu_model" in error
                and "reason" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_unsupported_provenance_schema_version(self) -> None:
        for mutation in (
            "bad_provenance_schema_version",
            "bad_provenance_schema_version_type",
            "bad_provenance_schema_version_bool",
        ):
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(
                any("surface-decoder-full" in error and "schema_version" in error for error in errors),
                errors,
            )

    def test_rejects_checked_artifact_hash_digest_mismatch(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="bad_artifact_hash")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "results.csv" in error
                and "sha256" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_checked_artifact_missing_hash_entry(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_artifact_hash")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and SURFACE_RESULTS_PATH in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_unsupported_checked_artifact_hash_shapes(self) -> None:
        for mutation, rule in [
            ("artifact_hashes_not_recorded", "recorded"),
            ("artifact_hash_entry_not_object", "object"),
            ("artifact_hash_missing_sha256", "sha256"),
            ("artifact_hash_sha256_not_string", "sha256"),
            ("artifact_hash_sha256_invalid_hex", "sha256"),
            ("artifact_hash_extra_algorithm", "unsupported"),
        ]:
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(
                any(
                    "surface-decoder-full" in error
                    and SURFACE_RESULTS_PATH in error
                    and rule in error
                    for error in errors
                ),
                (mutation, errors),
            )

    def test_rejects_checked_artifact_missing_artifact_hashes_field(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_artifact_hashes")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and SURFACE_RESULTS_PATH in error
                and "artifact_hashes" in error
                and "sha256" in error
                for error in errors
            ),
            errors,
        )
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and SURFACE_RESULTS_PATH in error
                and "recorded sha256 entries" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_checked_artifact_hash_extra_path(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="artifact_hash_extra_path")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "artifact_hashes" in error
                and "unexpected hash entry" in error
                and "benchmarks/surface_decoder_compare/results/full/unchecked.csv" in error
                for error in errors
            ),
            errors,
        )

    def test_site_root_validation_rejects_missing_copied_checked_artifact(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        site_root = repo / "_site"

        errors = check_site_manifest.validate_manifest(repo, built_manifest_path, site_root=site_root)

        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "benchmarks/surface_decoder_compare/results/full/results.csv" in error
                and "not copied" in error
                for error in errors
            ),
                errors,
            )

    def test_site_root_validation_rejects_copied_checked_artifact_hash_mismatch(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        site_root = repo / "_site"
        self.copy_checked_artifacts_to_site(repo, site_root)
        (site_root / SURFACE_RESULTS_PATH).write_text("mutated copied artifact\n", encoding="utf-8")

        errors = check_site_manifest.validate_manifest(repo, built_manifest_path, site_root=site_root)

        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "results.csv" in error
                and "sha256" in error
                for error in errors
            ),
            errors,
        )

    def test_copy_helper_copies_manifest_and_checked_artifacts_only(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest()
        site_root = repo / "_site-copy"

        errors = copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root)

        self.assertEqual(errors, [])
        self.assertTrue((site_root / "data/benchmark-site.json").is_file())
        self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").is_file())
        self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").is_file())
        self.assertFalse((site_root / "benchmarks/out/local-only.csv").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/unchecked.csv").exists())

    def test_copy_helper_rejects_unchecked_tracked_artifact(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="unchecked_tracked_artifact")
        site_root = repo / "_site-copy"

        errors = copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root)

        self.assertTrue(any("checked=True" in error for error in errors), errors)
        self.assertFalse((site_root / "data/benchmark-site.json").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/unchecked.csv").exists())

    def test_cli_accepts_source_manifest_with_built_site_root(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest()
        site_root = repo / "_site"
        self.assertEqual(
            copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root),
            [],
        )

        result = subprocess.run(
            [
                "python3",
                str(Path(check_site_manifest.__file__).resolve()),
                "--repo-root",
                ".",
                "--site-root",
                "_site",
                "site/benchmark-site.json",
            ],
            cwd=repo,
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_accepts_built_site_manifest_when_site_root_is_wired(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        (repo / "_site/index.html").write_text(
            '<section id="benchmarks">Benchmark Methodology Claims Policy<div id="benchmark-manifest"></div></section>\n'
            '<section id="checked-benchmark-results"><div id="checked-benchmark-result-cards" '
            'data-checked-items="surface-decoder-full bb-circuit-full"></div></section>\n',
            encoding="utf-8",
        )
        (repo / "_site/app.js").write_text(
            'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
            'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
            'family.status; family.claims_limit; item.status; item.claims_limit; '
            'item.artifacts; item.commands; item.caveats; item.provenance; renderProvenance; '
            'renderProvenance(item.provenance); artifact.checked; artifact.kind === "image";\n',
            encoding="utf-8",
        )
        errors = check_site_manifest.validate_manifest(repo, built_manifest_path)
        errors.extend(check_site_manifest.validate_site_root(repo / "_site", built_manifest_path))
        self.assertEqual(errors, [])

    def test_rejects_built_site_without_checked_result_wiring(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        (repo / "_site/app.js").write_text(
            'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
            'family.status; family.claims_limit; item.status; item.claims_limit;\n',
            encoding="utf-8",
        )
        errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
        self.assertTrue(any("checked result" in error for error in errors), errors)

    def test_rejects_built_site_without_provenance_renderer_wiring(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        (repo / "_site/app.js").write_text(
            'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
            'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
            'family.status; family.claims_limit; item.status; item.claims_limit; '
            'item.artifacts; item.commands; item.caveats; artifact.checked; artifact.kind === "image";\n',
            encoding="utf-8",
        )

        errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)

        self.assertTrue(
            any("provenance wiring" in error and "item.provenance" in error for error in errors),
            errors,
        )

    def test_rejects_built_site_artifact_reference_not_listed_in_manifest(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        index = repo / "_site/index.html"
        index.write_text(
            index.read_text(encoding="utf-8")
            + '<a href="benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
            encoding="utf-8",
        )
        errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
        self.assertTrue(
            any("not listed as a checked manifest artifact" in error for error in errors),
            errors,
        )

    def test_rejects_built_site_rstim_artifact_reference_not_listed_in_manifest(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        index = repo / "_site/index.html"
        missing_artifact = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/not-in-manifest.json"
        index.write_text(
            index.read_text(encoding="utf-8") + f'<a href="{missing_artifact}">bad</a>\n',
            encoding="utf-8",
        )
        errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
        self.assertTrue(
            any(missing_artifact in error and "not listed as a checked manifest artifact" in error for error in errors),
            errors,
        )

    def test_rejects_built_site_without_manifest_status_wiring(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        (repo / "_site/app.js").write_text('fetch("data/benchmark-site.json");\n', encoding="utf-8")
        errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
        self.assertTrue(any("status" in error and "claims_limit" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
