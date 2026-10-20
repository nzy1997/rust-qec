#!/usr/bin/env python3
"""Compare all packaged task/module mappings without sampling or decoder execution."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


def never_invoke(_bits, _config):
    raise RuntimeError("task validation must not execute a controller")


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
    rows = []
    with native_reference(args.upstream) as captured:
        from magna import ControllerRegistry, compile_circuit
        for name in captured.manifest["tasks"]:
            root = args.upstream.resolve() / "examples" / name
            run = json.loads((root / "run.json").read_text())
            physical, task = root / run["circuit"], root / run["task"]
            names = set()
            for path in root.glob("*.magna"):
                names.update(re.findall(r"CONTROL\[([^\]]+)\]", path.read_text()))
            names = sorted(names - {"XOR", "NOT"})
            registry = ControllerRegistry()
            for function in names:
                registry.register(function, never_invoke, version="validation-only-v1")
            original = compile_circuit(physical, task=task, controllers=registry, options=run["options"])
            evaluation = original.inspect()["evaluation"]
            target, recovery = evaluation["ideal_state"], evaluation["recovery"]
            expected = dict(outputs=evaluation["output_mapping"], target=None, recovery=None)
            if target is not None:
                expected["target"] = dict(qubits=target["preparation_width"],
                                          outputs=list(target["outputs"]) if target["outputs"] is not None else list(range(target["width"])),
                                          postselection={str(k): bool(v) for k, v in target["postselection"].items()})
            if recovery is not None:
                expected["recovery"] = dict(qubits=recovery["width"] + recovery["ancillas"],
                                            records=recovery["num_records"], history=recovery["route"] == "nonlinear")
            result = subprocess.run([str(binary), str(physical), str(task), *names], text=True,
                                    capture_output=True, timeout=60, check=True)
            actual = json.loads(result.stdout)
            if {key: actual[key] for key in expected} != expected:
                raise RuntimeError(f"task mapping mismatch for {name}: reference={expected}, Rust={actual}")
            inputs = [physical, task, root / "run.json"]
            if target is not None:
                inputs.append(Path(target["source"]))
            if recovery is not None:
                inputs.append(Path(recovery["source"]))
            rows.append(dict(task=name, inputs={p.relative_to(args.upstream.resolve()).as_posix(): reference.digest(p) for p in inputs},
                             reference=expected, rust=actual))
        document = dict(schema="rmagna.task-comparison.v1", upstream_commit=captured.manifest["upstream"]["commit"],
                        reference_manifest_sha256=captured.manifest_sha256, native_sha256=captured.native_sha256,
                        rust_binary_sha256=before, controller_mode="validation-only; callbacks fail if invoked",
                        tasks=captured.manifest["tasks"], cases=rows)
    if reference.digest(binary) != before:
        raise RuntimeError("Rust task validation binary changed during comparison")
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(f"matched {len(rows)} task/module mappings")


if __name__ == "__main__":
    main()
