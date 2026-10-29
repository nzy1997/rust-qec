#!/usr/bin/env python3
"""Validate the pinned Magna test inventory separately from Rust parity."""

from __future__ import annotations

import argparse
import ast
import importlib.util
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def local_module(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


reference = local_module("reference")
rust = local_module("coverage_rust")
CoverageError = rust.CoverageError
require = rust.require


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object,
                      parse_constant=lambda value: invalid_constant(value))


def invalid_constant(value: str):
    raise CoverageError(f"nonfinite JSON value: {value}")


def declarations(source: str, module: str, path: str, digest: str) -> dict[str, dict]:
    """Use source declarations without importing or running the simulator."""
    tree = ast.parse(source, filename=path)
    rows = {}
    accepted = set()
    class_names = set()
    for cls in tree.body:
        if not isinstance(cls, ast.ClassDef):
            continue
        require(cls.name not in class_names, f"ambiguous upstream class: {module}.{cls.name}")
        class_names.add(cls.name)
        test_case = any(isinstance(base, ast.Attribute) and base.attr == "TestCase"
                        and isinstance(base.value, ast.Name) and base.value.id == "unittest"
                        for base in cls.bases)
        if not test_case:
            continue
        methods = set()
        for node in cls.body:
            if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                continue
            require(node.name not in methods, f"ambiguous upstream method: {module}.{cls.name}.{node.name}")
            methods.add(node.name)
            if not node.name.startswith("test_"):
                continue
            require(isinstance(node, ast.FunctionDef), "unsupported async upstream test")
            case = f"{module}.{cls.name}.{node.name}"
            rows[case] = {"path": path, "source_sha256": digest,
                          "line": node.lineno, "end_line": node.end_lineno}
            accepted.add(id(node))
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test_"):
            require(id(node) in accepted, f"nested or unsupported upstream test: {module}.{node.name}")
    return rows


def inventory(upstream: Path, manifest: dict) -> tuple[dict, dict, dict]:
    reference.verify_source(upstream, manifest)
    modules = {}
    cases = {}
    for name in manifest["test_modules"]:
        path = f"tests/core/{name}"
        digest = manifest["upstream"]["files"][path]
        modules[name] = digest
        found = declarations((upstream / path).read_text(encoding="utf-8"),
                             Path(name).stem, path, digest)
        require(not set(found) & set(cases), "duplicate upstream canonical ID")
        cases.update(found)
    fixtures = {}
    for path, digest in manifest["upstream"]["files"].items():
        if path.startswith("tests/fixtures/") and Path(path).name.startswith("test_") and path.endswith(".py"):
            module = path[:-3].replace("/", ".")
            fixtures.update(declarations((upstream / path).read_text(encoding="utf-8"), module, path, digest))
    return modules, cases, fixtures


def validate(upstream: Path, ledger: object, run: object = None, *, root: Path = ROOT,
             manifest: dict | None = None, reference_sha256: str | None = None) -> dict:
    report = {"schema": "rstim.magna-coverage-report.v1", "inventory_valid": False,
              "parity_complete": False, "errors": [], "missing_ids": [],
              "duplicate_ids": [], "unknown_ids": [], "status_counts": {},
              "modules": {}, "cases": [], "fixture_exclusions": [],
              "rust_execution": None}
    try:
        manifest = reference.load_manifest() if manifest is None else manifest
        reference_sha256 = reference.digest(reference.MANIFEST) if reference_sha256 is None else reference_sha256
        report["reference_sha256"] = reference_sha256
        report["upstream_commit"] = manifest["upstream"]["commit"]
        modules, expected, fixtures = inventory(upstream, manifest)
        report["source_identity"] = {"modules": modules, "cases": expected, "fixtures": fixtures}
        rust.fields(ledger, {"schema", "reference_sha256", "upstream_commit", "modules",
                             "fixture_exclusions", "cases"}, "coverage ledger")
        require(ledger["schema"] == "rstim.magna-coverage.v1", "unsupported coverage ledger schema")
        require(ledger["reference_sha256"] == reference_sha256, "reference SHA-256 mismatch")
        require(ledger["upstream_commit"] == manifest["upstream"]["commit"], "upstream commit mismatch")
        require(ledger["modules"] == modules, "upstream module inventory or SHA-256 mismatch")
        require(isinstance(ledger["cases"], list), "coverage cases must be an array")
        ids = []
        for row in ledger["cases"]:
            require(isinstance(row, dict) and isinstance(row.get("id"), str), "invalid coverage case ID")
            ids.append(row["id"])
        counts = Counter(ids)
        report["missing_ids"] = sorted(set(expected) - set(ids))
        report["unknown_ids"] = sorted(set(ids) - set(expected))
        report["duplicate_ids"] = sorted(k for k, count in counts.items() if count > 1)
        require(not any(report[k] for k in ("missing_ids", "unknown_ids", "duplicate_ids")),
                "coverage canonical ID inventory mismatch")
        exclusions = ledger["fixture_exclusions"]
        require(isinstance(exclusions, list), "fixture exclusions must be an array")
        excluded = set()
        for row in exclusions:
            rust.fields(row, {"id", "path", "source_sha256", "reason"}, "fixture exclusion")
            case = rust.text(row["id"], "fixture ID")
            require(case in fixtures and case not in excluded, f"unknown or duplicate fixture exclusion: {case}")
            require(all(row[k] == fixtures[case][k] for k in ("path", "source_sha256")),
                    f"fixture source identity mismatch: {case}")
            rust.text(row["reason"], "fixture exclusion reason")
            excluded.add(case)
        require(excluded == set(fixtures), "missing fixture exclusion")
        report["fixture_exclusions"] = exclusions
        required = set()
        status_counts = Counter()
        per_module = {name: Counter() for name in modules}
        for row in ledger["cases"]:
            case = row["id"]
            status = row.get("status")
            require(isinstance(status, str) and status in {"verified", "partial", "unmapped", "architecture_specific"},
                    f"invalid coverage status: {case}")
            if status in {"verified", "partial"}:
                rust.fields(row, {"id", "status", "contract", "rust_tests"}
                            | ({"remaining_gap"} if status == "partial" else set()), case)
                if status == "partial":
                    rust.text(row["remaining_gap"], f"unresolved compatibility gap for {case}")
                rust.text(row["contract"], f"concrete semantic contract for {case}")
                require(isinstance(row["rust_tests"], list) and bool(row["rust_tests"]),
                        f"missing Rust witnesses: {case}")
                witnesses = [rust.witness(root, item) for item in row["rust_tests"]]
                require(len(witnesses) == len(set(witnesses)), f"duplicate Rust witness: {case}")
                required.update(witnesses)
            elif status == "unmapped":
                rust.fields(row, {"id", "status", "reason"}, case)
                rust.text(row["reason"], f"unmapped reason for {case}")
            else:
                rust.fields(row, {"id", "status", "platform", "reason", "contract"}, case)
                for key in ("platform", "reason", "contract"):
                    rust.text(row[key], f"architecture-specific {key} for {case}")
            status_counts[status] += 1
            per_module[case.split(".", 1)[0] + ".py"][status] += 1
        report["cases"] = ledger["cases"]
        report["status_counts"] = {k: status_counts[k] for k in ("verified", "partial", "unmapped", "architecture_specific")}
        report["modules"] = {name: {"source_sha256": modules[name], "total": sum(counts.values()),
                                   **{k: counts[k] for k in report["status_counts"]}}
                             for name, counts in per_module.items()}
        report["core_modules"] = len(modules)
        report["core_cases"] = len(expected)
        report["inventory_valid"] = True
        execution = rust.execution(root, run, required)
        report["rust_execution"] = execution
        report["parity_complete"] = bool(expected) and status_counts["verified"] == len(expected) \
            and execution["source_matches"] and not execution["errors"] \
            and not execution["missing"] and not execution["failed"]
    except (CoverageError, reference.ReferenceError, OSError, UnicodeError, SyntaxError, KeyError, TypeError) as error:
        report["errors"].append(str(error))
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--rust-run", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--complete-parity", action="store_true")
    args = parser.parse_args(argv)
    try:
        ledger = read_json(args.ledger)
        run = read_json(args.rust_run) if args.rust_run else None
        report = validate(args.upstream, ledger, run)
        report["ledger_sha256"] = rust.sha(args.ledger)
        if args.rust_run:
            report["rust_run_sha256"] = rust.sha(args.rust_run)
    except (CoverageError, ValueError, OSError, UnicodeError) as error:
        report = {"schema": "rstim.magna-coverage-report.v1", "inventory_valid": False,
                  "parity_complete": False, "errors": [str(error)]}
    report["complete_parity_required"] = args.complete_parity
    try:
        with args.out.open("x", encoding="utf-8") as output:
            json.dump(report, output, indent=2, allow_nan=False)
            output.write("\n")
    except OSError as error:
        print(f"cannot create new coverage report: {error}", file=sys.stderr)
        return 2
    print(json.dumps({key: report[key] for key in
                      ("inventory_valid", "parity_complete", "errors", "core_modules", "core_cases", "status_counts")
                      if key in report}))
    if not report["inventory_valid"] or (report.get("rust_execution") or {}).get("errors"):
        return 2
    return 0 if not args.complete_parity or report["parity_complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
