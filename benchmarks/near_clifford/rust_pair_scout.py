"""Paired warm Rust-only scout using frozen public-API probes on one host.

This measures source effects, not peer/SOTA performance. Both builds finish
before collection; original events, source identities and partial failures remain.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import time


ROOT = Path(__file__).resolve().parents[2]
ARCHIVE = ROOT / "benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-rust-ablation-2026-10-08/probe/warm-candidate"
NAMES = ["msc_d3_inject_cultivate_p1e-3", "msc_d5_inject_cultivate_p1e-3", "pure_surface_d7_r7_p1e-3", "pure_surface_d9_r9_p1e-3"]
PROBE_INPUTS = ["Cargo.toml", "Cargo.lock", "main.rs"] + [name + ".masks.json" for name in NAMES]
ENV_KEYS = ["RUSTFLAGS", "OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "RAYON_NUM_THREADS"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, root=ROOT):
    return subprocess.check_output(args, cwd=root, text=True, timeout=900)


def probe(root):
    return root / "drafts/rust-pair-scout-probe"


def identity(root):
    files = list((root / "rstim/src").rglob("*.rs"))
    files += [root / p for p in ["Cargo.toml", "Cargo.lock", "rstim/Cargo.toml"]]
    files += [root / "benchmarks/near_clifford/application_counts/fixtures" / (n + ".stim") for n in NAMES]
    files += [probe(root) / name for name in PROBE_INPUTS]
    return dict(head=run(["git", "rev-parse", "HEAD"], root).strip(),
                dirty=run(["git", "status", "--porcelain"], root),
                sources={str(p.relative_to(root)): digest(p) for p in sorted(files)},
                binary=digest(probe(root) / "target/release/counts-path-ablation"),
                driver_sha256=digest(Path(__file__)),
                environment={k: os.environ.get(k) for k in ENV_KEYS})


def prepare(baseline_ref):
    if not re.fullmatch(r"[0-9a-f]{40}", baseline_ref):
        raise ValueError("baseline-ref must be an exact 40-character commit SHA")
    if run(["git", "status", "--porcelain"]):
        raise ValueError("candidate checkout must be clean")
    baseline = ROOT / "drafts/rust-pair-scout-baseline"
    if baseline.exists():
        if run(["git", "rev-parse", "HEAD"], baseline).strip() != baseline_ref or run(["git", "status", "--porcelain"], baseline):
            raise ValueError("existing baseline does not match the requested clean source")
    else:
        run(["git", "worktree", "add", "--detach", str(baseline), baseline_ref])
    roots = dict(baseline=baseline, candidate=ROOT)
    for root in roots.values():
        directory = probe(root)
        directory.mkdir(parents=True, exist_ok=True)
        for name in PROBE_INPUTS:
            shutil.copyfile(ARCHIVE / name, directory / name)
        with (directory / "build.log").open("w") as log:
            subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path", str(directory / "Cargo.toml")], cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=900)
    return roots


def retain_probe_inputs(roots, out):
    retained = {}
    for role, root in roots.items():
        directory = out / "probe" / role
        directory.mkdir(parents=True)
        inputs = {}
        for name in PROBE_INPUTS:
            source = probe(root) / name
            snapshot = directory / name
            shutil.copyfile(source, snapshot)
            if digest(source) != digest(snapshot):
                raise ValueError("probe source changed while retaining its bytes")
            inputs[str(source.relative_to(root))] = dict(path=str(snapshot.relative_to(out)), sha256=digest(snapshot))
        log = directory / "build.log"
        shutil.copyfile(probe(root) / "build.log", log)
        retained[role] = dict(inputs=inputs, diagnostics={"build.log": dict(path=str(log.relative_to(out)), sha256=digest(log))})
    shutil.copyfile(Path(__file__), out / "original-driver.py")
    return retained


def verify_retained_inputs(out, identities, retained):
    if set(retained) != set(identities):
        raise ValueError("retained role inventory mismatch")
    for role, info in retained.items():
        expected = {str(Path("drafts/rust-pair-scout-probe") / name) for name in PROBE_INPUTS}
        if set(info["inputs"]) != expected or set(info["diagnostics"]) != {"build.log"}:
            raise ValueError("retained probe/diagnostic inventory mismatch")
        for source, entry in info["inputs"].items():
            if entry["path"] != str(Path("probe") / role / Path(source).name):
                raise ValueError("retained probe path mismatch")
            if digest(out / entry["path"]) != entry["sha256"] or entry["sha256"] != identities[role]["sources"].get(source):
                raise ValueError("retained probe bytes do not match measured source")
        entry = info["diagnostics"]["build.log"]
        if entry["path"] != str(Path("probe") / role / "build.log") or digest(out / entry["path"]) != entry["sha256"]:
            raise ValueError("retained diagnostic bytes changed")
        if digest(out / "original-driver.py") != identities[role]["driver_sha256"]:
            raise ValueError("retained driver bytes do not match measured driver")


def verify_closed_campaign(out):
    """Replay the complete fixed protocol before accepting a closed scout."""
    header = json.loads((out / "header.json").read_text())
    closure = json.loads((out / "closure.json").read_text())
    raw = (out / "events.jsonl").read_bytes()
    events = [json.loads(line) for line in raw.splitlines()]
    cases = [(n, s, p) for n in NAMES for s in [1, 64, 1024] for p in ["strict", "fused"]]
    roles = ["baseline", "candidate"]
    if header["schema"] not in ["exploratory.avx2-pair-ablation.v1", "exploratory.same-binary-warm-control.v1"] or header["cases"] != [list(c) for c in cases] or (header["pairs"], header["observations_per_process"], header["minimum_warm_observation_ns"]) != (5, 7, 50_000_000):
        raise ValueError("closed campaign protocol mismatch")
    if set(header["identities"]) != set(roles) or header["identities"] != closure["identities_after"] or any(i["dirty"] for i in header["identities"].values()):
        raise ValueError("closed campaign source/role identity mismatch")
    if header["schema"] == "exploratory.same-binary-warm-control.v1" and header["identities"]["baseline"] != header["identities"]["candidate"]:
        raise ValueError("null control source/binary identities differ")
    if not all(type(t) in [int, float] and math.isfinite(t) for t in [header["started"], closure["finished"]]) or closure["finished"] < header["started"] or closure["events"] != 288 or len(events) != 288 or digest(out / "events.jsonl") != closure["events_sha256"]:
        raise ValueError("closed campaign time/events/digest mismatch")
    verify_retained_inputs(out, header["identities"], header["retained"])
    schedule = [(r, c, "validate") for c in cases for r in roles]
    for round_index in range(5):
        ordered = cases[round_index:] + cases[:round_index]
        if round_index % 2:
            ordered.reverse()
        for index, case in enumerate(ordered):
            order_roles = roles[::-1] if (index + round_index) % 2 else roles
            schedule.extend((r, case, "bench") for r in order_roles)
    values = {c: {r: [] for r in roles} for c in cases}
    for index, (event, (role, case, action)) in enumerate(zip(events, schedule)):
        if (event["index"], event["route"], tuple(event["case"]), event["action"]) != (index, role, case, action):
            raise ValueError("closed campaign alternating schedule mismatch")
        result = event["result"]
        if result["shots"] != case[1] or result["policy"] != case[2]:
            raise ValueError("closed campaign result context mismatch")
        if action == "validate":
            if result.get("status") != "ok" or result.get("exact_records_counts_rng") is not True or result.get("native_counts_rng") is not True or result.get("seeds") != 4:
                raise ValueError("closed campaign records/counts/RNG failed")
        else:
            observations = result["observations"]
            if result["route"] != "native" or len(observations) != 7:
                raise ValueError("closed campaign native observation inventory mismatch")
            for o in observations:
                if type(o["calls"]) is not int or o["calls"] <= 0 or type(o["elapsed_ns"]) is not int or o["elapsed_ns"] < 50_000_000 or not math.isfinite(o["ns_per_call"]) or o["ns_per_call"] != o["elapsed_ns"] / o["calls"]:
                    raise ValueError("closed campaign invalid observation")
            values[case][role].append(statistics.median(o["ns_per_call"] for o in observations))
    expected = []
    for case in cases:
        v = values[case]
        a, b = statistics.median(v["baseline"]), statistics.median(v["candidate"])
        paired = [x / y for x, y in zip(v["baseline"], v["candidate"])]
        expected.append(dict(case=list(case), baseline_ns=a, candidate_ns=b, speedup=a/b, paired_range=[min(paired), max(paired)]))
    if json.loads((out / "summary.json").read_text()) != expected:
        raise ValueError("closed campaign derived statistics mismatch")
    return header, closure


def collect(roots, out, *, same_binary_control=False):
    out.mkdir(parents=True, exist_ok=False)
    retained = retain_probe_inputs(roots, out)
    before = {key: identity(root) for key, root in roots.items()}
    if any(info["dirty"] for info in before.values()):
        raise ValueError("measured checkout changed during preparation")
    if same_binary_control and (len(before) != 2 or len({str(root.resolve()) for root in roots.values()}) != 1 or len({info["binary"] for info in before.values()}) != 1):
        raise ValueError("same-binary control requires exactly two labels for one checkout and binary")
    verify_retained_inputs(out, before, retained)
    cases = [(n, shots, policy) for n in NAMES for shots in [1, 64, 1024] for policy in ["strict", "fused"]]
    header = dict(schema="exploratory.avx2-pair-ablation.v1", started=time.time(), identities=before,
                  pairs=5, observations_per_process=7, minimum_warm_observation_ns=50_000_000,
                  retained=retained,
                  cases=cases, rustc=run(["rustc", "-Vv"]), host=platform.platform(),
                  affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
                  scope="Paired same-host warm Rust source effect only; all 24 cells retained, no peers or SOTA claim")
    if same_binary_control:
        header["schema"] = "exploratory.same-binary-warm-control.v1"
        header["scope"] = "A/A null control: both labels execute the identical binary from the identical checkout; no source effect or peer claim"
    (out / "header.json").write_text(json.dumps(header, indent=2) + "\n")
    events = []

    def execute(key, case, action):
        name, shots, policy = case
        root = roots[key]
        directory = probe(root)
        args = [str(directory / "target/release/counts-path-ablation"), str(root / "benchmarks/near_clifford/application_counts/fixtures" / (name + ".stim")), str(directory / (name + ".masks.json")), str(shots), policy, "native", action, "7"]
        result = json.loads(run(args, root))
        event = dict(index=len(events), route=key, case=case, action=action, result=result)
        events.append(event)
        with (out / "events.jsonl").open("a") as f:
            f.write(json.dumps(event, separators=(",", ":")) + "\n")
        if action == "validate":
            if result.get("status") != "ok" or result.get("exact_records_counts_rng") is not True or result.get("native_counts_rng") is not True or result.get("seeds") != 4:
                raise ValueError("exact records/counts/RNG validation failed")
        return result

    for case in cases:
        for key in roots:
            execute(key, case, "validate")
    print("All 48 four-seed validations passed", flush=True)
    measurements = {str(c): {key: [] for key in roots} for c in cases}
    for round_index in range(5):
        order_cases = cases[round_index:] + cases[:round_index]
        if round_index % 2:
            order_cases.reverse()
        for index, case in enumerate(order_cases):
            order = list(roots)
            if (index + round_index) % 2:
                order.reverse()
            for key in order:
                result = execute(key, case, "bench")
                observations = result["observations"]
                if len(observations) != 7 or any(o["elapsed_ns"] < 50_000_000 or o["calls"] <= 0 or o["ns_per_call"] != o["elapsed_ns"] / o["calls"] for o in observations):
                    raise ValueError("incomplete warm observation")
                measurements[str(case)][key].append(statistics.median(o["ns_per_call"] for o in observations))
        print("Closed pair", round_index + 1, flush=True)
    after = {key: identity(root) for key, root in roots.items()}
    if before != after:
        raise ValueError("source/binary/driver/environment changed during measurement")
    verify_retained_inputs(out, after, retained)
    summary = []
    for case in cases:
        values = measurements[str(case)]
        a, b = statistics.median(values["baseline"]), statistics.median(values["candidate"])
        paired = [x / y for x, y in zip(values["baseline"], values["candidate"])]
        summary.append(dict(case=case, baseline_ns=a, candidate_ns=b, speedup=a / b, paired_range=[min(paired), max(paired)]))
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (out / "closure.json").write_text(json.dumps(dict(finished=time.time(), events=len(events), events_sha256=digest(out / "events.jsonl"), identities_after=after), indent=2) + "\n")
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-ref", required=True)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    roots = prepare(args.baseline_ref)
    collect(roots, args.out)
