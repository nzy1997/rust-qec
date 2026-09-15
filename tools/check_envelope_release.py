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
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_envelope_support as support  # noqa: E402

SCHEMA_VERSION = "rustqec.envelope-release-report.v1"
EQUIVALENCE_SCHEMA = "rustqec.source-equivalence.v1"
PASS_LINE = "PASS envelope release readiness"
DEFAULT_MATRIX = Path("docs/envelope-support.json")
DEFAULT_POLICY = Path("docs/envelope-compatibility-policy.md")
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


def is_ancestor(ancestor: str, descendant: str) -> bool | None:
    """True/False when git can decide, None when it cannot."""
    if ancestor == descendant:
        return True
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=REPO_ROOT, check=False,
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
        "target": target,
        "source_revision": revision,
        "revision_bound": bound,
        "equivalence_waiver": waived,
        "status": report.get("status"),
        "report": report,
    }


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

# The retained full resource report supports promotion only for candidates
# whose measurement-relevant sources are identical to the measured revision.
RETAINED_EQUIVALENCE_PATHS = (
    "benchmarks/atom_loss/readiness_resources.py",
    "docs/envelope-support.json",
    "Cargo.lock",
    "rustqec-cli/src", "rustqec-cli/Cargo.toml",
    "renvelope/src", "renvelope/Cargo.toml",
    "rmatching/src", "rmatching/Cargo.toml",
    "qec-ilp-core/src", "qec-ilp-core/Cargo.toml",
    "rstim/src", "rstim/Cargo.toml",
)


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


def correctness_gaps(decoder: str, evidence: dict[str, Any]) -> list[str]:
    """Per-decoder cases, coverage categories and row accounting must be intact."""
    label = "correctness"
    gaps: list[str] = []
    cases = evidence.get("cases") or []
    coverage = evidence.get("coverage") or {}
    end_to_end = [c for c in cases if c.get("evidence_level") == "independent-end-to-end"]
    if not end_to_end:
        gaps.append(f"{label}: no independent end-to-end case covers {decoder}")
    for case in end_to_end:
        name = case.get("name", "?")
        backend = (case.get("backends") or {}).get(decoder)
        if backend is None:
            gaps.append(f"{label}: case {name} did not execute {decoder}")
            continue
        if backend.get("rejected_rows"):
            gaps.append(f"{label}: case {name}: {decoder} predictions outside allowed optima")
        if not backend.get("flipped_prediction_rejected"):
            gaps.append(f"{label}: case {name}: flipped-answer control missing for {decoder}")
        if not backend.get("placeholder_invariance"):
            gaps.append(f"{label}: case {name}: placeholder invariance not shown for {decoder}")
        if not backend.get("checked_rows"):
            gaps.append(f"{label}: case {name}: zero rows checked for {decoder}")
        if any(m.get("decoder") == decoder for m in case.get("mismatches") or []):
            gaps.append(f"{label}: case {name}: mismatches recorded for {decoder}")
    categories = coverage.get("history_categories") or {}
    missing_categories = [c for c in REQUIRED_HISTORY_CATEGORIES if categories.get(c, 0) <= 0]
    if missing_categories:
        gaps.append(f"{label}: missing loss-history coverage: {', '.join(missing_categories)}")
    missing_channels = [
        c for c in REQUIRED_NOISE_CHANNELS if c not in (coverage.get("noise_channels") or [])
    ]
    if missing_channels:
        gaps.append(f"{label}: missing noise-channel coverage: {', '.join(missing_channels)}")
    if len(coverage.get("circuit_points") or []) < 2:
        gaps.append(f"{label}: missing circuit size/round coverage")
    if (coverage.get("placeholder_pairs") or 0) <= 0:
        gaps.append(f"{label}: missing placeholder-invariance coverage")
    if (coverage.get("checked_rows") or 0) <= 0:
        gaps.append(f"{label}: zero checked rows")
    if not coverage.get("randomized_differential_seeds"):
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
        missing += correctness_gaps(decoder, correctness)
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


def check_retained_report(path: Path, candidate: str, gaps: list[str]) -> dict[str, Any]:
    """Bind the committed full resource report to the candidate's sources.

    The report is a committed artifact, so it cannot name its own commit; the
    gate instead requires its measurement revision to be an ancestor of the
    candidate with zero changes in measurement-relevant sources since then.
    """
    record: dict[str, Any] = {"path": str(path)}
    if not path.is_file():
        gaps.append(f"retained full resource report missing: {path}")
        record["present"] = False
        return record
    record["present"] = True
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
    if not revision:
        gaps.append("retained resource report does not record its checkout revision")
        record["binding"] = "missing"
        return record
    decision = is_ancestor(str(revision), candidate)
    if decision is not True:
        gaps.append(
            f"retained resource report revision {revision} is not an ancestor of the "
            f"candidate {candidate}"
        )
        record["binding"] = "not-an-ancestor"
        return record
    diff = subprocess.run(
        ["git", "diff", "--name-only", str(revision), candidate, "--",
         *RETAINED_EQUIVALENCE_PATHS],
        capture_output=True, text=True, cwd=REPO_ROOT, check=False,
    )
    if diff.returncode:
        gaps.append("cannot verify retained-report source equivalence (git diff failed)")
        record["binding"] = "unverifiable"
        return record
    changed = [line for line in diff.stdout.splitlines() if line]
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


def evaluate(
    evidence_dir: Path,
    matrix_path: Path,
    policy_path: Path,
    candidate: str,
    targets: tuple[str, ...],
    equivalence_path: Path | None,
    retained_report_path: Path | None = None,
) -> dict[str, Any]:
    matrix = support.load_matrix(matrix_path)
    pairs = load_equivalence(equivalence_path)
    gaps: list[str] = []

    matrix_record = check_matrix_binding(matrix, candidate, gaps)
    policy_record = check_policy(policy_path, gaps)
    if retained_report_path is None:
        retained_report_path = REPO_ROOT / DEFAULT_RETAINED_REPORT
    retained_record = check_retained_report(retained_report_path, candidate, gaps)

    generated: dict[str, dict[str, Any] | None] = {}
    for name, (filename, schema) in GENERATED_EVIDENCE.items():
        generated[name] = check_generated_evidence(
            name, evidence_dir / filename, schema, candidate, pairs, gaps
        )
    installed: dict[str, dict[str, Any] | None] = {}
    for target in targets:
        installed[target] = check_installed_report(
            target, evidence_dir / f"installed-{target}.json", candidate, pairs, gaps
        )

    decisions: dict[str, Any] = {}
    for decoder, declaration in matrix["decoders"].items():
        proposed = declaration.get("proposed_release_maturity")
        coverage_gaps = decoder_coverage(decoder, generated, installed, matrix)
        blocking = list(coverage_gaps)
        if proposed != "supported-candidate":
            # Beta decoders do not gate the release, but their evidence state
            # is still reported.
            blocking = []
        earned = proposed == "supported-candidate" and not coverage_gaps
        if proposed == "supported-candidate" and coverage_gaps:
            gaps.append(
                f"decoder {decoder} proposed as supported-candidate lacks evidence: "
                + ", ".join(coverage_gaps)
            )
        decisions[decoder] = {
            "current_maturity": declaration.get("current_maturity"),
            "proposed_release_maturity": proposed,
            "decision": "supported" if earned else "beta",
            "coverage_gaps": coverage_gaps,
            "blocking_gaps": blocking,
        }

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
              retained_report_path: Path | None = None) -> int:
    """The gate must reject defective evidence bundles."""
    baseline = evaluate(evidence_dir, matrix_path, policy_path, candidate,
                        DEFAULT_TARGETS, None, retained_report_path)
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
                              DEFAULT_TARGETS, None, retained)
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

    passed = all([first, second, third, fourth, fifth, sixth])
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
    parser.add_argument("--out", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    try:
        candidate = args.candidate_revision or candidate_revision()
    except GateError as error:
        print(f"FAIL envelope release readiness: {error}", file=sys.stderr)
        return 2
    targets = tuple(args.targets) if args.targets else DEFAULT_TARGETS

    if args.self_test:
        return self_test(args.evidence_dir, args.matrix, args.policy, candidate,
                         args.retained_report)

    try:
        report = evaluate(args.evidence_dir, args.matrix, args.policy, candidate,
                          targets, args.equivalence, args.retained_report)
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
