#!/usr/bin/env python3
"""Compare packaged circuit parsing with the pinned reference, without sampling."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--rust-example", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import reference
    from witness_environment import native_reference

    binary = args.rust_example.resolve(strict=True)
    before = reference.digest(binary)
    with native_reference(args.upstream) as captured:
        from magna.circuit_file import ControllerRegistry, parse_circuit

        rows = []
        for task in captured.manifest["tasks"]:
            files = sorted((args.upstream / "examples" / task).glob("*.magna"))
            if not files:
                raise RuntimeError(f"task {task} has no circuit files")
            for path in files:
                text = path.read_text(encoding="utf-8")
                names = sorted(set(re.findall(r"CONTROL\[([^\]]+)\]", text)) - {"XOR", "NOT"})
                registry = ControllerRegistry()
                for name in names:
                    registry.register(name, never_invoke, version="parse-only-v1")
                original = parse_circuit(text, source=str(path), controllers=registry)
                expected = dict(format=original.format_version, qubits=original.num_qubits,
                                records=original.num_records, instructions=len(original.instructions),
                                detectors=sum(i.name == "DETECTOR" for i in original.instructions),
                                external_calls=len(original.analysis.external_instructions),
                                history=original.analysis.route == "nonlinear")
                run = subprocess.run([str(binary), str(path.resolve()), *names],
                                     text=True, capture_output=True, timeout=60, check=True)
                actual = json.loads(run.stdout)
                projected = {key: actual[key] for key in expected}
                if projected != expected:
                    raise RuntimeError(f"parser mismatch for {path}: reference={expected}, Rust={projected}")
                rows.append(dict(task=task, path=path.relative_to(args.upstream).as_posix(),
                                 sha256=reference.digest(path), reference=expected, rust=actual))
        document = dict(schema="rmagna.frontend-comparison.v1", upstream_commit=captured.manifest["upstream"]["commit"],
                        reference_manifest_sha256=captured.manifest_sha256, native_sha256=captured.native_sha256,
                        rust_binary_sha256=before, tasks=captured.manifest["tasks"], files=rows)
    if reference.digest(binary) != before:
        raise RuntimeError("Rust parser binary changed during comparison")
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(f"matched {len(rows)} circuit files across {len(document['tasks'])} tasks")


def never_invoke(_bits, _config):
    raise RuntimeError("a parser must not execute an external controller")


if __name__ == "__main__":
    main()
