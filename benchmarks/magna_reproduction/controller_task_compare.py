#!/usr/bin/env python3
"""Compare exact no-fault decoder tasks on explicitly prescribed native histories."""
import argparse
import contextlib
import importlib
import json
import os
import signal
from pathlib import Path
import subprocess
import sys
import tempfile

TASKS = ["dk", "logical-distillation", "tqd3", "tqd4", "tqd5"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--rust-example", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--tasks", nargs="+", choices=TASKS, default=TASKS)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--one", choices=TASKS, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import reference
    binary = args.rust_example.resolve(strict=True)
    before = reference.digest(binary)
    if args.one:
        print(json.dumps(one(args, binary), sort_keys=True))
        return
    if args.out is None:
        parser.error("--out is required")
    rows = []
    document = dict(schema="rmagna.controller-task-comparison.v1", status="running",
        scope="prescribed no-fault observations from two native entropy endpoints, replayed explicitly in Rust; not Monte Carlo or performance evidence",
        rust_binary_sha256=before, tasks=args.tasks, cases=rows)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(document, indent=2, sort_keys=True) + "\n")
    def checkpoint():
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=args.out.parent, delete=False) as output:
            path = Path(output.name)
            output.write(json.dumps(document, indent=2, sort_keys=True) + "\n")
        path.replace(args.out)
    for name in args.tasks:
        command = [sys.executable, "-I", __file__, "--upstream", str(args.upstream),
                   "--rust-example", str(binary), "--one", name, "--timeout", str(args.timeout)]
        process = subprocess.Popen(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=os.name == "posix")
        try:
            stdout, stderr = process.communicate(timeout=2 * args.timeout + 120)
            if process.returncode:
                row = dict(task=name, status="failed", returncode=process.returncode,
                           stdout=stdout, stderr=stderr)
            else:
                row = json.loads(stdout)
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
            row = dict(task=name, status="timeout", limit_seconds=2 * args.timeout + 120,
                       stdout=stdout, stderr=stderr)
        finally:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        rows.append(row)
        checkpoint()
        print(f"{name}: {row['status']}", flush=True)
    if reference.digest(binary) != before:
        document["status"] = "failed"
        document["failure"] = "Rust binary changed during comparison"
        checkpoint()
        raise RuntimeError(document["failure"])
    document["status"] = "complete" if all(r["status"] == "matched" for r in rows) and args.tasks == TASKS else "partial"
    checkpoint()
    if any(row["status"] != "matched" for row in rows):
        raise SystemExit(1)


def compare_result(result, expected, word, max_terms, classical):
    """Retain one history's raw output even when a successful child is malformed."""
    try:
        actual = json.loads(result.stdout)
        if not isinstance(actual, dict):
            raise ValueError("Rust result must be a JSON object")
        config = actual.get("configuration")
        if (not isinstance(config, dict) or type(config.get("max_terms")) is not int
                or config["max_terms"] != max_terms):
            raise ValueError("Rust example did not apply the declared term budget")
        compared = {key: actual[key] for key in expected}
        for key, value in compared.items():
            if key == "controller_calls":
                if not isinstance(value, list) or any(
                    not isinstance(call, dict) or not isinstance(call.get("name"), str)
                    or any(not isinstance(call.get(field), list)
                           or any(type(bit) is not bool for bit in call[field])
                           for field in ["inputs", "outputs"])
                    for call in value
                ):
                    raise ValueError("Rust controller_calls must contain named Boolean inputs/outputs")
                continue
            if value is None and key in ["raw_target_success", "target_normalization"]:
                continue
            if (not isinstance(value, dict) or type(value.get("power")) is not int
                    or not isinstance(value.get("coefficients"), list)
                    or len(value["coefficients"]) != 4
                    or any(not isinstance(c, str) for c in value["coefficients"])):
                raise ValueError(f"Rust {key} must be an exact scalar")
            for coefficient in value["coefficients"]:
                int(coefficient)
        if actual["raw_target_success"] is None and classical:
            # Native classical tasks name logical success target_success; Rust
            # reserves its optional field for quantum targets.
            from magna import ExactScalar
            def exact_scalar(v):
                return ExactScalar(tuple(map(int, v["coefficients"])), v["power"])
            difference = exact_scalar(actual["raw_acceptance"]) - exact_scalar(actual["raw_error"])
            compared["raw_target_success"] = dict(coefficients=list(map(str, difference.coefficients)), power=difference.power)
        if compared["target_normalization"] is None:
            compared["target_normalization"] = dict(coefficients=["1", "0", "0", "0"], power=0)
        return dict(constant_word=word, status="matched" if compared == expected else "mismatch",
                    reference=expected, compared=compared, rust=actual)
    except Exception as error:
        # Processing external output is part of the comparison. Fail this case
        # explicitly and continue so previous/later evidence and provenance survive.
        return dict(constant_word=word, status="failed", returncode=result.returncode,
                    error_type=type(error).__name__, error=str(error),
                    stdout=result.stdout, stderr=result.stderr, reference=expected)


def one(args, binary):
    import reference
    from witness_environment import native_reference
    root = args.upstream.resolve() / "examples" / args.one
    config_path = root / "controllers.json"
    config = json.loads(config_path.read_text())
    run = json.loads((root / "run.json").read_text())
    cases = []
    binary_hash = reference.digest(binary)
    with native_reference(args.upstream) as captured:
        from magna import ControllerRegistry, compile_circuit
        from magna.file_api import NoiseDraw
        registry = ControllerRegistry()
        calls = []
        sys.path.insert(0, str(root))
        for name, spec in config["bindings"].items():
            module, function = spec["import"].split(":")
            with contextlib.redirect_stdout(sys.stderr):
                callback = getattr(importlib.import_module(module), function)
            def recorded(bits, config, callback=callback, name=name):
                with contextlib.redirect_stdout(sys.stderr):
                    outputs = callback(bits, config)
                calls.append(dict(name=name, inputs=list(map(bool, bits)), outputs=list(map(bool, outputs))))
                return outputs
            registry.register(name, recorded, version=spec["version"], config=spec.get("config", {}))
        options = dict(run["options"], history_sampler="exact_marginal")
        compiled = compile_circuit(root / run["circuit"], task=root / run["task"], controllers=registry, options=options)
        class Constant:
            def __init__(self, one):
                self.one = one
            def getrandbits(self, width):
                return (1 << width) - 1 if self.one else 0
        def scalar(value):
            return dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
        for word in [0, 2**64 - 1]:
            calls.clear()
            original = compiled.sample(Constant(bool(word)), noise=NoiseDraw({}, ()))
            exact = original["exact"]
            expected = dict(raw_acceptance=scalar(exact["acceptance"]), raw_error=scalar(exact["error"]),
                            raw_target_success=scalar(exact["target_success"]), history_mass=scalar(exact["history_mass"]),
                            target_normalization=scalar(exact.get("target_normalization", dict(coefficients=[1, 0, 0, 0], power=0))),
                            controller_calls=calls.copy())
            if original["sampled_records"]:
                raise RuntimeError("task requests additional records; extend the explicit replay groups")
            command = [str(binary), str(root / run["circuit"]), str(root / run["task"]),
                       "--max-terms", str(options["max_terms"]), "--controllers", str(config_path), "--python", sys.executable,
                       "--module-root", str(root)]
            try:
                with tempfile.TemporaryDirectory(prefix="rmagna-prescribed-decoder-") as directory:
                    history = Path(directory) / "history.json"
                    history.write_text(json.dumps([call["inputs"] for call in calls]))
                    result = subprocess.run(command + ["--history", str(history)], text=True, capture_output=True, timeout=args.timeout)
            except subprocess.TimeoutExpired:
                cases.append(dict(constant_word=word, status="timeout", limit_seconds=args.timeout, reference=expected))
                continue
            if result.returncode:
                cases.append(dict(constant_word=word, status="failed", returncode=result.returncode,
                                  stdout=result.stdout, stderr=result.stderr, reference=expected))
                continue
            cases.append(compare_result(result, expected, word, options["max_terms"],
                                        "error_observables" in compiled.task["raw"]))
        if reference.digest(binary) != binary_hash:
            raise RuntimeError("Rust binary changed during comparison")
        return dict(task=args.one, status="matched" if all(c["status"] == "matched" for c in cases) else "failed",
                    upstream_commit=captured.manifest["upstream"]["commit"], native_sha256=captured.native_sha256,
                    reference_manifest_sha256=captured.manifest_sha256, rust_binary_sha256=binary_hash,
                    inputs={p.relative_to(args.upstream.resolve()).as_posix(): reference.digest(p)
                            for p in (config_path, root / "run.json", root / run["circuit"], root / run["task"])}, cases=cases)


if __name__ == "__main__":
    main()
