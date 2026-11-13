#!/usr/bin/env python3
"""Bounded, source-pinned paper-task pilot; not a paper precision campaign."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import remaining_contracts as contracts
import native_options
from controller_task_compare import validate_warm_result

reference = contracts.reference
require = contracts.require
read_json = contracts.read_json
OUTPUT_CONTRACT = "integrated_joint_acceptance_error_masses"
RAW_KEYS = ("raw_acceptance", "raw_error", "raw_target_success", "history_mass", "target_normalization")
PILOT_SCOPE = "physical_noise_after_prescribed_query_setup_not_paper_throughput"


def json_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def sha(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def count(value):
    return type(value) is int and value >= 0


def duration(value):
    try:
        return type(value) in (int, float) and math.isfinite(value) and value >= 0
    except OverflowError:
        return False


def validate_manifest(manifest, pinned):
    require(manifest["schema"] == "rmagna.paper-pilot-plan.v1", "unsupported pilot manifest")
    require(manifest["reference_sha256"] == reference.digest(HERE / "reference.json"), "reference manifest changed")
    require(manifest["upstream_commit"] == pinned["upstream"]["commit"], "upstream commit changed")
    require(manifest["paper"] == pinned["paper"]["arxiv"], "paper version changed")
    require(manifest["output_contract"] == OUTPUT_CONTRACT, "mixed output contract")
    tasks = manifest["tasks"]
    require(isinstance(tasks, list) and tasks and len(tasks) == len(set(tasks))
            and set(tasks) <= set(pinned["tasks"]), "invalid selected tasks")
    require(set(manifest["required_completed"]) <= set(tasks) and manifest["required_completed"],
            "missing required pilot successes")
    for key, low, high in (("shots", 2, 64), ("seed", 0, 2**64 - 1), ("warm_replays", 1, 16),
                           ("timeout_seconds", 1, 120), ("max_terms", 1, 4096)):
        require(type(manifest[key]) is int and low <= manifest[key] <= high, f"invalid {key}")
    require(type(manifest["native_max_quotient_rank"]) is int and manifest["native_max_quotient_rank"] == 12,
            "undeclared native rank policy")


def task_inputs(task, pinned):
    prefix = f"examples/{task}/"
    return {name: digest for name, digest in
            (pinned["upstream"]["files"] | pinned["upstream"].get("decoder_files", {})).items()
            if name.startswith(prefix)}


def scalar(value):
    require(isinstance(value, dict) and set(value) == {"coefficients", "power"}, "invalid exact scalar")
    c = value["coefficients"]
    require(isinstance(c, list) and len(c) == 4 and all(isinstance(x, str) and
            re.fullmatch(r"-?[0-9]{1,4096}", x) for x in c), "invalid exact coefficients")
    require(type(value["power"]) is int and abs(value["power"]) <= 1_000_000, "invalid scalar exponent")
    c = tuple(map(int, c))
    power = value["power"]
    if not any(c):
        return ((0, 0, 0, 0), 0)
    while all(x % 2 == 0 for x in c):
        c = tuple(x // 2 for x in c)
        power += 1
    return c, power


def exact_match(expected, actual):
    for key in RAW_KEYS:
        left, right = expected[key], actual[key]
        if key == "target_normalization" and right is None:
            right = dict(coefficients=["1", "0", "0", "0"], power=0)
        require(left is not None or key == "raw_target_success", f"missing required exact {key}")
        require((left is None) == (right is None), f"exact {key} presence mismatch")
        if left is not None:
            require(scalar(left) == scalar(right), f"exact {key} mismatch")
    for calls in (expected["controller_calls"], actual["controller_calls"]):
        require(isinstance(calls, list), "controller transcript must be a list")
        for call in calls:
            require(isinstance(call, dict) and set(call) == {"name", "inputs", "outputs"}
                    and isinstance(call["name"], str) and bool(call["name"])
                    and all(isinstance(call[k], list) and all(type(bit) is bool for bit in call[k])
                            for k in ("inputs", "outputs")), "invalid controller transcript")
    require(expected["controller_calls"] == actual["controller_calls"], "controller transcript mismatch")


def rust_configuration(actual, manifest, sampler):
    config = actual["configuration"]
    require(config["seed"] == manifest["seed"] and type(config["seed"]) is int, "Rust seed changed")
    require(config["history_sampler"] == sampler and config["causal_strategy"] == "FixedTemplate", "Rust sampling route changed")
    require(config["max_terms"] == manifest["max_terms"] and type(config["max_terms"]) is int, "Rust term limit changed")
    flags = argparse.Namespace(probability_preparation="native", density_preparation="native",
                               moment_preparation="native", native_transport="prepared",
                               native_max_quotient_rank=manifest["native_max_quotient_rank"],
                               native_max_terms=256)
    native_options.verify(actual, flags)
    validate_warm_result(actual, manifest["warm_replays"])
    require(actual["compilation"]["strategy"] == "SharedCommutingLayer", "Rust compiler changed")


def validate_rust_metrics(result):
    """The exported metrics and the outer diagnostic describe the same batch."""
    metrics = result["metrics"]
    for key in ("requested", "completed", "failed", "not_attempted", "seed"):
        require(count(metrics[key]) and metrics[key] == result[key], f"nested Rust metrics {key} changed")
    require(metrics["status"] == result["status"], "nested Rust metrics status changed")
    expected_failures = []
    for failure in result["failures"]:
        require(isinstance(failure["phase"], str), "invalid Rust failure phase")
        expected_failures.append(dict(phase=failure["phase"].lower(), shot=failure["shot"],
                                      type=failure["kind"], message=failure["message"]))
    require(metrics["failures"] == expected_failures, "nested Rust metrics failures changed")
    for key in ("network_compilations", "history_groups"):
        value = metrics["completed_resources"][key]
        require(count(value) and value == result[key], f"nested Rust metrics {key} changed")
    timings = metrics["timings"]
    require(duration(timings["sampling_seconds"]) and duration(timings["total_seconds"])
            and timings["sampling_seconds"] == result["sampling_seconds"]
            and timings["total_seconds"] >= timings["sampling_seconds"]
            and (timings["preparation_seconds"] is None or duration(timings["preparation_seconds"])),
            "nested Rust metrics timing changed")
    costs = metrics["execution_costs"]
    require(costs["scope"] == "this_call_including_failed_attempts"
            and costs["timing"] == "inclusive_wall_seconds", "nested Rust execution scope changed")
    for key in ("compilation", "query", "decoder", "support"):
        cost = costs[key]
        require(all(count(cost[k]) for k in ("attempted", "succeeded", "failed"))
                and cost["attempted"] == cost["succeeded"] + cost["failed"]
                and duration(cost["seconds"]), f"invalid nested Rust {key} cost")
    require(costs["query"]["attempted"] == result["query_attempts"]
            and costs["decoder"]["seconds"] == result["decoder_seconds"], "nested Rust execution cost changed")
    for key in ("weights", "efficiency"):
        if metrics[key] is not None:
            require(count(metrics[key]["completed"]) and metrics[key]["completed"] == result["completed"],
                    f"nested Rust {key} count changed")


def validate_noisy(stage, manifest, engine):
    payload = stage["payload"]
    if engine == "reference":
        require(sha(payload["native_sha256"]), "missing native extension identity")
        result = metrics = payload["result"]
    else:
        rust_configuration(payload, manifest, "AffineSupport")
        result = payload["pilot"]
        require(result["scope"] == PILOT_SCOPE, "Rust noisy pilot scope changed")
        require(all(count(result[k]) for k in ("requested", "completed", "failed", "not_attempted", "seed",
                                               "query_attempts", "network_compilations", "history_groups")),
                "invalid Rust pilot counters")
        require(duration(result["sampling_seconds"]) and duration(result["decoder_seconds"]), "invalid Rust pilot timing")
        validate_rust_metrics(result)
        metrics = result["metrics"]
    require(all(count(result[k]) for k in ("requested", "completed", "failed", "not_attempted", "seed")),
            "invalid noisy run counts")
    require(result["requested"] == manifest["shots"] and result["seed"] == manifest["seed"] and
            result["requested"] == result["completed"] + result["failed"] + result["not_attempted"],
            "noisy run request/count conservation changed")
    if result["status"] == "failed":
        require(isinstance(result["failures"], list) and result["failures"] and metrics["estimates"] is None,
                "failed noisy run lacks honest failure/estimate accounting")
        return False
    require(result["status"] == "complete" and result["completed"] == manifest["shots"] and
            result["failed"] == result["not_attempted"] == 0 and result["failures"] == [],
            "noisy pilot did not complete")
    if engine == "reference":
        reference.check_case_result(result, manifest["shots"], manifest["seed"])
    for key in ("acceptance", "error"):
        require(duration(metrics["estimates"][key]), f"invalid noisy {key} estimate")
        require(duration(metrics["standard_errors"][key]), f"invalid noisy {key} uncertainty")
    return True


def stage_success(stage):
    return stage["status"] == "complete"


def cell_result(cell, manifest):
    stages = cell["stages"]
    require(set(stages) == {"reference_prescribed", "rust_prescribed", "reference_noisy", "rust_noisy"},
            "missing or unknown pilot stage")
    for stage in stages.values():
        require(stage["status"] in ("complete", "failed", "timeout"), "unknown stage status")
        require(duration(stage["wall_seconds"]), "invalid process timing")
        if stage["status"] == "complete":
            require(type(stage["returncode"]) is int and stage["returncode"] == 0
                    and isinstance(stage["payload"], dict), "successful stage has no valid payload")
        elif stage["status"] == "timeout":
            require(stage["timeout_seconds"] == manifest["timeout_seconds"], "timeout policy changed")
        else:
            require(type(stage["returncode"]) is int and
                    (stage["returncode"] != 0 or bool(stage.get("parse_error"))), "failed stage lacks failure evidence")
    prescribed = all(stage_success(stages[k]) for k in ("reference_prescribed", "rust_prescribed"))
    if prescribed:
        native, rust = (stages[k]["payload"] for k in ("reference_prescribed", "rust_prescribed"))
        require(sha(native["native_sha256"]), "missing prescribed native identity")
        rust_configuration(rust, manifest, "ExactMarginal")
        exact_match(native, rust)
    noisy_complete = True
    for engine in ("reference", "rust"):
        stage = stages[engine + "_noisy"]
        if stage_success(stage):
            noisy_complete &= validate_noisy(stage, manifest, engine)
    hashes = {s["payload"]["native_sha256"] for k, s in stages.items()
              if k.startswith("reference_") and stage_success(s)}
    require(len(hashes) <= 1, "native extension changed across pilot stages")
    return "complete" if noisy_complete and all(stage_success(s) for s in stages.values()) else "negative"


def check_capture(manifest, results, pinned=None):
    results = results.resolve()
    pinned = reference.load_manifest() if pinned is None else pinned
    validate_manifest(manifest, pinned)
    receipt = read_json(reference.relative_file(results, "receipt.json"))
    require(receipt["schema"] == "rmagna.paper-pilot.v1" and receipt["status"] == "recorded", "incomplete pilot receipt")
    require(receipt["manifest_sha256"] == json_hash(manifest), "pilot manifest changed")
    require(receipt["output_contract"] == OUTPUT_CONTRACT, "mixed output contract")
    require(receipt["before"] == receipt["after"], "source changed during pilot")
    require(set(receipt["before"]) == {"source_files", "decoder_files", "tasks"} and
            all(count(v) for v in receipt["before"].values()) and
            receipt["before"]["decoder_files"] <= len(pinned["upstream"].get("decoder_files", {})),
            "invalid source verification counts")
    require(receipt["before"]["source_files"] == len(pinned["upstream"]["files"])
            and receipt["before"]["tasks"] == len(pinned["tasks"]), "incomplete source verification")
    if any(t.startswith("tqd") for t in manifest["tasks"]):
        require(receipt["before"]["decoder_files"] == len(pinned["upstream"]["decoder_files"]),
                "missing author decoder identities")
    require(sha(receipt["rust_binary_sha256"]) and sha(receipt["native_sha256"]), "missing engine identity")
    for name in ("paper_benchmark.py", "paper_reference.py", "worker.py", "native_options.py"):
        require(receipt["runner_files"][name] == reference.digest(HERE / name), f"runner changed: {name}")
    rows = receipt["cells"]
    require([r["task"] for r in rows] == manifest["tasks"], "missing or reordered selected cell")
    outcomes = {}
    for row in rows:
        path = reference.relative_file(results, row["file"])
        require(reference.digest(path) == row["sha256"], "cell hash changed")
        cell = read_json(path)
        require(cell["task"] == row["task"] and cell["inputs"] == task_inputs(row["task"], pinned), "task input/decoder identities changed")
        require(cell["output_contract"] == OUTPUT_CONTRACT, "mixed cell output contract")
        outcome = cell_result(cell, manifest)
        require(cell["outcome"] == row["outcome"] == outcome, "cell outcome changed")
        for key in ("reference_prescribed", "reference_noisy"):
            stage = cell["stages"][key]
            if stage_success(stage):
                require(stage["payload"]["native_sha256"] == receipt["native_sha256"], "native identity changed")
        outcomes[row["task"]] = outcome
    require(all(outcomes[t] == "complete" for t in manifest["required_completed"]), "required pilot success missing")
    return dict(status="PASS", selected=len(rows), completed=list(outcomes.values()).count("complete"),
                negative=list(outcomes.values()).count("negative"), scope="bounded_pilot_not_paper_reproduction")


def run_process(command, timeout, directory):
    start = time.monotonic()
    with tempfile.TemporaryFile(dir=directory) as stdout, tempfile.TemporaryFile(dir=directory) as stderr:
        process = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=os.name == "posix")
        try:
            code = process.wait(timeout=timeout)
            result = dict(status="complete" if code == 0 else "failed", returncode=code)
        except subprocess.TimeoutExpired:
            result = dict(status="timeout", timeout_seconds=timeout)
            process.kill()
            process.wait()
        finally:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        stdout.seek(0)
        stderr.seek(0)
        raw = stdout.read((8 << 20) + 1)
        result.update(argv=command, wall_seconds=time.monotonic() - start,
                      stderr=stderr.read(64 << 10).decode(errors="replace"))
        if result["status"] == "complete":
            try:
                require(len(raw) <= 8 << 20, "worker stdout exceeds 8 MiB")
                result["payload"] = json.loads(raw, object_pairs_hook=contracts.ledger_tools.unique_object,
                    parse_constant=contracts.ledger_tools.invalid_constant)
                require(isinstance(result["payload"], dict), "worker must emit one JSON object")
            except (ValueError, UnicodeError) as error:
                result.update(status="failed", parse_error=str(error), stdout=raw.decode(errors="replace"))
        else:
            result["stdout"] = raw.decode(errors="replace")
        return result


def save(path, value):
    # Each invocation owns its new output directory; checkpoints replace only its
    # own receipt, never an earlier campaign's evidence.
    temporary = path.with_suffix(".new")
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")
    temporary.replace(path)


def rust_command(binary, source, task, manifest, python):
    root = source / "examples" / task
    config = read_json(root / "run.json")
    command = [str(binary), str(root / config["circuit"]), str(root / config["task"]), str(manifest["seed"]),
               "--compiler", "shared", "--warm-replays", str(manifest["warm_replays"]),
               "--max-terms", str(manifest["max_terms"]), "--native-transport", "prepared",
               "--native-max-quotient-rank", "12", "--native-max-terms", "256"]
    for family in native_options.FAMILIES:
        command += [f"--{family}-preparation", "native"]
    if config.get("controllers"):
        command += ["--controllers", str(root / config["controllers"]), "--python", str(python),
                    "--module-root", str(root)]
    return command


def capture(args, manifest):
    pinned = reference.load_manifest()
    validate_manifest(manifest, pinned)
    source = args.upstream.resolve()
    binary = args.rust_example.resolve(strict=True)
    before = reference.verify_source(source, pinned, require_decoders=any(t.startswith("tqd") for t in manifest["tasks"]))
    args.out.mkdir(parents=True, exist_ok=False)
    git = subprocess.run(["git", "-C", str(HERE), "rev-parse", "HEAD"], text=True, capture_output=True, check=True)
    dirty = subprocess.run(["git", "-C", str(HERE), "status", "--porcelain"], text=True, capture_output=True, check=True)
    rustc = subprocess.run(["rustc", "--version"], text=True, capture_output=True, check=True)
    cpu = platform.processor()
    if sys.platform == "darwin":
        cpu = subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
    receipt = dict(schema="rmagna.paper-pilot.v1", status="running", manifest_sha256=json_hash(manifest),
        output_contract=OUTPUT_CONTRACT, before=before, rust_binary_sha256=reference.digest(binary),
        runner_files={n: reference.digest(HERE / n) for n in ("paper_benchmark.py", "paper_reference.py", "worker.py", "native_options.py")},
        host=dict(platform=platform.platform(), machine=platform.machine(), cpu=cpu, python=sys.version),
        rust_source=dict(commit=git.stdout.strip(), dirty=bool(dirty.stdout), rustc=rustc.stdout.strip()), cells=[])
    save(args.out / "receipt.json", receipt)
    native_hashes = set()
    timeout = manifest["timeout_seconds"]
    for task in manifest["tasks"]:
        stages = {}
        ref_cmd = [str(args.python.absolute()), "-I", "-B", str(HERE / "paper_reference.py"),
                   "--upstream", str(source), "--case", task, "--seed", str(manifest["seed"])]
        stages["reference_prescribed"] = run_process(ref_cmd, timeout, args.out)
        command = rust_command(binary, source, task, manifest, args.python.absolute())
        with tempfile.TemporaryDirectory(prefix="pilot-history-", dir=args.out) as directory:
            history_args = []
            native = stages["reference_prescribed"]
            if stage_success(native) and native["payload"]["controller_calls"]:
                history = Path(directory) / "history.json"
                history.write_text(json.dumps([r["inputs"] for r in native["payload"]["controller_calls"]]))
                history_args = ["--history", str(history)]
            stages["rust_prescribed"] = run_process(command + ["--history-sampler", "exact_marginal"] + history_args,
                                                    timeout, args.out)
        stages["reference_noisy"] = run_process([str(args.python.absolute()), "-I", "-B", str(HERE / "worker.py"),
            "--upstream", str(source), "--case", task, "--shots", str(manifest["shots"]),
            "--seed", str(manifest["seed"])], timeout, args.out)
        stages["rust_noisy"] = run_process(command + ["--history-sampler", "affine_support", "--pilot-shots", str(manifest["shots"])], timeout, args.out)
        for key in ("reference_prescribed", "reference_noisy"):
            if stage_success(stages[key]):
                native_hashes.add(stages[key]["payload"]["native_sha256"])
        cell = dict(task=task, output_contract=OUTPUT_CONTRACT, inputs=task_inputs(task, pinned), stages=stages)
        try:
            cell["outcome"] = cell_result(cell, manifest)
        except (ValueError, KeyError, TypeError) as error:
            cell.update(outcome="invalid", validation_error=str(error))
        path = args.out / (task + ".json")
        save(path, cell)
        receipt["cells"].append(dict(task=task, file=path.name, sha256=reference.digest(path), outcome=cell["outcome"]))
        save(args.out / "receipt.json", receipt)
        print(f"{task}: {cell['outcome']}" + (f" ({cell['validation_error']})" if "validation_error" in cell else ""), flush=True)
    receipt["after"] = reference.verify_source(source, pinned, require_decoders=any(t.startswith("tqd") for t in manifest["tasks"]))
    require(len(native_hashes) == 1, "missing or changed native extension identity")
    require(reference.digest(binary) == receipt["rust_binary_sha256"], "Rust executable changed")
    receipt.update(status="recorded", native_sha256=native_hashes.pop())
    save(args.out / "receipt.json", receipt)
    return check_capture(manifest, args.out, pinned)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run")
    run.add_argument("--manifest", type=Path, required=True)
    run.add_argument("--upstream", type=Path, required=True)
    run.add_argument("--python", type=Path, required=True)
    run.add_argument("--rust-example", type=Path, required=True)
    run.add_argument("--out", type=Path, required=True)
    check = sub.add_parser("check")
    check.add_argument("--manifest", type=Path, required=True)
    check.add_argument("--results", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        manifest = read_json(args.manifest)
        result = capture(args, manifest) if args.command == "run" else check_capture(manifest, args.results)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f"paper pilot: FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
