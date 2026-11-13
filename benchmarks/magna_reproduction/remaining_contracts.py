#!/usr/bin/env python3
"""Check outstanding-method requirements without claiming executable Rust parity."""

import argparse
import ast
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import coverage_ledger as ledger_tools

reference = ledger_tools.reference
read_json = ledger_tools.read_json


def require(condition, message):
    if not condition:
        raise ValueError(message)


def method_details(source, case_id):
    """Keep the whole method binding as well as a readable assertion inventory."""
    _, class_name, name = case_id.split(".")
    tree = ast.parse(source)
    cls, = [n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == class_name]
    method, = [n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == name]
    body = "\n".join(source.splitlines()[method.lineno - 1:method.end_lineno])
    checks = []
    for node in ast.walk(method):
        if isinstance(node, ast.Assert):
            checks.append(ast.unparse(node))
        elif isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            if (isinstance(node.func.value, ast.Name) and node.func.value.id == "self"
                    and node.func.attr.startswith(("assert", "fail"))):
                checks.append(ast.unparse(node))
    helpers = sorted(n.name for n in tree.body if isinstance(n, ast.FunctionDef))
    helpers += sorted(f"{n.name}.{m.name}" for n in tree.body if isinstance(n, ast.ClassDef)
                      for m in n.body if isinstance(m, ast.FunctionDef) and not m.name.startswith("test_"))
    # Imported helpers and native C++ suites are covered by the full reference
    # manifest, including their transitive sources; this is not a call-graph proof.
    imports = sorted(set(ast.unparse(n) for n in ast.walk(tree)
                         if isinstance(n, (ast.Import, ast.ImportFrom))))
    loops = [dict(line=n.lineno, target=ast.unparse(n.target), iterable=ast.unparse(n.iter))
             for n in ast.walk(method) if isinstance(n, ast.For)]
    return dict(method_sha256=hashlib.sha256(body.encode()).hexdigest(),
                required_checks=checks, loops=loops, module_helpers=helpers, module_imports=imports)


def check_contracts(catalog, ledger, manifest, *, root=ROOT, upstream=None):
    require(catalog.get("schema") == "rmagna.remaining-contracts.v1", "unsupported contract schema")
    require(catalog.get("reference_sha256") == reference.digest(HERE / "reference.json"),
            "contract reference manifest changed")
    require(catalog.get("upstream_commit") == manifest["upstream"]["commit"], "upstream commit changed")
    require(ledger.get("reference_sha256") == catalog["reference_sha256"], "ledger reference changed")
    current = {r["id"]: r["status"] for r in ledger["cases"]}
    require(len(current) == len(ledger["cases"]), "duplicate ledger method")
    snapshot = catalog["ledger_snapshot"]
    require(dict(Counter(current.values())) == snapshot, "ledger status counts changed; reconcile catalog explicitly")
    outstanding = {k: v for k, v in current.items() if v in ("unmapped", "partial")}
    rows = catalog["methods"]
    require(len(rows) == len(outstanding) and {r["id"] for r in rows} == set(outstanding),
            "missing, duplicate or unknown outstanding method")
    require(dict(Counter(r["category"] for r in rows)) == catalog["category_counts"],
            "contract category counts changed")
    require(set(catalog["category_counts"]) == {"G", "P", "V", "E", "U"}, "unknown category")
    require(catalog["parity_claim"] == "static_requirements_only", "static catalog cannot certify parity")
    require(catalog["helper_source_scope"] == manifest["upstream"]["files"], "helper source closure changed")
    require(catalog["author_decoder_files"] == manifest["upstream"].get("decoder_files", {}),
            "author decoder helper closure changed")
    require(catalog["packaged_tasks"] == manifest["tasks"], "packaged task inventory changed")
    paper = catalog["paper"]
    for key in ("arxiv", "archive_sha256", "kernel_commit", "files"):
        require(paper[key] == manifest["paper"][key], f"paper {key} changed")
    require(paper["primary_version"] == "2610.22315v1" and paper["later_version"] == "2610.22315v2"
            and paper["later_version_status"] == "separate_unassessed_delta",
            "paper version boundary changed")
    roadmap = catalog["roadmap"]
    require(set(roadmap) == {f"R{i:02}" for i in range(1, 13)} | {f"B{i:02}" for i in range(1, 7)},
            "roadmap must contain all 18 approved work items")
    require(len({r["issue"] for r in roadmap.values()}) == len(roadmap), "duplicate roadmap issue")
    pending = {k: set(v["depends_on"]) for k, v in roadmap.items()}
    require(all(v <= set(pending) for v in pending.values()), "unknown roadmap dependency")
    while pending:
        ready = {k for k, deps in pending.items() if not deps}
        require(ready, "cyclic roadmap dependencies")
        pending = {k: v - ready for k, v in pending.items() if k not in ready}
    declarations = None
    if upstream is not None:
        _, declarations, _ = ledger_tools.inventory(upstream, manifest)
    for row in rows:
        name = row["id"]
        require(row["ledger_status"] == outstanding[name], f"unsupported status promotion: {name}")
        require(row["issue"] in roadmap, f"missing roadmap owner: {name}")
        require(row["remaining_requirement"].strip() and row["assessment"].strip(), f"empty requirement: {name}")
        binding = row["source"]
        require(manifest["upstream"]["files"].get(binding["path"]) == binding["source_sha256"],
                f"source identity changed: {name}")
        require(row["rust_owners"], f"no Rust owner: {name}")
        for owner in row["rust_owners"]:
            path = reference.relative_file(root, owner["path"])
            require(path.suffix == ".rs" and path.is_file() and owner["anchor"] in path.read_text(),
                    f"Rust owner missing: {name}: {owner['path']}")
        if declarations is not None:
            require(declarations.get(name) == binding, f"method declaration changed: {name}")
            details = method_details((upstream / binding["path"]).read_text(), name)
            require(row["method"] == details, f"method/helper requirements changed: {name}")
    return dict(status="PASS", scope=catalog["parity_claim"], outstanding=len(rows),
                ledger_snapshot=snapshot, categories=catalog["category_counts"],
                tasks=len(catalog["packaged_tasks"]), paper_files=len(paper["files"]),
                source_files=len(catalog["helper_source_scope"]),
                upstream_verified=upstream is not None, roadmap_issues=len(roadmap))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check"])
    parser.add_argument("--manifest", type=Path, default=HERE / "remaining-contracts.json")
    parser.add_argument("--upstream", type=Path)
    args = parser.parse_args(argv)
    try:
        result = check_contracts(read_json(args.manifest), read_json(HERE / "coverage.json"),
                                 reference.load_manifest(), upstream=args.upstream)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, SyntaxError) as error:
        print(f"remaining contracts: FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
