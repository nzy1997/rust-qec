#!/usr/bin/env python3
"""Scope evaluator for the envelope-mle candidate support domain (issue #721).

Loads the machine-readable MLE candidate-domain/evidence plan
(``docs/envelope-mle-scope.json``) and answers two questions:

* Is a described decode job inside the declared supported domain?
  The verdict is ``in-domain`` or ``outside-supported-domain`` — a support
  boundary statement, never a decoder success/failure prediction. An
  out-of-domain input is unpromised, not automatically rejected.
* Does every required evidence case ID resolve to a pinned input/oracle
  (or an explicitly pending evidence requirement)?

Plan validation is strict so the promise cannot be widened silently:
removing a baseline required case, letting the solve-only timeout stand in
for compilation coverage, promoting the current maturity before the evidence
exists, or adding domain grid points without their evidence requirements all
fail validation.

Usage:
    python3 tools/envelope_mle_scope.py --plan docs/envelope-mle-scope.json --domain-table
    python3 tools/envelope_mle_scope.py --evaluate '{"circuit": {"family": "midswap", "distance": 3, "rounds": 2, "loss_rate": 0.002, "observables": 1}, "shots": 1024}'
    python3 tools/envelope_mle_scope.py --resolve
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "rustqec.envelope-mle-scope.v1"
DEFAULT_PLAN = Path("docs/envelope-mle-scope.json")
DEFAULT_MATRIX = Path("docs/envelope-support.json")
DECODER = "envelope-mle"

# Baseline required-case set mirrored from docs/envelope-mle-scope.json. Keep
# in sync with the plan's required_cases; validation fails loudly when the
# plan drops one of them, so a narrower evidence base cannot pass.
BASELINE_REQUIRED_CASES = (
    "correctness:midswap-d3-r2-fixture",
    "correctness:midswap-d3-r1-generated",
    "correctness:midswap-d3-r3-generated",
    "control:mini-circuit-known-answer",
    "control:lost-value-placeholder-invariance",
    "control:midswap-canonical-mle-known-answer",
    "control:midswap-fixture-mle-acceptance",
    "control:conventional-mle-candidate-explosion-rejection",
    "control:mle-solve-timeout",
    "control:mle-infeasible-shot",
    "control:mle-unsupported-detector-layout",
    "control:repeat-block-rejection",
    "resource:mle-d3r2-p002-b1024",
    "resource:mle-d3r2-p002-b16384",
    "resource:mle-d3r1-p010-b1024",
    "resource:mle-d3r1-p010-b16384",
    "resource:mle-eviction-wires24",
    "failure:fail-mle-candidate-limit",
    "failure:fail-mle-solve-timeout",
    "failure:fail-mle-infeasible",
    "installed:x86_64-unknown-linux-gnu",
    "installed:aarch64-apple-darwin",
)

CASE_KINDS = (
    "correctness-end-to-end",
    "correctness-compiler-only",
    "support-control",
    "resource-workload",
    "failure-semantics",
    "installed-platform",
)

END_TO_END_KIND = "correctness-end-to-end"


class ScopeError(Exception):
    """The plan itself is invalid; raised before any evaluation."""


def _fail(problems: list[str], message: str) -> None:
    problems.append(message)


def validate_plan(plan: dict[str, Any]) -> None:
    """Raise ScopeError unless the plan is a complete, honest contract."""
    problems: list[str] = []
    if plan.get("schema_version") != SCHEMA_VERSION:
        raise ScopeError(
            f"unsupported plan schema_version: {plan.get('schema_version')!r}")
    if plan.get("decoder") != DECODER:
        _fail(problems, f"plan decoder must be {DECODER!r}")
    if plan.get("required_build_features") != ["ilp"]:
        _fail(problems, "MLE requires exactly the ilp build feature")

    maturity = plan.get("maturity") or {}
    cases = plan.get("required_cases")
    if not isinstance(cases, list) or not cases:
        _fail(problems, "plan must declare required_cases")
        cases = []
    by_id: dict[str, dict[str, Any]] = {}
    for case in cases:
        cid = case.get("id")
        if not cid or cid in by_id:
            _fail(problems, f"duplicate or missing required case id: {cid!r}")
            continue
        by_id[cid] = case
        kind = case.get("kind")
        if kind not in CASE_KINDS:
            _fail(problems, f"case {cid}: unknown kind {kind!r}")
        if case.get("status") == "pending":
            if not case.get("pending_reason"):
                _fail(problems, f"case {cid}: pending evidence needs a pending_reason")
        else:
            # A fulfilled-or-pinnable case must name its pinned input and the
            # oracle/expectation that checks it.
            if not (case.get("input") or case.get("circuit") or case.get("target")):
                _fail(problems, f"case {cid}: missing pinned input/circuit/target")
            if not (case.get("oracle") or case.get("expected") or case.get("budget")):
                _fail(problems, f"case {cid}: missing oracle/expected/budget provenance")
        if kind in ("correctness-end-to-end", "correctness-compiler-only"):
            params = case.get("circuit_params") or {}
            if not all(isinstance(params.get(f), (int, float)) and params[f] > 0
                       for f in ("distance", "rounds", "loss_rate")):
                _fail(problems,
                      f"case {cid}: correctness cases must pin circuit_params "
                      "distance/rounds/loss_rate")
        if kind == "resource-workload":
            if not isinstance(case.get("real_circuit"), bool):
                _fail(problems, f"case {cid}: resource workloads must declare real_circuit")
            if case.get("real_circuit"):
                params = case.get("circuit_params") or {}
                if not all(isinstance(params.get(f), (int, float)) and params[f] > 0
                           for f in ("distance", "rounds", "loss_rate", "batch")):
                    _fail(problems,
                          f"case {cid}: real-circuit workloads must pin circuit_params "
                          "distance/rounds/loss_rate/batch")

    # Baseline coverage: removing a required case invalidates the plan.
    missing = [cid for cid in BASELINE_REQUIRED_CASES if cid not in by_id]
    if missing:
        _fail(problems,
              "required case(s) missing from plan: " + ", ".join(missing))

    # Maturity honesty: the current label may only leave beta once every
    # required case is fulfilled and a promotion release is recorded.
    pending = [cid for cid, case in by_id.items() if case.get("status") == "pending"]
    current = maturity.get("current")
    if current not in ("beta", "supported"):
        _fail(problems, f"maturity.current must be beta|supported, got {current!r}")
    if current == "supported" and (pending or not maturity.get("promotion_release")):
        _fail(problems,
              "maturity.current is promoted before the evidence exists: pending "
              f"cases {pending or '[]'} and promotion_release "
              f"{maturity.get('promotion_release')!r}")

    limits = plan.get("limits_and_semantics") or {}
    timeout = limits.get("timeout") or {}
    if timeout.get("scope") != "solve-phase-only":
        _fail(problems,
              "the MLE timeout must stay scoped to the solve phase; declaring "
              "compilation covered by --shot-timeout-ms is not allowed")
    candidate_limit = limits.get("candidate_limit") or {}
    if not isinstance(candidate_limit.get("max_envelope_candidates"), int):
        _fail(problems, "candidate limit must pin max_envelope_candidates")

    exclusions = plan.get("exclusions") or []
    if not any(e.get("id") == "conventional-family-candidate-limit" for e in exclusions):
        _fail(problems,
              "the conventional-family candidate-limit exclusion must be preserved")

    domain = plan.get("domain") or {}
    if domain.get("circuit_family") != "midswap":
        _fail(problems, "the MLE candidate domain is the midswap family only")
    if domain.get("flat_only") is not True:
        _fail(problems, "the MLE candidate domain is flat circuits only")
    observables = domain.get("observables")
    if observables != "1..=64":
        _fail(problems, f"observable bound must stay 1..=64, got {observables!r}")
    points = domain.get("points")
    if not isinstance(points, list) or not points:
        _fail(problems, "the domain must declare at least one finite point")
        points = []
    seen_points: set[str] = set()
    for point in points:
        pid = point.get("id")
        if not pid or pid in seen_points:
            _fail(problems, f"duplicate or missing domain point id: {pid!r}")
            continue
        seen_points.add(pid)
        for field in ("distance", "rounds", "loss_rate_max", "batch_max"):
            if not isinstance(point.get(field), (int, float)) or point[field] <= 0:
                _fail(problems, f"domain point {pid}: missing positive {field}")
        evidence = point.get("required_evidence")
        if not isinstance(evidence, list) or not evidence:
            _fail(problems, f"domain point {pid}: no required evidence declared")
            continue
        unknown = [cid for cid in evidence if cid not in by_id]
        if unknown:
            _fail(problems,
                  f"domain point {pid}: evidence references unknown case(s): "
                  + ", ".join(unknown))
            continue
        # Widening the grid without its evidence requirements fails here:
        # every point needs end-to-end correctness and real-circuit resource
        # coverage measured on its own circuit shape, not borrowed maxima
        # from a different point.
        def matches(cid: str) -> bool:
            params = by_id[cid].get("circuit_params") or {}
            return (params.get("distance") == point.get("distance")
                    and params.get("rounds") == point.get("rounds"))

        end_to_end = [
            cid for cid in evidence
            if by_id[cid]["kind"] == END_TO_END_KIND and matches(cid)
        ]
        if not end_to_end:
            _fail(problems,
                  f"domain point {pid}: no independent end-to-end correctness case "
                  "on this circuit shape; compiler-only or borrowed evidence cannot "
                  "cover a grid point")
        elif not any(
                float((by_id[cid].get("circuit_params") or {}).get("loss_rate", -1))
                == float(point["loss_rate_max"]) for cid in end_to_end):
            _fail(problems,
                  f"domain point {pid}: end-to-end correctness must be measured at "
                  f"the point's loss ceiling {point['loss_rate_max']}")
        real_workloads = [
            cid for cid in evidence
            if by_id[cid]["kind"] == "resource-workload"
            and by_id[cid].get("real_circuit") and matches(cid)
        ]
        batches = {
            int(by_id[cid]["circuit_params"]["batch"])
            for cid in real_workloads
            if isinstance((by_id[cid].get("circuit_params") or {}).get("batch"), int)
        }
        if not real_workloads:
            _fail(problems,
                  f"domain point {pid}: no real-circuit resource workload on this "
                  "circuit shape; Matching-only or synthetic cache evidence cannot "
                  "cover a grid point")
        elif max(batches or {0}) < int(point["batch_max"]):
            _fail(problems,
                  f"domain point {pid}: batch ceiling {point['batch_max']} exceeds "
                  "the largest measured real-circuit batch on this circuit shape "
                  f"{max(batches or {0})}")
        for cid in evidence:
            if by_id[cid].get("counts_toward_domain") is False:
                _fail(problems,
                      f"domain point {pid}: case {cid} is marked as not counting "
                      "toward the domain and cannot be required evidence")

    if problems:
        raise ScopeError("invalid MLE scope plan:\n  - " + "\n  - ".join(problems))


def load_plan(path: Path) -> dict[str, Any]:
    plan = json.loads(Path(path).read_text(encoding="utf-8"))
    validate_plan(plan)
    return plan


def _point_covers(point: dict[str, Any], circuit: dict[str, Any], shots: int) -> bool:
    return (
        circuit.get("distance") == point.get("distance")
        and circuit.get("rounds") == point.get("rounds")
        and 0 < float(circuit.get("loss_rate", -1)) <= float(point["loss_rate_max"])
        and 0 < shots <= int(point["batch_max"])
    )


def evaluate_case(plan: dict[str, Any], descriptor: dict[str, Any]) -> dict[str, Any]:
    """Classify a described decode job against the declared domain.

    Returns {"status": "in-domain", "point": <id>} or
    {"status": "outside-supported-domain", "reasons": [...]}. This is a
    support-boundary verdict only; it never predicts decode success.
    """
    reasons: list[str] = []
    circuit = descriptor.get("circuit") or {}
    shots = descriptor.get("shots")
    domain = plan["domain"]
    if circuit.get("family") != domain["circuit_family"]:
        reasons.append(
            f"circuit family {circuit.get('family')!r} is not {domain['circuit_family']!r}")
    if circuit.get("flat", True) is not True:
        reasons.append("only flat circuits are in the domain (REPEAT blocks are rejected)")
    basis = circuit.get("readout_basis", "Z")
    if basis != "Z":
        reasons.append(f"readout basis {basis!r} is not Z; X/Y-basis loss readouts are rejected")
    observables = circuit.get("observables", 1)
    if not isinstance(observables, int) or not 1 <= observables <= 64:
        reasons.append(f"observable count {observables!r} is outside 1..=64")
    if circuit.get("sweep_bits", 0) != 0:
        reasons.append("sweep bits are outside the domain")
    if not isinstance(shots, int) or shots <= 0:
        reasons.append(f"shot count {shots!r} is not a positive batch size")
    if not reasons:
        point = next(
            (p for p in domain["points"] if _point_covers(p, circuit, shots)), None)
        if point is None:
            reasons.append(
                "no declared domain point covers "
                f"distance={circuit.get('distance')}, rounds={circuit.get('rounds')}, "
                f"loss_rate={circuit.get('loss_rate')}, shots={shots}; the supported "
                "grid is finite and does not interpolate beyond its measured ceilings")
    if reasons:
        return {"status": "outside-supported-domain", "reasons": reasons}
    return {"status": "in-domain", "point": point["id"]}


def resolve_requirements(
    plan: dict[str, Any], matrix_path: Path = DEFAULT_MATRIX
) -> dict[str, dict[str, Any]]:
    """Map every required case ID to its pinned provenance or pending status."""
    matrix = json.loads((REPO_ROOT / matrix_path).read_text(encoding="utf-8")) \
        if not Path(matrix_path).is_absolute() else json.loads(Path(matrix_path).read_text(encoding="utf-8"))
    matrix_controls = {
        control["id"]: control
        for control in matrix.get("controls", [])
        if DECODER in control.get("decoders", [])
    }
    resolved: dict[str, dict[str, Any]] = {}
    for case in plan["required_cases"]:
        cid = case["id"]
        if case.get("status") == "pending":
            resolved[cid] = {
                "status": "pending",
                "requirement": case["pending_reason"],
                "kind": case["kind"],
            }
            continue
        record: dict[str, Any] = {
            "status": "pinned",
            "kind": case["kind"],
            "input": case.get("input") or case.get("circuit") or case.get("target"),
            "oracle": case.get("oracle") or case.get("expected") or case.get("budget"),
        }
        if case["kind"] == "support-control":
            control_id = case.get("matrix_control")
            if control_id not in matrix_controls:
                record["status"] = "pending"
                record["requirement"] = (
                    f"matrix control {control_id!r} is not declared for {DECODER} "
                    f"in {matrix_path}")
            else:
                record["matrix_control"] = control_id
        resolved[cid] = record
    return resolved


def domain_table(plan: dict[str, Any]) -> str:
    """Reviewable rendering of the accepted finite domain."""
    lines = [
        "Accepted finite MLE candidate domain (no interpolation beyond these points):",
        "| Point | Distance | Rounds | Loss rate | Batch | Required evidence |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for point in plan["domain"]["points"]:
        lines.append(
            f"| {point['id']} | {point['distance']} | {point['rounds']} "
            f"| (0, {point['loss_rate_max']}] | ≤ {point['batch_max']} "
            f"| {', '.join(point['required_evidence'])} |")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=DEFAULT_PLAN)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--evaluate", help="JSON decode-job descriptor to classify")
    parser.add_argument("--resolve", action="store_true",
                        help="resolve every required case ID to its provenance")
    parser.add_argument("--domain-table", action="store_true",
                        help="print the accepted finite domain as a table")
    args = parser.parse_args()
    try:
        plan = load_plan(args.plan)
    except (ScopeError, json.JSONDecodeError) as error:
        print(f"FAIL envelope MLE scope: {error}", file=sys.stderr)
        return 1
    if args.evaluate is not None:
        verdict = evaluate_case(plan, json.loads(args.evaluate))
        print(json.dumps(verdict, indent=2))
        return 0 if verdict["status"] == "in-domain" else 3
    if args.resolve:
        resolved = resolve_requirements(plan, args.matrix)
        print(json.dumps(resolved, indent=2))
        pending = [cid for cid, r in resolved.items() if r["status"] == "pending"]
        if pending:
            print("pending evidence requirements: " + ", ".join(pending), file=sys.stderr)
            return 3
        return 0
    if args.domain_table:
        print(domain_table(plan))
        return 0
    parser.print_help()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
