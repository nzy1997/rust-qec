"""Validate the explicit R04 typed decisions without hiding unresolved parity gaps."""
import argparse
from collections import Counter
import json
from pathlib import Path

import coverage_ledger as ledger_tools

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
require = ledger_tools.require
rust = ledger_tools.rust
reference = ledger_tools.reference


def check_decisions(decisions, ledger, manifest, *, root=ROOT, upstream=None):
    require(decisions.get("schema") == "rmagna.rust-snapshot-contracts.v1", "unsupported snapshot schema")
    require(decisions["reference_sha256"] == reference.digest(HERE / "reference.json")
            and decisions["upstream_commit"] == manifest["upstream"]["commit"], "snapshot reference changed")
    require((decisions["roadmap_key"], decisions["issue"], decisions["required_target"])
            == ("R04", 1294, "rust_snapshot_contracts"), "snapshot scope changed")
    rows = decisions["methods"]
    require(len(rows) == 27 and len({r["id"] for r in rows}) == 27, "snapshot method inventory changed")
    # IDs bind to explicit whole-method declarations in the source closure.
    expected = {r["id"]: r for r in ledger["cases"]}
    declarations = None
    if upstream is not None:
        _, declarations, _ = ledger_tools.inventory(upstream, manifest)
    witness_source = rust.safe_file(root, "rmagna/tests/rust_snapshot_contracts.rs")
    witness_hash = reference.digest(witness_source)
    witness_names = rust.flat_tests(witness_source.read_text())
    for row in rows:
        name, source, status = row["id"], row["source"], row["status"]
        require(manifest["upstream"]["files"].get(source["path"]) == source["source_sha256"],
                f"snapshot source identity changed: {name}")
        require(status in {"verified", "partial", "unmapped"}, "snapshot cannot introduce architecture exclusions")
        require(name in expected and expected[name]["status"] == status, f"snapshot ledger status mismatch: {name}")
        rust.text(row["contract"], f"snapshot typed contract: {name}")
        if status == "verified":
            require(not row["remaining_gap"], f"unresolved snapshot method cannot be verified: {name}")
        else:
            rust.text(row["remaining_gap"], f"snapshot unresolved gap: {name}")
        require((status != "unmapped") == bool(row["rust_tests"]), f"snapshot witness policy changed: {name}")
        if row["rust_tests"]:
            require(row["rust_tests"] == expected[name]["rust_tests"], f"snapshot witnesses differ from ledger: {name}")
            require(row["contract"] == expected[name]["contract"], f"snapshot contract differs from ledger: {name}")
            for witness in row["rust_tests"]:
                require(witness["target"] == "rust_snapshot_contracts", "snapshot witness target changed")
                rust.fields(witness, {"package", "target", "test", "source_sha256"}, "snapshot Rust witness")
                require(witness["package"] == "rmagna" and witness["source_sha256"] == witness_hash,
                        "snapshot Rust witness source changed")
                require(witness["test"] in witness_names, "snapshot Rust witness test missing")
        if status == "partial":
            require(row["remaining_gap"] == expected[name]["remaining_gap"], f"snapshot gap differs from ledger: {name}")
        if declarations is not None:
            require(declarations.get(name) == source, f"snapshot method declaration changed: {name}")
            from remaining_contracts import method_details
            details = method_details((upstream / source["path"]).read_text(), name)
            require(details["method_sha256"] == row["method_sha256"], f"snapshot complete method changed: {name}")
    require(set(decisions["consumers"]) == {"R09", "R11"}, "snapshot consumers changed")
    for key, issue in [("R09", 1299), ("R11", 1300)]:
        require(decisions["consumers"][key]["issue"] == issue, "snapshot consumer issue changed")
        rust.text(decisions["consumers"][key]["contract"], "snapshot consumer contract")
    for key in ["source_replacement", "callbacks", "fallback", "cache", "metadata"]:
        rust.text(decisions["policies"][key], f"snapshot policy {key}")
    require(set(decisions["fixtures"]) == {
        f"rmagna/tests/fixtures/{stem}.json" for stem in
        ["original_snapshot_queries", "original_sparse_ownership", "original_feedback_incidence"]
    }, "snapshot fixture inventory changed")
    for relative, digest in decisions["fixtures"].items():
        file = rust.safe_file(root, relative)
        require(reference.digest(file) == digest, f"snapshot fixture changed: {relative}")
        data = json.loads(file.read_text())
        require(data["upstream_commit"] == decisions["upstream_commit"], "snapshot fixture reference changed")
        require(data["reference_manifest_sha256"] == decisions["reference_sha256"], "snapshot fixture manifest changed")
        for source, source_hash in data["source_sha256"].items():
            require(manifest["upstream"]["files"].get(source) == source_hash, "snapshot fixture source changed")
        producer = rust.safe_file(root, f"benchmarks/magna_reproduction/{file.stem}.py")
        require(reference.digest(producer) == data["generator_sha256"], "snapshot fixture generator changed")
    return dict(status="PASS", scope=decisions["claims"], methods=len(rows),
                decisions=dict(Counter(r["status"] for r in rows)),
                upstream_verified=upstream is not None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path)
    args = parser.parse_args()
    try:
        result = check_decisions(ledger_tools.read_json(HERE / "rust-snapshot-contracts.json"),
                                 ledger_tools.read_json(HERE / "coverage.json"), reference.load_manifest(),
                                 upstream=args.upstream)
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(2, f"snapshot contracts: FAIL: {error}\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
