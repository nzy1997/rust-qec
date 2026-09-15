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


def decoder_coverage(
    decoder: str,
    generated: dict[str, dict[str, Any] | None],
    installed: dict[str, dict[str, Any] | None],
) -> list[str]:
    """Which evidence artifacts demonstrably exercise this decoder."""
    missing: list[str] = []
    support_result = (generated.get("support") or {}).get("evidence")
    if support_result is None or not any(
        r.get("decoder") == decoder and r.get("status") == "pass"
        for r in support_result.get("results", [])
    ):
        missing.append("support-matrix")
    resources = (generated.get("resources") or {}).get("evidence")
    if resources is None or not any(
        c.get("decoder") == decoder for c in resources.get("cases", [])
    ):
        missing.append("resource-envelope")
    for target, record in installed.items():
        report = (record or {}).get("report")
        controls = (report or {}).get("controls", [])
        if report is None or not any(
            c.get("decoder") == decoder and c.get("status") == "pass" for c in controls
        ):
            missing.append(f"installed:{target}")
    return missing


def evaluate(
    evidence_dir: Path,
    matrix_path: Path,
    policy_path: Path,
    candidate: str,
    targets: tuple[str, ...],
    equivalence_path: Path | None,
) -> dict[str, Any]:
    matrix = support.load_matrix(matrix_path)
    pairs = load_equivalence(equivalence_path)
    gaps: list[str] = []

    matrix_record = check_matrix_binding(matrix, candidate, gaps)
    policy_record = check_policy(policy_path, gaps)

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
        coverage_gaps = decoder_coverage(decoder, generated, installed)
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


def self_test(evidence_dir: Path, matrix_path: Path, policy_path: Path, candidate: str) -> int:
    """The gate must reject defective evidence bundles."""
    baseline = evaluate(evidence_dir, matrix_path, policy_path, candidate,
                        DEFAULT_TARGETS, None)
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
            transform(clone)
            report = evaluate(clone, matrix_path, policy_path, candidate,
                              DEFAULT_TARGETS, None)
            rejected = report["status"] == "fail" and any(
                needle in gap for gap in report["blocking_gaps"]
            )
            observations.append({
                "mutation": name,
                "rejected": rejected,
                "detail": report["blocking_gaps"][:2],
            })
            return rejected

    def drop_linux(clone: Path) -> None:
        (clone / f"installed-{DEFAULT_TARGETS[0]}.json").unlink()

    first = mutate("missing-platform-report", drop_linux, "missing installed-envelope report")

    def mismatch_revision(clone: Path) -> None:
        path = clone / "support.json"
        evidence = load_json(path)
        evidence["checkout_revision"] = "0" * 40
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    second = mutate("revision-mismatch", mismatch_revision, "is not the candidate")

    def fail_correctness(clone: Path) -> None:
        path = clone / "correctness.json"
        evidence = load_json(path)
        evidence["status"] = "fail"
        path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    third = mutate("failing-evidence", fail_correctness, "status is")

    passed = first and second and third
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
        return self_test(args.evidence_dir, args.matrix, args.policy, candidate)

    try:
        report = evaluate(args.evidence_dir, args.matrix, args.policy, candidate,
                          targets, args.equivalence)
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
