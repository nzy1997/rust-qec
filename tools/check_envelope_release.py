#!/usr/bin/env python3
"""Aggregate envelope release-readiness evidence into a promote/hold decision.

Collects the support-matrix result, the independent correctness suite result,
the resource-envelope result and one installed-envelope report per required
release target, then decides for every matrix decoder whether its declared
``proposed_release_maturity`` is earned. A decoder proposed as
``supported-candidate`` is promoted to ``supported`` only when every required
evidence artifact is present, passing, and revision-bound to the candidate;
otherwise the decoder stays ``beta`` with explicit blocking gaps and the gate
fails.

Binding rule: every generated evidence artifact must record the candidate
source revision (``checkout_revision`` for source-tree evidence,
``source_revision`` for installed-archive reports). A mismatch is a blocking
gap unless a ``--equivalence`` record documents why the two revisions are
source-equivalent for the evidence in question. The support matrix is a
committed file, so its ``applies_to.source_revision`` must instead be an
ancestor of (or equal to) the candidate revision.

Usage:
    python3 tools/check_envelope_release.py \
      --evidence-dir drafts/envelope-readiness \
      --matrix docs/envelope-support.json \
      --policy docs/envelope-compatibility-policy.md \
      --candidate-revision "$(git rev-parse HEAD)" \
      --out drafts/envelope-readiness/release-gate.json
    python3 tools/check_envelope_release.py --self-test \
      --evidence-dir drafts/envelope-readiness
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_envelope_support as support  # noqa: E402
from tools import envelope_mle_scope as mle_scope  # noqa: E402
from benchmarks.atom_loss import mle_candidate_resources as mle_resource_campaign  # noqa: E402
from benchmarks.atom_loss import retained_source  # noqa: E402

SCHEMA_VERSION = "rustqec.envelope-release-report.v1"
EQUIVALENCE_SCHEMA = "rustqec.source-equivalence.v1"
PASS_LINE = "PASS envelope release readiness"
DEFAULT_MATRIX = Path("docs/envelope-support.json")
DEFAULT_POLICY = Path("docs/envelope-compatibility-policy.md")
DEFAULT_MLE_SCOPE = Path("docs/envelope-mle-scope.json")
DEFAULT_TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin")

# Evidence artifacts the gate consumes, relative to the evidence directory.
GENERATED_EVIDENCE = {
    "support": ("support.json", support.RESULT_SCHEMA_VERSION),
    "correctness": ("correctness.json", "rustqec.envelope-readiness-correctness.v1"),
    "resources": ("resources.json", "rustqec.envelope-readiness-resources.v1"),
}
INSTALLED_SCHEMA = "rustqec.installed-envelope-report.v1"

# Sections the compatibility policy must keep for the promise to be checkable.
REQUIRED_POLICY_SECTIONS = (
    "## Frozen CLI arguments",
    "## Dataset interpretation",
    "## Prediction packing",
    "## Structured error codes",
    "## Statistics semantics",
    "## Compatible evolution",
    "## Breaking changes",
    "## Deprecation notices",
)


class GateError(Exception):
    pass


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def candidate_revision() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], capture_output=True, text=True,
        cwd=REPO_ROOT, check=False,
    )
    revision = result.stdout.strip()
    if result.returncode or not revision:
        raise GateError("cannot resolve the candidate revision; pass --candidate-revision")
    return revision


def is_ancestor(ancestor: str, descendant: str, repo_root: Path = REPO_ROOT) -> bool | None:
    """True/False when git can decide, None when it cannot."""
    if ancestor == descendant:
        return True
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=repo_root, check=False,
    )
    if result.returncode == 0:
        return True
    if result.returncode == 1:
        return False
    return None


def load_equivalence(path: Path | None) -> list[dict[str, Any]]:
    if path is None:
        return []
    record = load_json(path)
    if record.get("schema_version") != EQUIVALENCE_SCHEMA:
        raise GateError(f"unsupported equivalence schema: {record.get('schema_version')!r}")
    pairs = record.get("pairs")
    if not isinstance(pairs, list):
        raise GateError("equivalence record must declare pairs")
    for pair in pairs:
        if not pair.get("evidence_revision") or not pair.get("candidate_revision"):
            raise GateError("equivalence pair missing revisions")
        if not pair.get("checks"):
            raise GateError("equivalence pair must record the checks performed")
    return pairs


def equivalence_covers(pairs: list[dict[str, Any]], evidence_revision: str, candidate: str) -> bool:
    return any(
        pair["evidence_revision"] == evidence_revision
        and pair["candidate_revision"] == candidate
        for pair in pairs
    )


def check_policy(path: Path, gaps: list[str]) -> dict[str, Any]:
    if not path.is_file():
        gaps.append(f"compatibility policy missing: {path}")
        return {"path": str(path), "present": False}
    text = path.read_text(encoding="utf-8")
    missing = [section for section in REQUIRED_POLICY_SECTIONS if section not in text]
    for section in missing:
        gaps.append(f"compatibility policy missing required section: {section!r}")
    return {
        "path": str(path),
        "present": True,
        "sha256": support.sha256_file(path),
        "required_sections_missing": missing,
    }


def check_matrix_binding(matrix: dict[str, Any], candidate: str, gaps: list[str]) -> dict[str, Any]:
    pinned = matrix.get("applies_to", {}).get("source_revision")
    record: dict[str, Any] = {"matrix_source_revision": pinned, "candidate_revision": candidate}
    if not pinned:
        gaps.append("matrix applies_to.source_revision is missing")
        record["binding"] = "missing"
        return record
    decision = is_ancestor(pinned, candidate)
    if decision is True:
        record["binding"] = "ancestor-or-equal"
    elif decision is False:
        gaps.append(
            f"matrix applies_to.source_revision {pinned} is not an ancestor of the "
            f"candidate {candidate}; re-pin the matrix before gating this candidate"
        )
        record["binding"] = "not-an-ancestor"
    else:
        gaps.append(
            "cannot verify matrix ancestry (git merge-base unavailable); "
            "the gate requires a git checkout to bind the matrix to the candidate"
        )
        record["binding"] = "unverifiable"
    return record


def check_generated_evidence(
    name: str,
    path: Path,
    schema: str,
    candidate: str,
    pairs: list[dict[str, Any]],
    gaps: list[str],
) -> dict[str, Any] | None:
    if not path.is_file():
        gaps.append(f"missing {name} evidence: {path}")
        return None
    evidence = load_json(path)
    if evidence.get("schema_version") != schema:
        gaps.append(
            f"{name} evidence schema mismatch: expected {schema}, "
            f"observed {evidence.get('schema_version')!r}"
        )
        return None
    revision = evidence.get("checkout_revision")
    bound = revision == candidate
    waived = False
    if not bound:
        waived = bool(revision) and equivalence_covers(pairs, str(revision), candidate)
        if not waived:
            gaps.append(
                f"{name} evidence revision {revision!r} is not the candidate {candidate}; "
                "regenerate the evidence at the candidate revision or record a "
                "source-equivalence check"
            )
    if evidence.get("status") != "pass":
        gaps.append(f"{name} evidence status is {evidence.get('status')!r}, not 'pass'")
    return {
        "path": str(path),
        "sha256": support.sha256_file(path),
        "schema_version": schema,
        "checkout_revision": revision,
        "revision_bound": bound,
        "equivalence_waiver": waived,
        "status": evidence.get("status"),
        "evidence": evidence,
    }


def check_installed_report(
    target: str,
    path: Path,
    candidate: str,
    pairs: list[dict[str, Any]],
    gaps: list[str],
) -> dict[str, Any] | None:
    if not path.is_file():
        gaps.append(f"missing installed-envelope report for {target}: {path}")
        return None
    report = load_json(path)
    if report.get("schema_version") != INSTALLED_SCHEMA:
        gaps.append(
            f"installed report {path} schema mismatch: expected {INSTALLED_SCHEMA}, "
            f"observed {report.get('schema_version')!r}"
        )
        return None
    if report.get("target") != target:
        gaps.append(
            f"installed report {path} is for target {report.get('target')!r}, not {target}"
        )
    revision = report.get("source_revision")
    bound = revision == candidate
    waived = False
    if not bound:
        waived = bool(revision) and equivalence_covers(pairs, str(revision), candidate)
        if not waived:
            gaps.append(
                f"installed report for {target} tested revision {revision!r}, not the "
                f"candidate {candidate}; test an archive built from the candidate or record "
                "a source-equivalence check"
            )
    if report.get("status") != "pass":
        gaps.append(f"installed report for {target} status is {report.get('status')!r}")
    ilp = report.get("ilp", {})
    if ilp.get("expected") != ilp.get("available"):
        gaps.append(
            f"installed report for {target}: ilp expected={ilp.get('expected')} "
            f"available={ilp.get('available')}; official release archives are ILP-capable"
        )
    return {
        "path": str(path),
        "sha256": support.sha256_file(path),
        "portable_sha256": json_document_sha256(portable_installed_report(report)),
        "target": target,
        "source_revision": revision,
        "revision_bound": bound,
        "equivalence_waiver": waived,
        "status": report.get("status"),
        "report": report,
    }


def portable_installed_report(report: dict[str, Any]) -> dict[str, Any]:
    """Machine-independent installed report retained in publication bundles."""
    archive = report.get("archive") or {}
    return {
        "schema_version": report["schema_version"],
        "target": report["target"],
        "source_revision": report["source_revision"],
        "ilp": report["ilp"],
        "matrix": {
            "sha256": report["matrix"]["sha256"],
            "schema_version": report["matrix"]["schema_version"],
        },
        "binary": {
            "sha256": report["binary"]["sha256"],
            "version": report["binary"]["version"],
            "advertised_decoders": report["binary"]["advertised_decoders"],
        },
        "archive": {
            "filename": Path(str(archive.get("path") or archive.get("filename", ""))).name,
            "sha256": archive.get("sha256"),
        },
        "controls": report.get("controls", []),
        "previous_fixture_semantics": report.get("previous_fixture_semantics"),
        "status": report["status"],
        "problems": report.get("problems", []),
    }


def json_document_sha256(document: dict[str, Any]) -> str:
    payload = json.dumps(document, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


# Coverage contracts mirrored from the suites that produce the evidence. Keep
# these in sync with benchmarks/atom_loss/readiness_correctness.py; the gate
# intentionally fails loudly when the suite's structure drifts.
REQUIRED_NOISE_CHANNELS = ("X_ERROR", "DEPOLARIZE1", "DEPOLARIZE2")
REQUIRED_HISTORY_CATEGORIES = (
    "no_loss",
    "multiple_simultaneous_losses",
    "loss_in_different_rounds",
    "reset_restoring_wire",
)
RETAINED_MANIFEST_SCHEMA = "rustqec.envelope-resources-manifest.v1"
DEFAULT_RETAINED_REPORT = Path("benchmarks/atom_loss/readiness/resources/manifest.json")

# MLE candidate-scope contracts mirrored from docs/envelope-mle-scope.json and
# benchmarks/atom_loss/mle_candidate_resources.py; the gate intentionally
# fails loudly when the plan or the campaign drifts from these values.
MLE_RESOURCES_SCHEMA = mle_resource_campaign.SCHEMA
MLE_EXPECTED_BUDGET = mle_resource_campaign.BUDGET
MLE_REQUIRED_WORKLOADS = tuple(
    workload["id"] for workload in mle_resource_campaign.WORKLOADS
)
MLE_FAILURE_CODES = {
    case_id: expectation[0]
    for case_id, expectation in mle_resource_campaign.EXPECTED_FAILURE_CODES.items()
}

# The retained full resource report supports promotion only for candidates
# whose measurement-relevant sources are identical to the measured revision.
# This covers the measurement driver, its transitive helper modules (corpus
# and circuit generation, dataset packaging, workload execution), the input
# fixtures the workloads consume, and the build configuration that affects
# the measured performance.
RETAINED_EQUIVALENCE_PATHS = retained_source.PATHS


def measurement_matrix_projection(matrix: dict[str, Any]) -> dict[str, Any]:
    """Return only matrix fields that can change the measured resource claim.

    Publication URLs, asset names, and prose do not affect the circuits or
    workloads used by the retained campaign. Keep every other field so a
    changed contract, control, scope, or decoder configuration still makes
    the retained measurements stale.
    """
    projected = json.loads(json.dumps(matrix))
    projected.pop("purpose", None)
    # The source revision is provenance metadata, not a measurement input.
    # History cleanup may replace it with the surviving equivalent commit
    # without changing the controls or workloads described by the matrix.
    applies_to = projected.get("applies_to")
    if isinstance(applies_to, dict):
        applies_to.pop("source_revision", None)
        if not applies_to:
            projected.pop("applies_to", None)
    for decoder in (projected.get("decoders") or {}).values():
        if isinstance(decoder, dict):
            decoder.pop("published_support", None)
    return projected


def git_json(revision: str, path: str, repo_root: Path) -> dict[str, Any] | None:
    result = subprocess.run(
        ["git", "show", f"{revision}:{path}"],
        capture_output=True, text=True, cwd=repo_root, check=False,
    )
    if result.returncode:
        return None
    try:
        document = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return document if isinstance(document, dict) else None


def expected_controls(matrix: dict[str, Any], decoder: str) -> set[str]:
    """Every control the matrix declares for this decoder (variants expanded)."""
    return {
        control["id"]
        for control in support.expand_controls(matrix)
        if decoder in control["decoders"]
    }


def support_gaps(decoder: str, results: list[dict[str, Any]], matrix: dict[str, Any],
                 label: str) -> list[str]:
    """The complete declared control set must be executed and passing."""
    expected = expected_controls(matrix, decoder)
    observed: dict[str, Any] = {}
    for record in results:
        if record.get("decoder") == decoder:
            observed.setdefault(record.get("control"), record.get("status"))
    missing = sorted(control for control in expected if control not in observed)
    failing = sorted(
        control for control, status in observed.items()
        if control in expected and status != "pass"
    )
    gaps = []
    if missing:
        shown = ", ".join(missing[:3]) + ("..." if len(missing) > 3 else "")
        gaps.append(
            f"{label}: incomplete control set for {decoder}: "
            f"{len(observed)}/{len(expected)} controls executed, missing {shown}"
        )
    if failing:
        gaps.append(f"{label}: failing controls for {decoder}: {', '.join(failing[:3])}")
    return gaps


def correctness_case_gaps(decoder: str, cases: list[dict[str, Any]]) -> list[str]:
    """Per-decoder case details must be complete and internally consistent.

    Row-level quantities are checked against the case's own detail records, so
    a truncated prediction vector or a hand-written checked_rows cannot pass.
    """
    label = "correctness"
    gaps: list[str] = []
    end_to_end = [c for c in cases if c.get("evidence_level") == "independent-end-to-end"]
    if not end_to_end:
        gaps.append(f"{label}: no independent end-to-end case covers {decoder}")
    for case in end_to_end:
        name = case.get("name", "?")
        backend = (case.get("backends") or {}).get(decoder)
        if backend is None:
            gaps.append(f"{label}: case {name} did not execute {decoder}")
            continue
        rows = case.get("rows")
        allowed = (case.get("allowed_answers") or {}).get(decoder)
        if not isinstance(rows, int) or rows <= 0:
            gaps.append(f"{label}: case {name}: missing row count")
            continue
        if not isinstance(allowed, list) or len(allowed) != rows:
            gaps.append(
                f"{label}: case {name}: allowed-answer count does not match case rows "
                f"for {decoder}"
            )
            continue
        predictions = backend.get("predictions")
        if not isinstance(predictions, list) or len(predictions) != rows:
            gaps.append(
                f"{label}: case {name}: {decoder} prediction count "
                f"{len(predictions) if isinstance(predictions, list) else 'n/a'} "
                f"does not match case rows {rows}"
            )
        if backend.get("checked_rows") != rows:
            gaps.append(
                f"{label}: case {name}: {decoder} checked_rows "
                f"{backend.get('checked_rows')!r} does not match case rows {rows}"
            )
        singleton = sum(1 for answers in allowed if len(answers) == 1)
        if backend.get("unique_optimum_rows") != singleton:
            gaps.append(f"{label}: case {name}: {decoder} unique-optimum count inconsistent")
        if backend.get("allowed_tie_rows") != rows - singleton:
            gaps.append(f"{label}: case {name}: {decoder} allowed-tie count inconsistent")
        if isinstance(predictions, list) and len(predictions) == rows:
            half = rows // 2
            if backend.get("placeholder_invariance") != (predictions[:half] == predictions[half:]):
                gaps.append(
                    f"{label}: case {name}: {decoder} placeholder_invariance flag "
                    "inconsistent with the recorded predictions"
                )
            if backend.get("prediction_sha256") != hashlib.sha256(
                bytes(predictions)
            ).hexdigest():
                gaps.append(
                    f"{label}: case {name}: {decoder} prediction hash inconsistent "
                    "with the recorded predictions"
                )
        if backend.get("rejected_rows"):
            gaps.append(f"{label}: case {name}: {decoder} predictions outside allowed optima")
        if not backend.get("flipped_prediction_rejected"):
            gaps.append(f"{label}: case {name}: flipped-answer control missing for {decoder}")
        if not backend.get("placeholder_invariance"):
            gaps.append(f"{label}: case {name}: placeholder invariance not shown for {decoder}")
        if any(m.get("decoder") == decoder for m in case.get("mismatches") or []):
            gaps.append(f"{label}: case {name}: mismatches recorded for {decoder}")
    return gaps


def recomputed_coverage(cases: list[dict[str, Any]]) -> dict[str, Any]:
    """Recompute the coverage summary from case details, mirroring the suite."""
    categories: dict[str, int] = defaultdict(int)
    channels: set[str] = set()
    points = []
    placeholder_pairs = 0
    total_rows = 0
    checked_rows = 0
    tie_rows = 0
    for case in cases:
        for history in case.get("histories") or []:
            if history.get("exercised"):
                categories[history.get("category")] += 1
        channels |= set(case.get("noise_channels_exercised") or [])
        points.append({
            "case": case.get("name"),
            "detectors": case.get("detectors"),
            "evidence_level": case.get("evidence_level"),
        })
        placeholder_pairs += case.get("placeholder_pairs") or 0
        total_rows += case.get("rows") or 0
        if case.get("evidence_level") == "independent-end-to-end":
            for outcome in (case.get("backends") or {}).values():
                tie_rows += outcome.get("allowed_tie_rows") or 0
                checked_rows += outcome.get("checked_rows") or 0
    return {
        "history_categories": {c: categories[c] for c in REQUIRED_HISTORY_CATEGORIES},
        "single_loss_histories": categories["single_loss"],
        "noise_channels": sorted(channels),
        "circuit_points": points,
        "placeholder_pairs": placeholder_pairs,
        "total_rows": total_rows,
        "checked_rows": checked_rows,
        "allowed_tie_rows": tie_rows,
        "randomized_differential_seeds": [case.get("seed") for case in cases],
    }


def correctness_summary_gaps(evidence: dict[str, Any]) -> list[str]:
    """The declared coverage summary must equal the case details recomputed."""
    label = "correctness"
    gaps: list[str] = []
    cases = evidence.get("cases") or []
    declared = evidence.get("coverage") or {}
    expected = recomputed_coverage(cases)
    inconsistent = [
        field for field, value in expected.items() if declared.get(field) != value
    ]
    if inconsistent:
        gaps.append(
            f"{label}: declared coverage does not match the case details: "
            + ", ".join(inconsistent)
        )
    categories = expected["history_categories"]
    missing_categories = [c for c in REQUIRED_HISTORY_CATEGORIES if categories.get(c, 0) <= 0]
    if missing_categories:
        gaps.append(f"{label}: missing loss-history coverage: {', '.join(missing_categories)}")
    missing_channels = [
        c for c in REQUIRED_NOISE_CHANNELS if c not in expected["noise_channels"]
    ]
    if missing_channels:
        gaps.append(f"{label}: missing noise-channel coverage: {', '.join(missing_channels)}")
    if len(expected["circuit_points"]) < 2:
        gaps.append(f"{label}: missing circuit size/round coverage")
    if expected["placeholder_pairs"] <= 0:
        gaps.append(f"{label}: missing placeholder-invariance coverage")
    if expected["checked_rows"] <= 0:
        gaps.append(f"{label}: zero checked rows")
    if not expected["randomized_differential_seeds"]:
        gaps.append(f"{label}: no randomized differential seeds")
    return gaps


def resources_gaps(decoder: str, evidence: dict[str, Any]) -> list[str]:
    cases = evidence.get("cases") or []
    workloads = [
        c for c in cases
        if c.get("decoder") == decoder and c.get("kind") != "failure-semantics"
    ]
    gaps = []
    if not any(c.get("exit_code") == 0 and not c.get("output_rule_problems") for c in workloads):
        gaps.append(f"resource-envelope: no successful workload case for {decoder}")
    if not any(c.get("kind") == "cache-eviction" for c in workloads):
        gaps.append(f"resource-envelope: no cache-eviction case for {decoder}")
    return gaps


def decoder_coverage(
    decoder: str,
    generated: dict[str, dict[str, Any] | None],
    installed: dict[str, dict[str, Any] | None],
    matrix: dict[str, Any],
) -> list[str]:
    """Which evidence artifacts demonstrably exercise this decoder, in full."""
    missing: list[str] = []
    support_result = (generated.get("support") or {}).get("evidence")
    if support_result is None:
        missing.append("support-matrix: report unavailable")
    else:
        missing += support_gaps(decoder, support_result.get("results", []), matrix,
                                "support-matrix")
    correctness = (generated.get("correctness") or {}).get("evidence")
    if correctness is None:
        missing.append("correctness: report unavailable")
    else:
        missing += correctness_case_gaps(decoder, correctness.get("cases") or [])
    resources = (generated.get("resources") or {}).get("evidence")
    if resources is None:
        missing.append("resource-envelope: report unavailable")
    else:
        missing += resources_gaps(decoder, resources)
    for target, record in installed.items():
        report = (record or {}).get("report")
        if report is None:
            missing.append(f"installed:{target}")
        else:
            missing += support_gaps(decoder, report.get("controls", []), matrix,
                                    f"installed:{target}")
    return missing


def check_retained_report(path: Path, candidate: str, gaps: list[str],
                          repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    """Bind the committed full resource report to the candidate's sources.

    The measured commit may be absent after a squash merge. The retained Git
    input inventory must still match the candidate exactly, and the measured
    commit's tree is checked too whenever that object is available.
    """
    record: dict[str, Any] = {"path": str(path)}
    if not path.is_file():
        gaps.append(f"retained full resource report missing: {path}")
        record["present"] = False
        return record
    record["present"] = True
    record["sha256"] = support.sha256_file(path)
    manifest = load_json(path)
    if manifest.get("schema_version") != RETAINED_MANIFEST_SCHEMA:
        gaps.append(
            f"retained resource report schema mismatch: {manifest.get('schema_version')!r}"
        )
        return record
    if manifest.get("status") != "pass":
        gaps.append("retained resource report does not record a passing campaign")
    revision = manifest.get("checkout_revision")
    record["checkout_revision"] = revision
    if (not isinstance(revision, str) or len(revision) != 40
            or any(char not in "0123456789abcdef" for char in revision)
            or set(revision) == {"0"}):
        gaps.append("retained resource report revision is missing or invalid")
        record["binding"] = "missing"
        return record
    measured_inputs = manifest.get("source_inputs")
    if not isinstance(measured_inputs, dict) or not measured_inputs:
        gaps.append("retained resource report source inventory is missing")
        record["binding"] = "missing-inventory"
        return record
    source_available = subprocess.run(
        ["git", "-C", str(repo_root), "cat-file", "-e", revision + "^{commit}"],
        capture_output=True, check=False,
    ).returncode == 0
    try:
        if source_available and retained_source.inventory(repo_root, revision) != measured_inputs:
            gaps.append("retained resource report source inventory differs from measured revision")
            record["binding"] = "invalid-inventory"
            return record
        candidate_inputs = retained_source.inventory(repo_root, candidate)
    except (subprocess.CalledProcessError, ValueError):
        gaps.append("cannot verify retained-report source equivalence (git inventory failed)")
        record["binding"] = "unverifiable"
        return record
    changed = [name for name in sorted(measured_inputs.keys() | candidate_inputs.keys())
               if measured_inputs.get(name) != candidate_inputs.get(name)]
    matrix_path = "docs/envelope-support.json"
    if matrix_path in changed:
        measured_matrix = git_json(str(revision), matrix_path, repo_root) if source_available else None
        candidate_matrix = git_json(candidate, matrix_path, repo_root)
        if (measured_matrix is not None and candidate_matrix is not None
                and measurement_matrix_projection(measured_matrix)
                == measurement_matrix_projection(candidate_matrix)):
            changed.remove(matrix_path)
            record["ignored_publication_metadata_changes"] = [matrix_path]
    record["changed_equivalence_paths"] = changed
    if changed:
        shown = ", ".join(changed[:5]) + ("..." if len(changed) > 5 else "")
        gaps.append(
            f"retained full resource report was measured at {revision} but "
            f"measurement-relevant sources changed since: {shown}; rerun the full "
            "resource campaign at the candidate"
        )
        record["binding"] = "sources-differ"
    else:
        record["binding"] = "source-equivalent"
    return record


def check_mle_scope_plan(
    path: Path | None,
    candidate: str,
    gaps: list[str],
) -> dict[str, Any] | None:
    """Load and bind the MLE candidate scope plan to the candidate revision."""
    if path is None:
        return None
    if not path.is_file():
        gaps.append(f"MLE scope plan missing: {path}")
        return None
    try:
        plan = mle_scope.load_plan(path)
    except (mle_scope.ScopeError, json.JSONDecodeError) as error:
        gaps.append(f"MLE scope plan invalid: {error}")
        return None
    pinned = plan.get("applies_to", {}).get("source_revision")
    if not pinned:
        gaps.append("MLE scope plan applies_to.source_revision is missing")
    else:
        decision = is_ancestor(str(pinned), candidate)
        if decision is not True:
            gaps.append(
                f"MLE scope plan revision {pinned} is not an ancestor of the "
                f"candidate {candidate}; re-pin the plan before gating this candidate"
            )
    return plan


def mle_plan_gaps(
    plan: dict[str, Any],
    generated: dict[str, dict[str, Any] | None],
    installed: dict[str, dict[str, Any] | None],
    plan_path: Path,
) -> list[str]:
    """Map the scope plan's required case IDs onto the actual evidence.

    Every required case must be fulfilled by evidence of its own declared
    kind and shape: compiler-only output never satisfies an end-to-end
    requirement, Matching-only or synthetic runs never count as real-circuit
    MLE coverage, budgets fixed before measurement are enforced against the
    recorded values, and both installed reports must be ILP-capable.
    """
    label = "mle-scope"
    gaps: list[str] = []
    correctness = (generated.get("correctness") or {}).get("evidence")
    support_result = (generated.get("support") or {}).get("evidence")
    mle_resources = (generated.get("mle-resources") or {}).get("evidence")
    corr_cases = {c.get("name"): c for c in (correctness or {}).get("cases", [])}
    support_results = {
        (r.get("control"), r.get("decoder")): r
        for r in (support_result or {}).get("results", [])
    }
    res_cases = {c.get("id"): c for c in (mle_resources or {}).get("cases", [])}

    if mle_resources is not None:
        gaps.extend(
            f"{label}: {problem}"
            for problem in mle_resource_campaign.verify_document(
                mle_resources, plan_path
            )
        )

    declared_workloads = {
        c["workload_id"] for c in plan["required_cases"] if c["kind"] == "resource-workload"
    }
    if declared_workloads != set(MLE_REQUIRED_WORKLOADS):
        gaps.append(
            f"{label}: declared resource workloads {sorted(declared_workloads)} drift "
            f"from the gate's mirrored set {sorted(MLE_REQUIRED_WORKLOADS)}"
        )

    for case in plan["required_cases"]:
        cid, kind = case["id"], case["kind"]
        if kind == "correctness-end-to-end":
            found = corr_cases.get(case["case_name"])
            if found is None:
                gaps.append(f"{label}: required correctness case {case['case_name']} "
                            "not executed")
            elif found.get("evidence_level") != "independent-end-to-end":
                gaps.append(f"{label}: correctness case {case['case_name']} evidence "
                            f"level {found.get('evidence_level')!r} is not "
                            "independent-end-to-end; compiler-only evidence cannot "
                            "fulfil this requirement")
            elif "envelope-mle" not in (found.get("backends") or {}):
                gaps.append(f"{label}: correctness case {case['case_name']} did not "
                            "execute envelope-mle")
            elif found.get("status") != "pass":
                gaps.append(f"{label}: correctness case {case['case_name']} is not passing")
            else:
                expected_identity = case.get("evidence_identity") or {}
                for field in ("source", "circuit_sha256", "seed"):
                    if found.get(field) != expected_identity.get(field):
                        gaps.append(
                            f"{label}: correctness case {case['case_name']} {field} "
                            f"{found.get(field)!r} does not match pinned identity "
                            f"{expected_identity.get(field)!r}"
                        )
                if found.get("circuit_params") != case.get("circuit_params"):
                    gaps.append(
                        f"{label}: correctness case {case['case_name']} circuit_params "
                        "do not match the pinned plan"
                    )
        elif kind == "correctness-compiler-only":
            found = corr_cases.get(case["case_name"])
            if found is None:
                gaps.append(f"{label}: supporting compiler-output case {case['case_name']} "
                            "not executed")
            elif found.get("evidence_level") != "compiler-output-only":
                gaps.append(f"{label}: case {case['case_name']} evidence level "
                            f"{found.get('evidence_level')!r} is not compiler-output-only")
            else:
                expected_identity = case.get("evidence_identity") or {}
                for field in ("source", "circuit_sha256", "seed"):
                    if found.get(field) != expected_identity.get(field):
                        gaps.append(
                            f"{label}: compiler-only case {case['case_name']} {field} "
                            "does not match the pinned identity"
                        )
                if found.get("circuit_params") != case.get("circuit_params"):
                    gaps.append(
                        f"{label}: compiler-only case {case['case_name']} "
                        "circuit_params do not match the pinned plan"
                    )
        elif kind == "support-control":
            record = support_results.get((case["matrix_control"], "envelope-mle"))
            if record is None:
                gaps.append(f"{label}: matrix control {case['matrix_control']} not "
                            "executed for envelope-mle")
            elif record.get("status") != "pass":
                gaps.append(f"{label}: matrix control {case['matrix_control']} is not "
                            "passing for envelope-mle")
        elif kind == "resource-workload":
            if mle_resources is None:
                gaps.append(f"{label}: MLE candidate resource report unavailable; "
                            "run benchmarks/atom_loss/mle_candidate_resources.py")
                break
            record = res_cases.get(case["workload_id"])
            if record is None:
                gaps.append(f"{label}: required workload {case['workload_id']} not measured")
                continue
            if record.get("exit_code") != 0 or record.get("output_rule_problems"):
                gaps.append(f"{label}: workload {case['workload_id']} did not succeed "
                            "cleanly")
            if record.get("wall_seconds", float("inf")) > \
                    MLE_EXPECTED_BUDGET["per_case_wall_seconds"]:
                gaps.append(f"{label}: workload {case['workload_id']} wall "
                            f"{record.get('wall_seconds'):.1f}s exceeds the declared "
                            "budget; narrow the candidate promise explicitly instead "
                            "of relabeling omitted evidence")
            if bool(record.get("real_circuit")) != bool(case.get("real_circuit")):
                gaps.append(f"{label}: workload {case['workload_id']} real_circuit "
                            "flag disagrees with the plan")
            expected_params = case.get("circuit_params") or {}
            params = record.get("circuit_params") or {}
            for field in ("distance", "rounds", "loss_rate", "batch"):
                if params.get(field) != expected_params.get(field):
                    gaps.append(f"{label}: workload {case['workload_id']} measured "
                                f"{field}={params.get(field)!r}, plan declares "
                                f"{expected_params.get(field)!r}")
        elif kind == "failure-semantics":
            record = res_cases.get(case["workload_id"]) if mle_resources else None
            code = MLE_FAILURE_CODES.get(case["workload_id"])
            if record is None:
                gaps.append(f"{label}: failure-semantics case {case['workload_id']} "
                            "not executed")
            elif record.get("error_code") != code or record.get("completed_shots") != 0:
                gaps.append(f"{label}: failure-semantics case {case['workload_id']} "
                            f"expected {code} with zero completed shots")
            elif case["workload_id"] == "fail-mle-solve-timeout" and \
                    not record.get("compilation_outside_timeout"):
                gaps.append(f"{label}: {case['workload_id']} must show compilation "
                            "outside the solve-phase timeout")
        elif kind == "installed-platform":
            report = (installed.get(case["target"]) or {}).get("report")
            if report is not None:
                ilp = report.get("ilp", {})
                if ilp.get("expected") is not True or ilp.get("available") is not True:
                    gaps.append(
                        f"{label}: installed report for {case['target']} is not an "
                        f"ILP-capable expectation (expected={ilp.get('expected')}, "
                        f"available={ilp.get('available')}); MLE Supported requires "
                        "ILP-capable archives on both native targets"
                    )
    return gaps


def evaluate(
    evidence_dir: Path,
    matrix_path: Path,
    policy_path: Path,
    candidate: str,
    targets: tuple[str, ...],
    equivalence_path: Path | None,
    retained_report_path: Path | None = None,
    mle_scope_path: Path | None = DEFAULT_MLE_SCOPE,
) -> dict[str, Any]:
    matrix = support.load_matrix(matrix_path)
    pairs = load_equivalence(equivalence_path)
    gaps: list[str] = []
    if mle_scope_path is not None and not Path(mle_scope_path).is_absolute():
        mle_scope_path = REPO_ROOT / mle_scope_path

    matrix_record = check_matrix_binding(matrix, candidate, gaps)
    policy_record = check_policy(policy_path, gaps)
    if retained_report_path is None:
        retained_report_path = REPO_ROOT / DEFAULT_RETAINED_REPORT
    retained_record = check_retained_report(retained_report_path, candidate, gaps)

    # The MLE Supported proposal lives in the scope plan (issue #721); the
    # matrix may also propose MLE directly at publication time. Either way the
    # plan must be present, valid and fulfilled. MLE-specific gaps block only
    # the MLE decision: MLE validation must never hold back a Matching
    # release (issue #723).
    matrix_mle_proposed = (
        matrix["decoders"].get("envelope-mle", {}).get("proposed_release_maturity")
        == "supported-candidate"
    )
    mle_gaps: list[str] = []
    mle_plan = check_mle_scope_plan(mle_scope_path, candidate, mle_gaps)
    plan_mle_proposed = bool(
        mle_plan
        and mle_plan.get("maturity", {}).get("proposed_release_maturity")
        == "supported-candidate"
    )
    mle_enforced = matrix_mle_proposed or plan_mle_proposed
    if mle_enforced and mle_plan is None:
        mle_gaps.append(
            "an MLE Supported proposal requires a valid MLE scope plan "
            f"({mle_scope_path}); the proposal is rejected until the plan exists"
        )
    if mle_plan is not None:
        plan_current = mle_plan.get("maturity", {}).get("current")
        matrix_current = matrix["decoders"].get("envelope-mle", {}).get("current_maturity")
        if matrix_current == "supported" and plan_current != "supported":
            mle_gaps.append(
                "matrix promotes envelope-mle to supported ahead of the scope plan "
                f"(plan maturity.current={plan_current!r}); publish the plan's "
                "promotion record together with the matrix claim"
            )

    generated: dict[str, dict[str, Any] | None] = {}
    for name, (filename, schema) in GENERATED_EVIDENCE.items():
        generated[name] = check_generated_evidence(
            name, evidence_dir / filename, schema, candidate, pairs, gaps
        )
    if mle_enforced:
        generated["mle-resources"] = check_generated_evidence(
            "mle-resources", evidence_dir / "mle-resources.json",
            MLE_RESOURCES_SCHEMA, candidate, pairs, mle_gaps
        )
    correctness_evidence = (generated.get("correctness") or {}).get("evidence")
    if correctness_evidence is not None:
        gaps += correctness_summary_gaps(correctness_evidence)
    installed: dict[str, dict[str, Any] | None] = {}
    for target in targets:
        installed[target] = check_installed_report(
            target, evidence_dir / f"installed-{target}.json", candidate, pairs, gaps
        )

    # Every gap recorded so far (evidence status, revision binding, retained
    # report, matrix, policy, correctness summary) blocks promotion: a decoder
    # must never read "supported" while its evidence base is failing.
    evidence_gaps = list(gaps)
    decisions: dict[str, Any] = {}
    for decoder, declaration in matrix["decoders"].items():
        proposed = declaration.get("proposed_release_maturity")
        if decoder == "envelope-mle" and mle_enforced:
            proposed = "supported-candidate"
        coverage_gaps = decoder_coverage(decoder, generated, installed, matrix)
        blocking = list(coverage_gaps)
        if proposed == "supported-candidate":
            blocking += evidence_gaps
            if decoder == "envelope-mle":
                blocking += mle_gaps
                if mle_plan is not None:
                    blocking += mle_plan_gaps(
                        mle_plan, generated, installed, Path(mle_scope_path)
                    )
        else:
            # Beta decoders do not gate the release, but their evidence state
            # is still reported.
            blocking = []
        earned = proposed == "supported-candidate" and not blocking
        if proposed == "supported-candidate" and blocking:
            shown = "; ".join(blocking[:4]) + ("..." if len(blocking) > 4 else "")
            gaps.append(
                f"decoder {decoder} proposed as supported-candidate is held at beta: "
                + shown
            )
        decision_record = {
            "current_maturity": declaration.get("current_maturity"),
            "proposed_release_maturity": proposed,
            "decision": "supported" if earned else "beta",
            "coverage_gaps": coverage_gaps,
            "blocking_gaps": blocking,
        }
        if decoder == "envelope-mle":
            decision_record["scope_plan"] = (
                {
                    "path": str(mle_scope_path),
                    "proposed_release_maturity": mle_plan.get("maturity", {}).get(
                        "proposed_release_maturity"),
                    "enforced": mle_enforced,
                }
                if mle_plan is not None
                else {"path": str(mle_scope_path), "enforced": mle_enforced,
                      "present": False}
            )
        decisions[decoder] = decision_record

    status = "pass" if not gaps else "fail"
    return {
        "schema_version": SCHEMA_VERSION,
        "candidate_revision": candidate,
        "required_targets": list(targets),
        "matrix": {
            "path": str(matrix_path),
            "sha256": support.sha256_file(matrix_path),
            **matrix_record,
        },
        "compatibility_policy": policy_record,
        "retained_resource_report": retained_record,
        "mle_scope_plan": {
            "path": str(mle_scope_path),
            "present": mle_plan is not None,
            "enforced": mle_enforced,
            "sha256": (
                support.sha256_file(Path(mle_scope_path)) if mle_plan is not None else None
            ),
            "source_revision": (
                mle_plan.get("applies_to", {}).get("source_revision")
                if mle_plan is not None else None
            ),
        },
        "evidence": {
            name: ({k: v for k, v in record.items() if k != "evidence"} if record else None)
            for name, record in generated.items()
        },
        "installed": {
            target: ({k: v for k, v in record.items() if k != "report"} if record else None)
            for target, record in installed.items()
        },
        "decoders": decisions,
        "status": status,
        "blocking_gaps": gaps,
    }


def self_test(evidence_dir: Path, matrix_path: Path, policy_path: Path, candidate: str,
              retained_report_path: Path | None = None,
              mle_scope_path: Path | None = DEFAULT_MLE_SCOPE) -> int:
    """The gate must reject defective evidence bundles."""
    baseline = evaluate(evidence_dir, matrix_path, policy_path, candidate,
                        DEFAULT_TARGETS, None, retained_report_path, mle_scope_path)
    observations: list[dict[str, Any]] = [
        {"mutation": "baseline", "status": baseline["status"],
         "detail": baseline["blocking_gaps"][:1]},
    ]
    if baseline["status"] != "pass":
        print("FAIL envelope release readiness self-test: baseline evidence does not pass; "
              "generate the full evidence bundle first", file=sys.stderr)
        return 1

    def mutate(name: str, transform, needle: str) -> bool:
        with tempfile.TemporaryDirectory(prefix="envelope-release-selftest-") as temporary:
            clone = Path(temporary) / "evidence"
            shutil.copytree(evidence_dir, clone)
            retained = transform(clone, Path(temporary))
            report = evaluate(clone, matrix_path, policy_path, candidate,
                              DEFAULT_TARGETS, None, retained, mle_scope_path)
            rejected = report["status"] == "fail" and any(
                needle in gap for gap in report["blocking_gaps"]
            )
            observations.append({
                "mutation": name,
                "rejected": rejected,
                "detail": report["blocking_gaps"][:2],
            })
            return rejected

    def drop_linux(clone: Path, temporary: Path) -> Path | None:
        (clone / f"installed-{DEFAULT_TARGETS[0]}.json").unlink()
        return None

    first = mutate("missing-platform-report", drop_linux, "missing installed-envelope report")

    def mismatch_revision(clone: Path, temporary: Path) -> Path | None:
        path = clone / "support.json"
        evidence = load_json(path)
        evidence["checkout_revision"] = "0" * 40
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    second = mutate("revision-mismatch", mismatch_revision, "is not the candidate")

    def fail_correctness(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        evidence["status"] = "fail"
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    third = mutate("failing-evidence", fail_correctness, "status is")

    def hollow_correctness(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        evidence["cases"] = []
        evidence["coverage"] = {}
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    fourth = mutate("hollowed-correctness", hollow_correctness, "correctness")

    def partial_support(clone: Path, temporary: Path) -> Path | None:
        path = clone / "support.json"
        evidence = load_json(path)
        evidence["results"] = [
            r for r in evidence["results"]
            if r.get("control") == "mini-circuit-known-answer"
        ]
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    fifth = mutate("partial-support-controls", partial_support, "incomplete control set")

    def stale_retained(clone: Path, temporary: Path) -> Path:
        source = retained_report_path or REPO_ROOT / DEFAULT_RETAINED_REPORT
        clone_retained = temporary / "retained-manifest.json"
        manifest = load_json(source)
        manifest["checkout_revision"] = "0" * 40
        clone_retained.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        return clone_retained

    sixth = mutate("retained-report-revision-mismatch", stale_retained,
                   "retained resource report revision")

    def truncate_predictions(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            for backend in (case.get("backends") or {}).values():
                predictions = backend.get("predictions")
                if predictions:
                    backend["predictions"] = [predictions[0]]
                backend["checked_rows"] = 1 if predictions else 0
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    seventh = mutate("truncated-decoder-predictions", truncate_predictions,
                     "does not match case rows")

    def fudge_coverage(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        evidence["coverage"]["checked_rows"] = 1
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    eighth = mutate("declared-coverage-inconsistent", fudge_coverage,
                    "does not match the case details")

    with tempfile.TemporaryDirectory(prefix="envelope-release-selftest-") as temporary:
        clone = Path(temporary) / "evidence"
        shutil.copytree(evidence_dir, clone)
        fail_correctness(clone, Path(temporary))
        demoted_report = evaluate(clone, matrix_path, policy_path, candidate,
                                  DEFAULT_TARGETS, None, None)
    demoted = demoted_report["decoders"]["envelope-matching"]
    ninth = (demoted_report["status"] == "fail"
             and demoted["decision"] == "beta"
             and bool(demoted["blocking_gaps"]))
    observations.append({
        "mutation": "supported-decision-demoted",
        "rejected": ninth,
        "detail": {
            "decision": demoted["decision"],
            "blocking_gaps": demoted["blocking_gaps"][:2],
        },
    })

    tenth = ("Cargo.toml" in RETAINED_EQUIVALENCE_PATHS
             and "benchmarks/atom_loss/requirements.txt" in RETAINED_EQUIVALENCE_PATHS
             and "benchmarks/atom_loss/decoder_reference.py" in RETAINED_EQUIVALENCE_PATHS
             and "benchmarks/atom_loss/chain_reference.py" in RETAINED_EQUIVALENCE_PATHS
             and "benchmarks/atom_loss/fixtures" in RETAINED_EQUIVALENCE_PATHS
             and "rustqec-cli/tests/fixtures/current_rstim_atom_loss"
             in RETAINED_EQUIVALENCE_PATHS)
    observations.append({
        "mutation": "equivalence-paths-cover-build-config",
        "rejected": tenth,
        "detail": sorted(RETAINED_EQUIVALENCE_PATHS),
    })

    def helper_source_mutation() -> bool:
        """Editing a measurement helper must invalidate the retained report."""
        with tempfile.TemporaryDirectory(prefix="envelope-release-selftest-") as temporary:
            repo = Path(temporary) / "repo"
            helper = repo / "benchmarks" / "atom_loss" / "decoder_reference.py"
            helper.parent.mkdir(parents=True)

            def git(*argv: str) -> str:
                result = subprocess.run(
                    ["git", "-c", "user.name=selftest", "-c",
                     "user.email=selftest@example.invalid", *argv],
                    cwd=repo, capture_output=True, text=True, check=True,
                )
                return result.stdout.strip()

            git("init", "-q")
            helper.write_text("def noise(): return 'X_ERROR(0.1)'\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-q", "-m", "baseline helper")
            measured = git("rev-parse", "HEAD")
            helper.write_text("def noise(): return 'X_ERROR(0.2)'\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-q", "-m", "change the measured corpus")
            candidate_head = git("rev-parse", "HEAD")

            manifest_path = Path(temporary) / "manifest.json"
            manifest_path.write_text(json.dumps({
                "schema_version": RETAINED_MANIFEST_SCHEMA,
                "status": "pass",
                "checkout_revision": measured,
                "source_inputs": retained_source.inventory(repo, measured),
            }) + "\n", encoding="utf-8")
            mutated_gaps: list[str] = []
            mutated = check_retained_report(manifest_path, candidate_head,
                                            mutated_gaps, repo)
            control_gaps: list[str] = []
            control = check_retained_report(manifest_path, measured,
                                            control_gaps, repo)
            return (
                mutated["binding"] == "sources-differ"
                and "benchmarks/atom_loss/decoder_reference.py"
                in mutated["changed_equivalence_paths"]
                and any("measurement-relevant sources changed" in gap
                        for gap in mutated_gaps)
                and control["binding"] == "source-equivalent"
                and not control_gaps
            )

    eleventh = helper_source_mutation()
    observations.append({
        "mutation": "helper-source-change-invalidates-retained-report",
        "rejected": eleventh,
        "detail": "decoder_reference.py edit must flip the binding to sources-differ",
    })

    def squash_source_equivalence() -> bool:
        with tempfile.TemporaryDirectory(prefix="envelope-squash-selftest-") as temporary:
            root = Path(temporary)
            repo = root / "repo"
            repo.mkdir()

            def git(directory: Path, *args: str) -> str:
                return subprocess.check_output(
                    ["git", "-C", str(directory), "-c", "user.name=selftest", "-c",
                     "user.email=selftest@example.invalid", *args], text=True,
                ).strip()

            git(repo, "init", "-q")
            helper = repo / "benchmarks/atom_loss/decoder_reference.py"
            helper.parent.mkdir(parents=True)
            helper.write_text("# measured helper\n")
            git(repo, "add", ".")
            git(repo, "commit", "-qm", "measured")
            measured = git(repo, "rev-parse", "HEAD")
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": RETAINED_MANIFEST_SCHEMA,
                "status": "pass",
                "checkout_revision": measured,
                "source_inputs": retained_source.inventory(repo, measured),
            }))
            git(repo, "checkout", "--orphan", "squashed")
            git(repo, "add", ".")
            git(repo, "commit", "-qm", "squashed content")
            clone = root / "clone"
            subprocess.run(["git", "clone", "-q", "--no-local", "--depth", "1",
                            "--single-branch", "--branch", "squashed", str(repo),
                            str(clone)], check=True)
            if subprocess.run(["git", "-C", str(clone), "cat-file", "-e",
                               measured + "^{commit}"], capture_output=True).returncode == 0:
                return False
            gaps: list[str] = []
            clean = check_retained_report(manifest, git(clone, "rev-parse", "HEAD"),
                                          gaps, clone)
            helper = clone / "benchmarks/atom_loss/decoder_reference.py"
            helper.write_text("# changed helper\n")
            git(clone, "add", ".")
            git(clone, "commit", "-qm", "changed content")
            changed_gaps: list[str] = []
            changed = check_retained_report(manifest, git(clone, "rev-parse", "HEAD"),
                                            changed_gaps, clone)
            helper.write_text("# measured helper\n")
            added_helper = clone / "benchmarks/atom_loss/new_measurement_helper.py"
            added_helper.write_text("# newly added input\n")
            git(clone, "add", ".")
            git(clone, "commit", "-qm", "added helper")
            added_gaps: list[str] = []
            added = check_retained_report(manifest, git(clone, "rev-parse", "HEAD"),
                                          added_gaps, clone)
            return (clean["binding"] == "source-equivalent" and not gaps
                    and changed["binding"] == "sources-differ"
                    and any("measurement-relevant sources changed" in gap
                            for gap in changed_gaps)
                    and added["binding"] == "sources-differ"
                    and "benchmarks/atom_loss/new_measurement_helper.py"
                    in added["changed_equivalence_paths"])

    squash_checked = squash_source_equivalence()
    observations.append({
        "mutation": "squash-missing-source-object-still-checks-inputs",
        "rejected": squash_checked,
        "detail": "squash checkout accepts equal inputs and rejects changed inputs",
    })

    def remove_mle_real_circuit_case(clone: Path, temporary: Path) -> Path | None:
        """Drop one MLE-required real-circuit case; keep the declared coverage
        internally consistent so only the MLE scope mapping can catch it."""
        path = clone / "correctness.json"
        evidence = load_json(path)
        evidence["cases"] = [
            c for c in evidence["cases"] if c.get("name") != "midswap-d3-r1-generated"
        ]
        evidence["coverage"] = recomputed_coverage(evidence["cases"])
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    twelfth = mutate("mle-real-circuit-case-removed", remove_mle_real_circuit_case,
                     "midswap-d3-r1-generated")

    def mle_workload_over_budget(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case["id"] == "mle-d3r1-p010-b16384":
                case["wall_seconds"] = MLE_EXPECTED_BUDGET["per_case_wall_seconds"] * 2
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    thirteenth = mutate("mle-workload-over-budget", mle_workload_over_budget,
                        "exceeds the declared budget")

    def compiler_only_substitution(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case.get("name") == "midswap-d3-r2-p002-generated":
                case["evidence_level"] = "compiler-output-only"
                case.pop("backends", None)
                case.pop("allowed_answers", None)
        evidence["coverage"] = recomputed_coverage(evidence["cases"])
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    fourteenth = mutate("compiler-only-substituted-for-end-to-end",
                        compiler_only_substitution, "independent-end-to-end")

    def correctness_identity_substitution(clone: Path, temporary: Path) -> Path | None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case.get("name") == "midswap-d3-r1-generated":
                case["circuit_sha256"] = "0" * 64
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    identity_bound = mutate(
        "mle-correctness-input-identity-substituted",
        correctness_identity_substitution,
        "does not match pinned identity",
    )

    def no_ilp_installed(clone: Path, temporary: Path) -> Path | None:
        path = clone / f"installed-{DEFAULT_TARGETS[0]}.json"
        report = load_json(path)
        report["ilp"] = {"expected": False, "available": False}
        report["binary"]["advertised_decoders"] = ["envelope-matching"]
        report["controls"] = [
            c for c in report["controls"] if c.get("decoder") == "envelope-matching"
        ]
        path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        return None

    fifteenth = mutate("no-ilp-installed-report", no_ilp_installed,
                       "ILP-capable")

    def mle_total_wall_over_budget(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        evidence["total_wall_seconds"] = MLE_EXPECTED_BUDGET["total_wall_seconds"] * 2
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    sixteenth = mutate("mle-total-wall-over-budget", mle_total_wall_over_budget,
                       "total wall")

    def mle_peak_rss_over_budget(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        evidence["cases"][0]["peak_rss_watermark_bytes"] = \
            MLE_EXPECTED_BUDGET["peak_rss_bytes"] * 2
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    seventeenth = mutate("mle-peak-rss-over-budget", mle_peak_rss_over_budget,
                         "peak RSS")

    def mle_failure_exit_code_changed(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case["id"] == "fail-mle-candidate-limit":
                case["exit_code"] = 99
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    eighteenth = mutate("mle-failure-exit-code-changed", mle_failure_exit_code_changed,
                        "exit 2")

    def mle_cache_hits_removed(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case["id"] == "mle-d3r2-p002-b1024":
                case["cache"]["hits"] = 0
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    nineteenth = mutate("mle-cache-hits-removed", mle_cache_hits_removed,
                        "recorded no cache hits")

    def mle_completed_shots_changed(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case["id"] == "mle-d3r1-p010-b1024":
                case["completed_shots"] = 1023
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    twentieth = mutate("mle-completed-shots-changed", mle_completed_shots_changed,
                       "completed shots")

    def mle_output_rule_violation(clone: Path, temporary: Path) -> Path | None:
        path = clone / "mle-resources.json"
        evidence = load_json(path)
        for case in evidence["cases"]:
            if case["id"] == "fail-mle-infeasible":
                # Keep the cached verdict deceptively clean. Replay must derive
                # the violation from raw output state and reject it anyway.
                case["predictions"] = {
                    "installed": True,
                    "pre_existing": False,
                    "unchanged": False,
                }
                case["stats_written"] = False
                case["output_rule_problems"] = []
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        return None

    twenty_first = mutate("mle-output-rule-violation", mle_output_rule_violation,
                          "failed run installed a prediction file")

    # Decision-level control: under each MLE-specific mutation the Matching
    # decision must stay supported while MLE falls back to beta with an
    # actionable gap.
    with tempfile.TemporaryDirectory(prefix="envelope-release-selftest-") as temporary:
        clone = Path(temporary) / "evidence"
        shutil.copytree(evidence_dir, clone)
        remove_mle_real_circuit_case(clone, Path(temporary))
        decision_report = evaluate(clone, matrix_path, policy_path, candidate,
                                   DEFAULT_TARGETS, None, None, mle_scope_path)
    matching_decision = decision_report["decoders"]["envelope-matching"]
    mle_decision = decision_report["decoders"]["envelope-mle"]
    twenty_second = (matching_decision["decision"] == "supported"
                     and mle_decision["decision"] == "beta"
                     and bool(mle_decision["blocking_gaps"]))
    observations.append({
        "mutation": "mle-demoted-matching-preserved",
        "rejected": twenty_second,
        "detail": {
            "matching": matching_decision["decision"],
            "mle": mle_decision["decision"],
            "mle_blocking_gaps": mle_decision["blocking_gaps"][:2],
        },
    })

    passed = all([first, second, third, fourth, fifth, sixth, seventh, eighth,
                  ninth, tenth, eleventh, squash_checked, twelfth, thirteenth, fourteenth,
                  identity_bound, fifteenth, sixteenth, seventeenth, eighteenth, nineteenth,
                  twentieth, twenty_first, twenty_second])
    print(json.dumps({"self_test_mutations": observations}, indent=2))
    if passed:
        print(f"{PASS_LINE} self-test")
        return 0
    print("FAIL envelope release readiness self-test: not all mutations were rejected",
          file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-dir", type=Path, default=Path("drafts/envelope-readiness"))
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--candidate-revision",
                        help="release candidate source revision (default: git HEAD)")
    parser.add_argument("--require-target", action="append", dest="targets",
                        help="release target requiring an installed report "
                             f"(default: {', '.join(DEFAULT_TARGETS)})")
    parser.add_argument("--equivalence", type=Path,
                        help="recorded source-equivalence checks for revision mismatches")
    parser.add_argument("--retained-report", type=Path,
                        help="committed full resource report to bind to the candidate "
                             f"(default: {DEFAULT_RETAINED_REPORT})")
    parser.add_argument("--mle-scope", type=Path, default=DEFAULT_MLE_SCOPE,
                        help="MLE candidate scope plan enforcing the MLE Supported "
                             f"proposal (default: {DEFAULT_MLE_SCOPE})")
    parser.add_argument("--no-mle-scope", action="store_true",
                        help="do not load an MLE scope plan; only valid when neither "
                             "the matrix nor a plan proposes MLE Supported")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    try:
        candidate = args.candidate_revision or candidate_revision()
    except GateError as error:
        print(f"FAIL envelope release readiness: {error}", file=sys.stderr)
        return 2
    targets = tuple(args.targets) if args.targets else DEFAULT_TARGETS
    mle_scope = None if args.no_mle_scope else args.mle_scope

    if args.self_test:
        return self_test(args.evidence_dir, args.matrix, args.policy, candidate,
                         args.retained_report, mle_scope)

    try:
        report = evaluate(args.evidence_dir, args.matrix, args.policy, candidate,
                          targets, args.equivalence, args.retained_report, mle_scope)
    except (GateError, support.MatrixError) as error:
        print(f"FAIL envelope release readiness: {error}", file=sys.stderr)
        return 1
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if report["status"] == "pass":
        promoted = [
            name for name, decision in report["decoders"].items()
            if decision["decision"] == "supported"
        ]
        print(f"{PASS_LINE} targets={len(report['required_targets'])} "
              f"promoted={','.join(promoted) or 'none'}")
        return 0
    print("FAIL envelope release readiness", file=sys.stderr)
    for gap in report["blocking_gaps"]:
        print(f"  - {gap}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
