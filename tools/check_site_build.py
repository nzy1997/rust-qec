#!/usr/bin/env python3
"""Validate a built static site tree for reviewer-readable site contracts."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Callable, Iterable
from urllib.parse import urlsplit

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import tools.check_site_manifest as check_site_manifest


@dataclass(frozen=True)
class CheckResult:
    status: str
    area: str
    detail: str


@dataclass
class SiteFixture:
    tempdir: tempfile.TemporaryDirectory[str]
    repo_root: Path
    site_root: Path

    def cleanup(self) -> None:
        self.tempdir.cleanup()


REQUIRED_FILES = (
    "index.html",
    "styles.css",
    "app.js",
    "data/benchmark-site.json",
    "QP101-ZY.md",
    "qp101.schema.json",
    "examples/basic.qp101.json",
    "examples/repeat-detector.qp101.json",
    "examples/atom-loss-sample.qp101.json",
    "gallery/basic-site.svg",
    "gallery/repeat-detector-site.svg",
    "gallery/atom-loss-sample.svg",
)
QP101_REQUIRED_FILES = (
    "qp101.schema.json",
    "QP101-ZY.md",
    "examples/basic.qp101.json",
    "examples/repeat-detector.qp101.json",
    "examples/atom-loss-sample.qp101.json",
    "gallery/basic-site.svg",
    "gallery/repeat-detector-site.svg",
    "gallery/atom-loss-sample.svg",
)
REQUIRED_ANCHORS = (
    "docs-home",
    "workspace-overview",
    "feature-walkthroughs",
    "benchmark-evidence",
    "checked-benchmark-results",
    "benchmarks",
    "benchmark-manifest",
    "checked-benchmark-result-cards",
    "qp101",
    "schema-browser",
    "operations",
    "gallery",
    "examples",
)
CLAIMS_POLICY_PHRASES = (
    "Claims Policy",
    "Publishable Evidence",
    "Local-Only Evidence",
    "smoke checks verify wiring",
    "full evidence can describe the committed checked run",
    "committed-run evidence",
    "not a general decoder ordering claim",
)
CHECKED_ARTIFACT_REFERENCE_RE = re.compile(
    r"benchmarks/(?:surface_decoder_compare|bb_circuit_bposd_compare)/results/full/[A-Za-z0-9._/-]+"
    r"|benchmarks/rstim_vs_stim_simulator/(?:results/full/[A-Za-z0-9._/-]+|cases\.full\.toml|fixtures/[A-Za-z0-9._/-]+\.stim)"
)
STRING_LITERAL_PATH_RE = re.compile(r"""["']([A-Za-z0-9_./-]+\.[A-Za-z0-9]+(?:[?#][^"']*)?)["']""")
ROOT_LEVEL_SITE_FILES = {"qp101.schema.json", "QP101-ZY.md", "styles.css", "app.js", "index.html"}


class HtmlCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.ids: set[str] = set()
        self.hrefs: list[str] = []
        self.srcs: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        for key, value in attrs:
            if value is None:
                continue
            if key == "id":
                self.ids.add(value)
            elif key == "href":
                self.hrefs.append(value)
            elif key == "src":
                self.srcs.append(value)


def fail(area: str, detail: str) -> CheckResult:
    return CheckResult("FAIL", area, detail)


def pass_(area: str, detail: str) -> CheckResult:
    return CheckResult("PASS", area, detail)


def is_external_link(value: str) -> bool:
    return value.startswith(("http://", "https://", "mailto:", "tel:"))


def normalize_local_reference(value: str) -> str | None:
    if not value or value.startswith("#") or is_external_link(value):
        return None
    split = urlsplit(value)
    if split.scheme or split.netloc:
        return None
    path = split.path
    if not path:
        return None
    return path.lstrip("/")


def check_non_empty_files(site_root: Path, relative_paths: Iterable[str], area: str) -> CheckResult:
    missing: list[str] = []
    empty: list[str] = []
    for relative in relative_paths:
        candidate = site_root / relative
        if not candidate.is_file():
            missing.append(relative)
        elif candidate.stat().st_size == 0:
            empty.append(relative)
    if missing or empty:
        parts: list[str] = []
        if missing:
            parts.append("missing " + ", ".join(missing))
        if empty:
            parts.append("empty " + ", ".join(empty))
        return fail(area, "; ".join(parts))
    return pass_(area, "schema, protocol, examples, and gallery assets are present")


def collect_local_string_paths(*texts: str) -> set[str]:
    found: set[str] = set()
    for text in texts:
        for value in STRING_LITERAL_PATH_RE.findall(text):
            normalized = normalize_local_reference(value)
            if normalized is not None and ("/" in normalized or normalized in ROOT_LEVEL_SITE_FILES):
                found.add(normalized)
    return found


def resolve_site_reference(site_root: Path, value: str) -> tuple[str | None, Path | None, str | None]:
    normalized = normalize_local_reference(value)
    if normalized is None:
        return None, None, None
    candidate = (site_root / normalized).resolve(strict=False)
    try:
        candidate.relative_to(site_root)
    except ValueError:
        return normalized, None, f"path escape outside built site: {normalized}"
    return normalized, candidate, None


def read_text_file(path: Path) -> tuple[str | None, str | None]:
    try:
        return path.read_text(encoding="utf-8"), None
    except UnicodeDecodeError as exc:
        return None, f"{path.name}: invalid UTF-8 ({exc})"
    except OSError as exc:
        return None, f"{path.name}: {exc.strerror or exc}"


def check_workspace_overview(site_root: Path, index_text: str, collector: HtmlCollector, app_text: str) -> CheckResult:
    missing_anchors = [anchor for anchor in REQUIRED_ANCHORS if anchor not in collector.ids]
    if missing_anchors:
        return fail("workspace overview", f"missing required anchors: {', '.join(missing_anchors)}")

    missing_refs: list[str] = []
    for href in collector.hrefs:
        if href.startswith("#"):
            anchor = href[1:]
            if anchor and anchor not in collector.ids:
                missing_refs.append(f"missing same-page anchor #{anchor}")
            continue
        normalized, candidate, error = resolve_site_reference(site_root, href)
        if error is not None:
            missing_refs.append(error)
        elif normalized is not None and candidate is not None and not candidate.exists():
            missing_refs.append(normalized)
    for src in collector.srcs:
        normalized, candidate, error = resolve_site_reference(site_root, src)
        if error is not None:
            missing_refs.append(error)
        elif normalized is not None and candidate is not None and not candidate.exists():
            missing_refs.append(normalized)

    string_paths = collect_local_string_paths(index_text, app_text)
    missing_string_paths: list[str] = []
    for path in sorted(string_paths):
        normalized, candidate, error = resolve_site_reference(site_root, path)
        if error is not None:
            missing_string_paths.append(error)
        elif normalized is not None and candidate is not None and not candidate.exists():
            missing_string_paths.append(normalized)
    if missing_refs or missing_string_paths:
        details = missing_refs + missing_string_paths
        return fail("workspace overview", "missing local references: " + ", ".join(details))

    return pass_("workspace overview", "required anchors and local built-site references are present")


def check_manifest_and_artifacts(site_root: Path, repo_root: Path) -> tuple[list[CheckResult], dict[str, object] | None, list[str]]:
    results: list[CheckResult] = []
    manifest_path = site_root / "data/benchmark-site.json"
    try:
        manifest_errors = check_site_manifest.validate_manifest(repo_root, manifest_path, site_root=site_root)
    except (OSError, UnicodeDecodeError) as exc:
        manifest_errors = [f"data/benchmark-site.json could not be read during manifest validation: {exc}"]
    try:
        site_errors = check_site_manifest.validate_site_root(site_root, manifest_path)
    except (OSError, UnicodeDecodeError) as exc:
        site_errors = [f"data/benchmark-site.json could not be read during site-root validation: {exc}"]
    combined = manifest_errors + site_errors
    manifest: dict[str, object] | None = None
    try:
        loaded = check_site_manifest.load_json(manifest_path)
    except (FileNotFoundError, json.JSONDecodeError, OSError, UnicodeDecodeError):
        loaded = None
    if isinstance(loaded, dict):
        manifest = loaded
    if combined:
        results.append(fail("manifest and copied artifacts", "; ".join(combined)))
    else:
        results.append(pass_("manifest and copied artifacts", "manifest schema and copied checked artifacts validated"))
    return results, manifest, combined


def check_benchmark_methodology(index_text: str) -> CheckResult:
    missing = [phrase for phrase in CLAIMS_POLICY_PHRASES if phrase not in index_text]
    if missing:
        return fail("benchmark methodology", "missing claims-policy phrases: " + ", ".join(missing))
    return pass_("benchmark methodology", "claims-policy phrases are present")


def check_checked_artifacts(site_root: Path, index_text: str, app_text: str, manifest: dict[str, object] | None) -> CheckResult:
    if manifest is None:
        return fail("checked benchmark artifacts", "manifest could not be loaded")
    checked = check_site_manifest.iter_checked_artifact_paths(manifest)
    if not checked:
        return fail("checked benchmark artifacts", "no checked artifacts are listed in the manifest")
    missing = [artifact for _, artifact in checked if not (site_root / artifact).is_file()]
    if missing:
        return fail("checked benchmark artifacts", "missing copied checked artifacts: " + ", ".join(missing))

    checked_paths = {artifact for _, artifact in checked}
    references = sorted(set(CHECKED_ARTIFACT_REFERENCE_RE.findall(index_text + "\n" + app_text)))
    unexpected = [ref for ref in references if ref not in checked_paths]
    if unexpected:
        return fail(
            "checked benchmark artifacts",
            "references not listed as a checked manifest artifact: " + ", ".join(unexpected),
        )

    return pass_(
        "checked benchmark artifacts",
        f"{len(checked)} checked artifacts copied: {', '.join(path for _, path in checked)}",
    )


PROVENANCE_ERROR_MARKERS = (
    "provenance",
    "artifact_hashes",
    "sha256",
    "copied artifact",
)


def relevant_provenance_errors(errors: list[str]) -> list[str]:
    return [error for error in errors if any(marker in error for marker in PROVENANCE_ERROR_MARKERS)]


def plural(count: int, singular: str, plural_form: str | None = None) -> str:
    if count == 1:
        return f"{count} {singular}"
    return f"{count} {plural_form or singular + 's'}"


def summarize_provenance_item(item_id: str, item: dict[str, object], checked_paths: set[str]) -> str:
    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        return f"{item_id} (provenance missing from built manifest)"

    recorded_fields = 0
    not_recorded_fields = 0
    for field, value in provenance.items():
        if field == "schema_version" or not isinstance(value, dict):
            continue
        if value.get("status") == "recorded":
            recorded_fields += 1
        elif value.get("status") == "not_recorded":
            not_recorded_fields += 1

    artifact_hash_count = 0
    artifact_hashes = provenance.get("artifact_hashes")
    if isinstance(artifact_hashes, dict) and isinstance(artifact_hashes.get("value"), dict):
        artifact_hash_count = sum(
            1
            for path, entry in artifact_hashes["value"].items()
            if path in checked_paths and isinstance(entry, dict) and isinstance(entry.get("sha256"), str)
        )

    return (
        f"{item_id} ({plural(recorded_fields, 'recorded field')}, "
        f"{plural(not_recorded_fields, 'not_recorded field')}, "
        f"{plural(artifact_hash_count, 'checked artifact hash', 'checked artifact hashes')})"
    )


def check_checked_provenance(manifest: dict[str, object] | None, delegated_errors: list[str]) -> CheckResult:
    if delegated_errors:
        provenance_errors = relevant_provenance_errors(delegated_errors)
        details = provenance_errors if provenance_errors else delegated_errors
        return fail(
            "checked benchmark provenance",
            "provenance exposure could not be confirmed: " + "; ".join(details),
        )
    if manifest is None:
        return fail("checked benchmark provenance", "manifest could not be loaded")

    checked = check_site_manifest.iter_checked_artifacts(manifest)
    if not checked:
        return fail("checked benchmark provenance", "no checked evidence items are listed in the manifest")

    by_item: dict[str, tuple[dict[str, object], set[str]]] = {}
    for item, item_id, artifact_path in checked:
        _, paths = by_item.setdefault(item_id, (item, set()))
        paths.add(artifact_path)

    summaries = [summarize_provenance_item(item_id, item, paths) for item_id, (item, paths) in sorted(by_item.items())]
    return pass_("checked benchmark provenance", "; ".join(summaries) + " exposed through manifest-backed renderer")


def find_family(manifest: dict[str, object], family_id: str) -> dict[str, object] | None:
    families = manifest.get("families")
    if not isinstance(families, list):
        return None
    for family in families:
        if isinstance(family, dict) and family.get("id") == family_id:
            return family
    return None


def check_local_only_future(site_root: Path, manifest: dict[str, object] | None) -> CheckResult:
    if manifest is None:
        return fail("local-only/future classifications", "manifest could not be loaded")
    qec_code = find_family(manifest, "qec-code-random-window")
    rstim_vs_stim = find_family(manifest, "rstim-vs-stim-simulator")
    if qec_code is None or rstim_vs_stim is None:
        return fail("local-only/future classifications", "missing qec-code or rstim-vs-stim families")
    qec_status = qec_code.get("status")
    rstim_status = rstim_vs_stim.get("status")
    if qec_status not in {"local-only", "partial"}:
        return fail("local-only/future classifications", f"qec-code family must stay local-only or partial, got {qec_status!r}")
    if rstim_status not in {"future", "partial"}:
        return fail(
            "local-only/future classifications",
            f"rstim-vs-stim family must stay future or partial, got {rstim_status!r}",
        )

    checked = check_site_manifest.iter_checked_artifact_paths(manifest)
    forbidden_prefixes = (
        "benchmarks/out/qec_code_random_window/",
        "benchmarks/out/rstim_vs_stim_simulator/",
    )
    bad_paths = [path for _, path in checked if path.startswith(forbidden_prefixes)]
    if bad_paths:
        return fail("local-only/future classifications", "forbidden checked artifacts under benchmarks/out/: " + ", ".join(bad_paths))

    out_dir = site_root / "benchmarks/out"
    if out_dir.exists():
        visible = [path for path in out_dir.rglob("*") if path.is_file()]
        if visible:
            return fail(
                "local-only/future classifications",
                "built site must not publish local-only or future artifacts under benchmarks/out/",
            )

    if rstim_status == "partial":
        rstim_paths = [path for _, path in checked if path.startswith("benchmarks/rstim_vs_stim_simulator/")]
        if not rstim_paths:
            return fail(
                "local-only/future classifications",
                "rstim-vs-stim partial family must publish checked artifacts under benchmarks/rstim_vs_stim_simulator/",
            )

    return pass_(
        "local-only/future classifications",
        "qec-code local-only/partial and rstim-vs-stim future/partial classifications are preserved, including rstim-vs-stim partial checked evidence",
    )


def check_site_build(site_root: Path, repo_root: Path | None = None) -> list[CheckResult]:
    repo_root = repo_root or Path.cwd()
    site_root = site_root.resolve()
    results: list[CheckResult] = []

    required_result = check_non_empty_files(site_root, REQUIRED_FILES, "required files")
    if required_result.status == "FAIL":
        results.append(required_result)
    else:
        results.append(pass_("required files", "core built-site files are present"))

    results.append(check_non_empty_files(site_root, QP101_REQUIRED_FILES, "QP101 assets"))

    index_text, index_error = read_text_file(site_root / "index.html")
    app_text, app_error = read_text_file(site_root / "app.js")
    collector: HtmlCollector | None = None
    workspace_read_errors = [error for error in (index_error, app_error) if error is not None]

    if index_text is not None:
        collector = HtmlCollector()
        collector.feed(index_text)

    if workspace_read_errors:
        results.append(fail("workspace overview", "could not read built-site files: " + "; ".join(workspace_read_errors)))
    elif collector is not None and app_text is not None:
        results.append(check_workspace_overview(site_root, index_text, collector, app_text))

    manifest_results, manifest, manifest_site_errors = check_manifest_and_artifacts(site_root, repo_root)
    results.extend(manifest_results)
    results.append(check_checked_provenance(manifest, manifest_site_errors + workspace_read_errors))
    if index_text is None:
        results.append(fail("benchmark methodology", "could not read built-site files: index.html"))
    else:
        results.append(check_benchmark_methodology(index_text))

    if index_text is None or app_text is None:
        unreadable = ", ".join(
            name for name, text in (("index.html", index_text), ("app.js", app_text)) if text is None
        )
        results.append(fail("checked benchmark artifacts", f"could not read built-site files: {unreadable}"))
    else:
        results.append(check_checked_artifacts(site_root, index_text, app_text, manifest))
    results.append(check_local_only_future(site_root, manifest))

    return results


def format_summary(results: list[CheckResult]) -> str:
    lines = [f"{result.status} {result.area}: {result.detail}" for result in results]
    warnings = sum(result.status == "WARN" for result in results)
    failures = sum(result.status == "FAIL" for result in results)
    overall = "PASS" if failures == 0 else "FAIL"
    lines.append(f"SUMMARY: {overall} ({len(results)} checks, {warnings} warnings, {failures} failures)")
    return "\n".join(lines)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def make_fixture_site() -> SiteFixture:
    tempdir = tempfile.TemporaryDirectory()
    repo_root = Path(tempdir.name)
    site_root = repo_root / "_site"
    site_root.mkdir(parents=True)

    write_text(repo_root / ".gitignore", "/benchmarks/out/\n")
    write_text(repo_root / "docs/showcases/benchmark-evidence.md", "# Benchmark Evidence\n")
    write_text(repo_root / "benchmarks/qec_code_random_window/README.md", "# Random Window\n")
    write_text(repo_root / ".github/workflows/ci.yml", "name: ci\n")
    write_text(repo_root / "benchmarks/surface_decoder_compare/results/full/results.csv", "distance,shots\n")
    write_text(repo_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png", "png\n")
    write_text(repo_root / "benchmarks/bb_circuit_bposd_compare/results/full/results.csv", "distance,shots\n")
    write_text(repo_root / "benchmarks/bb_circuit_bposd_compare/results/full/summary.md", "# Summary\n")
    write_text(repo_root / "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png", "png\n")
    write_text(repo_root / "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md", "# Gaps\n")
    write_text(repo_root / "docs/showcases/rstim-vs-stim-simulator.md", "# rstim vs Stim\n")
    write_text(repo_root / "benchmarks/rstim_vs_stim_simulator/README.md", "# rstim fixtures\n")
    write_text(repo_root / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json", '{"case":"d11"}\n')
    write_text(repo_root / "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md", "# speed report\n")
    write_text(
        repo_root / "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
        '{"status":"PASS"}\n',
    )
    write_text(repo_root / "benchmarks/rstim_vs_stim_simulator/cases.full.toml", "[[cases]]\nid = 'd11'\n")
    write_text(
        repo_root / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
        "M 0\n",
    )

    provenance_reason = "historical fixture predates canonical provenance capture"

    def fixture_provenance(commands: list[str], artifact_hashes: dict[str, dict[str, str]]) -> dict[str, object]:
        return {
            "schema_version": 1,
            "artifact_date": {"status": "not_recorded", "reason": provenance_reason},
            "source_commit": {"status": "not_recorded", "reason": provenance_reason},
            "commands": {"status": "recorded", "value": commands},
            "os": {"status": "not_recorded", "reason": provenance_reason},
            "cpu_model": {"status": "not_recorded", "reason": provenance_reason},
            "rust_version": {"status": "not_recorded", "reason": provenance_reason},
            "python_version": {"status": "not_recorded", "reason": provenance_reason},
            "dependency_versions": {"status": "not_recorded", "reason": provenance_reason},
            "external_repository_commits": {"status": "not_recorded", "reason": provenance_reason},
            "seed_policy": {"status": "not_recorded", "reason": provenance_reason},
            "build_profile": {"status": "not_recorded", "reason": provenance_reason},
            "shots_or_error_budget": {"status": "not_recorded", "reason": provenance_reason},
            "artifact_hashes": {"status": "recorded", "value": artifact_hashes},
        }

    def rstim_vs_stim_provenance(commands: list[str], artifact_hashes: dict[str, dict[str, str]]) -> dict[str, object]:
        provenance = fixture_provenance(commands, artifact_hashes)
        provenance["seed_policy"] = {
            "status": "recorded",
            "value": {
                "correctness_seeds": [12345],
                "speed_rstim_variants_seed": 1234,
                "speed_stim_cli_seed_policy": "Stim CLI speed variant is timed through the recorded perf runner command without a seed-bearing sampler output.",
            },
        }
        return provenance

    surface_hashes = {
        "benchmarks/surface_decoder_compare/results/full/results.csv": {
            "sha256": "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
        },
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png": {
            "sha256": "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"
        },
    }
    bb_hashes = {
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv": {
            "sha256": "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
        },
        "benchmarks/bb_circuit_bposd_compare/results/full/summary.md": {
            "sha256": "88501c2f5e6660af97d9cafe49c86afa7adff4dc92cbe9e27141b4ef45642ee8"
        },
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png": {
            "sha256": "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"
        },
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md": {
            "sha256": "f999c0097163daf1dbd5fc72414fa2f5ec0fd43a13ef088f4c7fd32372dc61d4"
        },
    }
    rstim_vs_stim_hashes = {
        "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json": {
            "sha256": "068c6cda6256254832b1f07979a475a1d747288cbdfaae6291e03697c2b3261d"
        },
        "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md": {
            "sha256": "ad2ce5a1a049d02dc3ef15ec90609362b12e580c172fe8a13f6c16071c73a2f4"
        },
        "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json": {
            "sha256": "423b0a945a73ecb5ab748c7c796af9328a4639bf6921af982e71ad00924f46e9"
        },
        "benchmarks/rstim_vs_stim_simulator/cases.full.toml": {
            "sha256": "f86f77dff5135b7273d64aa8fd01a8d55901e2222a175ae97922db423cabccd6"
        },
        "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim": {
            "sha256": "efb8217cc5ffbb305255ac47281b17964df5cf6cb2268e63450f06ce0e001fdb"
        },
        "docs/showcases/rstim-vs-stim-simulator.md": {
            "sha256": "382c8ba936ac311bfbf2b2d3da55618cd551f2840a4f284f12980986f992a72b"
        },
    }

    manifest = {
        "schema_version": 1,
        "families": [
            {
                "id": "surface-decoder-comparison",
                "title": "Surface Decoder Comparison",
                "status": "existing",
                "source_docs": ["docs/showcases/benchmark-evidence.md"],
                "claims_limit": "Checked full-tier artifacts are committed-run evidence, not a general decoder ordering claim.",
                "evidence_items": [
                    {
                        "id": "surface-decoder-full",
                        "title": "Checked surface-decoder full artifacts",
                        "status": "existing",
                        "tier": "full",
                        "artifacts": [
                            {
                                "path": "benchmarks/surface_decoder_compare/results/full/results.csv",
                                "kind": "csv",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                                "kind": "image",
                                "checked": True,
                            },
                        ],
                        "commands": ["make surface-decoder-compare-full"],
                        "provenance": fixture_provenance(["make surface-decoder-compare-full"], surface_hashes),
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                        "claims_limit": "Checked full-tier artifacts are committed-run evidence, not a general decoder ordering claim.",
                    }
                ],
            },
            {
                "id": "bb-circuit-bposd-comparison",
                "title": "BB Circuit BP-OSD Comparison",
                "status": "partial",
                "source_docs": ["docs/showcases/benchmark-evidence.md"],
                "claims_limit": "Checked full-tier artifacts are committed-run evidence, not a general decoder ordering claim.",
                "evidence_items": [
                    {
                        "id": "bb-circuit-full",
                        "title": "Checked BB full artifacts",
                        "status": "existing",
                        "tier": "full",
                        "artifacts": [
                            {
                                "path": "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
                                "kind": "csv",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/bb_circuit_bposd_compare/results/full/summary.md",
                                "kind": "markdown",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
                                "kind": "image",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
                                "kind": "markdown",
                                "checked": True,
                            },
                        ],
                        "commands": ["make bb-circuit-bposd-compare-full"],
                        "provenance": fixture_provenance(["make bb-circuit-bposd-compare-full"], bb_hashes),
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                        "claims_limit": "Checked full-tier artifacts are committed-run evidence, not a general decoder ordering claim.",
                    }
                ],
            },
            {
                "id": "qec-code-random-window",
                "title": "qec-code Random Window",
                "status": "local-only",
                "source_docs": ["benchmarks/qec_code_random_window/README.md"],
                "claims_limit": "Smoke and framework outputs are local implementation checks under ignored output directories unless promoted by a future checked-artifact policy.",
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
                    "docs/showcases/benchmark-evidence.md",
                    "docs/showcases/rstim-vs-stim-simulator.md",
                    "benchmarks/rstim_vs_stim_simulator/README.md",
                ],
                "claims_limit": "Partial checked evidence for recorded workloads and recorded environments only, not broad rstim/Stim parity.",
                "evidence_items": [
                    {
                        "id": "rstim-vs-stim-full",
                        "title": "Checked rstim versus Stim full artifacts",
                        "status": "existing",
                        "tier": "full",
                        "artifacts": [
                            {
                                "path": "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
                                "kind": "speed-summary",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
                                "kind": "speed-report",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                                "kind": "correctness-summary",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                                "kind": "fixture-manifest",
                                "checked": True,
                            },
                            {
                                "path": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                                "kind": "stim-fixture",
                                "checked": True,
                            },
                            {
                                "path": "docs/showcases/rstim-vs-stim-simulator.md",
                                "kind": "showcase",
                                "checked": True,
                            },
                        ],
                        "commands": [
                            "python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                            "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness --cases benchmarks/rstim_vs_stim_simulator/cases.full.toml --shots 1024 --out /tmp/rstim-vs-stim-correctness.json",
                            "cargo run -p rstim --bin rstim -- perf ci --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-vs-stim-perf-ci",
                            "cp /tmp/rstim-vs-stim-perf-ci/summary.json benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
                            "cp /tmp/rstim-vs-stim-perf-ci/report.md benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
                            "cp /tmp/rstim-vs-stim-correctness.json benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                        ],
                        "provenance": rstim_vs_stim_provenance(
                            [
                                "python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                                "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness --cases benchmarks/rstim_vs_stim_simulator/cases.full.toml --shots 1024 --out /tmp/rstim-vs-stim-correctness.json",
                                "cargo run -p rstim --bin rstim -- perf ci --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-vs-stim-perf-ci",
                                "cp /tmp/rstim-vs-stim-perf-ci/summary.json benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
                                "cp /tmp/rstim-vs-stim-perf-ci/report.md benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
                                "cp /tmp/rstim-vs-stim-correctness.json benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                            ],
                            rstim_vs_stim_hashes,
                        ),
                        "provenance_requirements": [
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
                        ],
                        "provenance_sources": [
                            "docs/showcases/rstim-vs-stim-simulator.md",
                            "benchmarks/rstim_vs_stim_simulator/README.md",
                        ],
                        "caveats": [
                            "Partial checked evidence for recorded workloads and recorded environments.",
                            "This is not broad rstim/Stim parity.",
                        ],
                        "claims_limit": "Partial checked evidence for recorded workloads and recorded environments only, not broad rstim/Stim parity.",
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
                        "commands": ["cargo test"],
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": [".github/workflows/ci.yml"],
                        "claims_limit": "Regression gate evidence only.",
                    }
                ],
            },
        ],
    }
    manifest_text = json.dumps(manifest, indent=2) + "\n"
    write_text(repo_root / "site/benchmark-site.json", manifest_text)
    write_text(site_root / "data/benchmark-site.json", manifest_text)

    for relative, text in {
        "styles.css": "body { font-family: sans-serif; }\n",
        "QP101-ZY.md": "# QP101-ZY\n",
        "qp101.schema.json": '{ "title": "QP101" }\n',
        "examples/basic.qp101.json": '{ "name": "basic" }\n',
        "examples/repeat-detector.qp101.json": '{ "name": "repeat" }\n',
        "examples/atom-loss-sample.qp101.json": '{ "name": "atom-loss" }\n',
        "gallery/basic-site.svg": "<svg><title>basic</title></svg>\n",
        "gallery/repeat-detector-site.svg": "<svg><title>repeat</title></svg>\n",
        "gallery/atom-loss-sample.svg": "<svg><title>atom-loss</title></svg>\n",
        "benchmarks/surface_decoder_compare/results/full/results.csv": "distance,shots\n",
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png": "png\n",
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv": "distance,shots\n",
        "benchmarks/bb_circuit_bposd_compare/results/full/summary.md": "# Summary\n",
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png": "png\n",
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md": "# Gaps\n",
        "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json": '{"case":"d11"}\n',
        "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md": "# speed report\n",
        "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json": '{"status":"PASS"}\n',
        "benchmarks/rstim_vs_stim_simulator/cases.full.toml": "[[cases]]\nid = 'd11'\n",
        "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim": "M 0\n",
        "docs/showcases/rstim-vs-stim-simulator.md": "# rstim vs Stim\n",
    }.items():
        write_text(site_root / relative, text)

    write_text(
        site_root / "index.html",
        """<!doctype html>
<html lang="en">
<body>
  <nav>
    <a href="#docs-home">Overview</a>
    <a href="#workspace-overview">Workspace</a>
    <a href="#feature-walkthroughs">Walkthroughs</a>
    <a href="#benchmark-evidence">Evidence</a>
    <a href="#checked-benchmark-results">Results</a>
    <a href="#benchmarks">Methodology</a>
    <a href="#qp101">QP101</a>
    <a href="#operations">Operations</a>
    <a href="#gallery">Gallery</a>
    <a href="#examples">Examples</a>
  </nav>
  <section id="docs-home"></section>
  <section id="workspace-overview"></section>
  <section id="feature-walkthroughs"></section>
  <section id="benchmark-evidence"></section>
  <section id="checked-benchmark-results">
    <div id="checked-benchmark-result-cards"></div>
    <a href="benchmarks/surface_decoder_compare/results/full/results.csv">surface csv</a>
    <img src="benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png" alt="surface">
  </section>
  <section id="benchmarks">
    <h2>Benchmark Methodology</h2>
    <p>smoke checks verify wiring</p>
    <p>full evidence can describe the committed checked run only.</p>
    <p>checked full-tier artifacts are committed-run evidence.</p>
    <p>not a general decoder ordering claim.</p>
    <article><h3>Publishable Evidence</h3></article>
    <article><h3>Local-Only Evidence</h3></article>
    <article><h3>Claims Policy</h3></article>
    <div id="benchmark-manifest"></div>
  </section>
  <section id="qp101">
    <a href="qp101.schema.json">schema</a>
    <a href="QP101-ZY.md">protocol</a>
    <a href="examples/basic.qp101.json">basic</a>
  </section>
  <section id="schema-browser"></section>
  <section id="operations"></section>
  <section id="gallery">
    <img src="gallery/basic-site.svg" alt="basic">
    <img src="gallery/repeat-detector-site.svg" alt="repeat">
    <img src="gallery/atom-loss-sample.svg" alt="atom">
  </section>
  <section id="examples">
    <a href="examples/repeat-detector.qp101.json">repeat</a>
    <a href="examples/atom-loss-sample.qp101.json">atom loss</a>
  </section>
</body>
</html>
""",
    )
    write_text(
        site_root / "app.js",
        """const checkedBenchmarkItems = ["surface-decoder-full", "bb-circuit-full", "rstim-vs-stim-full"];
function renderBenchmarkManifest(manifest) { return manifest; }
function renderCheckedBenchmarkResults(manifest) { return manifest; }
function renderProvenance(provenance) { return provenance; }
const manifestMarkers = ["family.status", "family.claims_limit", "item.status", "item.claims_limit"];
const checkedMarkers = ["item.artifacts", "item.commands", "item.caveats", "artifact.checked"];
const provenanceMarkers = ["item.provenance", "renderProvenance(item.provenance)", "artifact_hashes"];
fetch("data/benchmark-site.json");
const localRefs = [
  "qp101.schema.json",
  "examples/basic.qp101.json",
  "examples/repeat-detector.qp101.json",
  "examples/atom-loss-sample.qp101.json",
  "gallery/basic-site.svg",
  "gallery/repeat-detector-site.svg",
  "gallery/atom-loss-sample.svg",
  "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
  "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
];
artifact.kind === "image";
""",
    )

    subprocess.run(["git", "init", "-q"], cwd=repo_root, check=True)
    subprocess.run(
        [
            "git",
            "add",
            ".gitignore",
            "docs/showcases/benchmark-evidence.md",
            "benchmarks/qec_code_random_window/README.md",
            ".github/workflows/ci.yml",
            "benchmarks/surface_decoder_compare/results/full/results.csv",
            "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
            "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
            "benchmarks/bb_circuit_bposd_compare/results/full/summary.md",
            "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
            "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
            "docs/showcases/rstim-vs-stim-simulator.md",
            "benchmarks/rstim_vs_stim_simulator/README.md",
            "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
            "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
            "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
            "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
            "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
            "site/benchmark-site.json",
        ],
        cwd=repo_root,
        check=True,
    )

    return SiteFixture(tempdir=tempdir, repo_root=repo_root, site_root=site_root)


def remove_surface_provenance(manifest_path: Path) -> None:
    manifest = check_site_manifest.load_json(manifest_path)
    if isinstance(manifest, dict):
        families = manifest.get("families")
        if isinstance(families, list):
            for family in families:
                if isinstance(family, dict) and family.get("id") == "surface-decoder-comparison":
                    items = family.get("evidence_items")
                    if isinstance(items, list) and items and isinstance(items[0], dict):
                        items[0].pop("provenance", None)
                        break
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def run_self_test() -> list[str]:
    failures: list[str] = []
    fixture = make_fixture_site()
    try:
        results = check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        if any(result.status == "FAIL" for result in results):
            failures.append("valid fixture should pass:\n" + format_summary(results))

        mutations: list[tuple[str, Callable[[SiteFixture], None], str]] = [
            ("missing_qp101_schema", lambda f: (f.site_root / "qp101.schema.json").unlink(), "qp101.schema.json"),
            (
                "missing_checked_plot",
                lambda f: (f.site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").unlink(),
                "surface_decoder_compare.png",
            ),
            (
                "missing_claims_policy",
                lambda f: (f.site_root / "index.html").write_text(
                    (f.site_root / "index.html").read_text(encoding="utf-8").replace("Claims Policy", "Claims"),
                    encoding="utf-8",
                ),
                "Claims Policy",
            ),
            (
                "unmanifested_checked_link",
                lambda f: (f.site_root / "index.html").write_text(
                    (f.site_root / "index.html").read_text(encoding="utf-8")
                    + '<a href="benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
                    encoding="utf-8",
                ),
                "not listed as a checked manifest artifact",
            ),
            (
                "missing_surface_provenance",
                lambda f: remove_surface_provenance(f.site_root / "data/benchmark-site.json"),
                "surface-decoder-full",
            ),
            (
                "corrupt_copied_checked_artifact",
                lambda f: (f.site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").write_text(
                    "mutated copied artifact\n",
                    encoding="utf-8",
                ),
                "benchmarks/surface_decoder_compare/results/full/results.csv",
            ),
            (
                "html_escape_outside_site",
                lambda f: (
                    (f.repo_root / "outside.txt").write_text("outside\n", encoding="utf-8"),
                    (f.site_root / "index.html").write_text(
                        (f.site_root / "index.html").read_text(encoding="utf-8").replace(
                            'href="QP101-ZY.md"', 'href="../outside.txt"', 1
                        ),
                        encoding="utf-8",
                    ),
                ),
                "path escape outside built site: ../outside.txt",
            ),
            (
                "js_escape_outside_site",
                lambda f: (
                    (f.repo_root / "outside.txt").write_text("outside\n", encoding="utf-8"),
                    (f.site_root / "app.js").write_text(
                        (f.site_root / "app.js").read_text(encoding="utf-8") + '\nconst escaped = "../outside.txt";\n',
                        encoding="utf-8",
                    ),
                ),
                "path escape outside built site: ../outside.txt",
            ),
            (
                "invalid_utf8_index",
                lambda f: (f.site_root / "index.html").write_bytes(b"\xff\xfe\xfa"),
                "index.html: invalid UTF-8",
            ),
            (
                "invalid_utf8_app",
                lambda f: (f.site_root / "app.js").write_bytes(b"\xff\xfe\xfa"),
                "app.js: invalid UTF-8",
            ),
        ]

        for name, mutate, marker in mutations:
            mutated = make_fixture_site()
            try:
                mutate(mutated)
                mutated_results = check_site_build(mutated.site_root, repo_root=mutated.repo_root)
                summary = format_summary(mutated_results)
                if name == "missing_surface_provenance":
                    mutation_failed = any(
                        result.status == "FAIL"
                        and result.area == "checked benchmark provenance"
                        and "surface-decoder-full" in result.detail
                        and "provenance" in result.detail
                        for result in mutated_results
                    )
                elif name == "corrupt_copied_checked_artifact":
                    mutation_failed = any(
                        result.status == "FAIL"
                        and result.area == "checked benchmark provenance"
                        and marker in result.detail
                        and "sha256" in result.detail
                        for result in mutated_results
                    )
                else:
                    mutation_failed = any(result.status == "FAIL" and marker in result.detail for result in mutated_results)
                if not mutation_failed:
                    failures.append(f"{name} did not fail as expected:\n{summary}")
            finally:
                mutated.cleanup()
    finally:
        fixture.cleanup()
    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("site_root", nargs="?", help="Built site root to validate")
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root for manifest validation")
    parser.add_argument("--self-test", action="store_true", help="Run the checker self-test")
    args = parser.parse_args(argv)

    if args.self_test:
        failures = run_self_test()
        if failures:
            for failure in failures:
                print(f"FAIL self-test: {failure}")
            return 1
        print("PASS self-test: required fixture mutations fail and the valid fixture passes")
        return 0

    if not args.site_root:
        parser.error("site_root is required unless --self-test is used")

    results = check_site_build(Path(args.site_root), repo_root=args.repo_root)
    print(format_summary(results))
    return 0 if all(result.status != "FAIL" for result in results) else 1


if __name__ == "__main__":
    sys.exit(main())
