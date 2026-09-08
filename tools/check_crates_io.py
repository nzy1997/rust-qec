#!/usr/bin/env python3
"""Check the first-publication graph, package metadata and actual file lists."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys


def metadata_errors(packages: list[dict], policy: dict) -> list[str]:
    errors = []
    members = {package["name"]: package for package in packages}
    order = policy["publish_order"]
    if len(order) != len(set(order)):
        errors.append("publication order contains duplicates")
    if set(members) != set(order) | set(policy["deferred"]):
        errors.append("publication policy does not classify every workspace member")
    for name, package in members.items():
        if name not in order:
            if package["publish"] != []:
                errors.append(f"{name}: deferred packages must set publish=false")
            continue
        if package["publish"] != ["crates-io"]:
            errors.append(f"{name}: publish must explicitly select crates-io")
        for key in ("description", "repository", "homepage", "documentation", "readme", "license", "rust_version"):
            if not package.get(key):
                errors.append(f"{name}: missing {key}")
        for dep in package["dependencies"]:
            if dep["name"] not in members or dep["kind"] == "dev":
                continue
            upstream = dep["name"]
            if dep["req"] == "*":
                errors.append(f"{name}: internal dependency {upstream} needs a registry version")
            if upstream not in order or order.index(upstream) >= order.index(name):
                errors.append(f"{name}: {upstream} must be published earlier (including optional dependencies)")
    expected_bins = {"rustqec-cli": {"rustqec"}, "rstim": set(), "qec-code": set(), "rmatching": {"rmatching_cli"}}
    for name, expected in expected_bins.items():
        if name not in members:
            continue
        actual = {target["name"] for target in members[name]["targets"]
                  if "bin" in target["kind"] and not target.get("required-features")}
        if actual != expected:
            errors.append(f"{name}: default installation exposes {sorted(actual)}, expected {sorted(expected)}")
    for name in ("rstim", "qec-code"):
        bins = [target for target in members[name]["targets"] if "bin" in target["kind"] and target["name"] == name]
        if len(bins) != 1 or "cli" not in bins[0].get("required-features", []):
            errors.append(f"{name}: compatibility binary must require cli")
    matching = members["rmatching"]
    if any(dep["name"] == "rstim" and dep["kind"] != "dev" for dep in matching["dependencies"]):
        errors.append("rmatching: simulator dependency belongs only in tests or the private benchmark package")
    if any("bin" in target["kind"] and target["name"] != "rmatching_cli" for target in matching["targets"]):
        errors.append("rmatching: benchmark binaries must live outside the published package")
    return errors


MINIMAL_FORBIDDEN = {
    "rstim": {"qec-code", "qec-ilp-core", "clap", "highs", "highs-sys"},
    "qec-code": {"clap", "qec-ilp-core", "highs", "highs-sys"},
    "qec-ilp-core": {"highs", "highs-sys", "gurobi", "bindgen", "cmake"},
    "rmatching": {"rstim", "qec-code", "qec-ilp-core", "clap", "highs", "highs-sys"},
    "rustqec-cli": {"highs", "highs-sys", "qec-ilp-core", "renvelope"},
}


def dependency_errors(name: str, tree: str) -> list[str]:
    found = {line.split()[0] for line in tree.splitlines() if line.split()} & MINIMAL_FORBIDDEN[name]
    return [f"{name}: default dependency graph includes {sorted(found)}"] if found else []


def command(*args: str, cwd: Path) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{' '.join(args)}\n{result.stderr.strip()}")
    return result.stdout


def check(repo: Path) -> None:
    policy = json.loads((repo / "tools/crates_io_packages.json").read_text())
    metadata = json.loads(command("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1", cwd=repo))
    packages = metadata["packages"]
    errors = metadata_errors(packages, policy)
    members = {package["name"]: package for package in packages}
    for name in policy["publish_order"]:
        package = members[name]
        directory = Path(package["manifest_path"]).parent
        files = set(command("cargo", "package", "-p", name, "--list", "--allow-dirty", cwd=repo).splitlines())
        for required in ("README.md", "LICENSE"):
            if required not in files or not (directory / required).is_file():
                errors.append(f"{name}: package is missing {required}")
        if (directory / "LICENSE").is_file() and (directory / "LICENSE").read_bytes() != (repo / "LICENSE").read_bytes():
            errors.append(f"{name}: package LICENSE differs from the repository license")
        forbidden = [entry for entry in files if any(part in {"target", "node_modules", "__pycache__", "drafts", "plans"}
                                                    for part in Path(entry).parts)]
        if forbidden:
            errors.append(f"{name}: unrelated files in package: {forbidden}")
        if name == "rstim":
            for asset in ("index.html", "app.js", "shot-viewer.css", "pkg/rstim_shot_web_bg.wasm"):
                if f"assets/shot-viewer/{asset}" not in files:
                    errors.append(f"rstim: missing compile-time viewer asset {asset}")
        print(f"PACKAGE {name} {package['version']}: {len(files)} files")
    for name in MINIMAL_FORBIDDEN:
        tree = command("cargo", "tree", "--locked", "-p", name, "--edges", "normal,build", "--prefix", "none", cwd=repo)
        errors.extend(dependency_errors(name, tree))
    if errors:
        raise RuntimeError("\n".join(errors))
    print("PASS publication graph, metadata, default binaries, package files, and lightweight CLI")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        check(args.repo_root.resolve())
    except (RuntimeError, OSError, ValueError) as error:
        print(f"FAIL crates.io readiness: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
