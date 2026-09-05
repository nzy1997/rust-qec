#!/usr/bin/env python3
"""Record one validated native support CI cell as machine-readable evidence."""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
from pathlib import Path


EXPECTED_CLI_OUTPUT = {
    "schema_version": "rustqec.cli.v1",
    "status": "ok",
    "command": "circuit.stats",
    "result": {
        "instruction_count": 2,
        "repeat_blocks": 0,
        "max_repeat_depth": 0,
        "num_qubits": 1,
        "num_measurements": 1,
        "num_detectors": 1,
        "num_observables": 0,
        "num_ticks": 0,
        "num_sweep_bits": 0,
    },
    "warnings": [],
    "artifacts": [],
}


def run(*argv: str) -> str:
    return subprocess.run(argv, check=True, text=True, capture_output=True).stdout.strip()


def rustc_details(rustc: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in run(str(rustc), "--version", "--verbose").splitlines():
        if ": " in line:
            key, value = line.split(": ", 1)
            fields[key] = value
    if "release" not in fields or "host" not in fields:
        raise SystemExit("rustc --version --verbose omitted release or host")
    return fields


def os_version() -> str:
    if platform.system() == "Linux":
        values = {}
        for line in Path("/etc/os-release").read_text().splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                values[key] = value.strip('"')
        return values.get("VERSION_ID", "")
    if platform.system() == "Darwin":
        return platform.mac_ver()[0]
    return platform.release()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cell", required=True)
    parser.add_argument("--runner-label", required=True)
    parser.add_argument("--expected-target", required=True)
    parser.add_argument("--toolchain", required=True)
    parser.add_argument("--cli-json", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    rustc = Path(os.environ["RUSTC"]).resolve()
    rustdoc = Path(os.environ["RUSTDOC"]).resolve()
    cargo = Path(os.environ["CARGO"]).resolve()
    selected_rustc = Path(run("rustup", "which", "--toolchain", args.toolchain, "rustc")).resolve()
    if rustc != selected_rustc:
        raise SystemExit(f"RUSTC {rustc} does not match rustup selection {selected_rustc}")
    if rustdoc.parent != rustc.parent or cargo.parent != rustc.parent:
        raise SystemExit("cargo, rustc, and rustdoc do not come from one toolchain directory")

    compiler = rustc_details(rustc)
    if compiler["host"] != args.expected_target:
        raise SystemExit(f"rustc host {compiler['host']} != {args.expected_target}")
    if args.toolchain != "stable" and compiler["release"] != args.toolchain:
        raise SystemExit(f"rustc release {compiler['release']} != {args.toolchain}")

    cli = json.loads(args.cli_json.read_text())
    if cli != EXPECTED_CLI_OUTPUT:
        raise SystemExit(f"one-qubit CLI output is invalid: {cli}")

    metadata = json.loads(run(str(cargo), "metadata", "--locked", "--no-deps", "--format-version", "1"))
    workspace_ids = set(metadata["workspace_members"])
    packages = sorted(
        (
            {
                "name": package["name"],
                "version": package["version"],
                "rust_version": package["rust_version"],
            }
            for package in metadata["packages"]
            if package["id"] in workspace_ids
        ),
        key=lambda package: package["name"],
    )
    if not packages or any(not package["rust_version"] for package in packages):
        raise SystemExit("every workspace package must declare rust-version")

    runner_os = os.environ.get("RUNNER_OS", platform.system())
    runner_arch = os.environ.get("RUNNER_ARCH", "")
    machine = platform.machine()
    version = os_version()
    if args.runner_label == "ubuntu-24.04" and not (runner_os == "Linux" and runner_arch == "X64" and machine == "x86_64" and version == "24.04"):
        raise SystemExit(f"runner is not Ubuntu 24.04 x86_64: {runner_os} {runner_arch} {machine} {version}")
    if args.runner_label == "macos-15" and not (runner_os == "macOS" and runner_arch == "ARM64" and machine == "arm64" and version.startswith("15.")):
        raise SystemExit(f"runner is not macOS 15 arm64: {runner_os} {runner_arch} {machine} {version}")

    evidence = {
        "schema_version": "rustqec.native-support.v1",
        "cell": args.cell,
        "status": "pass",
        "tested_sha": os.environ.get("GITHUB_SHA", ""),
        "source_head_sha": os.environ.get("SOURCE_HEAD_SHA", ""),
        "requested_toolchain": args.toolchain,
        "compiler": {
            "release": compiler["release"],
            "host": compiler["host"],
            "commit_hash": compiler.get("commit-hash", ""),
            "path": str(rustc),
            "rustup_which": str(selected_rustc),
            "cargo_path": str(cargo),
        },
        "runner": {
            "label": args.runner_label,
            "os": runner_os,
            "arch": runner_arch,
            "machine": machine,
            "os_version": version,
            "image_os": os.environ.get("ImageOS", ""),
            "image_version": os.environ.get("ImageVersion", ""),
        },
        "target": args.expected_target,
        "cli_output": cli,
        "packages": packages,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(f"recorded {args.cell}: rustc {compiler['release']} ({compiler['host']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
