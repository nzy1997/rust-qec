#!/usr/bin/env python3
"""Compare prescribed no-fault affine task masses; retain unsupported controller tasks."""
import argparse
import json
from pathlib import Path
import random
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
    binary_hash = reference.digest(binary)
    rows = []
    scalar = lambda value: dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
    with native_reference(args.upstream) as captured:
        from magna import compile_circuit
        from magna.file_api import NoiseDraw
        for name in captured.manifest["tasks"]:
            root = args.upstream.resolve() / "examples" / name
            run = json.loads((root / "run.json").read_text())
            if run.get("controllers"):
                rows.append(dict(task=name, status="unsupported", reason="the Rust example has no external-controller bridge"))
                continue
            physical, task = root / run["circuit"], root / run["task"]
            try:
                original = compile_circuit(physical, task=task, options=run["options"])
                sample = original.sample(random.Random(20261026), noise=NoiseDraw({}, ()))
                if sample["decoder_calls"] or sample["recovery_decoder_calls"] or sample["sampled_parities"]:
                    raise RuntimeError("an affine task unexpectedly sampled a controller history")
                exact = sample["exact"]
                expected = dict(raw_acceptance=scalar(exact["acceptance"]), raw_error=scalar(exact["error"]),
                                raw_target_success=scalar(exact["target_success"]), history_mass=scalar(exact["history_mass"]),
                                target_normalization=scalar(exact.get("target_normalization", dict(coefficients=[1, 0, 0, 0], power=0))))
                rust = subprocess.run([str(binary), str(physical), str(task)], text=True, capture_output=True, timeout=40)
                if rust.returncode:
                    rows.append(dict(task=name, status="failed", returncode=rust.returncode, stdout=rust.stdout, stderr=rust.stderr))
                    continue
                actual = json.loads(rust.stdout)
                if {key: actual[key] for key in expected} != expected:
                    rows.append(dict(task=name, status="mismatch", reference=expected, rust=actual))
                    continue
                rows.append(dict(task=name, status="matched", reference=expected, rust=actual,
                                 inputs={p.relative_to(args.upstream.resolve()).as_posix(): reference.digest(p) for p in (physical, task, root / "run.json")}))
            except subprocess.TimeoutExpired:
                rows.append(dict(task=name, status="timeout", limit_seconds=40))
            except Exception as error:
                rows.append(dict(task=name, status="failed", error_type=type(error).__name__, message=str(error)))
            print(f"{name}: {rows[-1]['status']}", flush=True)
        failures = [row for row in rows if row["status"] not in {"matched", "unsupported"}]
        document = dict(schema="rmagna.affine-task-comparison.v1", status="partial", seed=20261026,
                        scope="prescribed no-fault exact masses; no paper precision or performance claim",
                        upstream_commit=captured.manifest["upstream"]["commit"], reference_manifest_sha256=captured.manifest_sha256,
                        native_sha256=captured.native_sha256, rust_binary_sha256=binary_hash,
                        tasks=captured.manifest["tasks"], cases=rows, affine_subset_complete=not failures)
    if reference.digest(binary) != binary_hash:
        raise RuntimeError("Rust task evaluator binary changed during comparison")
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(document, indent=2, sort_keys=True) + "\n")
    counts = {status: sum(row["status"] == status for row in rows) for status in sorted({r["status"] for r in rows})}
    print(json.dumps(counts, sort_keys=True))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
